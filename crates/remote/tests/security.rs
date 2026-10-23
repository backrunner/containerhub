use containerhub_remote::{channel::Handshake, crypto::*, identity::*, store::*};
use std::sync::{Arc, Barrier};

const TIME: u64 = 10_000;
fn identity(
    owner: &SecretKey,
    device: &SecretKey,
    revision: u64,
    revoked: Vec<String>,
) -> (Signed, Signed) {
    (
        owner
            .sign(
                GRANT_DOMAIN,
                &DeviceGrant {
                    version: VERSION,
                    owner: owner.public(),
                    device: device.public(),
                    serial: nonce(),
                    agents: Scope::All,
                    environments: Scope::All,
                    permissions: vec![Permission::Read, Permission::ManageTrust],
                    not_before: TIME,
                    expires_at: TIME + 3600,
                },
            )
            .unwrap(),
        owner
            .sign(
                POLICY_DOMAIN,
                &OwnerPolicy {
                    version: VERSION,
                    owner: owner.public(),
                    revision,
                    revoked_devices: revoked,
                    issued_at: TIME,
                    expires_at: TIME + 3600,
                },
            )
            .unwrap(),
    )
}
fn proof(
    agent: &PublicKey,
    issuer: &SecretKey,
    owner: &SecretKey,
    device: &SecretKey,
    channel: &str,
    revision: u64,
    revoked: Vec<String>,
) -> Signed {
    let (grant, policy) = identity(owner, device, revision, revoked);
    let credential = issuer
        .sign(
            APP_DOMAIN,
            &AppCredential {
                version: VERSION,
                device: device.public(),
                audience: "test".into(),
                not_before: TIME,
                expires_at: TIME + 3600,
            },
        )
        .unwrap();
    device
        .sign(
            CLIENT_DOMAIN,
            &ClientProof {
                version: VERSION,
                agent: agent.clone(),
                channel: channel.into(),
                claim: true,
                grant,
                policy,
                app_credential: Some(credential),
            },
        )
        .unwrap()
}
fn store(path: &std::path::Path, agent: &SecretKey, issuer: &SecretKey) -> BindingStore {
    BindingStore::initialize(
        path,
        Installation {
            version: VERSION,
            agent: agent.public(),
            app_issuer: issuer.public(),
            audience: "test".into(),
        },
    )
    .unwrap()
}

#[test]
fn simultaneous_claims_across_database_connections_have_exactly_one_owner() {
    let directory = tempfile::tempdir().unwrap();
    let issuer = SecretKey::generate();
    let agent = SecretKey::generate();
    drop(store(directory.path(), &agent, &issuer));
    let barrier = Arc::new(Barrier::new(8));
    let mut threads = vec![];
    for i in 0..8 {
        let path = directory.path().to_path_buf();
        let barrier = barrier.clone();
        let request = proof(
            &agent.public(),
            &issuer,
            &SecretKey::generate(),
            &SecretKey::generate(),
            &i.to_string(),
            1,
            vec![],
        );
        threads.push(std::thread::spawn(move || {
            let db = BindingStore::open(&path).unwrap();
            barrier.wait();
            db.authenticate(&request, &i.to_string(), TIME).is_ok()
        }));
    }
    assert_eq!(
        threads
            .into_iter()
            .map(|t| usize::from(t.join().unwrap()))
            .sum::<usize>(),
        1
    );
    BindingStore::open(directory.path()).unwrap();
}

#[test]
fn signatures_and_channel_binding_are_required_before_claim() {
    let directory = tempfile::tempdir().unwrap();
    let issuer = SecretKey::generate();
    let agent = SecretKey::generate();
    let db = store(directory.path(), &agent, &issuer);
    let owner = SecretKey::generate();
    let device = SecretKey::generate();
    let signed = proof(&agent.public(), &issuer, &owner, &device, "one", 1, vec![]);
    assert!(db.authenticate(&signed, "two", TIME).is_err());
    let forged = proof(
        &agent.public(),
        &SecretKey::generate(),
        &owner,
        &device,
        "one",
        1,
        vec![],
    );
    assert!(db.authenticate(&forged, "one", TIME).is_err());
    let mut claims: ClientProof = signed.unverified().unwrap();
    claims.app_credential = None;
    assert!(
        db.authenticate(&device.sign(CLIENT_DOMAIN, &claims).unwrap(), "one", TIME)
            .is_err()
    );
    assert!(db.authenticate(&signed, "one", TIME + 3600).is_err());
    assert!(db.authenticate(&signed, "one", TIME).is_ok());
}

#[test]
fn owner_delegates_an_independent_device_and_revocation_ends_existing_authorization() {
    let directory = tempfile::tempdir().unwrap();
    let issuer = SecretKey::generate();
    let agent = SecretKey::generate();
    let db = store(directory.path(), &agent, &issuer);
    let owner = SecretKey::generate();
    let native = SecretKey::generate();
    let browser = SecretKey::generate();
    let first = proof(
        &agent.public(),
        &issuer,
        &owner,
        &native,
        "native",
        1,
        vec![],
    );
    db.authenticate(&first, "native", TIME).unwrap();
    let mut second: ClientProof = proof(
        &agent.public(),
        &issuer,
        &owner,
        &browser,
        "browser",
        1,
        vec![],
    )
    .unverified()
    .unwrap();
    // Same revision must reuse the SAME policy bytes, not create an unrelated policy.
    second.policy = first.unverified::<ClientProof>().unwrap().policy;
    second.claim = false;
    second.app_credential = None;
    let signed = browser.sign(CLIENT_DOMAIN, &second).unwrap();
    let (auth, _) = db.authenticate(&signed, "browser", TIME).unwrap();
    db.authorize(&auth, Permission::Read, "environment", TIME)
        .unwrap();
    assert!(
        db.authorize(&auth, Permission::Lifecycle, "environment", TIME)
            .is_err()
    );
    let revoke = proof(
        &agent.public(),
        &issuer,
        &owner,
        &native,
        "revoke",
        2,
        vec![browser.public().id()],
    );
    db.authenticate(&revoke, "revoke", TIME).unwrap();
    assert!(db.session_valid(&auth, TIME).is_err());
    assert!(db.authenticate(&signed, "browser", TIME).is_err());
    let resurrect = proof(
        &agent.public(),
        &issuer,
        &owner,
        &native,
        "restore",
        3,
        vec![],
    );
    assert!(db.authenticate(&resurrect, "restore", TIME).is_err());
}

#[test]
fn database_loss_or_pre_binding_restore_never_reopens_claiming() {
    let directory = tempfile::tempdir().unwrap();
    let issuer = SecretKey::generate();
    let agent = SecretKey::generate();
    let db = store(directory.path(), &agent, &issuer);
    db.authenticate(
        &proof(
            &agent.public(),
            &issuer,
            &SecretKey::generate(),
            &SecretKey::generate(),
            "bind",
            1,
            vec![],
        ),
        "bind",
        TIME,
    )
    .unwrap();
    drop(db);
    let connection = rusqlite::Connection::open(directory.path().join("binding.sqlite3")).unwrap();
    connection.execute("DELETE FROM binding", []).unwrap();
    drop(connection);
    assert!(BindingStore::open(directory.path()).is_err());
    std::fs::remove_file(directory.path().join("binding.sqlite3")).unwrap();
    assert!(BindingStore::open(directory.path()).is_err());
    assert!(
        BindingStore::initialize(
            directory.path(),
            Installation {
                version: VERSION,
                agent: agent.public(),
                app_issuer: issuer.public(),
                audience: "test".into()
            }
        )
        .is_err()
    );
}

#[test]
fn operation_ids_deduplicate_across_restart_and_reject_different_bodies() {
    let directory = tempfile::tempdir().unwrap();
    let db = store(
        directory.path(),
        &SecretKey::generate(),
        &SecretKey::generate(),
    );
    let id = nonce();
    assert!(
        db.accept_operation(&id, "owner", "start app", "queued")
            .unwrap()
            .0
    );
    db.finish_operation(&id, "done").unwrap();
    drop(db);
    let db = BindingStore::open(directory.path()).unwrap();
    assert_eq!(
        db.accept_operation(&id, "owner", "start app", "queued")
            .unwrap(),
        (false, "done".into())
    );
    assert!(
        db.accept_operation(&id, "owner", "delete app", "queued")
            .is_err()
    );
    assert!(
        db.accept_operation(&id, "other", "start app", "queued")
            .is_err()
    );
}

#[test]
fn noise_rejects_ciphertext_tampering_and_replay_and_reassembles_bounded_records() {
    let mut a = Handshake::new(true).unwrap();
    let mut b = Handshake::new(false).unwrap();
    b.read(&a.write().unwrap()).unwrap();
    a.read(&b.write().unwrap()).unwrap();
    let mut a = a.finish().unwrap();
    let mut b = b.finish().unwrap();
    assert_eq!(a.binding, b.binding);
    let payload = vec![b'x'; 256 * 1024];
    let frames = a.seal(&payload).unwrap();
    assert!(frames.len() > 1);
    let replay = frames[0].clone();
    let mut result = None;
    for frame in frames {
        result = b.open(&frame).unwrap();
    }
    assert_eq!(result.unwrap(), payload);
    assert!(b.open(&replay).is_err());
    let mut tampered = a.seal(b"secret command").unwrap().remove(0);
    tampered[10] ^= 0xff;
    assert!(b.open(&tampered).is_err());
    assert!(a.seal(&vec![0; 1024 * 1024 + 1]).is_err());
}
