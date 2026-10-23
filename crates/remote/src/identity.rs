use crate::crypto::*;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 1;
pub const MAX_LEASE_SECONDS: u64 = 24 * 60 * 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Read,
    Logs,
    Lifecycle,
    ManageTrust,
    ManageContainers,
    ManageImages,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "scope",
    content = "ids",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Scope {
    All,
    Only(Vec<String>),
}
impl Scope {
    pub fn allows(&self, id: &str) -> bool {
        match self {
            Self::All => true,
            Self::Only(ids) => ids.iter().any(|i| i == id),
        }
    }
    fn validate(&self) -> Result<()> {
        if let Self::Only(ids) = self {
            ensure!(
                !ids.is_empty()
                    && ids.len() <= 128
                    && ids.iter().all(|s| !s.is_empty() && s.len() <= 256),
                "invalid authorization scope"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceGrant {
    pub version: u32,
    pub owner: PublicKey,
    pub device: PublicKey,
    pub serial: String,
    pub agents: Scope,
    pub environments: Scope,
    pub permissions: Vec<Permission>,
    pub not_before: u64,
    pub expires_at: u64,
}
impl DeviceGrant {
    pub fn verify(document: &Signed, now: u64, agent: &str) -> Result<Self> {
        let candidate: Self = document.unverified()?;
        let grant: Self = candidate.owner.verify(GRANT_DOMAIN, document)?;
        ensure!(grant.version == VERSION, "unsupported grant version");
        grant.owner.validate()?;
        grant.device.validate()?;
        grant.agents.validate()?;
        grant.environments.validate()?;
        ensure!(
            grant.agents.allows(agent),
            "device is not authorized for this agent"
        );
        ensure!(
            !grant.serial.is_empty() && grant.serial.len() <= 128,
            "invalid grant serial"
        );
        ensure!(
            !grant.permissions.is_empty() && grant.permissions.len() <= 6,
            "invalid permissions"
        );
        ensure!(
            grant.not_before <= now && now < grant.expires_at,
            "device grant is not currently valid"
        );
        ensure!(
            grant.expires_at.saturating_sub(grant.not_before) <= 30 * MAX_LEASE_SECONDS,
            "device grant validity exceeds limit"
        );
        Ok(grant)
    }
    pub fn permits(&self, permission: Permission, environment: &str) -> bool {
        self.permissions.contains(&permission) && self.environments.allows(environment)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerPolicy {
    pub version: u32,
    pub owner: PublicKey,
    pub revision: u64,
    pub revoked_devices: Vec<String>,
    pub issued_at: u64,
    pub expires_at: u64,
}
impl OwnerPolicy {
    pub fn verify(document: &Signed, owner: &PublicKey, now: u64) -> Result<Self> {
        let policy: Self = owner.verify(POLICY_DOMAIN, document)?;
        ensure!(
            policy.version == VERSION && &policy.owner == owner,
            "invalid owner policy"
        );
        ensure!(
            policy.revision > 0 && policy.revision <= 9_007_199_254_740_991,
            "invalid policy revision"
        );
        ensure!(
            policy.revoked_devices.len() <= 512
                && policy.revoked_devices.iter().all(|s| s.len() <= 128),
            "revocation list too large"
        );
        ensure!(
            policy.issued_at <= now && now < policy.expires_at,
            "owner policy lease expired or clock incorrect"
        );
        ensure!(
            policy.expires_at.saturating_sub(policy.issued_at) <= MAX_LEASE_SECONDS,
            "policy lease too long"
        );
        Ok(policy)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppCredential {
    pub version: u32,
    pub device: PublicKey,
    pub audience: String,
    pub not_before: u64,
    pub expires_at: u64,
}
impl AppCredential {
    pub fn verify(
        document: &Signed,
        issuer: &PublicKey,
        device: &PublicKey,
        audience: &str,
        now: u64,
    ) -> Result<Self> {
        let credential: Self = issuer.verify(APP_DOMAIN, document)?;
        ensure!(
            credential.version == VERSION
                && &credential.device == device
                && credential.audience == audience,
            "invalid application credential"
        );
        ensure!(
            credential.not_before <= now && now < credential.expires_at,
            "application credential expired"
        );
        ensure!(
            credential.expires_at.saturating_sub(credential.not_before) <= MAX_LEASE_SECONDS,
            "application credential validity exceeds limit"
        );
        Ok(credential)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentProof {
    pub version: u32,
    pub agent: PublicKey,
    pub channel: String,
    pub policy: Option<Signed>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientProof {
    pub version: u32,
    pub agent: PublicKey,
    pub channel: String,
    pub claim: bool,
    pub grant: Signed,
    pub policy: Signed,
    pub app_credential: Option<Signed>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingReceipt {
    pub version: u32,
    pub agent: PublicKey,
    pub owner: PublicKey,
    pub bound_at: u64,
    pub policy_revision: u64,
}

/// Created only after signatures, channel binding and durable store checks succeed.
pub struct Authorization {
    pub(crate) grant: DeviceGrant,
}
impl Authorization {
    pub fn device_id(&self) -> String {
        self.grant.device.id()
    }
    pub fn owner_id(&self) -> String {
        self.grant.owner.id()
    }
    pub fn grant(&self) -> &DeviceGrant {
        &self.grant
    }
}
