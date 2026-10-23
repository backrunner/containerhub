//! Selected environment inspection is independent from the active workspace.
mod charts;
mod details;
mod overview;
use crate::*;
use std::collections::VecDeque;
use workspaces::{EnvironmentPreferences, Target};

pub(super) struct EnvironmentView {
    pub target: Target,
    pub preference: EnvironmentPreferences,
    pub connection: Option<containerhub_runtime::ConnectionInfo>,
    pub client: Option<Arc<dyn ManagementClient>>,
    pub generation: u64,
    pub host: Option<HostSnapshot>,
    pub history: VecDeque<HostSnapshot>,
    pub containers: Option<Snapshot>,
    pub diagnostics: Option<RuntimeDiagnostics>,
    pub error: Option<String>,
    pub host_error: Option<String>,
    pub received_at: Option<u64>,
    pub request_ms: Option<u128>,
    pub paused: bool,
    pub range: u64,
    pub capabilities_open: bool,
    pub search: Entity<InputState>,
    pub query: String,
}
impl EnvironmentView {
    pub fn new(window: &mut Window, cx: &mut Context<Workspace>, demo: bool) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索环境…"));
        cx.subscribe(&search, |view, input, event, cx| {
            if matches!(event, InputEvent::Change) {
                view.environment.query = input.read(cx).value().to_string();
                cx.notify();
            }
        })
        .detach();
        Self {
            target: if demo { Target::Demo } else { Target::Local },
            preference: Default::default(),
            connection: None,
            client: None,
            generation: 0,
            host: None,
            history: VecDeque::new(),
            containers: None,
            diagnostics: None,
            error: None,
            host_error: None,
            received_at: None,
            request_ms: None,
            paused: false,
            range: 60,
            capabilities_open: false,
            search,
            query: String::new(),
        }
    }
    pub fn label(&self) -> String {
        if self.preference.name.is_empty() {
            self.target.label()
        } else {
            self.preference.name.clone()
        }
    }
    pub fn select(
        &mut self,
        target: Target,
        preference: EnvironmentPreferences,
        connection: Option<containerhub_runtime::ConnectionInfo>,
        client: anyhow::Result<Arc<dyn ManagementClient>>,
    ) {
        self.generation += 1;
        self.target = target;
        self.preference = preference;
        self.connection = connection;
        self.history.clear();
        self.host = None;
        self.containers = None;
        self.diagnostics = None;
        self.received_at = None;
        self.request_ms = None;
        self.host_error = None;
        self.paused = false;
        match client {
            Ok(client) => {
                self.client = Some(client);
                self.error = None;
            }
            Err(error) => {
                self.client = None;
                self.error = Some(error.to_string());
            }
        }
    }
    pub fn receive(&mut self, sample: backend::environment::Sample) {
        if self.generation != sample.generation || self.paused {
            return;
        }
        self.request_ms = sample.host.is_ok().then_some(sample.request_ms);
        match sample.host {
            Ok(host) => {
                if let Err(error) = host.validate() {
                    self.host_error = Some(error.message);
                    return;
                }
                // Do not mix data from different namespaces, including compromised/misconfigured providers.
                if sample
                    .containers
                    .as_ref()
                    .is_ok_and(|s| s.environment != host.environment)
                {
                    self.host_error = Some("主机与容器环境不匹配".into());
                    return;
                }
                if self
                    .history
                    .back()
                    .is_none_or(|last| last.observed_at <= host.observed_at)
                {
                    if self
                        .history
                        .back()
                        .is_some_and(|last| last.observed_at == host.observed_at)
                    {
                        self.history.pop_back();
                    }
                    self.history.retain(|old| {
                        old.environment == host.environment
                            && host.observed_at.saturating_sub(old.observed_at) <= 300
                    });
                    self.history.push_back(host.clone());
                    while self.history.len() > 151 {
                        self.history.pop_front();
                    }
                    self.received_at = Some(sample.received_at);
                    self.host = Some(host);
                    self.host_error = None;
                }
            }
            Err(error) => self.host_error = Some(error.message),
        }
        match sample.containers {
            Ok(containers) => {
                self.containers = Some(containers);
                self.error = None;
            }
            Err(error) => self.error = Some(error.message),
        }
        if let Ok(report) = sample.diagnostics {
            self.diagnostics = Some(report);
        }
    }
}
impl Workspace {
    pub(super) fn request_environment(&mut self) {
        let enabled = self.is_active && self.page == Page::Environments && !self.environment.paused;
        let request = self
            .environment
            .client
            .clone()
            .filter(|_| enabled)
            .map(|client| backend::environment::Watch {
                generation: self.environment.generation,
                client,
                interval: self.environment.preference.interval,
            });
        self.backend.environment.send_replace(request);
    }
    pub(super) fn refresh_environment(&mut self, cx: &mut Context<Self>) {
        self.environment.generation += 1;
        self.environment.paused = false;
        self.request_environment();
        // The active runtime service action uses its own generation and identity checks.
        self.refresh_diagnostics(cx);
    }
}
