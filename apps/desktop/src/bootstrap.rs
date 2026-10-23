use crate::*;

pub(super) fn run() {
    #[cfg(feature = "ui-validation")]
    if std::env::args().any(|arg| arg == "--validate-ui") {
        ui_validation::run();
        return;
    }
    let arguments: Vec<String> = std::env::args().collect();
    let option = |name: &str| {
        arguments.iter().position(|a| a == name).map(|i| {
            arguments
                .get(i + 1)
                .cloned()
                .expect("option requires a value")
        })
    };
    let remote_profile = option("--remote-profile").map(PathBuf::from);
    let remote_agent = option("--remote-agent");
    let demo = cfg!(feature = "demo-preview") || arguments.iter().any(|arg| arg == "--demo");
    assert!(
        !demo || remote_profile.is_none(),
        "--demo and --remote-profile are separate modes"
    );
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .expect("HOME is required")
        .join("Library/Application Support/ContainerHub");
    let (target, cached) = if let Some(profile) = remote_profile {
        let directory = profile.canonicalize().expect("Remote profile directory");
        let profile = containerhub_runtime::RemoteProfile::open(&directory)
            .expect("Unable to open remote profile");
        let agents = profile.agents();
        let (agent, label) = agents
            .iter()
            .find(|(id, _)| remote_agent.as_ref().is_some_and(|selected| selected == id))
            .or_else(|| (remote_agent.is_none() && agents.len() == 1).then(|| &agents[0]))
            .expect("Select a bound Agent ID when there are zero or multiple environments")
            .clone();
        (
            workspaces::Target::Remote {
                directory,
                agent,
                label,
            },
            Some(profile),
        )
    } else if demo {
        (workspaces::Target::Demo, None)
    } else {
        (workspaces::Target::Local, None)
    };
    let loaded = workspaces::load(&base, &target, cached).expect("Unable to open workspace");
    application().with_assets(AppAssets).run(move |cx| {
        init(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-r", Refresh, Some("ContainerHub")),
            KeyBinding::new("cmd-f", FocusSearch, Some("ContainerHub")),
            KeyBinding::new("cmd-k", FocusSearch, Some("ContainerHub")),
            KeyBinding::new("escape", CloseDetails, Some("ContainerHub")),
        ]);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1440.), px(900.)),
                cx,
            ))),
            window_min_size: Some(size(px(760.), px(560.))),
            app_id: Some("dev.containerhub.desktop".into()),
            ..TitleBar::window_options()
        };
        open_window(options, cx, move |window, cx| {
            window.set_window_title("ContainerHub");
            cx.new(|cx| workspaces::WorkspaceHost::new(base, target, loaded, window, cx))
        })
        .expect("Unable to open ContainerHub");
        cx.activate(true);
    });
}
