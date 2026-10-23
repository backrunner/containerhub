use super::*;
use containerhub_runtime::{ConnectionInfo, Endpoint};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct EnvironmentPreferences {
    pub name: String,
    pub notes: String,
    pub interval: u64,
    pub endpoint: Option<Endpoint>,
    pub apple_binary: Option<PathBuf>,
    pub docker_binary: Option<PathBuf>,
}
impl Default for EnvironmentPreferences {
    fn default() -> Self {
        Self {
            name: String::new(),
            notes: String::new(),
            interval: 2,
            endpoint: None,
            apple_binary: None,
            docker_binary: None,
        }
    }
}
pub(super) fn read(base: &std::path::Path) -> HashMap<String, EnvironmentPreferences> {
    backend::read_json(&base.join("environment-preferences.json")).unwrap_or_default()
}
impl WorkspaceHost {
    pub(super) fn preference(&self, target: &Target) -> EnvironmentPreferences {
        self.preferences
            .get(&target.key())
            .cloned()
            .unwrap_or_else(|| {
                let mut preference = EnvironmentPreferences::default();
                if matches!(target, Target::Local) {
                    preference.apple_binary =
                        backend::read_json::<Settings>(&self.base.join("local/settings.json"))
                            .and_then(|s| s.apple_binary);
                }
                preference
            })
    }
    pub(super) fn label(&self, target: &Target) -> String {
        let preference = self.preference(target);
        if preference.name.is_empty() {
            target.label()
        } else {
            preference.name
        }
    }
    pub(super) fn connection(&self, target: &Target) -> anyhow::Result<Option<ConnectionInfo>> {
        match target {
            Target::Remote {
                directory, agent, ..
            } => {
                let Some(profile) = self.profiles.get(directory) else {
                    return Ok(None);
                };
                let mut info = profile.connection(agent)?;
                if let Some(endpoint) = self.preference(target).endpoint {
                    info.endpoint = endpoint;
                }
                Ok(Some(info))
            }
            _ => Ok(None),
        }
    }
    pub(super) fn inspection_client(
        &mut self,
        target: &Target,
    ) -> anyhow::Result<Arc<dyn ManagementClient>> {
        if let Some(session) = self.sessions.get(&target.key()) {
            return Ok(session.client.clone());
        }
        let preference = self.preference(target);
        let runtime = match target {
            Target::Local => {
                let settings: Settings =
                    backend::read_json(&self.base.join("local/settings.json")).unwrap_or_default();
                Runtime::apple_at(if self.preferences.contains_key(&target.key()) {
                    preference.apple_binary.as_deref()
                } else {
                    settings.apple_binary.as_deref()
                })?
            }
            Target::Demo => Runtime::demo(),
            Target::Docker { context } => {
                Runtime::docker(context, preference.docker_binary.as_deref())?
            }
            Target::Remote {
                directory, agent, ..
            } => {
                if !self.profiles.contains_key(directory) {
                    self.profiles
                        .insert(directory.clone(), RemoteProfile::open(directory)?);
                }
                self.profiles[directory]
                    .runtime_configured(Some(agent), preference.endpoint.as_ref())?
            }
        };
        Ok(runtime.client)
    }
    pub(super) fn inspect_environment(&mut self, target: Target, cx: &mut Context<Self>) {
        let result = self.inspection_client(&target);
        let preference = self.preference(&target);
        let connection = self.connection(&target).ok().flatten();
        self.sessions[&self.active].view.update(cx, |view, cx| {
            view.environment
                .select(target, preference, connection, result);
            view.request_environment();
            cx.notify();
        });
    }
    pub(super) fn save_preference(
        &mut self,
        target: &Target,
        preference: EnvironmentPreferences,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(!preference.name.trim().is_empty(), "请填写环境名称");
        anyhow::ensure!(preference.name.chars().count() <= 60, "名称最多 60 个字符");
        anyhow::ensure!(
            !self
                .targets
                .iter()
                .any(|item| item.key() != target.key() && self.label(item) == preference.name),
            "环境名称已存在"
        );
        anyhow::ensure!(
            [2, 5, 10].contains(&preference.interval),
            "采样间隔可选 2、5 或 10 秒"
        );
        anyhow::ensure!(
            preference.notes.chars().count() <= 240,
            "备注最多 240 个字符"
        );
        if let Some(session) = self.sessions.get(&target.key()) {
            let original = self.preference(target);
            let changed = original.endpoint != preference.endpoint
                || original.apple_binary != preference.apple_binary
                || original.docker_binary != preference.docker_binary;
            anyhow::ensure!(
                !changed
                    || !session
                        .view
                        .read(cx)
                        .operations
                        .iter()
                        .any(|o| o.state.is_active() || o.state == OperationState::UnknownOutcome),
                "请先完成或核对该环境中的任务，再修改连接"
            );
        }
        // Build with the original profile credentials and pin. An edit never claims a new owner.
        let runtime = match target {
            Target::Remote {
                directory, agent, ..
            } => {
                let profile = self
                    .profiles
                    .get(directory)
                    .ok_or_else(|| anyhow::anyhow!("请先读取该环境"))?;
                Some(profile.runtime_configured(Some(agent), preference.endpoint.as_ref())?)
            }
            Target::Local => {
                if let Some(path) = &preference.apple_binary {
                    containerhub_apple::validate_binary(path)?;
                }
                Some(Runtime::apple_at(preference.apple_binary.as_deref())?)
            }
            Target::Demo => None,
            Target::Docker { context } => Some(Runtime::docker(
                context,
                preference.docker_binary.as_deref(),
            )?),
        };
        let mut saved = self.preferences.clone();
        saved.insert(target.key(), preference.clone());
        containerhub_management::storage::save_json(
            &self.base.join("environment-preferences.json"),
            &saved,
        )?;
        self.preferences = saved;
        if let Some(session) = self.sessions.get(&target.key()) {
            if let Some(runtime) = runtime {
                session.client.replace(runtime.client);
            }
            if let Target::Local = target {
                session.view.update(cx, |view, _| {
                    view.settings.apple_binary = preference.apple_binary.clone();
                });
            }
        }
        self.inspect_environment(target.clone(), cx);
        Ok(())
    }
}
