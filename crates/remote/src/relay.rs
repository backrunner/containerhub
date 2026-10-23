//! A relay pairs two outbound sockets and forwards opaque, bounded binary frames.
//! Route possession proves the Agent key; controller authorization happens only inside E2E.
use crate::{
    crypto::*,
    network::{self, Socket},
    server::Agent,
};
use anyhow::{Context, Result, bail, ensure};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::Duration,
};
use tokio::{
    net::TcpListener,
    sync::{Mutex, Semaphore, oneshot},
};
use tokio_tungstenite::tungstenite::Message;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Challenge {
    challenge: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Registration {
    route: String,
    challenge: String,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case", deny_unknown_fields)]
enum Join {
    Agent { key: PublicKey, proof: Signed },
    Controller { route: String },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ready {
    paired: bool,
}
struct Waiting {
    token: String,
    tx: oneshot::Sender<Socket>,
}
type Routes = Arc<Mutex<HashMap<String, VecDeque<Waiting>>>>;

pub async fn serve(listener: TcpListener) -> Result<()> {
    let routes: Routes = Arc::new(Mutex::new(HashMap::new()));
    let slots = Arc::new(Semaphore::new(64));
    loop {
        let (stream, _) = listener.accept().await?;
        let Ok(slot) = slots.clone().try_acquire_owned() else {
            drop(stream);
            continue;
        };
        let routes = routes.clone();
        tokio::spawn(async move {
            let _slot = slot;
            if let Ok(socket) = network::accept(stream).await {
                let _ = pair(routes, socket).await;
            }
        });
    }
}
async fn send_json(socket: &mut Socket, value: &impl Serialize) -> Result<()> {
    network::send_frame(socket, serde_json::to_vec(value)?).await
}
async fn receive_json<T: serde::de::DeserializeOwned>(socket: &mut Socket) -> Result<T> {
    Ok(serde_json::from_slice(
        &network::receive_frame(socket).await?,
    )?)
}
async fn pair(routes: Routes, mut socket: Socket) -> Result<()> {
    let challenge = nonce();
    send_json(
        &mut socket,
        &Challenge {
            challenge: challenge.clone(),
        },
    )
    .await?;
    match receive_json::<Join>(&mut socket).await? {
        Join::Agent { key, proof } => {
            let registration: Registration = key.verify(RELAY_DOMAIN, &proof)?;
            ensure!(
                registration.route == key.id() && registration.challenge == challenge,
                "invalid relay registration proof"
            );
            let route = registration.route;
            let token = nonce();
            let (tx, rx) = oneshot::channel();
            {
                let mut map = routes.lock().await;
                let queue = map.entry(route.clone()).or_default();
                ensure!(queue.len() < 8, "too many waiting sockets for this agent");
                queue.push_back(Waiting {
                    token: token.clone(),
                    tx,
                });
            }
            // A route is not reserved forever by an idle socket.
            let result = tokio::time::timeout(Duration::from_secs(30), rx).await;
            {
                let mut map = routes.lock().await;
                if let Some(queue) = map.get_mut(&route) {
                    queue.retain(|w| w.token != token);
                    if queue.is_empty() {
                        map.remove(&route);
                    }
                }
            }
            let mut controller = result.context("relay wait expired")??;
            send_json(&mut socket, &Ready { paired: true }).await?;
            send_json(&mut controller, &Ready { paired: true }).await?;
            forward(socket, controller).await
        }
        Join::Controller { route } => {
            ensure!(
                route.starts_with("p256:") && route.len() <= 128,
                "invalid agent route"
            );
            let mut map = routes.lock().await;
            let queue = map.get_mut(&route).context("agent is offline")?;
            while let Some(waiting) = queue.pop_front() {
                match waiting.tx.send(socket) {
                    Ok(()) => return Ok(()),
                    Err(returned) => socket = returned,
                }
            }
            bail!("agent is offline")
        }
    }
}
async fn forward(left: Socket, right: Socket) -> Result<()> {
    let (mut left_write, mut left_read) = left.split();
    let (mut right_write, mut right_read) = right.split();
    let result:Result<()>=tokio::time::timeout(Duration::from_secs(300),async {
        loop {
            tokio::select! {
                frame=left_read.next()=>{
                    let frame=frame.context("agent disconnected")??;
                    match frame {
                        Message::Binary(bytes)=>{ensure!(bytes.len()<=crate::channel::MAX_FRAME,"frame too large");tokio::time::timeout(network::IO_TIMEOUT,right_write.send(Message::Binary(bytes))).await??;},
                        Message::Ping(bytes)=>{left_write.send(Message::Pong(bytes)).await?;},
                        Message::Pong(_)=>{},
                        _=>return Ok(()),
                    }
                },
                frame=right_read.next()=>{
                    let frame=frame.context("controller disconnected")??;
                    match frame {
                        Message::Binary(bytes)=>{ensure!(bytes.len()<=crate::channel::MAX_FRAME,"frame too large");tokio::time::timeout(network::IO_TIMEOUT,left_write.send(Message::Binary(bytes))).await??;},
                        Message::Ping(bytes)=>{right_write.send(Message::Pong(bytes)).await?;},
                        Message::Pong(_)=>{},
                        _=>return Ok(()),
                    }
                }
            }
        }
    }).await?;
    result
}
pub async fn controller(endpoint: &str, route: &str, development: bool) -> Result<Socket> {
    let mut socket = network::connect(endpoint, development).await?;
    let _: Challenge = receive_json(&mut socket).await?;
    send_json(
        &mut socket,
        &Join::Controller {
            route: route.into(),
        },
    )
    .await?;
    ensure!(
        receive_json::<Ready>(&mut socket).await?.paired,
        "relay pairing failed"
    );
    Ok(socket)
}
/// Each worker keeps one outbound slot ready and reconnects after transport loss.
pub async fn outbound(agent: Arc<Agent>, endpoint: String, development: bool) -> Result<()> {
    network::validate_endpoint(&endpoint, development)?;
    let mut backoff = 1;
    loop {
        let result = async {
            let mut socket = network::connect(&endpoint, development).await?;
            let challenge: Challenge = receive_json(&mut socket).await?;
            let registration = Registration {
                route: agent.public_key().id(),
                challenge: challenge.challenge,
            };
            send_json(
                &mut socket,
                &Join::Agent {
                    key: agent.public_key(),
                    proof: agent.sign(RELAY_DOMAIN, &registration)?,
                },
            )
            .await?;
            // Pending agent slots expire at the relay in 30 seconds, then reconnect.
            let ready: Ready = tokio::time::timeout(Duration::from_secs(35), async {
                loop {
                    match socket.next().await.context("relay closed")?? {
                        Message::Binary(bytes) => {
                            return Ok::<Ready, anyhow::Error>(serde_json::from_slice(&bytes)?);
                        }
                        Message::Ping(_) | Message::Pong(_) => socket.flush().await?,
                        _ => bail!("relay closed"),
                    }
                }
            })
            .await??;
            ensure!(ready.paired, "relay pairing failed");
            backoff = 1;
            agent.clone().session(socket).await
        }
        .await;
        let _ = result;
        tokio::time::sleep(Duration::from_millis(
            backoff * 250 + u64::from(rand_core::RngCore::next_u32(&mut rand_core::OsRng) % 250),
        ))
        .await;
        backoff = (backoff * 2).min(32);
    }
}
