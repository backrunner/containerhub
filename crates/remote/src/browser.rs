//! Browser owns its WebCrypto signing key. WASM only handles ephemeral Noise channel keys.
use crate::{
    channel::{Channel, Handshake},
    crypto::{PublicKey, Signed},
};
use wasm_bindgen::prelude::*;

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
#[wasm_bindgen]
pub struct BrowserChannel {
    handshake: Option<Handshake>,
    channel: Option<Channel>,
}
#[wasm_bindgen]
impl BrowserChannel {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<BrowserChannel, JsValue> {
        Ok(Self {
            handshake: Some(Handshake::new(true).map_err(js_error)?),
            channel: None,
        })
    }
    pub fn start(&mut self) -> Result<Vec<u8>, JsValue> {
        self.handshake
            .as_mut()
            .ok_or_else(|| js_error("handshake already complete"))?
            .write()
            .map_err(js_error)
    }
    pub fn finish(&mut self, response: &[u8]) -> Result<String, JsValue> {
        let mut handshake = self
            .handshake
            .take()
            .ok_or_else(|| js_error("handshake already complete"))?;
        handshake.read(response).map_err(js_error)?;
        let channel = handshake.finish().map_err(js_error)?;
        let binding = channel.binding.clone();
        self.channel = Some(channel);
        Ok(binding)
    }
    /// Batch encoding for JS only: each outgoing WebSocket frame has a u32 BE length prefix here.
    /// Strip these batch lengths before sending frames; they are not part of the wire protocol.
    pub fn seal(&mut self, json: &str) -> Result<Vec<u8>, JsValue> {
        let frames = self
            .channel
            .as_mut()
            .ok_or_else(|| js_error("handshake incomplete"))?
            .seal(json.as_bytes())
            .map_err(js_error)?;
        let mut output = vec![];
        for frame in frames {
            output.extend_from_slice(&(frame.len() as u32).to_be_bytes());
            output.extend_from_slice(&frame);
        }
        Ok(output)
    }
    pub fn open(&mut self, frame: &[u8]) -> Result<Option<String>, JsValue> {
        self.channel
            .as_mut()
            .ok_or_else(|| js_error("handshake incomplete"))?
            .open(frame)
            .map_err(js_error)?
            .map(String::from_utf8)
            .transpose()
            .map_err(js_error)
    }
}
#[wasm_bindgen]
pub fn verify_document(public_key: &str, domain: &str, document: &str) -> Result<String, JsValue> {
    let document: Signed = serde_json::from_str(document).map_err(js_error)?;
    let value: serde_json::Value = PublicKey(public_key.into())
        .verify(domain, &document)
        .map_err(js_error)?;
    serde_json::to_string(&value).map_err(js_error)
}
