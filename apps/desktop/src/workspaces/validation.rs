use super::*;
use gpui_kit::test::{TestQueryExt, TestWindowExt};

pub(crate) fn run(snapshot: &Snapshot) {
    let text = gpui_kit::platform::current_platform(true).text_system();
    let mut cx = HeadlessAppContext::with_platform(
        text,
        Arc::new(AppAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let (handle, host) = cx
        .update(|cx| {
            open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1440.), px(900.)),
                    })),
                    focus: false,
                    show: false,
                    ..TitleBar::window_options()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| {
                        let weak = cx.entity().downgrade();
                        let targets = vec![Target::Local, Target::Demo];
                        let mut sessions = HashMap::new();
                        for target in &targets {
                            let (backend, events, _, _) = Backend::isolated();
                            let view = cx.new(|cx| {
                                let mut view = Workspace::with_backend(
                                    window,
                                    cx,
                                    Settings::default(),
                                    backend,
                                    events,
                                    true,
                                );
                                view.workspace_host = Some(weak.clone());
                                view.environment.target = target.clone();
                                view.is_active = target == &Target::Demo;
                                view.receive(Event::Snapshot(Ok(snapshot.clone())));
                                view
                            });
                            sessions.insert(
                                target.key(),
                                Session {
                                    view,
                                    client: Arc::new(ClientSlot::new(Arc::new(
                                        containerhub_core::fake::FakeProvider::default(),
                                    ))),
                                    _lock: File::open("/dev/null").unwrap(),
                                    _profile_lock: None,
                                },
                            );
                        }
                        WorkspaceHost {
                            base: PathBuf::from(".local/ui-validation/workspace-fixture"),
                            targets,
                            preferences: HashMap::new(),
                            sessions,
                            profiles: HashMap::new(),
                            active: "demo".into(),
                            loading: false,
                        }
                    })
                },
            )
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        let demo = host.read(cx).sessions["demo"].view.clone();
        demo.update(cx, |view, cx| {
            view.query = "web".into();
            view.filter = 1;
            view.batch.insert(snapshot.containers[0].key.clone());
            cx.notify();
        });
        window.render_frame(cx);
        window.click("workspace-switcher", cx);
        let item = window.find_by_label("本机").path().last().unwrap().clone();
        window.click(item, cx);
        assert_eq!(host.read(cx).active, "local");
        assert!(!demo.read(cx).is_active);
        assert!(demo.read(cx).backend.stats.borrow().is_none());
        assert!(demo.read(cx).backend.usage.borrow().is_none());
        assert!(demo.read(cx).backend.logs.borrow().is_none());
        let local = &host.read(cx).sessions["local"].view;
        assert!(local.read(cx).query.is_empty() && local.read(cx).batch.is_empty());
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.click("workspace-switcher", cx);
        let item = window
            .find_by_label("演示工作空间")
            .path()
            .last()
            .unwrap()
            .clone();
        window.click(item, cx);
        assert_eq!(host.read(cx).active, "demo");
        let demo = &host.read(cx).sessions["demo"].view;
        assert_eq!(demo.read(cx).query, "web");
        assert_eq!(demo.read(cx).filter, 1);
        assert_eq!(demo.read(cx).batch.len(), 1);
        assert!(demo.read(cx).is_active);
        assert!(demo.read(cx).backend.usage.borrow().is_some());
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        let demo = host.read(cx).sessions["demo"].view.clone();
        window.activate_window();
        demo.update(cx, |v, cx| {
            v.navigate(Page::Environments, cx);
            v.notice = None;
        });
        window.render_frame(cx);
        window.click(("environment-row", 0usize), cx);
        assert_eq!(
            host.read(cx).active,
            "demo",
            "inspection must not activate target"
        );
        assert_eq!(demo.read(cx).environment.target, Target::Local);
        window.right_click(("environment-row", 1usize), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.capture_screenshot(handle)
        .unwrap()
        .save(".local/ui-validation/environment-context-menu.png")
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        let demo = host.read(cx).sessions["demo"].view.clone();
        window.render_frame(cx);
        let edit = window.find_by_label("编辑…").path().last().unwrap().clone();
        window.click(edit, cx);
        assert_eq!(
            demo.read(cx).environment.target,
            Target::Local,
            "right-click must not change inspected target"
        );
        let input = window
            .find("environment-name")
            .path()
            .last()
            .unwrap()
            .clone();
        window.click(input, cx);
        window.press("secondary-a", cx);
        window.input("取消不保存", cx);
        window.press("escape", cx);
        assert_eq!(host.read(cx).label(&Target::Demo), "演示工作空间");
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.right_click(("environment-row", 1usize), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        let demo = host.read(cx).sessions["demo"].view.clone();
        window.render_frame(cx);
        let edit = window.find_by_label("编辑…").path().last().unwrap().clone();
        window.click(edit, cx);
        let input = window
            .find("environment-name")
            .path()
            .last()
            .unwrap()
            .clone();
        window.click(input, cx);
        window.press("secondary-a", cx);
        window.input("验收环境", cx);
        window.click(("environment-interval", 5usize), cx);
        window.click("ok", cx);
        assert_eq!(host.read(cx).label(&Target::Demo), "验收环境");
        assert_eq!(host.read(cx).label(&Target::Local), "本机");
        let persisted = preferences::read(&host.read(cx).base);
        assert_eq!(persisted["demo"].interval, 5);
        assert_eq!(persisted["demo"].name, "验收环境");
        assert_eq!(host.read(cx).active, "demo");
        let old_generation = demo.read(cx).environment.generation;
        window.click("host-pause", cx);
        assert!(demo.read(cx).backend.environment.borrow().is_none());
        assert!(demo.read(cx).environment.generation > old_generation);
        let left = window.find("environment-list").bounds();
        let right = window.find("environments-scroll").bounds();
        assert!(
            (left.top() - right.top()).abs() <= px(1.)
                && (left.bottom() - right.bottom()).abs() <= px(1.)
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.click("add-environment", cx);
        window.click("ok", cx);
        assert_eq!(
            host.read(cx).targets.len(),
            2,
            "invalid form must stay open"
        );
        for (id, text) in [
            ("add-environment-name", "Docker 验收"),
            ("add-docker-context", "fixture"),
        ] {
            let input = window.find(id).path().last().unwrap().clone();
            window.click(input, cx);
            window.press("secondary-a", cx);
            window.input(text, cx);
        }
        window.click("ok", cx);
        assert_eq!(host.read(cx).targets.len(), 3);
        assert_eq!(host.read(cx).active, "demo");
        assert_eq!(
            host.read(cx).label(&Target::Docker {
                context: "fixture".into()
            }),
            "Docker 验收"
        );
        assert_eq!(
            preferences::read(&host.read(cx).base)["docker:fixture"].name,
            "Docker 验收"
        );
    })
    .unwrap();
    cx.run_until_parked();
    println!(
        "PASS workspace switcher: isolated sessions preserve search/filter/selection; inactive polling pauses"
    );
}
