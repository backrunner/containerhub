mod files;
mod operations;
use crate::{crypto::*, identity::*};
use anyhow::{Context, Result, ensure};
use files::sync_directory;
pub use files::{create_private, private_directory, read_json, replace_private};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Installation {
    pub version: u32,
    pub agent: PublicKey,
    pub app_issuer: PublicKey,
    pub audience: String,
}

/// One database is shared by both transports. Opening never creates a new installation.
pub struct BindingStore {
    connection: Mutex<Connection>,
    directory: PathBuf,
    pub installation: Installation,
}
impl BindingStore {
    pub fn initialize(directory: &Path, installation: Installation) -> Result<Self> {
        installation.agent.validate()?;
        installation.app_issuer.validate()?;
        ensure!(
            installation.version == VERSION && !installation.audience.is_empty(),
            "invalid installation"
        );
        private_directory(directory)?;
        // Create the marker first: an interrupted initialization must fail closed on open.
        create_private(
            &directory.join("installation.json"),
            &serde_json::to_vec(&installation)?,
        )?;
        let db = directory.join("binding.sqlite3");
        create_private(&db, &[])?;
        let connection = connect(&db)?;
        connection.execute_batch(
            "BEGIN IMMEDIATE;
            PRAGMA application_id = 1128813890;
            PRAGMA user_version = 1;
            CREATE TABLE installation (id INTEGER PRIMARY KEY CHECK(id=1), agent TEXT NOT NULL);
            CREATE TABLE binding (id INTEGER PRIMARY KEY CHECK(id=1), owner TEXT NOT NULL,
                bound_at INTEGER NOT NULL, revision INTEGER NOT NULL, policy_hash TEXT NOT NULL,
                expires_at INTEGER NOT NULL, last_time INTEGER NOT NULL, policy TEXT NOT NULL);
            CREATE TABLE revoked (device TEXT PRIMARY KEY);
            CREATE TABLE operations (id TEXT PRIMARY KEY, owner TEXT NOT NULL, digest TEXT NOT NULL,
                request TEXT NOT NULL, result TEXT NOT NULL);
            COMMIT;",
        )?;
        connection.execute(
            "INSERT INTO installation VALUES(1, ?1)",
            [installation.agent.0.as_str()],
        )?;
        sync_directory(directory)?;
        Ok(Self {
            connection: Mutex::new(connection),
            directory: directory.into(),
            installation,
        })
    }
    pub fn open(directory: &Path) -> Result<Self> {
        let installation: Installation = read_json(&directory.join("installation.json"))?;
        ensure!(
            installation.version == VERSION,
            "unsupported installation version"
        );
        installation.agent.validate()?;
        installation.app_issuer.validate()?;
        let mut connection = connect(&directory.join("binding.sqlite3"))?;
        let version: i64 = connection.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let app: i64 = connection.query_row("PRAGMA application_id", [], |r| r.get(0))?;
        ensure!(
            version == 1 && app == 1128813890,
            "binding database requires recovery"
        );
        let key: String =
            connection.query_row("SELECT agent FROM installation WHERE id=1", [], |r| {
                r.get(0)
            })?;
        ensure!(
            key == installation.agent.0,
            "agent identity does not match installation"
        );
        let integrity: String = connection.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        ensure!(integrity == "ok", "binding database requires recovery");
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        verify_checkpoint(&tx, directory)?;
        tx.commit()?;
        Ok(Self {
            connection: Mutex::new(connection),
            directory: directory.into(),
            installation,
        })
    }

    pub fn authenticate(
        &self,
        signed: &Signed,
        channel: &str,
        now: u64,
    ) -> Result<(Authorization, BindingReceipt)> {
        let candidate: ClientProof = signed.unverified()?;
        let grant = DeviceGrant::verify(&candidate.grant, now, &self.installation.agent.id())?;
        let proof: ClientProof = grant.device.verify(CLIENT_DOMAIN, signed)?;
        ensure!(
            proof.version == VERSION
                && proof.agent == self.installation.agent
                && proof.channel == channel,
            "client proof belongs to a different session or agent"
        );
        // Verify the exact grant in the authenticated envelope (not just a caller's candidate).
        ensure!(proof.grant == candidate.grant, "grant mismatch");
        let policy = OwnerPolicy::verify(&proof.policy, &grant.owner, now)?;
        ensure!(
            !policy.revoked_devices.contains(&grant.device.id()),
            "device revoked"
        );
        let policy_hash = digest(&decode(&proof.policy.payload)?);
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("store lock poisoned"))?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        verify_checkpoint(&tx, &self.directory)?;
        let existing: Option<(String, u64, u64, String, u64)> = tx
            .query_row(
                "SELECT owner, bound_at, revision, policy_hash, last_time FROM binding WHERE id=1",
                [],
                |r| {
                    Ok((
                        r.get(0)?,
                        read_u64(r, 1)?,
                        read_u64(r, 2)?,
                        r.get(3)?,
                        read_u64(r, 4)?,
                    ))
                },
            )
            .optional()?;
        let bound_at;
        if let Some((owner, time, revision, hash, last_time)) = existing {
            ensure!(
                owner == grant.owner.0,
                "agent is already bound to another owner"
            );
            ensure!(
                now.saturating_add(60) >= last_time,
                "clock rollback detected"
            );
            ensure!(
                policy.revision >= revision,
                "stale owner policy; synchronize trusted device records"
            );
            ensure!(
                policy.revision != revision || hash == policy_hash,
                "conflicting policy at the same revision"
            );
            // Revocations are tombstones: a newer snapshot cannot silently resurrect old keys.
            let mut statement = tx.prepare("SELECT device FROM revoked")?;
            for device in statement.query_map([], |r| r.get::<_, String>(0))? {
                ensure!(
                    policy.revoked_devices.contains(&device?),
                    "policy would remove a revocation"
                );
            }
            bound_at = time;
        } else {
            ensure!(
                proof.claim,
                "agent is unbound; an explicit first claim is required"
            );
            ensure!(
                grant.permissions.contains(&Permission::ManageTrust),
                "first claimant requires owner administration permission"
            );
            let credential = proof
                .app_credential
                .as_ref()
                .context("verified application credential required for first claim")?;
            AppCredential::verify(
                credential,
                &self.installation.app_issuer,
                &grant.device,
                &self.installation.audience,
                now,
            )?;
            bound_at = now;
        }
        tx.execute("INSERT INTO binding VALUES(1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET revision=excluded.revision, policy_hash=excluded.policy_hash,
                expires_at=excluded.expires_at, last_time=MAX(binding.last_time,excluded.last_time), policy=excluded.policy",
            params![grant.owner.0, i64::try_from(bound_at)?, i64::try_from(policy.revision)?, policy_hash, i64::try_from(policy.expires_at)?, i64::try_from(now)?, serde_json::to_string(&proof.policy)?])?;
        for device in &policy.revoked_devices {
            tx.execute("INSERT OR IGNORE INTO revoked VALUES(?1)", [device])?;
        }
        // Prepare the external checkpoint before commit. Any interruption between these durable
        // writes leaves a mismatch requiring recovery, never a silently reopened installation.
        let checkpoint = Checkpoint {
            owner: grant.owner.0.clone(),
            revision: policy.revision,
            policy_hash,
        };
        replace_private(
            &self.directory.join("binding-checkpoint.json"),
            &serde_json::to_vec(&checkpoint)?,
        )?;
        tx.commit()?;
        let receipt = BindingReceipt {
            version: VERSION,
            agent: self.installation.agent.clone(),
            owner: grant.owner.clone(),
            bound_at,
            policy_revision: policy.revision,
        };
        Ok((Authorization { grant }, receipt))
    }

    pub fn current_policy(&self) -> Result<Option<Signed>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("store lock poisoned"))?;
        let json: Option<String> = connection
            .query_row("SELECT policy FROM binding WHERE id=1", [], |r| r.get(0))
            .optional()?;
        json.map(|json| serde_json::from_str(&json).map_err(Into::into))
            .transpose()
    }

    pub fn session_valid(&self, auth: &Authorization, now: u64) -> Result<()> {
        ensure!(
            auth.grant.not_before <= now && now < auth.grant.expires_at,
            "device grant expired"
        );
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("store lock poisoned"))?;
        let (owner, expires, last_time): (String, u64, u64) = connection.query_row(
            "SELECT owner, expires_at, last_time FROM binding WHERE id=1",
            [],
            |r| Ok((r.get(0)?, read_u64(r, 1)?, read_u64(r, 2)?)),
        )?;
        ensure!(
            owner == auth.grant.owner.0 && now < expires && now.saturating_add(60) >= last_time,
            "binding policy expired or changed"
        );
        let revoked: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM revoked WHERE device=?1)",
            [auth.device_id()],
            |r| r.get(0),
        )?;
        ensure!(!revoked, "device revoked");
        Ok(())
    }
    pub fn authorize(
        &self,
        auth: &Authorization,
        permission: Permission,
        environment: &str,
        now: u64,
    ) -> Result<()> {
        self.session_valid(auth, now)?;
        ensure!(
            auth.grant.permits(permission, environment),
            "operation outside device authorization"
        );
        Ok(())
    }
}

fn connect(path: &Path) -> Result<Connection> {
    ensure!(
        path.is_file() && !path.symlink_metadata()?.file_type().is_symlink(),
        "binding database missing or unsafe; recovery required"
    );
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(Duration::from_secs(3))?;
    connection.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;",
    )?;
    Ok(connection)
}
fn read_u64(row: &rusqlite::Row, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Checkpoint {
    owner: String,
    revision: u64,
    policy_hash: String,
}
fn verify_checkpoint(connection: &Connection, directory: &Path) -> Result<()> {
    let stored: Option<Checkpoint> = connection
        .query_row(
            "SELECT owner,revision,policy_hash FROM binding WHERE id=1",
            [],
            |r| {
                Ok(Checkpoint {
                    owner: r.get(0)?,
                    revision: read_u64(r, 1)?,
                    policy_hash: r.get(2)?,
                })
            },
        )
        .optional()?;
    let path = directory.join("binding-checkpoint.json");
    match stored {
        Some(checkpoint) => {
            let external: Checkpoint =
                read_json(&path).context("binding checkpoint missing; recovery required")?;
            ensure!(
                checkpoint == external,
                "binding database/checkpoint mismatch; recovery required"
            );
        }
        None => ensure!(
            !path.exists(),
            "binding database rolled back; recovery required"
        ),
    }
    Ok(())
}
