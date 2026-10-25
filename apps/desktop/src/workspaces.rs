//! Native workspace sessions. Each retains its backend, journal and selection.
mod adding;
#[cfg(feature = "ui-validation")]
pub(crate) use adding::fixture as add_form_fixture;
mod editing;
#[cfg(feature = "ui-validation")]
pub(crate) use editing::edit_form_fixture;
mod management;
mod preferences;
pub(super) use preferences::EnvironmentPreferences;
mod picker;
use crate::*;
use containerhub_runtime::{ClientSlot, RemoteProfile, Runtime};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs::File};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum Target {
    Local,
    Demo,
    Docker {
        context: String,
    },
    Remote {
        directory: PathBuf,
        agent: String,
        label: String,
    },
}
impl Target {
    fn available(&self) -> bool {
        !matches!(self, Self::Demo) || cfg!(feature = "demo-preview")
    }

    pub(super) fn key(&self) -> String {
        match self {
            Self::Local => "local".into(),
            Self::Demo => "demo".into(),
            Self::Docker { context } => format!("docker:{context}"),
            Self::Remote {
                directory, agent, ..
            } => format!("{}:{agent}", directory.display()),
        }
    }
    pub(super) fn label(&self) -> String {
        match self {
            Self::Local => "本机".into(),
            Self::Demo => "演示工作空间".into(),
            Self::Docker { context } => format!("Docker · {context}"),
            Self::Remote { label, .. } => label.clone(),
        }
    }
}
pub(super) struct Loaded {
    runtime: Runtime,
    data: PathBuf,
    lock: File,
    profile: Option<(PathBuf, RemoteProfile)>,
}
pub(super) fn load(
    base: &std::path::Path,
    target: &Target,
    cached: Option<RemoteProfile>,
) -> anyhow::Result<Loaded> {
    let preferences = preferences::read(base);
    let preference = preferences.get(&target.key()).cloned().unwrap_or_default();
    let mut profile = None;
    let runtime = match target {
        Target::Local => {
            let settings: Settings =
                backend::read_json(&base.join("local/settings.json")).unwrap_or_default();
            Runtime::apple_at(if preferences.contains_key(&target.key()) {
                preference.apple_binary.as_deref()
            } else {
                settings.apple_binary.as_deref()
            })?
        }
        Target::Demo => Runtime::demo()?,
        Target::Docker { context } => {
            Runtime::docker(context, preference.docker_binary.as_deref())?
        }
        Target::Remote {
            directory, agent, ..
        } => {
            let remote = match cached {
                Some(profile) => profile,
                None => RemoteProfile::open(directory)?,
            };
            let runtime = remote.runtime_configured(Some(agent), preference.endpoint.as_ref())?;
            profile = Some((directory.clone(), remote));
            runtime
        }
    };
    let data = base.join(&runtime.storage_id);
    std::fs::create_dir_all(&data)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(data.join("instance.lock"))?;
    lock.try_lock()
        .map_err(|_| anyhow::anyhow!("该工作空间已被其他 ContainerHub 进程使用"))?;
    Ok(Loaded {
        runtime,
        data,
        lock,
        profile,
    })
}
struct Session {
    view: Entity<Workspace>,
    client: Arc<ClientSlot>,
    _lock: File,
    _profile_lock: Option<Arc<File>>,
}
pub(super) struct WorkspaceHost {
    base: PathBuf,
    targets: Vec<Target>,
    preferences: HashMap<String, EnvironmentPreferences>,
    sessions: HashMap<String, Session>,
    profiles: HashMap<PathBuf, RemoteProfile>,
    active: String,
    loading: bool,
}
impl WorkspaceHost {
    pub(crate) fn active_label(&self) -> String {
        self.targets
            .iter()
            .find(|target| target.key() == self.active)
            .map(|target| self.label(target))
            .unwrap_or_else(|| "工作空间".into())
    }
    pub fn new(
        base: PathBuf,
        target: Target,
        loaded: Loaded,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let saved: Vec<Target> =
            backend::read_json(&base.join("workspaces.json")).unwrap_or_default();
        let mut host = Self {
            preferences: preferences::read(&base),
            base,
            targets: vec![Target::Local],
            sessions: HashMap::new(),
            profiles: HashMap::new(),
            active: target.key(),
            loading: false,
        };
        #[cfg(feature = "demo-preview")]
        host.remember(Target::Demo);
        for item in saved {
            host.remember(item);
        }
        host.install(target, loaded, window, cx);
        host
    }
    fn remember(&mut self, target: Target) {
        if target.available() && !self.targets.iter().any(|known| known.key() == target.key()) {
            self.targets.push(target);
        }
    }
    fn save_catalog(&self) -> anyhow::Result<()> {
        // Only paths, public Agent IDs and display labels. No keys or credentials.
        containerhub_management::storage::save_json(
            &self.base.join("workspaces.json"),
            &self.targets,
        )?;
        Ok(())
    }
    fn install(
        &mut self,
        target: Target,
        loaded: Loaded,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some((directory, profile)) = loaded.profile {
            self.profiles.insert(directory, profile);
        }
        let Runtime {
            client,
            demo,
            remote_label,
            profile_lock,
            ..
        } = loaded.runtime;
        let client = Arc::new(ClientSlot::new(client));
        let preference = self.preference(&target);
        let weak = cx.entity().downgrade();
        let view = cx.new(|cx| {
            let mut view = Workspace::new(window, cx, client.clone(), loaded.data, demo);
            view.remote_label = remote_label;
            view.environment.target = target.clone();
            view.environment.preference = preference;
            view.environment.connection = self.connection(&target).ok().flatten();
            view.workspace_host = Some(weak);
            view
        });
        self.remember(target.clone());
        self.sessions.insert(
            target.key(),
            Session {
                view,
                client,
                _lock: loaded.lock,
                _profile_lock: profile_lock,
            },
        );
        self.select(target.key(), window, cx);
    }
    fn select(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(old) = self.sessions.get(&self.active) {
            old.view.update(cx, |view, _| {
                view.is_active = false;
                if view.inspector_closing.is_some() {
                    view.close_details();
                }
                view.cancel_image_batch();
                view.request_environment();
                view.request_stats();
                view.request_usage();
                view.request_logs(false);
            });
        }
        self.active = key;
        self.sessions[&self.active].view.update(cx, |view, cx| {
            view.is_active = true;
            view.request_environment();
            view.settings.appearance.apply(window, cx);
            view.request_stats();
            view.request_usage();
            view.request_logs(false);
            view.focus.focus(window, cx);
            cx.notify();
        });
        self.loading = false;
        cx.notify();
    }
    fn activate(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        if self.sessions.contains_key(&target.key()) {
            self.select(target.key(), window, cx);
            return;
        }
        self.loading = true;
        let cached = match &target {
            Target::Remote { directory, .. } => self.profiles.get(directory).cloned(),
            _ => None,
        };
        let base = self.base.clone();
        let destination = target.clone();
        let task = cx
            .background_executor()
            .spawn(async move { load(&base, &destination, cached) });
        cx.spawn_in(window, async move |host, cx| {
            let result = task.await;
            let _ = host.update_in(cx, |host, window, cx| {
                host.loading = false;
                match result {
                    Ok(loaded) => host.install(target, loaded, window, cx),
                    Err(error) => host.notice(format!("无法切换工作空间：{error}"), cx),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn notice(&self, message: String, cx: &mut Context<Self>) {
        self.sessions[&self.active].view.update(cx, |view, cx| {
            view.notice = Some(message);
            cx.notify();
        });
    }
}
impl Render for WorkspaceHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.sessions[&self.active].view.clone()
    }
}

#[cfg(feature = "ui-validation")]
pub(crate) mod validation;

#[cfg(all(test, not(feature = "demo-preview")))]
mod distribution_tests {
    use super::{Target, WorkspaceHost};
    use std::{collections::HashMap, path::PathBuf};

    #[test]
    fn legacy_demo_entry_is_skipped_without_losing_saved_real_environments() {
        let saved: Vec<Target> = serde_json::from_str(
            r#"[
                {"kind":"local"},
                {"kind":"demo"},
                {"kind":"docker","context":"my-context"},
                {"kind":"remote","directory":"/profile","agent":"agent-1","label":"Server"}
            ]"#,
        )
        .unwrap();
        let mut host = WorkspaceHost {
            base: PathBuf::new(),
            targets: vec![Target::Local],
            preferences: HashMap::new(),
            sessions: HashMap::new(),
            profiles: HashMap::new(),
            active: "local".into(),
            loading: false,
        };
        for target in saved {
            host.remember(target);
        }
        assert_eq!(host.targets.len(), 3);
        assert!(!host.targets.contains(&Target::Demo));
        assert_eq!(host.targets[1].key(), "docker:my-context");
        assert_eq!(host.targets[2].label(), "Server");
    }
}
