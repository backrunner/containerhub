use crate::channel::{Channel, Handshake, MAX_FRAME};
use anyhow::{Context, Result, bail, ensure};
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use serde::{Serialize, de::DeserializeOwned};
use std::{pin::Pin, time::Duration};
use tokio::{net::TcpStream, time::timeout};
use tokio_tungstenite::{
    accept_async_with_config, connect_async_with_config,
    tungstenite::{Error, Message, protocol::WebSocketConfig},
};

pub const IO_TIMEOUT: Duration = Duration::from_secs(15);
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
pub trait FrameSocket:
    Stream<Item = Result<Message, Error>> + Sink<Message, Error = Error> + Send + Unpin
{
}
impl<T> FrameSocket for T where
    T: Stream<Item = Result<Message, Error>> + Sink<Message, Error = Error> + Send + Unpin
{
}
pub type Socket = Pin<Box<dyn FrameSocket>>;
fn config() -> WebSocketConfig {
    WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME))
        .max_frame_size(Some(MAX_FRAME))
        .write_buffer_size(0)
        .max_write_buffer_size(MAX_FRAME * 2)
}
pub fn validate_endpoint(endpoint: &str, development: bool) -> Result<()> {
    let url = url::Url::parse(endpoint)?;
    ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
            && url.query().is_none()
            && url.host_str().is_some(),
        "endpoint must have a host and cannot contain credentials, queries or fragments"
    );
    let loopback = url
        .host_str()
        .is_some_and(|host| host == "127.0.0.1" || host == "[::1]" || host == "::1");
    ensure!(
        url.scheme() == "wss" || (url.scheme() == "ws" && loopback),
        "use wss:// for non-loopback endpoints"
    );
    ensure!(
        !development || loopback,
        "development credentials only connect to literal loopback addresses"
    );
    Ok(())
}
pub async fn connect(endpoint: &str, development: bool) -> Result<Socket> {
    validate_endpoint(endpoint, development)?;
    let (socket, _) = timeout(
        HANDSHAKE_TIMEOUT,
        connect_async_with_config(endpoint, Some(config()), true),
    )
    .await??;
    Ok(Box::pin(socket))
}
pub async fn accept(stream: TcpStream) -> Result<Socket> {
    Ok(Box::pin(
        timeout(
            HANDSHAKE_TIMEOUT,
            accept_async_with_config(stream, Some(config())),
        )
        .await??,
    ))
}
pub async fn send_frame(socket: &mut Socket, bytes: Vec<u8>) -> Result<()> {
    ensure!(bytes.len() <= MAX_FRAME, "frame too large");
    timeout(IO_TIMEOUT, socket.send(Message::Binary(bytes.into()))).await??;
    Ok(())
}
pub async fn receive_frame(socket: &mut Socket) -> Result<Vec<u8>> {
    timeout(IO_TIMEOUT, async {
        loop {
            match socket.next().await.context("connection closed")?? {
                Message::Binary(bytes) => {
                    ensure!(bytes.len() <= MAX_FRAME, "frame too large");
                    return Ok(bytes.to_vec());
                }
                Message::Ping(_) | Message::Pong(_) => {
                    socket.flush().await?;
                }
                Message::Close(_) => bail!("connection closed"),
                _ => bail!("binary protocol frame required"),
            }
        }
    })
    .await?
}
pub struct SecureSocket {
    pub socket: Socket,
    pub channel: Channel,
}
impl SecureSocket {
    pub async fn handshake(mut socket: Socket, initiator: bool) -> Result<Self> {
        timeout(HANDSHAKE_TIMEOUT, async move {
            let mut handshake = Handshake::new(initiator)?;
            if initiator {
                send_frame(&mut socket, handshake.write()?).await?;
                handshake.read(&receive_frame(&mut socket).await?)?;
            } else {
                handshake.read(&receive_frame(&mut socket).await?)?;
                send_frame(&mut socket, handshake.write()?).await?;
            }
            Ok(Self {
                socket,
                channel: handshake.finish()?,
            })
        })
        .await?
    }
    pub async fn send(&mut self, value: &impl Serialize) -> Result<()> {
        let bytes = zeroize::Zeroizing::new(serde_json::to_vec(value)?);
        timeout(IO_TIMEOUT, async {
            for frame in self.channel.seal(&bytes)? {
                send_frame(&mut self.socket, frame).await?;
            }
            Ok(())
        })
        .await?
    }
    pub async fn receive<T: DeserializeOwned>(&mut self) -> Result<T> {
        timeout(IO_TIMEOUT, async {
            loop {
                if let Some(bytes) = self.channel.open(&receive_frame(&mut self.socket).await?)? {
                    let bytes = zeroize::Zeroizing::new(bytes);
                    return Ok(serde_json::from_slice(&bytes)?);
                }
            }
        })
        .await?
    }
}
