mod backend;
mod layout;
mod logs;
mod theme;
#[cfg(feature = "ui-validation")]
mod ui_validation;

use backend::{Backend, Event, Request, Settings};
use containerhub_core::Resource;
use containerhub_core::*;
use controls::ControlStyle;
use gpui_kit::assets::IconName as Glyph;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Selectable, Sizable, TitleBar, WindowExt,
    button::*,
    dialog::DialogButtonProps,
    input::{Input, InputEvent, InputState},
};
use gpui_kit::prelude::*;
use gpui_kit::*;
use layout::Layout;
use logs::{DetailTab, LogView};
use std::{borrow::Cow, path::PathBuf, sync::Arc};
use theme::{Appearance, Palette};

gpui_kit::actions!(containerhub, [Quit, Refresh, FocusSearch, CloseDetails]);
// Observe the production elements without adding layout wrappers.
macro_rules! observed {
    ($element:expr) => {{
        #[cfg(feature = "ui-validation")]
        {
            use gpui_kit::test::TestSupportExt;
            $element.test_support()
        }
        #[cfg(not(feature = "ui-validation"))]
        {
            $element
        }
    }};
}
mod actions;
mod container_actions;
mod containers;
mod controls;
mod details;
mod environments;
mod forms;
mod images;
mod inspect;
mod inspector_motion;
mod log_export;
mod log_selection;
mod navigation;
mod pagination;
mod port_editor;
mod runtime_service;
mod runtime_settings;
mod selection;
mod shell;
mod stacks;
mod stats;
mod summary;
mod task_view;
mod usage;
mod workspaces;
struct AppAssets;
impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if path == "brand/icon.png" {
            return Ok(Some(Cow::Borrowed(include_bytes!(
                "../../../assets/app-icon.png"
            ))));
        }
        assets::AllAssets.load(path)
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        assets::AllAssets.list(path)
    }
}
#[derive(Clone, Copy, PartialEq)]
enum Page {
    Containers,
    Stacks,
    Resources(ResourceKind),
    Tasks,
    Settings,
    Environments,
}
impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::Containers => "容器",
            Self::Stacks => "Stacks",
            Self::Resources(k) => k.label(),
            Self::Tasks => "任务中心",
            Self::Settings => "设置",
            Self::Environments => "环境管理",
        }
    }
}
struct Workspace {
    backend: Backend,
    snapshot: Option<Snapshot>,
    diagnostics: Option<Result<RuntimeDiagnostics, String>>,
    diagnostics_loading: bool,
    error: Option<String>,
    notice: Option<String>,
    page: Page,
    search: Entity<InputState>,
    query: String,
    filter: u8,
    pagination: pagination::Pagination,
    container_scroll: UniformListScrollHandle,
    selected: Option<ResourceKey>,
    inspector_closing: Option<std::time::Instant>,
    initial_selection_pending: bool,
    resources: Option<Result<Vec<Resource>, String>>,
    resource_generation: u64,
    image: images::ImageView,
    inspect: inspect::InspectView,
    logs: LogView,
    stats: stats::StatsView,
    usage: usage::UsageView,
    environment: environments::EnvironmentView,
    stack: stacks::StackView,
    detail_tab: DetailTab,
    operations: Vec<Operation>,
    settings: Settings,
    demo: bool,
    is_active: bool,
    remote_label: Option<String>,
    workspace_host: Option<WeakEntity<workspaces::WorkspaceHost>>,
    batch: std::collections::HashSet<ResourceKey>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}
impl Workspace {
    fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        provider: Arc<dyn ManagementClient>,
        data: PathBuf,
        demo: bool,
    ) -> Self {
        let settings: Settings =
            backend::read_json(&data.join("settings.json")).unwrap_or_default();
        let (backend, events) = Backend::start(provider.clone(), data);
        let mut view = Self::with_backend(window, cx, settings, backend, events, demo);
        view.environment.client = Some(provider);
        view
    }
    fn with_backend(
        window: &mut Window,
        cx: &mut Context<Self>,
        settings: Settings,
        backend: Backend,
        events: async_channel::Receiver<Event>,
        demo: bool,
    ) -> Self {
        settings.appearance.apply(window, cx);
        let logs = LogView::new(window, cx);
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索名称、镜像或标签名…"));
        let stack = stacks::StackView::new(window, cx);
        let subscriptions = vec![
            cx.subscribe(&stack.search, |this, input, event, cx| {
                if matches!(event, InputEvent::Change) {
                    this.stack.query = input.read(cx).value().to_string();
                    cx.notify();
                }
            }),
            cx.subscribe(&logs.search, |this, input, event, cx| {
                if matches!(event, InputEvent::Change) {
                    this.logs.query = input.read(cx).value().to_string();
                    cx.notify();
                }
            }),
            cx.subscribe(&search, |this, input, event, cx| {
                if matches!(event, InputEvent::Change) {
                    this.query = input.read(cx).value().to_string();
                    this.reset_container_page();
                    cx.notify();
                }
            }),
            cx.observe_window_appearance(window, |this, window, cx| {
                if this.is_active && this.settings.appearance == Appearance::System {
                    this.settings.appearance.apply(window, cx);
                    cx.notify();
                }
            }),
        ];
        cx.spawn(async move |entity, cx| {
            while let Ok(event) = events.recv().await {
                if entity
                    .update(cx, |this, cx| {
                        this.receive(event);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Self {
            backend,
            snapshot: None,
            diagnostics: None,
            diagnostics_loading: false,
            error: None,
            notice: None,
            page: Page::Containers,
            stack,
            search,
            query: String::new(),
            filter: 0,
            pagination: Default::default(),
            container_scroll: UniformListScrollHandle::new(),
            selected: None,
            inspector_closing: None,
            initial_selection_pending: true,
            resources: None,
            resource_generation: 0,
            image: Default::default(),
            logs,
            stats: Default::default(),
            usage: Default::default(),
            environment: environments::EnvironmentView::new(window, cx, demo),
            detail_tab: DetailTab::Overview,
            inspect: Default::default(),
            operations: vec![],
            settings,
            demo,
            is_active: true,
            remote_label: None,
            workspace_host: None,
            batch: Default::default(),
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }
    fn receive(&mut self, event: Event) {
        match event {
            Event::Environment(sample) => self.environment.receive(*sample),
            Event::Snapshot(Ok(snapshot)) => {
                self.batch
                    .retain(|key| snapshot.containers.iter().any(|c| &c.key == key));
                if self
                    .selected
                    .as_ref()
                    .is_some_and(|k| !snapshot.containers.iter().any(|c| &c.key == k))
                {
                    self.close_details();
                }
                self.snapshot = Some(snapshot);
                if self.pagination.clamp(self.visible().len()) {
                    self.container_scroll.scroll_to_item(0, ScrollStrategy::Top);
                }
                self.error = None;
            }
            Event::Diagnostics(result) => {
                self.diagnostics_loading = false;
                self.diagnostics = Some(result.map_err(|error| error.message));
            }
            Event::RuntimeConfigured(result) => match result {
                Ok(path) => {
                    self.settings.apple_binary = path;
                    self.notice = Some("CLI 路径已保存，下次启动生效".into());
                }
                Err(error) => self.notice = Some(error.message),
            },
            Event::Snapshot(Err(error)) => self.error = Some(error.message),
            Event::Resources(kind, generation, result) => {
                if self.page == Page::Resources(kind) && generation == self.resource_generation {
                    if kind == ResourceKind::Image
                        && let Ok(items) = &result
                    {
                        let before = self.image.selected.len();
                        self.image
                            .selected
                            .retain(|reference| items.iter().any(|r| &r.name == reference));
                        if self.image.selected.len() != before {
                            self.cancel_image_batch();
                        }
                    }
                    self.resources = Some(result.map_err(|e| e.message));
                }
            }
            Event::ContainerDetails(generation, result) => {
                let result = *result;
                if generation == self.inspect.generation {
                    self.inspect.loading = false;
                    self.inspect.result = Some(
                        result
                            .and_then(|details| {
                                if Some(&details.container.key) != self.selected.as_ref() {
                                    Err(ManagementError::new(ErrorKind::Conflict, "容器身份已改变"))
                                } else {
                                    Ok(details)
                                }
                            })
                            .map_err(|e| e.message),
                    );
                }
            }
            Event::ControlError(message) => self.notice = Some(message),
            Event::Logs(generation, result) => {
                self.logs
                    .receive(generation, result, self.selected.as_ref())
            }
            Event::Image(generation, result) => {
                if self.page == Page::Resources(ResourceKind::Image)
                    && generation == self.image.generation
                {
                    self.image.result = Some(
                        result
                            .and_then(|image| {
                                if self
                                    .snapshot
                                    .as_ref()
                                    .is_none_or(|s| s.environment != image.key.environment)
                                {
                                    Err(ManagementError::new(
                                        ErrorKind::Conflict,
                                        "镜像详情环境不匹配",
                                    ))
                                } else {
                                    Ok(image)
                                }
                            })
                            .map_err(|e| e.message),
                    );
                }
            }
            Event::Stats(generation, result) => {
                if generation == self.stats.generation {
                    match result {
                        Ok(sample) if self.selected.as_ref() == Some(&sample.key) => {
                            self.stats.record(sample);
                        }
                        Ok(_) => {}
                        Err(error) => self.stats.error = Some(error.message),
                    }
                }
            }
            Event::Usage(generation, results) => {
                if generation == self.usage.generation {
                    for (key, result) in results {
                        if !self.usage.keys.contains(&key) {
                            continue;
                        }
                        match result {
                            Ok(sample) if sample.key == key => {
                                self.usage.samples.insert(key, sample);
                            }
                            _ => {
                                self.usage.samples.remove(&key);
                            }
                        }
                    }
                }
            }
            Event::Operation(op) => {
                if !op.state.is_active()
                    && self
                        .operations
                        .iter()
                        .any(|old| old.id == op.id && old.state.is_active())
                {
                    self.backend.send(Request::Refresh);
                    if matches!(op.command, Command::StartRuntime { .. }) {
                        self.diagnostics_loading = self.backend.send(Request::Diagnostics);
                    }
                    if let Page::Resources(kind) = self.page {
                        self.resource_generation += 1;
                        self.backend
                            .send(Request::Resources(kind, self.resource_generation));
                    }
                    if let Some(CommandResult::Container(container)) = &op.result {
                        self.selected = Some(container.key.clone());
                        self.detail_tab = DetailTab::Overview;
                    }
                    if matches!(op.command, Command::DeleteImage { .. }) {
                        if op.state == OperationState::Succeeded {
                            self.close_image();
                        } else if let Some(reference) = self.image.reference.clone() {
                            self.request_image(reference);
                        }
                    }
                }
                if let Some(old) = self.operations.iter_mut().find(|o| o.id == op.id) {
                    *old = op;
                } else {
                    self.operations.push(op);
                }
                if self.operations.len() > 100 {
                    self.operations.remove(0);
                }
            }
            Event::WriterUnavailable => {
                self.notice = Some("操作记录无法读取或保存，写操作已暂停；请检查数据目录。".into());
                for operation in &mut self.operations {
                    if operation.state == OperationState::Submitting {
                        operation.state = OperationState::Failed;
                        operation.finished_at = Some(now());
                        operation.message = Some("写入服务不可用，操作未提交".into());
                    }
                }
            }
            Event::StorageError => {
                self.notice = Some("无法保存本地设置或操作结果，请检查磁盘空间和目录权限。".into())
            }
        }
        self.request_stats();
        self.request_usage();
    }
    fn send(&mut self, request: Request, cx: &mut Context<Self>) {
        if !self.backend.send(request) {
            self.notice = Some("请求队列已满，请稍后再试。".into());
        }
        cx.notify();
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if let Some(reference) = self.image.reference.clone() {
            self.request_image(reference);
        }
        self.request_logs(false);
        self.request_inspect();
        self.send(Request::Refresh, cx);
        if self.page == Page::Environments {
            self.refresh_environment(cx);
        }
        if let Page::Resources(kind) = self.page {
            self.resources = None;
            self.resource_generation += 1;
            self.send(Request::Resources(kind, self.resource_generation), cx);
        }
    }
    fn navigate(&mut self, page: Page, cx: &mut Context<Self>) {
        if self.inspector_closing.is_some() {
            self.close_details();
        }
        self.cancel_image_batch();
        self.close_image();
        self.page = page;
        if page == Page::Environments {
            self.refresh_environment(cx);
        }
        self.request_environment();
        self.request_stats();
        self.request_usage();
        self.request_logs(false);
        if let Page::Resources(kind) = page {
            self.resources = None;
            self.resource_generation += 1;
            self.send(Request::Resources(kind, self.resource_generation), cx);
        }
        cx.notify();
    }

    fn visible(&self) -> Vec<Container> {
        let mut rows: Vec<Container> = self
            .snapshot
            .as_ref()
            .map(|s| {
                s.containers
                    .iter()
                    .filter(|c| {
                        c.matches(&self.query)
                            && (self.filter == 0
                                || (self.filter == 1 && c.state.is_running())
                                || (self.filter == 2 && c.state == ContainerState::Stopped))
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        sort_containers(&mut rows, self.settings.sort, self.settings.sort_descending);
        rows
    }
}
fn join(items: Vec<String>) -> String {
    if items.is_empty() {
        "未配置".into()
    } else {
        items.join("\n")
    }
}
fn empty(text: impl Into<SharedString>, p: Palette) -> impl IntoElement {
    div().flex_1().p_8().text_color(p.muted).child(text.into())
}
mod bootstrap;
fn main() {
    bootstrap::run();
}
