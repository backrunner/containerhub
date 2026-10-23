use crate::{crypto::*, identity::*};
use anyhow::{Context, Result, ensure};
use std::{path::PathBuf, sync::Mutex};
pub struct Credentials {
    pub(super) device: SecretKey,
    grant: Signed,
    policy: Mutex<Signed>,
    pub(super) app_credential: Option<Signed>,
    authority: Option<SecretKey>,
    policy_path: Option<PathBuf>,
}
impl Credentials {
    pub fn new(
        device: SecretKey,
        grant: Signed,
        policy: Signed,
        app_credential: Option<Signed>,
        authority: Option<SecretKey>,
    ) -> Self {
        Self {
            device,
            grant,
            policy: Mutex::new(policy),
            app_credential,
            authority,
            policy_path: None,
        }
    }
    pub(crate) fn with_policy_path(mut self, path: PathBuf) -> Self {
        self.policy_path = Some(path);
        self
    }
    pub fn latest_policy(&self) -> Result<Signed> {
        Ok(self
            .policy
            .lock()
            .map_err(|_| anyhow::anyhow!("policy lock poisoned"))?
            .clone())
    }
    pub(super) fn owner_id(&self) -> Result<String> {
        let candidate: DeviceGrant = self.grant.unverified()?;
        let grant: DeviceGrant = candidate.owner.verify(GRANT_DOMAIN, &self.grant)?;
        ensure!(
            grant.device == self.device.public(),
            "device key does not match grant"
        );
        Ok(grant.owner.id())
    }
    pub(super) fn documents(
        &self,
        remote: Option<&Signed>,
        agent: &str,
        time: u64,
    ) -> Result<(DeviceGrant, Signed, Signed)> {
        let candidate: DeviceGrant = self.grant.unverified()?;
        let mut grant: DeviceGrant = candidate.owner.verify(GRANT_DOMAIN, &self.grant)?;
        ensure!(
            grant.device == self.device.public(),
            "device key does not match grant"
        );
        // Verify structure and scope at issuance even if a root-authorized lease needs renewal.
        DeviceGrant::verify(&self.grant, grant.not_before, agent)?;
        ensure!(
            grant.not_before <= time,
            "grant clock is ahead of local clock"
        );
        let mut cache = self
            .policy
            .lock()
            .map_err(|_| anyhow::anyhow!("policy lock poisoned"))?;
        let mut signed = match remote {
            Some(remote) => newer_policy(&grant.owner, &cache, remote)?,
            None => cache.clone(),
        };
        let mut policy: OwnerPolicy = grant.owner.verify(POLICY_DOMAIN, &signed)?;
        OwnerPolicy::verify(&signed, &grant.owner, policy.issued_at)?;
        ensure!(
            policy.issued_at <= time,
            "policy clock is ahead of local clock"
        );
        ensure!(
            !policy.revoked_devices.contains(&grant.device.id()),
            "device revoked"
        );
        let mut signed_grant = self.grant.clone();
        if let Some(root) = &self.authority {
            ensure!(root.public() == grant.owner, "owner authority mismatch");
            if policy.expires_at <= time.saturating_add(60) {
                policy.revision = policy
                    .revision
                    .checked_add(1)
                    .context("policy revision exhausted")?;
                policy.issued_at = time;
                policy.expires_at = time + 3600;
                signed = root.sign(POLICY_DOMAIN, &policy)?;
            }
            if grant.expires_at <= time.saturating_add(300) {
                grant.not_before = time;
                grant.expires_at = time + 30 * MAX_LEASE_SECONDS;
                grant.serial = nonce();
                signed_grant = root.sign(GRANT_DOMAIN, &grant)?;
            }
        }
        let grant = DeviceGrant::verify(&signed_grant, time, agent)?;
        OwnerPolicy::verify(&signed, &grant.owner, time)?;
        if signed != *cache {
            // Persist before sending: a restart must not sign another policy at this revision.
            if let Some(path) = &self.policy_path {
                crate::store::replace_private(path, &serde_json::to_vec(&signed)?)?;
            }
            *cache = signed.clone();
        }
        Ok((grant, signed_grant, signed))
    }
}
pub(crate) fn newer_policy(owner: &PublicKey, local: &Signed, remote: &Signed) -> Result<Signed> {
    let a: OwnerPolicy = owner.verify(POLICY_DOMAIN, local)?;
    let b: OwnerPolicy = owner.verify(POLICY_DOMAIN, remote)?;
    OwnerPolicy::verify(local, owner, a.issued_at)?;
    OwnerPolicy::verify(remote, owner, b.issued_at)?;
    ensure!(
        a.revision != b.revision || local.payload == remote.payload,
        "conflicting owner policy"
    );
    let (older, newer, document) = if a.revision > b.revision {
        (&b, &a, local)
    } else {
        (&a, &b, remote)
    };
    ensure!(
        older
            .revoked_devices
            .iter()
            .all(|id| newer.revoked_devices.contains(id)),
        "owner policy removed revocation records"
    );
    Ok(document.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn fixture(authority: bool) -> (Credentials, PublicKey) {
        let root = SecretKey::generate();
        let device = SecretKey::generate();
        let owner = root.public();
        let grant = root
            .sign(
                GRANT_DOMAIN,
                &DeviceGrant {
                    version: VERSION,
                    owner: owner.clone(),
                    device: device.public(),
                    serial: nonce(),
                    agents: Scope::Only(vec!["agent-a".into()]),
                    environments: Scope::Only(vec!["env-a".into()]),
                    permissions: vec![Permission::Read],
                    not_before: 100,
                    expires_at: 200,
                },
            )
            .unwrap();
        let policy = root
            .sign(
                POLICY_DOMAIN,
                &OwnerPolicy {
                    version: VERSION,
                    owner: owner.clone(),
                    revision: 5,
                    revoked_devices: vec!["previously-revoked-device".into()],
                    issued_at: 100,
                    expires_at: 200,
                },
            )
            .unwrap();
        (
            Credentials::new(device, grant, policy, None, authority.then_some(root)),
            owner,
        )
    }

    #[test]
    fn root_renews_expired_leases_without_expanding_scope_or_losing_revocations() {
        let (credentials, owner) = fixture(true);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("policy-cache.json");
        let credentials = credentials.with_policy_path(path.clone());
        let (grant, _, signed) = credentials.documents(None, "agent-a", 201).unwrap();
        assert_eq!(grant.permissions, vec![Permission::Read]);
        assert_eq!(grant.environments, Scope::Only(vec!["env-a".into()]));
        assert_eq!(grant.not_before, 201);
        let policy = OwnerPolicy::verify(&signed, &owner, 201).unwrap();
        assert_eq!(policy.revision, 6);
        assert_eq!(policy.revoked_devices, vec!["previously-revoked-device"]);
        let persisted: Signed = crate::store::read_json(&path).unwrap();
        assert_eq!(persisted, signed);
        assert!(credentials.documents(None, "agent-b", 201).is_err());
        let (_, _, next) = credentials.documents(None, "agent-a", 202).unwrap();
        assert_eq!(next, signed);
    }

    #[test]
    fn concurrent_renewals_share_one_revision_and_signed_document() {
        let (credentials, _) = fixture(true);
        let credentials = Arc::new(credentials);
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let credentials = credentials.clone();
                std::thread::spawn(move || credentials.documents(None, "agent-a", 201).unwrap().2)
            })
            .collect();
        let documents: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert!(documents.iter().all(|signed| signed == &documents[0]));
    }

    #[test]
    fn delegated_device_cannot_renew_or_accept_conflicting_revocations() {
        let (credentials, owner) = fixture(false);
        assert!(credentials.documents(None, "agent-a", 201).is_err());
        assert!(credentials.documents(None, "agent-a", 199).is_ok());
        let (root_credentials, root_owner) = fixture(true);
        let root = root_credentials.authority.as_ref().unwrap();
        let current = root_credentials.latest_policy().unwrap();
        let mut policy: OwnerPolicy = root_owner.verify(POLICY_DOMAIN, &current).unwrap();
        policy.revoked_devices.clear();
        let conflict = root.sign(POLICY_DOMAIN, &policy).unwrap();
        assert!(newer_policy(&root_owner, &current, &conflict).is_err());
        policy.revision += 1;
        let rollback = root.sign(POLICY_DOMAIN, &policy).unwrap();
        assert!(newer_policy(&root_owner, &current, &rollback).is_err());
        assert!(newer_policy(&owner, &current, &rollback).is_err());
    }
}
