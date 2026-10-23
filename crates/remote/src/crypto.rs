use anyhow::{Result, bail, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use p256::ecdsa::{
    Signature, SigningKey, VerifyingKey,
    signature::{Signer, Verifier},
};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub const GRANT_DOMAIN: &str = "containerhub.device-grant.v1";
pub const POLICY_DOMAIN: &str = "containerhub.owner-policy.v1";
pub const APP_DOMAIN: &str = "containerhub.app-credential.v1";
pub const AGENT_DOMAIN: &str = "containerhub.agent-proof.v1";
pub const CLIENT_DOMAIN: &str = "containerhub.client-proof.v1";
pub const RELAY_DOMAIN: &str = "containerhub.relay-proof.v1";

pub fn encode(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}
pub fn decode(value: &str) -> Result<Vec<u8>> {
    Ok(URL_SAFE_NO_PAD.decode(value)?)
}
pub fn digest(bytes: &[u8]) -> String {
    encode(&Sha256::digest(bytes))
}
pub fn nonce() -> String {
    let mut bytes = [0; 32];
    OsRng.fill_bytes(&mut bytes);
    encode(&bytes)
}

/// SEC1 uncompressed P-256 public key, compatible with WebCrypto's `raw` format.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PublicKey(pub String);
impl PublicKey {
    pub fn validate(&self) -> Result<()> {
        let bytes = decode(&self.0)?;
        ensure!(
            bytes.len() == 65 && bytes[0] == 4,
            "invalid P-256 public key"
        );
        VerifyingKey::from_sec1_bytes(&bytes)?;
        Ok(())
    }
    pub fn id(&self) -> String {
        format!("p256:{}", digest(self.0.as_bytes()))
    }
    pub fn verify<T: DeserializeOwned>(&self, domain: &str, document: &Signed) -> Result<T> {
        self.validate()?;
        ensure!(
            document.payload.len() <= 64 * 1024,
            "signed payload too large"
        );
        let payload = decode(&document.payload)?;
        let signature = Signature::from_slice(&decode(&document.signature)?)?;
        VerifyingKey::from_sec1_bytes(&decode(&self.0)?)?
            .verify(&signing_input(domain, &payload), &signature)?;
        Ok(serde_json::from_slice(&payload)?)
    }
}

/// Never Debug/Serialize a private key. Persistence is an explicit credential-store operation.
pub struct SecretKey(SigningKey);
impl SecretKey {
    pub fn generate() -> Self {
        Self(SigningKey::random(&mut OsRng))
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Ok(Self(SigningKey::from_slice(bytes)?))
    }
    pub fn secret_bytes(&self) -> Zeroizing<Vec<u8>> {
        Zeroizing::new(self.0.to_bytes().to_vec())
    }
    pub fn public(&self) -> PublicKey {
        PublicKey(encode(
            self.0.verifying_key().to_encoded_point(false).as_bytes(),
        ))
    }
    pub fn sign(&self, domain: &str, value: &impl Serialize) -> Result<Signed> {
        let payload = serde_json::to_vec(value)?;
        ensure!(payload.len() <= 48 * 1024, "signed payload too large");
        let signature: Signature = self.0.sign(&signing_input(domain, &payload));
        Ok(Signed {
            payload: encode(&payload),
            signature: encode(signature.to_bytes().as_slice()),
        })
    }
}

/// Signature covers the exact decoded payload bytes, not JSON re-serialization.
/// P-256/SHA-256 signature uses IEEE P1363 r||s (64 bytes), as WebCrypto does.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signed {
    pub payload: String,
    pub signature: String,
}
impl Signed {
    /// Only for finding the candidate verification key. Never authorize from unverified data.
    pub fn unverified<T: DeserializeOwned>(&self) -> Result<T> {
        if self.payload.len() > 64 * 1024 {
            bail!("signed payload too large");
        }
        Ok(serde_json::from_slice(&decode(&self.payload)?)?)
    }
}
pub fn signing_input(domain: &str, payload: &[u8]) -> Vec<u8> {
    let mut input = b"ContainerHub-Signature\0".to_vec();
    input.extend_from_slice(&(domain.len() as u32).to_be_bytes());
    input.extend_from_slice(domain.as_bytes());
    input.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    input.extend_from_slice(payload);
    input
}
