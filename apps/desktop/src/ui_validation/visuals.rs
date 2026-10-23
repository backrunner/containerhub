//! Repeatable pixel evidence for states which structural checks cannot judge.
use super::*;

pub(super) fn capture(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    view: &Entity<Workspace>,
    appearance: Appearance,
    output: &std::path::Path,
    name: &str,
) {
    let save = |cx: &mut HeadlessAppContext, stage: &str| {
        cx.run_until_parked();
        cx.capture_screenshot(handle)
            .unwrap()
            .save(output.join(format!("{name}-{stage}.png")))
            .unwrap();
    };
    cx.update_window(handle, |_, window, cx| {
        appearance.apply(window, cx);
        view.update(cx, |view, cx| {
            view.settings.appearance = appearance;
            view.notice = None;
            // The preceding recovery check deliberately replaces the identity.
            // Restore it only in this isolated fixture to show the safe retry UI.
            let created = match view.operations[0].result.as_ref().unwrap() {
                CommandResult::Container(container) => (**container).clone(),
                _ => panic!("expected creation checkpoint fixture"),
            };
            view.snapshot.as_mut().unwrap().containers[0] = created;
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find(("retry-operation", 989usize)).is_some());
    })
    .unwrap();
    save(cx, "partial-result");
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.close_details();
            view.operations.clear();
            view.remote_label = None;
            view.page = Page::Containers;
            view.error = Some("无法连接运行时，正在显示上次同步结果。请检查服务后刷新。".into());
            cx.notify();
        });
        window.render_frame(cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, _| {
        assert_eq!(window.find("create-container").disabled(), Some(true));
        assert_eq!(window.find("pull-image").disabled(), Some(true));
        assert_ne!(window.find("refresh").disabled(), Some(true));
    })
    .unwrap();
    save(cx, "offline");
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.error = None;
            cx.notify();
        });
        window.click("create-container", cx);
    })
    .unwrap();
    save(cx, "create");
    cx.update_window(handle, |_, window, cx| {
        window.scroll(
            "create-scroll",
            ScrollDelta::Pixels(point(px(0.), px(-200.))),
            cx,
        );
        window.click("create-advanced", cx);
        window.scroll(
            "create-scroll",
            ScrollDelta::Pixels(point(px(0.), px(-280.))),
            cx,
        );
    })
    .unwrap();
    save(cx, "create-advanced");
    cx.update_window(handle, |_, window, cx| {
        window.scroll(
            "create-scroll",
            ScrollDelta::Pixels(point(px(0.), px(-1000.))),
            cx,
        );
        window.click("add-secret", cx);
        window.scroll(
            "create-scroll",
            ScrollDelta::Pixels(point(px(0.), px(-1000.))),
            cx,
        );
        window.click("create-startup", cx);
        window.scroll(
            "create-scroll",
            ScrollDelta::Pixels(point(px(0.), px(-1000.))),
            cx,
        );
        window.render_frame(cx);
        let submit = window.find("ok").bounds();
        assert!(submit.bottom() <= window.viewport_size().height);
    })
    .unwrap();
    save(cx, "create-startup");
    cx.update_window(handle, |_, window, cx| window.press("escape", cx))
        .unwrap();
    cx.run_until_parked();
    refined_views(cx, handle, view, output, name);
}

fn refined_views(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    view: &Entity<Workspace>,
    output: &std::path::Path,
    name: &str,
) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let provider = containerhub_core::fake::FakeProvider::default();
    let snapshot = runtime.block_on(provider.snapshot()).unwrap();
    let container = snapshot
        .containers
        .iter()
        .find(|c| c.key.id == "web-api")
        .unwrap()
        .clone();
    let details = runtime
        .block_on(provider.inspect_container(&container.key))
        .unwrap();
    let resources = runtime
        .block_on(provider.resources(ResourceKind::Image))
        .unwrap();
    let diagnostics = runtime.block_on(provider.diagnostics()).unwrap();
    let save = |cx: &mut HeadlessAppContext, stage: &str| {
        cx.run_until_parked();
        cx.capture_screenshot(handle)
            .unwrap()
            .save(output.join(format!("{name}-{stage}.png")))
            .unwrap();
    };
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.demo = true;
            view.snapshot = Some(snapshot.clone());
            view.page = Page::Containers;
            view.query.clear();
            view.search
                .update(cx, |state, cx| state.set_value("", window, cx));
            view.filter = 0;
            view.batch.clear();
            view.operations.clear();
            view.notice = None;
            view.select_container(container.key.clone());
            view.detail_tab = DetailTab::Inspect;
            view.inspect.result = Some(Ok(details));
            view.inspect.loading = false;
            view.request_usage();
            let samples = snapshot
                .containers
                .iter()
                .filter(|c| c.state.is_running())
                .map(|c| (c.key.clone(), runtime.block_on(provider.stats(&c.key))))
                .collect();
            view.receive(Event::Usage(view.usage.generation, samples));
            view.stats
                .record(runtime.block_on(provider.stats(&container.key)).unwrap());
            cx.notify();
        });
        window.render_frame(cx);
    })
    .unwrap();
    save(cx, "config");
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.detail_tab = DetailTab::Overview;
            cx.notify();
        });
        window.render_frame(cx);
    })
    .unwrap();
    save(cx, "ports-overview");
    cx.update_window(handle, |_, window, cx| {
        ports::open(window, cx);
    })
    .unwrap();
    save(cx, "ports-modal");
    cx.update_window(handle, |_, window, cx| {
        let button = window.find("save-ports").bounds();
        assert!(button.bottom() <= window.viewport_size().height);
        window.press("escape", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.pagination.size = 5;
            view.pagination.page = 1;
            if window.viewport_size().width < px(1280.) {
                view.close_details();
            }
            cx.notify();
        });
        window.render_frame(cx);
    })
    .unwrap();
    save(cx, "pagination");
    view.update(cx, |view, _| {
        view.pagination = Default::default();
    });

    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.page = Page::Resources(ResourceKind::Image);
            view.close_image();
            view.image.selected = resources.iter().take(2).map(|r| r.name.clone()).collect();
            view.resources = Some(Ok(resources));
            cx.notify();
        });
        window.render_frame(cx);
    })
    .unwrap();
    save(cx, "images-multi");
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.page = Page::Environments;
            view.diagnostics = Some(Ok(diagnostics.clone()));
            let mut host = runtime.block_on(provider.host_snapshot()).unwrap();
            view.environment.target = workspaces::Target::Demo;
            for i in 0..31 {
                host.observed_at = now().saturating_sub(60 - i * 2);
                host.cpu_percent = Some(24.6 + (i as f64 * 0.4).sin() * 8.);
                view.environment.history.push_back(host.clone());
            }
            view.environment.host = Some(host);
            view.environment.containers = Some(snapshot.clone());
            view.environment.diagnostics = Some(diagnostics);
            view.environment.error = None;
            view.diagnostics_loading = false;
            cx.notify();
        });
        window.render_frame(cx);
    })
    .unwrap();
    save(cx, "environments");
    cx.update_window(handle, |_, window, cx| {
        workspaces::edit_form_fixture(false, window, cx);
        window.render_frame(cx);
    })
    .unwrap();
    save(cx, "environment-edit");
    cx.update_window(handle, |_, window, cx| {
        assert!(window.find("ok").bounds().bottom() <= window.viewport_size().height);
        assert!(window.find("environment-direct").bounds().right() <= window.viewport_size().width);
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        workspaces::edit_form_fixture(true, window, cx);
        window.render_frame(cx);
    })
    .unwrap();
    save(cx, "environment-edit-docker");
    cx.update_window(handle, |_, window, cx| {
        assert!(window.find("ok").bounds().bottom() <= window.viewport_size().height);
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();

    for (mode, label) in [(0, "environment-add-docker"), (1, "environment-add-agent")] {
        cx.update_window(handle, |_, window, cx| {
            workspaces::add_form_fixture(mode, window, cx);
            window.render_frame(cx);
        })
        .unwrap();
        save(cx, label);
        cx.update_window(handle, |_, window, cx| {
            assert!(window.find("ok").bounds().bottom() <= window.viewport_size().height);
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
    }

    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.demo = false;
            view.environment.diagnostics = None;
            view.settings.apple_binary = Some("/private/fixture/container".into());
            let report = view.diagnostics.as_mut().unwrap().as_mut().unwrap();
            report.cli_version = Some("1.6.0".into());
            report.cli_path = Some("/private/fixture/container".into());
            report.service_error = Some(ManagementError::new(
                ErrorKind::Unavailable,
                "服务未启动。可启动本机运行时，或在终端完成首次安装。",
            ));
            cx.notify();
        });
        window.render_frame(cx);
    })
    .unwrap();
    save(cx, "runtime-offline");
    cx.update_window(handle, |_, window, cx| {
        window.click("runtime-start", cx);
    })
    .unwrap();
    save(cx, "runtime-confirm");
    cx.update_window(handle, |_, window, cx| {
        assert!(window.find("ok").bounds().bottom() <= window.viewport_size().height);
        window.press("escape", cx);
    })
    .unwrap();
}
