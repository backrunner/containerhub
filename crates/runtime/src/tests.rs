use super::*;
use containerhub_remote::{
    client::Endpoint,
    crypto::SecretKey,
    identity::{BindingReceipt, VERSION},
    profile::{Connection, ControllerProfile, instance_lock},
};
#[test]
fn one_profile_can_open_multiple_agents_without_releasing_its_exclusive_lock() {
    let directory = std::env::temp_dir().join(format!(
        "containerhub-runtime-test-{}-{}",
        std::process::id(),
        containerhub_core::now()
    ));
    let mut profile = ControllerProfile::create(&directory, true).unwrap();
    let mut agents = Vec::new();
    for _ in 0..2 {
        let agent = SecretKey::generate().public();
        agents.push(agent.id());
        profile
            .add_connection(
                &directory,
                Connection {
                    endpoint: Endpoint::Direct {
                        url: "ws://127.0.0.1:1".into(),
                    },
                    receipt: BindingReceipt {
                        version: VERSION,
                        agent,
                        owner: profile.owner.clone(),
                        bound_at: containerhub_core::now(),
                        policy_revision: 1,
                    },
                },
            )
            .unwrap();
    }
    let opened = RemoteProfile::open(&directory).unwrap();
    assert_eq!(opened.agents().len(), 2);
    let first = opened.runtime(Some(&agents[0])).unwrap();
    let second = opened.runtime(Some(&agents[1])).unwrap();
    assert_ne!(first.storage_id, second.storage_id);
    assert!(opened.runtime(None).is_err());
    assert!(opened.runtime(Some("unknown-agent")).is_err());
    drop(opened);
    assert!(instance_lock(&directory).is_err());
    drop(first);
    assert!(instance_lock(&directory).is_err());
    drop(second);
    let lock = instance_lock(&directory).unwrap();
    drop(lock);
    std::fs::remove_dir_all(directory).unwrap();
}
