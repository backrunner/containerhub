mod checkpoint;
mod operations;
use crate::{
    crypto::*,
    identity::*,
    network::{self, SecureSocket, Socket},
    rpc::*,
    store::BindingStore,
};
use anyhow::{Context, Result, ensure};
use containerhub_core::{
    Command, ErrorKind, ManagementClient, ManagementError, OperationState, RuntimeFeature,
    SecretEnvironment, now,
};
use containerhub_management::scheduler::Scheduler;
use std::{sync::Arc, time::Duration};
use tokio::{net::TcpListener, sync::Semaphore};

pub struct Agent {
    pub store: Arc<BindingStore>,
    key: SecretKey,
    provider: Arc<dyn ManagementClient>,
    environment: String,
    scheduler: Scheduler,
    controls:
        std::sync::Mutex<std::collections::HashMap<String, containerhub_core::ExecutionControl>>,
}
impl Agent {
    pub fn new(
        store: Arc<BindingStore>,
        key: SecretKey,
        provider: Arc<dyn ManagementClient>,
        environment: String,
    ) -> Result<Arc<Self>> {
        ensure!(
            store.installation.agent == key.public(),
            "agent credential does not match installation"
        );
        // A crash after durable accept must not blindly re-execute a mutation.
        for (id, json) in store.operation_records()? {
            let mut operation: RemoteOperation = serde_json::from_str(&json)?;
            if operation.state.is_active() {
                operation.state = OperationState::UnknownOutcome;
                operation.error = Some(ManagementError::new(
                    ErrorKind::UnknownOutcome,
                    "Agent 重启时操作未完成；请先查询实际资源状态",
                ));
                store.finish_operation(&id, &serde_json::to_string(&operation)?)?;
            }
        }
        Ok(Arc::new(Self {
            store,
            key,
            provider,
            environment,
            scheduler: Scheduler::default(),
            controls: Default::default(),
        }))
    }
    pub fn public_key(&self) -> PublicKey {
        self.key.public()
    }
    pub fn sign(&self, domain: &str, value: &impl serde::Serialize) -> Result<Signed> {
        self.key.sign(domain, value)
    }
    pub async fn serve(self: Arc<Self>, listener: TcpListener) -> Result<()> {
        let slots = Arc::new(Semaphore::new(32));
        loop {
            let (stream, _) = listener.accept().await?;
            let Ok(slot) = slots.clone().try_acquire_owned() else {
                drop(stream);
                continue;
            };
            let agent = self.clone();
            tokio::spawn(async move {
                let _slot = slot;
                if let Ok(socket) = network::accept(stream).await {
                    let _ = agent.session(socket).await;
                }
            });
        }
    }
    pub async fn session(self: Arc<Self>, socket: Socket) -> Result<()> {
        let mut secure = SecureSocket::handshake(socket, false).await?;
        let proof = self.key.sign(
            AGENT_DOMAIN,
            &AgentProof {
                version: VERSION,
                agent: self.public_key(),
                channel: secure.channel.binding.clone(),
                policy: self.store.current_policy()?,
            },
        )?;
        secure.send(&proof).await?;
        let client: Signed = secure.receive().await?;
        let (authorization, receipt) =
            self.store
                .authenticate(&client, &secure.channel.binding, now())?;
        secure
            .send(&self.key.sign("containerhub.binding-receipt.v1", &receipt)?)
            .await?;
        loop {
            // Do not cancel receive halfway through a fragmented record on each tick.
            let request = {
                let receive = secure.receive::<Request>();
                tokio::pin!(receive);
                let mut timer = tokio::time::interval(Duration::from_secs(1));
                loop {
                    tokio::select! {
                        message=&mut receive => break message?,
                        _=timer.tick() => self.store.session_valid(&authorization,now())?,
                    }
                }
            };
            let result = self.dispatch(&authorization, request).await;
            let response = result.unwrap_or_else(|error| {
                Response::Error(ManagementError::new(
                    ErrorKind::PermissionDenied,
                    error.to_string(),
                ))
            });
            self.store.session_valid(&authorization, now())?;
            secure.send(&response).await?;
        }
    }
    async fn dispatch(
        self: &Arc<Self>,
        auth: &Authorization,
        request: Request,
    ) -> Result<Response> {
        match request {
            Request::HostSnapshot => {
                self.store
                    .authorize(auth, Permission::Read, &self.environment, now())?;
                Ok(match self.provider.host_snapshot().await {
                    Ok(sample) => {
                        ensure!(
                            sample.environment == self.environment,
                            "provider returned wrong host environment"
                        );
                        sample.validate()?;
                        Response::HostSnapshot(sample)
                    }
                    Err(error) => Response::Error(error),
                })
            }
            Request::Diagnostics => {
                self.store
                    .authorize(auth, Permission::Read, &self.environment, now())?;
                Ok(match self.provider.diagnostics().await {
                    Ok(mut report) => {
                        ensure!(
                            report.environment == self.environment,
                            "provider returned wrong environment"
                        );
                        for check in &mut report.checks {
                            if check.feature == RuntimeFeature::StartRuntime {
                                check.available = false;
                                check.reason = Some("服务启动目前仅限本机".into());
                                continue;
                            }
                            let permission = match check.feature {
                                RuntimeFeature::Lifecycle => Permission::Lifecycle,
                                RuntimeFeature::PullImage
                                | RuntimeFeature::CancelPull
                                | RuntimeFeature::DeleteImage => Permission::ManageImages,
                                RuntimeFeature::CreateContainer
                                | RuntimeFeature::CreateAndStart
                                | RuntimeFeature::StartupCommand
                                | RuntimeFeature::SecretEnvironment
                                | RuntimeFeature::AdvancedCreate
                                | RuntimeFeature::RebuildPorts
                                | RuntimeFeature::DeleteContainer => Permission::ManageContainers,
                                RuntimeFeature::Logs | RuntimeFeature::BootLogs => Permission::Logs,
                                _ => Permission::Read,
                            };
                            if !auth.grant().permits(permission, &self.environment)
                                || (matches!(
                                    check.feature,
                                    RuntimeFeature::CreateAndStart | RuntimeFeature::RebuildPorts
                                ) && !auth
                                    .grant()
                                    .permits(Permission::Lifecycle, &self.environment))
                            {
                                check.available = false;
                                check.reason = Some("当前设备未获此操作授权".into());
                            }
                        }
                        Response::Diagnostics(report)
                    }
                    Err(error) => Response::Error(error),
                })
            }
            Request::Snapshot => {
                self.store
                    .authorize(auth, Permission::Read, &self.environment, now())?;
                Ok(match self.provider.snapshot().await {
                    Ok(mut snapshot) => {
                        ensure!(
                            snapshot.environment == self.environment,
                            "provider returned wrong environment"
                        );
                        snapshot.lifecycle_supported &= auth
                            .grant()
                            .permits(Permission::Lifecycle, &self.environment);
                        snapshot.capabilities.cancel_pull &= auth
                            .grant()
                            .permits(Permission::ManageImages, &self.environment);
                        snapshot.capabilities.pull_image &= auth
                            .grant()
                            .permits(Permission::ManageImages, &self.environment);
                        snapshot.capabilities.delete_image &= auth
                            .grant()
                            .permits(Permission::ManageImages, &self.environment);
                        snapshot.capabilities.rebuild_ports &= snapshot.lifecycle_supported
                            && auth
                                .grant()
                                .permits(Permission::ManageContainers, &self.environment);
                        snapshot.capabilities.create_startup &= auth
                            .grant()
                            .permits(Permission::ManageContainers, &self.environment);
                        snapshot.capabilities.create_and_start &= snapshot.lifecycle_supported
                            && auth
                                .grant()
                                .permits(Permission::ManageContainers, &self.environment);
                        snapshot.capabilities.create_secrets &= auth
                            .grant()
                            .permits(Permission::ManageContainers, &self.environment);
                        snapshot.capabilities.create_advanced &= auth
                            .grant()
                            .permits(Permission::ManageContainers, &self.environment);
                        snapshot.capabilities.create_container &= auth
                            .grant()
                            .permits(Permission::ManageContainers, &self.environment);
                        snapshot.capabilities.delete_container &= auth
                            .grant()
                            .permits(Permission::ManageContainers, &self.environment);
                        Response::Snapshot(snapshot)
                    }
                    Err(error) => Response::Error(error),
                })
            }
            Request::Stats { key } => {
                ensure!(key.environment == self.environment, "wrong environment");
                self.store
                    .authorize(auth, Permission::Read, &key.environment, now())?;
                Ok(match self.provider.stats(&key).await {
                    Ok(stats) => Response::Stats(stats),
                    Err(error) => Response::Error(error),
                })
            }
            Request::Resources(kind) => {
                self.store
                    .authorize(auth, Permission::Read, &self.environment, now())?;
                Ok(match self.provider.resources(kind).await {
                    Ok(rows) => Response::Resources(rows),
                    Err(error) => Response::Error(error),
                })
            }
            Request::InspectImage {
                environment,
                reference,
            } => {
                ensure!(environment == self.environment, "wrong environment");
                self.store
                    .authorize(auth, Permission::Read, &environment, now())?;
                Ok(
                    match self.provider.inspect_image(&environment, &reference).await {
                        Ok(image) => Response::ImageDetails(image),
                        Err(error) => Response::Error(error),
                    },
                )
            }
            Request::Logs { key, source } => {
                ensure!(key.environment == self.environment, "wrong environment");
                self.store
                    .authorize(auth, Permission::Logs, &key.environment, now())?;
                Ok(match self.provider.logs(&key, source).await {
                    Ok(logs) => Response::Logs(logs),
                    Err(error) => Response::Error(error),
                })
            }
            Request::InspectContainer { key } => {
                ensure!(key.environment == self.environment, "wrong environment");
                self.store
                    .authorize(auth, Permission::Read, &self.environment, now())?;
                Ok(match self.provider.inspect_container(&key).await {
                    Ok(details) => Response::ContainerDetails(details),
                    Err(error) => Response::Error(error),
                })
            }
            Request::Submit(request) => {
                self.submit(auth, request, SecretEnvironment::default())
                    .await
            }
            Request::SubmitWithSecrets { request, secrets } => {
                self.submit(auth, request, secrets).await
            }
            Request::Cancel { request_id } => self.cancel(auth, &request_id),
            Request::Operation { request_id } => {
                self.store
                    .authorize(auth, Permission::Read, &self.environment, now())?;
                let Some((request, result)) =
                    self.store.operation(&request_id, &auth.owner_id())?
                else {
                    return Ok(Response::Error(ManagementError::new(
                        ErrorKind::NotFound,
                        "未找到原任务记录；不会自动重新提交",
                    )));
                };
                let request: Mutation = serde_json::from_str(&request)?;
                self.store.authorize(
                    auth,
                    Permission::Read,
                    request.command.environment(),
                    now(),
                )?;
                let mut operation: RemoteOperation = serde_json::from_str(&result)?;
                if let Some(control) = self.controls.lock().expect("controls").get(&request_id) {
                    operation.progress = control.progress();
                }
                Ok(Response::Operation(operation))
            }
        }
    }
}

fn permission(command: &Command) -> Result<Permission> {
    Ok(match command {
        Command::StartRuntime { .. } => anyhow::bail!("runtime startup is local-only"),
        Command::Lifecycle { .. } => Permission::Lifecycle,
        Command::PullImage { .. } | Command::DeleteImage { .. } => Permission::ManageImages,
        Command::SwarmService { .. }
        | Command::CreateContainer { .. }
        | Command::DeleteContainer { .. }
        | Command::RebuildPorts { .. } => Permission::ManageContainers,
    })
}
fn authorize_command(store: &BindingStore, auth: &Authorization, command: &Command) -> Result<()> {
    store.authorize(auth, permission(command)?, command.environment(), now())?;
    if command.starts_after_create()
        || matches!(
            command,
            Command::RebuildPorts { .. } | Command::SwarmService { .. }
        )
    {
        store.authorize(auth, Permission::Lifecycle, command.environment(), now())?;
    }
    Ok(())
}
