//! Versioned remote identity, durable ownership and end-to-end transport.
//! The production enrollment issuer is deliberately not implemented by a shared client secret.
pub mod channel;
#[cfg(feature = "native")]
pub mod client;
pub mod crypto;
pub mod identity;
#[cfg(feature = "native")]
pub mod network;
#[cfg(feature = "native")]
pub mod profile;
#[cfg(feature = "native")]
pub mod relay;
#[cfg(feature = "native")]
pub mod rpc;
#[cfg(feature = "native")]
pub mod server;
#[cfg(feature = "native")]
pub mod store;

#[cfg(feature = "browser")]
mod browser;

#[cfg(feature = "native")]
pub mod credential_store;
