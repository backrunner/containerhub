use crate::{
    client::{Credentials, Endpoint, RemoteClient},
    crypto::*,
    identity::*,
    store::*,
};
use anyhow::{Context, Result, ensure};
use containerhub_core::now;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, sync::Arc};

pub use crate::credential_store::KeyReference;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentConfig {
    pub version: u32,
    pub key: KeyReference,
    pub development: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub endpoint: Endpoint,
    pub receipt: BindingReceipt,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControllerProfile {
    pub version: u32,
    pub owner: PublicKey,
    pub device: PublicKey,
    pub root_key: Option<KeyReference>,
    pub device_key: KeyReference,
    pub grant: Signed,
    pub policy: Signed,
    pub app_credential: Option<Signed>,
    pub connections: Vec<Connection>,
    pub development: bool,
}
impl ControllerProfile {
    pub fn create(directory: &Path, development: bool) -> Result<Self> {
        private_directory(directory)?;
        ensure!(
            !directory.join("profile.json").exists(),
            "profile already exists"
        );
        let owner = SecretKey::generate();
        let device = SecretKey::generate();
        let time = now();
        let grant = owner.sign(
            GRANT_DOMAIN,
            &DeviceGrant {
                version: VERSION,
                owner: owner.public(),
                device: device.public(),
                serial: nonce(),
                agents: Scope::All,
                environments: Scope::All,
                permissions: vec![
                    Permission::Read,
                    Permission::Logs,
                    Permission::Lifecycle,
                    Permission::ManageTrust,
                    Permission::ManageContainers,
                    Permission::ManageImages,
                ],
                not_before: time,
                expires_at: time + 30 * MAX_LEASE_SECONDS,
            },
        )?;
        let policy = owner.sign(
            POLICY_DOMAIN,
            &OwnerPolicy {
                version: VERSION,
                owner: owner.public(),
                revision: 1,
                revoked_devices: vec![],
                issued_at: time,
                expires_at: time + 3600,
            },
        )?;
        let root_key = KeyReference::store(directory, &owner, development)?;
        let device_key = KeyReference::store(directory, &device, development)?;
        let profile = Self {
            version: VERSION,
            owner: owner.public(),
            device: device.public(),
            root_key: Some(root_key),
            device_key,
            grant,
            policy,
            app_credential: None,
            connections: vec![],
            development,
        };
        create_private(
            &directory.join("profile.json"),
            &serde_json::to_vec_pretty(&profile)?,
        )?;
        Ok(profile)
    }
    pub fn load(directory: &Path) -> Result<Self> {
        let mut profile: Self = read_json(&directory.join("profile.json"))?;
        ensure!(
            !profile.development || cfg!(feature = "development"),
            "development state requires an explicit development build"
        );
        ensure!(profile.version == VERSION, "unsupported controller profile");
        profile.owner.validate()?;
        profile.device.validate()?;
        let grant: DeviceGrant = profile.owner.verify(GRANT_DOMAIN, &profile.grant)?;
        ensure!(
            grant.owner == profile.owner && grant.device == profile.device,
            "profile grant identity mismatch"
        );
        let cache = directory.join("policy-cache.json");
        if cache.try_exists()? {
            profile.policy =
                crate::client::newer_policy(&profile.owner, &profile.policy, &read_json(&cache)?)?;
        }
        Ok(profile)
    }
    pub fn save(&self, directory: &Path) -> Result<()> {
        replace_private(
            &directory.join("profile.json"),
            &serde_json::to_vec_pretty(self)?,
        )
    }
    pub fn credentials(&self, directory: &Path) -> Result<Credentials> {
        let device = self.device_key.load(directory)?;
        ensure!(
            device.public() == self.device,
            "profile device key mismatch"
        );
        let authority = if self.root_key.is_some() {
            Some(self.owner_key(directory)?)
        } else {
            None
        };
        Ok(Credentials::new(
            device,
            self.grant.clone(),
            self.policy.clone(),
            self.app_credential.clone(),
            authority,
        )
        .with_policy_path(directory.join("policy-cache.json")))
    }
    pub fn owner_key(&self, directory: &Path) -> Result<SecretKey> {
        let key = self
            .root_key
            .as_ref()
            .context("this device cannot authorize other devices")?
            .load(directory)?;
        ensure!(key.public() == self.owner, "owner key mismatch");
        Ok(key)
    }
    pub fn enable_management(&mut self, directory: &Path) -> Result<()> {
        let root = self.owner_key(directory)?;
        let mut grant: DeviceGrant = self.owner.verify(GRANT_DOMAIN, &self.grant)?;
        ensure!(
            grant.owner == self.owner && grant.device == self.device,
            "grant identity mismatch"
        );
        for permission in [Permission::ManageContainers, Permission::ManageImages] {
            if !grant.permissions.contains(&permission) {
                grant.permissions.push(permission);
            }
        }
        grant.serial = nonce();
        grant.not_before = now();
        grant.expires_at = now() + 30 * MAX_LEASE_SECONDS;
        self.grant = root.sign(GRANT_DOMAIN, &grant)?;
        self.save(directory)
    }
    pub fn refresh_policy(&mut self, directory: &Path, revoke: Option<&str>) -> Result<()> {
        let root = self.owner_key(directory)?;
        let mut policy: OwnerPolicy = self.owner.verify(POLICY_DOMAIN, &self.policy)?;
        ensure!(
            policy.owner == self.owner && policy.version == VERSION,
            "policy identity mismatch"
        );
        if let Some(device) = revoke {
            ensure!(
                device != self.device.id(),
                "cannot revoke the last local administrator through this command"
            );
            if !policy.revoked_devices.iter().any(|d| d == device) {
                policy.revoked_devices.push(device.into());
            }
        }
        policy.revision = policy
            .revision
            .checked_add(1)
            .context("policy revision exhausted")?;
        policy.issued_at = now();
        policy.expires_at = now() + 3600;
        self.policy = root.sign(POLICY_DOMAIN, &policy)?;
        OwnerPolicy::verify(&self.policy, &self.owner, now())?;
        replace_private(
            &directory.join("policy-cache.json"),
            &serde_json::to_vec(&self.policy)?,
        )?;
        self.save(directory)
    }
    pub fn add_connection(&mut self, directory: &Path, connection: Connection) -> Result<()> {
        ensure!(
            connection.receipt.owner == self.owner,
            "binding owner mismatch"
        );
        if let Some(existing) = self
            .connections
            .iter_mut()
            .find(|c| c.receipt.agent == connection.receipt.agent)
        {
            *existing = connection;
        } else {
            self.connections.push(connection);
        }
        self.save(directory)
    }
    pub fn client(&self, directory: &Path, agent_id: Option<&str>) -> Result<RemoteClient> {
        let connection = match agent_id {
            Some(id) => self
                .connections
                .iter()
                .find(|c| c.receipt.agent.id() == id)
                .context("unknown bound agent")?,
            None => {
                ensure!(
                    self.connections.len() == 1,
                    "select a bound Agent ID when there are zero or multiple environments"
                );
                &self.connections[0]
            }
        };
        Ok(RemoteClient {
            endpoint: connection.endpoint.clone(),
            credentials: Arc::new(self.credentials(directory)?),
            pin: connection.receipt.agent.clone(),
            development: self.development,
        })
    }
}
pub fn instance_lock(directory: &Path) -> Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(directory.join("instance.lock"))?;
    file.try_lock()
        .context("state is already in use by another process")?;
    Ok(file)
}
