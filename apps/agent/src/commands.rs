use crate::cli::Command;
use anyhow::{Context, Result, ensure};
use containerhub_core::{LOCAL_ENVIRONMENT, ManagementClient, ProviderKind, now};
use containerhub_remote::{
    client::{self, Endpoint},
    crypto::*,
    identity::*,
    profile::*,
    server::Agent,
    store::*,
};
use std::sync::Arc;

pub(crate) async fn run(command: Command) -> Result<()> {
    match command {
        Command::Init { state, app_issuer } => {
            let issuer: PublicKey = read_json(&app_issuer)?;
            private_directory(&state)?;
            let _lock = instance_lock(&state)?;
            ensure!(
                !state.join("installation.json").exists(),
                "installation already exists"
            );
            let key = SecretKey::generate();
            BindingStore::initialize(
                &state,
                Installation {
                    version: VERSION,
                    agent: key.public(),
                    app_issuer: issuer,
                    audience: "containerhub.production.v1".into(),
                },
            )?;
            let reference = KeyReference::store(&state, &key, false)?;
            create_private(
                &state.join("agent.json"),
                &serde_json::to_vec(&AgentConfig {
                    version: VERSION,
                    key: reference,
                    development: false,
                })?,
            )?;
            println!("{}", serde_json::to_string(&key.public())?);
        }
        Command::Serve {
            state,
            listen,
            relay,
            apple_binary,
            docker_context,
            docker_binary,
        } => {
            let _lock = instance_lock(&state)?;
            let config: AgentConfig = read_json(&state.join("agent.json"))?;
            ensure!(config.version == VERSION, "unsupported Agent configuration");
            ensure!(
                !config.development || (apple_binary.is_none() && docker_context.is_none()),
                "Apple binary is not used by a development Agent"
            );
            if config.development {
                ensure!(
                    cfg!(feature = "development"),
                    "development state requires an explicit development build"
                );
                ensure!(
                    listen.ip().is_loopback(),
                    "development Agent must listen on loopback"
                );
            }
            let store = Arc::new(BindingStore::open(&state)?);
            let key = config.key.load(&state)?;
            let kind = if docker_context.is_some() {
                ProviderKind::Docker
            } else {
                ProviderKind::Apple
            };
            let environment = if let Some(context) = &docker_context {
                format!("agent:{}:docker:{context}", key.public().id())
            } else {
                format!("agent:{}:apple:current-user", key.public().id())
            };
            let local: Arc<dyn ManagementClient> = if config.development {
                #[cfg(feature = "development")]
                {
                    Arc::new(containerhub_core::fake::FakeProvider::default())
                }
                #[cfg(not(feature = "development"))]
                anyhow::bail!("development state requires an explicit development build")
            } else if let Some(context) = &docker_context {
                Arc::new(containerhub_docker::DockerProvider::new(
                    context,
                    docker_binary.as_deref(),
                )?)
            } else {
                Arc::new(match apple_binary {
                    Some(path) => containerhub_apple::AppleProvider::with_binary(path)?,
                    None => containerhub_apple::AppleProvider::default(),
                })
            };
            let local_environment = if config.development || docker_context.is_some() {
                local.snapshot().await?.environment
            } else {
                LOCAL_ENVIRONMENT.into()
            };
            let provider = Arc::new(containerhub_core::environment::EnvironmentClient::new(
                local,
                local_environment,
                environment.clone(),
                kind,
            ));
            let agent = Agent::new(store, key, provider, environment)?;
            if let Some(url) = relay {
                containerhub_remote::network::validate_endpoint(&url, config.development)?;
                for _ in 0..4 {
                    let agent = agent.clone();
                    let url = url.clone();
                    tokio::spawn(async move {
                        let _ =
                            containerhub_remote::relay::outbound(agent, url, config.development)
                                .await;
                    });
                }
            }
            let listener = tokio::net::TcpListener::bind(listen).await?;
            eprintln!(
                "Agent {} listening on {}{}",
                agent.public_key().id(),
                listener.local_addr()?,
                if config.development {
                    " (isolated demo runtime)"
                } else {
                    ""
                }
            );
            tokio::select! { result=agent.serve(listener)=>result?, _=tokio::signal::ctrl_c()=>{} }
        }
        Command::Relay { listen } => {
            let listener = tokio::net::TcpListener::bind(listen).await?;
            eprintln!("Ciphertext relay listening on {}", listener.local_addr()?);
            tokio::select! {result=containerhub_remote::relay::serve(listener)=>result?,_=tokio::signal::ctrl_c()=>{}}
        }
        Command::Identity { profile } => {
            private_directory(&profile)?;
            let _lock = instance_lock(&profile)?;
            let profile = ControllerProfile::create(&profile, false)?;
            println!("{}", serde_json::to_string(&profile.device)?);
        }
        Command::EnableManagement { profile } => {
            let _lock = instance_lock(&profile)?;
            let mut data = ControllerProfile::load(&profile)?;
            data.enable_management(&profile)?;
        }
        Command::ImportCredential {
            profile,
            credential,
        } => {
            let _lock = instance_lock(&profile)?;
            let mut data = ControllerProfile::load(&profile)?;
            let signed: Signed = read_json(&credential)?;
            let candidate: AppCredential = signed.unverified()?;
            ensure!(
                candidate.device == data.device,
                "credential is for a different device"
            );
            // Its signature and issuer are checked by the Agent, not trusted from this import.
            data.app_credential = Some(signed);
            data.save(&profile)?;
        }
        Command::Connect {
            profile,
            url,
            relay_agent,
            agent,
            claim,
        } => {
            let _lock = instance_lock(&profile)?;
            let mut data = ControllerProfile::load(&profile)?;
            ensure!(
                !data.development || cfg!(feature = "development"),
                "development state requires a development build"
            );
            let endpoint = if let Some(route) = relay_agent {
                Endpoint::Relay { url, route }
            } else {
                Endpoint::Direct { url }
            };
            let pin = if let Some(id) = agent {
                Some(
                    data.connections
                        .iter()
                        .find(|c| c.receipt.agent.id() == id)
                        .context("unknown Agent ID")?
                        .receipt
                        .agent
                        .clone(),
                )
            } else {
                None
            };
            ensure!(
                claim || pin.is_some(),
                "use --agent for reconnect or --claim for first ownership claim"
            );
            let credentials = data.credentials(&profile)?;
            let (_, receipt) = client::authenticate(
                &endpoint,
                &credentials,
                pin.as_ref(),
                claim,
                data.development,
            )
            .await?;
            data.policy = credentials.latest_policy()?;
            data.add_connection(
                &profile,
                Connection {
                    endpoint,
                    receipt: receipt.clone(),
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&receipt)?);
        }
        Command::Snapshot { profile, agent } => {
            let _lock = instance_lock(&profile)?;
            let data = ControllerProfile::load(&profile)?;
            ensure!(
                !data.development || cfg!(feature = "development"),
                "development state requires a development build"
            );
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &data.client(&profile, agent.as_deref())?.snapshot().await?
                )?
            );
        }
        Command::Revoke { profile, device } => {
            let _lock = instance_lock(&profile)?;
            let mut data = ControllerProfile::load(&profile)?;
            // Adopt signed policies from reachable Agents before issuing the next revision.
            let credentials = data.credentials(&profile)?;
            for connection in &data.connections {
                let _ = client::authenticate(
                    &connection.endpoint,
                    &credentials,
                    Some(&connection.receipt.agent),
                    false,
                    data.development,
                )
                .await;
            }
            data.policy = credentials.latest_policy()?;
            data.refresh_policy(&profile, Some(&device))?;
            let credentials = data.credentials(&profile)?;
            let mut failed = 0;
            for connection in &data.connections {
                match client::authenticate(
                    &connection.endpoint,
                    &credentials,
                    Some(&connection.receipt.agent),
                    false,
                    data.development,
                )
                .await
                {
                    Ok(_) => println!("{}: revoked", connection.receipt.agent.id()),
                    Err(error) => {
                        failed += 1;
                        eprintln!("{}: pending ({error})", connection.receipt.agent.id());
                    }
                }
            }
            ensure!(
                failed == 0,
                "{failed} Agent(s) have not received revocation; offline leases remain bounded"
            );
        }
        Command::Grant {
            profile,
            device_public,
            output,
            allow_lifecycle,
            allow_management,
        } => {
            let _lock = instance_lock(&profile)?;
            let mut data = ControllerProfile::load(&profile)?;
            let device: PublicKey = read_json(&device_public)?;
            device.validate()?;
            ensure!(
                !data.connections.is_empty(),
                "bind an Agent before delegating access"
            );
            data.refresh_policy(&profile, None)?;
            let mut permissions = vec![Permission::Read, Permission::Logs];
            if allow_lifecycle {
                permissions.push(Permission::Lifecycle);
            }
            if allow_management {
                permissions.extend([Permission::ManageContainers, Permission::ManageImages]);
            }
            let grant = data.owner_key(&profile)?.sign(
                GRANT_DOMAIN,
                &DeviceGrant {
                    version: VERSION,
                    owner: data.owner.clone(),
                    device,
                    serial: nonce(),
                    agents: Scope::Only(
                        data.connections
                            .iter()
                            .map(|c| c.receipt.agent.id())
                            .collect(),
                    ),
                    environments: Scope::All,
                    permissions,
                    not_before: now(),
                    expires_at: now() + MAX_LEASE_SECONDS,
                },
            )?;
            // Public authorization only. Never export root_key/device_key references or private bytes.
            let bundle = serde_json::json!({"version":VERSION,"owner":data.owner,"grant":grant,"policy":data.policy,"connections":data.connections});
            create_private(&output, &serde_json::to_vec_pretty(&bundle)?)?;
        }
    }
    Ok(())
}
