//! Noise NN with mandatory, separately domain-bound P-256 identity proofs after handshake.
//! NN by itself is NOT authenticated. No application requests may precede those proofs.
use crate::crypto::encode;
use anyhow::{Result, ensure};
use zeroize::Zeroizing;

pub const PATTERN: &str = "Noise_NN_25519_ChaChaPoly_SHA256";
pub const PROLOGUE: &[u8] = b"ContainerHub.Remote.v1;auth=P256-SHA256;framing=1";
pub const MAX_FRAME: usize = 65_535;
pub const MAX_RECORD: usize = 1024 * 1024;
const CHUNK: usize = 60 * 1024;

pub struct Handshake(snow::HandshakeState);
impl Handshake {
    pub fn new(initiator: bool) -> Result<Self> {
        let builder = snow::Builder::new(PATTERN.parse()?).prologue(PROLOGUE)?;
        Ok(Self(if initiator {
            builder.build_initiator()?
        } else {
            builder.build_responder()?
        }))
    }
    pub fn write(&mut self) -> Result<Vec<u8>> {
        let mut buffer = vec![0; MAX_FRAME];
        let n = self.0.write_message(&[], &mut buffer)?;
        buffer.truncate(n);
        Ok(buffer)
    }
    pub fn read(&mut self, message: &[u8]) -> Result<()> {
        ensure!(message.len() <= 256, "handshake message too large");
        let mut buffer = [0; 256];
        ensure!(
            self.0.read_message(message, &mut buffer)? == 0,
            "unexpected handshake payload"
        );
        Ok(())
    }
    pub fn finish(self) -> Result<Channel> {
        ensure!(self.0.is_handshake_finished(), "handshake incomplete");
        let binding = encode(self.0.get_handshake_hash());
        Ok(Channel {
            state: self.0.into_transport_mode()?,
            binding,
            incoming: Zeroizing::new(vec![]),
            expected: None,
        })
    }
}

pub struct Channel {
    state: snow::TransportState,
    pub binding: String,
    incoming: Zeroizing<Vec<u8>>,
    expected: Option<usize>,
}
impl Channel {
    pub fn seal(&mut self, payload: &[u8]) -> Result<Vec<Vec<u8>>> {
        ensure!(
            !payload.is_empty() && payload.len() <= MAX_RECORD,
            "invalid record length"
        );
        let mut frames = vec![];
        for (index, part) in payload.chunks(CHUNK).enumerate() {
            let mut plain = Zeroizing::new(Vec::with_capacity(part.len() + 8));
            plain.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            plain.extend_from_slice(&((index * CHUNK) as u32).to_be_bytes());
            plain.extend_from_slice(part);
            let mut frame = vec![0; plain.len() + 16];
            let n = self.state.write_message(&plain, &mut frame)?;
            frame.truncate(n);
            frames.push(frame);
        }
        Ok(frames)
    }
    pub fn open(&mut self, frame: &[u8]) -> Result<Option<Vec<u8>>> {
        ensure!(frame.len() <= MAX_FRAME, "encrypted frame too large");
        let mut plain = Zeroizing::new(vec![0; frame.len()]);
        let n = self.state.read_message(frame, &mut plain)?;
        ensure!(n > 8, "empty or truncated record fragment");
        let total = u32::from_be_bytes(plain[0..4].try_into()?) as usize;
        let offset = u32::from_be_bytes(plain[4..8].try_into()?) as usize;
        ensure!(
            total > 0 && total <= MAX_RECORD && offset == self.incoming.len(),
            "invalid record fragment position"
        );
        ensure!(
            self.expected.is_none_or(|old| old == total),
            "interleaved records"
        );
        ensure!(offset + n - 8 <= total, "record length overflow");
        self.expected = Some(total);
        if self.incoming.is_empty() {
            self.incoming.reserve_exact(total);
        }
        self.incoming.extend_from_slice(&plain[8..n]);
        if self.incoming.len() == total {
            self.expected = None;
            Ok(Some(std::mem::take(&mut *self.incoming)))
        } else {
            Ok(None)
        }
    }
}
