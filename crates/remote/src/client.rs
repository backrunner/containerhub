use crate::{
    crypto::*,
    identity::*,
    network::{self, SecureSocket},
    relay,
    rpc::*,
};
use anyhow::{Result, ensure};
use containerhub_core::*;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Endpoint {
    Direct {
        url: String,
    },
    Auto {
        direct: String,
        relay: String,
        route: String,
    },
    Relay {
        url: String,
        route: String,
    },
}
mod credentials;
mod operations;
pub use credentials::Credentials;
pub(crate) use credentials::newer_policy;
pub async fn authenticate(
    endpoint: &Endpoint,
    credentials: &Credentials,
    pin: Option<&PublicKey>,
    claim: bool,
    development: bool,
) -> Result<(SecureSocket, BindingReceipt)> {
    ensure!(
        claim || pin.is_some(),
        "an existing binding requires an agent identity pin"
    );
    let socket = match endpoint {
        Endpoint::Direct { url } => network::connect(url, development).await?,
        Endpoint::Auto {
            direct,
            relay: relay_url,
            route,
        } => {
            network::validate_endpoint(direct, development)?;
            network::validate_endpoint(relay_url, development)?;
            // Fall back only before the encrypted handshake. Identity failures never retry elsewhere.
            match tokio::time::timeout(
                Duration::from_secs(3),
                network::connect(direct, development),
            )
            .await
            {
                Ok(Ok(socket)) => socket,
                _ => relay::controller(relay_url, route, development).await?,
            }
        }
        Endpoint::Relay { url, route } => relay::controller(url, route, development).await?,
    };
    let mut secure = SecureSocket::handshake(socket, true).await?;
    let signed: Signed = secure.receive().await?;
    let candidate: AgentProof = signed.unverified()?;
    let proof: AgentProof = candidate.agent.verify(AGENT_DOMAIN, &signed)?;
    ensure!(
        proof.version == VERSION && proof.channel == secure.channel.binding,
        "invalid agent channel proof"
    );
    if let Some(pin) = pin {
        ensure!(
            pin == &proof.agent,
            "agent identity changed; connection refused"
        );
    }
    if let Endpoint::Relay { route, .. } | Endpoint::Auto { route, .. } = endpoint {
        ensure!(
            &proof.agent.id() == route,
            "relay connected a different agent"
        );
    }
    let (grant, signed_grant, policy) =
        credentials.documents(proof.policy.as_ref(), &proof.agent.id(), now())?;
    let auth = credentials.device.sign(
        CLIENT_DOMAIN,
        &ClientProof {
            version: VERSION,
            agent: proof.agent.clone(),
            channel: secure.channel.binding.clone(),
            claim,
            grant: signed_grant,
            policy,
            app_credential: credentials.app_credential.clone(),
        },
    )?;
    secure.send(&auth).await?;
    let receipt: Signed = secure.receive().await?;
    let receipt: BindingReceipt = proof
        .agent
        .verify("containerhub.binding-receipt.v1", &receipt)?;
    ensure!(
        receipt.version == VERSION && receipt.agent == proof.agent && receipt.owner == grant.owner,
        "invalid binding receipt"
    );
    Ok((secure, receipt))
}

pub struct RemoteClient {
    pub endpoint: Endpoint,
    pub credentials: Arc<Credentials>,
    pub pin: PublicKey,
    pub development: bool,
}
impl RemoteClient {
    pub async fn request(&self, request: &Request) -> Result<Response> {
        let (mut secure, _) = authenticate(
            &self.endpoint,
            &self.credentials,
            Some(&self.pin),
            false,
            self.development,
        )
        .await?;
        secure.send(request).await?;
        let response = secure.receive().await?;
        Ok(response)
    }
}
fn transport_error(error: anyhow::Error) -> ManagementError {
    ManagementError::new(ErrorKind::Unavailable, format!("远程连接失败：{error}"))
}
fn unexpected() -> ManagementError {
    ManagementError::new(ErrorKind::InvalidData, "Agent 返回了不匹配的响应")
}
#[async_trait::async_trait]
impl ManagementClient for RemoteClient {
    async fn host_snapshot(&self) -> Result<HostSnapshot, ManagementError> {
        match self
            .request(&Request::HostSnapshot)
            .await
            .map_err(transport_error)?
        {
            Response::HostSnapshot(sample)
                if sample
                    .environment
                    .starts_with(&format!("agent:{}:", self.pin.id())) =>
            {
                sample.validate()?;
                Ok(sample)
            }
            Response::Error(error) => Err(error),
            _ => Err(unexpected()),
        }
    }
    async fn diagnostics(&self) -> Result<RuntimeDiagnostics, ManagementError> {
        match self
            .request(&Request::Diagnostics)
            .await
            .map_err(transport_error)?
        {
            Response::Diagnostics(report)
                if report
                    .environment
                    .starts_with(&format!("agent:{}:", self.pin.id())) =>
            {
                Ok(report)
            }
            Response::Error(error) => Err(error),
            _ => Err(unexpected()),
        }
    }
    async fn snapshot(&self) -> Result<Snapshot, ManagementError> {
        match self
            .request(&Request::Snapshot)
            .await
            .map_err(transport_error)?
        {
            Response::Snapshot(snapshot) => Ok(snapshot),
            Response::Error(e) => Err(e),
            _ => Err(unexpected()),
        }
    }
    async fn resources(&self, kind: ResourceKind) -> Result<Vec<Resource>, ManagementError> {
        match self
            .request(&Request::Resources(kind))
            .await
            .map_err(transport_error)?
        {
            Response::Resources(rows) => Ok(rows),
            Response::Error(e) => Err(e),
            _ => Err(unexpected()),
        }
    }
    async fn inspect_image(
        &self,
        environment: &str,
        reference: &str,
    ) -> Result<ImageDetails, ManagementError> {
        validate_image_query(environment, reference)?;
        match self
            .request(&Request::InspectImage {
                environment: environment.into(),
                reference: reference.into(),
            })
            .await
            .map_err(transport_error)?
        {
            Response::ImageDetails(image)
                if image.key.environment == environment
                    && image.used_by.iter().all(|k| k.environment == environment) =>
            {
                image.key.validate()?;
                Ok(image)
            }
            Response::Error(error) => Err(error),
            _ => Err(unexpected()),
        }
    }
    async fn logs(
        &self,
        key: &ResourceKey,
        source: LogSource,
    ) -> Result<LogSnapshot, ManagementError> {
        match self
            .request(&Request::Logs {
                key: key.clone(),
                source,
            })
            .await
            .map_err(transport_error)?
        {
            Response::Logs(logs) if logs.key == *key && logs.source == source => Ok(logs),
            Response::Error(e) => Err(e),
            _ => Err(unexpected()),
        }
    }
    async fn lifecycle(
        &self,
        key: &ResourceKey,
        action: LifecycleAction,
    ) -> Result<(), ManagementError> {
        self.execute(&Command::Lifecycle {
            key: key.clone(),
            action,
        })
        .await
        .map(|_| ())
    }
    async fn stats(&self, key: &ResourceKey) -> Result<ContainerStats, ManagementError> {
        match self
            .request(&Request::Stats { key: key.clone() })
            .await
            .map_err(transport_error)?
        {
            Response::Stats(stats) if stats.key == *key => Ok(stats),
            Response::Error(error) => Err(error),
            _ => Err(unexpected()),
        }
    }
    async fn inspect_container(
        &self,
        key: &ResourceKey,
    ) -> Result<ContainerDetails, ManagementError> {
        match self
            .request(&Request::InspectContainer { key: key.clone() })
            .await
            .map_err(transport_error)?
        {
            Response::ContainerDetails(details) if details.container.key == *key => Ok(details),
            Response::Error(error) => Err(error),
            _ => Err(unexpected()),
        }
    }
    async fn execute(&self, command: &Command) -> Result<CommandResult, ManagementError> {
        self.execute_controlled(command, &ExecutionControl::default())
            .await
    }
    fn prepare_execution(
        &self,
        command: &Command,
    ) -> Result<Option<ExecutionReceipt>, ManagementError> {
        self.prepare(command).map(Some)
    }
    async fn execute_controlled(
        &self,
        command: &Command,
        control: &ExecutionControl,
    ) -> Result<CommandResult, ManagementError> {
        let receipt = self.prepare(command)?;
        self.execute_prepared(command, Some(&receipt), control)
            .await
    }
    async fn execute_prepared(
        &self,
        command: &Command,
        receipt: Option<&ExecutionReceipt>,
        control: &ExecutionControl,
    ) -> Result<CommandResult, ManagementError> {
        let receipt = receipt.ok_or_else(unexpected)?;
        self.submit_prepared(command, receipt, control).await
    }
    async fn resume_execution(
        &self,
        command: &Command,
        receipt: &ExecutionReceipt,
        control: &ExecutionControl,
    ) -> Result<RecoveredExecution, ManagementError> {
        let request = self.original_request(command, receipt)?;
        self.follow(&request, receipt, control, None, Duration::from_secs(900))
            .await
    }
}
