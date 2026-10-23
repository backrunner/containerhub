use anyhow::{Context, Result};
use containerhub_core::now;
use containerhub_remote::{crypto::*, identity::*, profile::*, store::*};
use std::path::PathBuf;

fn main() -> Result<()> {
    let directory = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .context("usage: development_fixture <new-state-directory>")?,
    );
    anyhow::ensure!(!directory.exists(), "fixture directory must be new");
    private_directory(&directory)?;
    let agent_dir = directory.join("agent");
    private_directory(&agent_dir)?;
    let profile_dir = directory.join("controller");
    private_directory(&profile_dir)?;
    let issuer = SecretKey::generate();
    let agent = SecretKey::generate();
    BindingStore::initialize(
        &agent_dir,
        Installation {
            version: VERSION,
            agent: agent.public(),
            app_issuer: issuer.public(),
            audience: "containerhub.development.v1".into(),
        },
    )?;
    let reference = KeyReference::store(&agent_dir, &agent, true)?;
    create_private(
        &agent_dir.join("agent.json"),
        &serde_json::to_vec(&AgentConfig {
            version: VERSION,
            key: reference,
            development: true,
        })?,
    )?;
    let mut controller = ControllerProfile::create(&profile_dir, true)?;
    controller.app_credential = Some(issuer.sign(
        APP_DOMAIN,
        &AppCredential {
            version: VERSION,
            device: controller.device.clone(),
            audience: "containerhub.development.v1".into(),
            not_before: now(),
            expires_at: now() + 3600,
        },
    )?);
    controller.save(&profile_dir)?;
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"agent_state":agent_dir,"controller_profile":profile_dir,"agent_id":agent.public().id(),"runtime":"isolated fake provider","expires_in_seconds":3600})
        )?
    );
    Ok(())
}
