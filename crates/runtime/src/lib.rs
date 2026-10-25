//! Runtime composition for native hosts. Credentials stay in Rust; no UI or Node dependency.
mod client;
pub const DEMO_ENABLED: bool = cfg!(feature = "demo");
pub use client::ClientSlot;
use containerhub_core::ManagementClient;
pub use containerhub_remote::client::Endpoint;
use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::Arc,
};

pub struct Runtime {
    pub client: Arc<dyn ManagementClient>,
    pub storage_id: String,
    pub remote_label: Option<String>,
    pub demo: bool,
    pub profile_lock: Option<Arc<File>>,
}
impl Runtime {
    pub fn apple() -> Self {
        Self {
            client: Arc::new(containerhub_apple::AppleProvider::default()),
            storage_id: "local".into(),
            remote_label: None,
            demo: false,
            profile_lock: None,
        }
    }
    pub fn apple_at(binary: Option<&Path>) -> anyhow::Result<Self> {
        let mut runtime = Self::apple();
        if let Some(path) = binary {
            runtime.client = Arc::new(containerhub_apple::AppleProvider::with_binary(
                path.to_owned(),
            )?);
        }
        Ok(runtime)
    }
    pub fn docker(context: &str, binary: Option<&Path>) -> anyhow::Result<Self> {
        Ok(Self {
            client: Arc::new(containerhub_docker::DockerProvider::new(context, binary)?),
            storage_id: format!(
                "docker/{}",
                containerhub_remote::crypto::digest(context.as_bytes())
            ),
            remote_label: None,
            demo: false,
            profile_lock: None,
        })
    }
    pub fn demo() -> anyhow::Result<Self> {
        #[cfg(feature = "demo")]
        {
            Ok(Self {
                client: Arc::new(containerhub_core::fake::FakeProvider::default()),
                storage_id: "demo".into(),
                remote_label: None,
                demo: true,
                profile_lock: None,
            })
        }
        #[cfg(not(feature = "demo"))]
        anyhow::bail!("示例数据未包含在此构建中；预览需要显式启用 demo feature")
    }
    pub fn remote(directory: &Path, agent: Option<&str>) -> anyhow::Result<Self> {
        RemoteProfile::open(directory)?.runtime(agent)
    }
}

/// A profile owns one exclusive lock while allowing several bound agents to be
/// open in the same native host. Only public agent metadata leaves this layer.
#[derive(Clone)]
pub struct RemoteProfile {
    directory: PathBuf,
    profile: containerhub_remote::profile::ControllerProfile,
    lock: Arc<File>,
}
impl RemoteProfile {
    pub fn open(directory: &Path) -> anyhow::Result<Self> {
        let directory = directory.canonicalize()?;
        let lock = Arc::new(containerhub_remote::profile::instance_lock(&directory)?);
        let profile = containerhub_remote::profile::ControllerProfile::load(&directory)?;
        anyhow::ensure!(
            !profile.development || cfg!(feature = "development"),
            "development remote profiles require an explicit development build"
        );
        Ok(Self {
            directory,
            profile,
            lock,
        })
    }
    pub fn agents(&self) -> Vec<(String, String)> {
        self.profile
            .connections
            .iter()
            .map(|connection| {
                let id = connection.receipt.agent.id();
                let label = format!(
                    "远程 {}{}",
                    &id[5..13],
                    if self.profile.development {
                        " · 示例"
                    } else {
                        ""
                    }
                );
                (id, label)
            })
            .collect()
    }
    pub fn connection(&self, agent: &str) -> anyhow::Result<ConnectionInfo> {
        let c = self
            .profile
            .connections
            .iter()
            .find(|c| c.receipt.agent.id() == agent)
            .ok_or_else(|| anyhow::anyhow!("未找到已绑定 Agent"))?;
        Ok(ConnectionInfo {
            endpoint: c.endpoint.clone(),
            agent: agent.into(),
            bound_at: c.receipt.bound_at,
            development: self.profile.development,
        })
    }
    pub fn runtime(&self, agent: Option<&str>) -> anyhow::Result<Runtime> {
        self.runtime_configured(agent, None)
    }
    pub fn runtime_configured(
        &self,
        agent: Option<&str>,
        endpoint: Option<&Endpoint>,
    ) -> anyhow::Result<Runtime> {
        let mut client = self.profile.client(&self.directory, agent)?;
        if let Some(endpoint) = endpoint {
            validate_connection(endpoint, &client.pin.id(), client.development)?;
            client.endpoint = endpoint.clone();
        }
        let id = client.pin.id();
        let storage_id = format!(
            "remote/{}",
            containerhub_remote::crypto::digest(id.as_bytes())
        );
        let label = self
            .agents()
            .into_iter()
            .find(|(agent, _)| agent == &id)
            .unwrap()
            .1;
        Ok(Runtime {
            client: Arc::new(client),
            storage_id,
            remote_label: Some(label),
            demo: self.profile.development,
            profile_lock: Some(self.lock.clone()),
        })
    }
}

#[derive(Clone, Debug)]
pub struct ConnectionInfo {
    pub endpoint: Endpoint,
    pub agent: String,
    pub bound_at: u64,
    pub development: bool,
}
pub fn validate_connection(
    endpoint: &Endpoint,
    agent: &str,
    development: bool,
) -> anyhow::Result<()> {
    use containerhub_remote::network::validate_endpoint;
    match endpoint {
        Endpoint::Direct { url } => validate_endpoint(url, development)?,
        Endpoint::Relay { url, route } => {
            anyhow::ensure!(route == agent, "中继目标必须保持原 Agent 身份");
            validate_endpoint(url, development)?;
        }
        Endpoint::Auto {
            direct,
            relay,
            route,
        } => {
            anyhow::ensure!(route == agent, "中继目标必须保持原 Agent 身份");
            validate_endpoint(direct, development)?;
            validate_endpoint(relay, development)?;
        }
    }
    Ok(())
}

#[cfg(all(test, feature = "development"))]
mod tests;

#[cfg(all(test, not(feature = "demo")))]
mod distribution_tests {
    #[test]
    fn ordinary_runtime_rejects_demo_instead_of_returning_sample_resources() {
        let error = super::Runtime::demo()
            .err()
            .expect("demo must be unavailable");
        assert!(error.to_string().contains("示例数据未包含在此构建中"));
    }
}
