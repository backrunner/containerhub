//! Opt-in validation using the real Metal offscreen renderer. No live runtime,
//! user preferences, OS input injection or operating-system window capture.
mod create_start;
use super::*;
use containerhub_core::{LogSnapshot, LogSource};
use gpui_kit::test::{TestQueryExt, TestWindowExt};
mod diagnostics;
mod image_batch;
mod inspector_motion;
mod pagination;
mod ports;
mod recovery;
mod runtime_service;
mod stacks;
mod startup;
mod visuals;

pub fn run() {
    let structure_only = std::env::args().any(|arg| arg == "--structure-only");
    let output = PathBuf::from(".local/ui-validation");
    std::fs::create_dir_all(&output).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let provider = containerhub_core::fake::FakeProvider::default();
    let snapshot = runtime.block_on(provider.snapshot()).unwrap();
    workspaces::validation::run(&snapshot);
    let primary = snapshot
        .containers
        .iter()
        .find(|c| c.key.id == "minio")
        .unwrap();
    let diagnostics = runtime.block_on(provider.diagnostics()).unwrap();
    let unused_reference = "example.invalid/ui-unused:v1";
    runtime
        .block_on(provider.execute(&Command::PullImage {
            environment: "demo".into(),
            reference: unused_reference.into(),
        }))
        .unwrap();
    let image_rows = runtime
        .block_on(provider.resources(ResourceKind::Image))
        .unwrap();
    let used_image = runtime
        .block_on(provider.inspect_image("demo", &image_rows[0].name))
        .unwrap();
    let unused_image = runtime
        .block_on(provider.inspect_image("demo", unused_reference))
        .unwrap();
    for appearance in [Appearance::Light, Appearance::Dark] {
        for (width, height) in [(760., 560.), (1040., 700.), (1440., 900.)] {
            let name = format!(
                "{}-{}x{}",
                if appearance == Appearance::Dark {
                    "dark"
                } else {
                    "light"
                },
                width as u32,
                height as u32
            );
            println!("Checking {name}…");
            let text_system = gpui_kit::platform::current_platform(true).text_system();
            let mut cx = if structure_only {
                HeadlessAppContext::with_asset_source(text_system, Arc::new(AppAssets))
            } else {
                HeadlessAppContext::with_platform(
                    text_system,
                    Arc::new(AppAssets),
                    gpui_kit::platform::current_headless_renderer,
                )
            };
            cx.update(|cx| {
                gpui_kit::init(cx);
                // Deterministic settled geometry: synthetic pointer events must
                // not chase the dialog entrance animation between frames.
                cx.set_reduce_motion(true);
            });
            let (backend, events, requests, writes) = Backend::isolated();
            let (handle, view) = cx
                .update(|cx| {
                    open_window(
                        WindowOptions {
                            window_bounds: Some(WindowBounds::Windowed(Bounds {
                                origin: Default::default(),
                                size: size(px(width), px(height)),
                            })),
                            focus: false,
                            show: false,
                            ..TitleBar::window_options()
                        },
                        cx,
                        |window, cx| {
                            cx.new(|cx| {
                                let mut view = Workspace::with_backend(
                                    window,
                                    cx,
                                    Settings {
                                        appearance,
                                        ..Default::default()
                                    },
                                    backend,
                                    events,
                                    true,
                                );
                                view.receive(Event::Snapshot(Ok(snapshot.clone())));
                                let samples = snapshot
                                    .containers
                                    .iter()
                                    .filter(|c| c.state.is_running())
                                    .map(|c| {
                                        (c.key.clone(), runtime.block_on(provider.stats(&c.key)))
                                    })
                                    .collect();
                                view.receive(Event::Usage(view.usage.generation, samples));
                                view
                            })
                        },
                    )
                })
                .unwrap();
            cx.update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                let sidebar = window.find("sidebar").bounds();
                assert_eq!(sidebar.size.width, px(Layout::for_width(width).sidebar));
                for control in ["refresh", "settings", "pull-image", "create-container"] {
                    let b = window.find(control).bounds();
                    assert!(b.left() >= px(0.) && b.right() <= px(width));
                    assert!(
                        b.top() >= px(0.) && b.bottom() <= px(height),
                        "{name}: {control} outside window: {b:?}"
                    );
                }
                for control in ["refresh", "settings"] {
                    let b = window.find(control).bounds();
                    assert_eq!(b.size, size(px(32.), px(32.)), "{control} must be square");
                }
                let gear = window.find("settings").bounds();
                for control in ["container-sort", "container-sort-direction"] {
                    assert_eq!(window.find(control).bounds().size, size(px(24.), px(24.)));
                }
                let action = window
                    .find((SharedString::from("action-mailpit"), 0usize))
                    .bounds();
                let restart = window
                    .find((SharedString::from("restart-mailpit"), 0usize))
                    .bounds();
                let more = window
                    .find((SharedString::from("more-mailpit"), 0usize))
                    .bounds();
                assert_eq!(action.size.height, px(28.));
                assert_eq!(restart.size, size(px(28.), px(28.)));
                assert_eq!(more.size, size(px(28.), px(28.)));
                assert_eq!(restart.left() - action.right(), px(8.));
                assert_eq!(action.size.width, px(64.));
                assert_eq!(more.left() - restart.right(), px(8.));
                let search_command = window.find("command-search").bounds();
                assert!(gear.left() >= search_command.right() && gear.bottom() <= px(54.));
                window.click("select-all-containers", cx);
                assert_eq!(view.read(cx).batch.len(), 8);
                window.click(("select-container", 0usize), cx);
                assert_eq!(view.read(cx).batch.len(), 7);
                assert!(window.try_find_by_label("部分已选，全选当前页").is_some());
                window.click("select-all-containers", cx);
                assert_eq!(view.read(cx).batch.len(), 8);
                window.click("select-all-containers", cx);
                assert!(view.read(cx).batch.is_empty());
                let header = window.find("container-header").bounds();
                let row = window.find(("container", 0usize)).bounds();
                assert!(
                    (header.right() - row.right()).abs() <= px(1.),
                    "{name}: rows must fill the table, not shrink to content"
                );
                assert_eq!(
                    window.find("container-status-header").bounds().left(),
                    window.find(("container-status", 0usize)).bounds().left()
                );
                let pull = window.find("pull-image").bounds();
                let create = window.find("create-container").bounds();
                assert!(
                    create.left() - pull.right() <= px(10.),
                    "{name}: page actions must stay grouped"
                );
                assert_eq!(
                    cx.theme().tokens.button_primary.color,
                    Palette::get(appearance == Appearance::Dark).primary
                );
                window.click("status-filter", cx);
                let filter = window
                    .find_by_label("已停止")
                    .path()
                    .last()
                    .unwrap()
                    .clone();
                window.click(filter, cx);
                assert_eq!(view.read(cx).visible().len(), 3);
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                window.click("status-filter", cx);
                let filter = window
                    .find_by_label("全部状态")
                    .path()
                    .last()
                    .unwrap()
                    .clone();
                window.click(filter, cx);
                assert_eq!(view.read(cx).visible().len(), 8);
                // Input uses actual production focus and text dispatch; this is not an OS IME test.
                let search = window.find_by_label("搜索名称、镜像或标签名…");
                window.click(search.path().last().unwrap().clone(), cx);
                window.input("postgres", cx);
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                assert_eq!(view.read(cx).visible().len(), 1);
                window.press("secondary-a", cx);
                window.input("中文不存在", cx);
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                assert_eq!(view.read(cx).visible().len(), 0);
                window.press("secondary-a", cx);
                window.press("backspace", cx);
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                assert_eq!(view.read(cx).visible().len(), 8);
                window.render_frame(cx);
                diagnostics::sort_controls(window, cx, &view, &requests);
            })
            .unwrap();
            pagination::check(&mut cx, handle, &view);
            if !structure_only {
                cx.capture_screenshot(handle)
                    .unwrap()
                    .save(output.join(format!("{name}-list.png")))
                    .unwrap();
            }
            cx.update_window(handle, |_, window, cx| {
                let index = view
                    .read(cx)
                    .visible()
                    .iter()
                    .position(|c| c.key == primary.key)
                    .unwrap();
                window.click(("container", index), cx);
                assert_eq!(view.read(cx).selected.as_ref(), Some(&primary.key));
                assert_eq!(window.try_find("list-pane").is_some(), width >= 1280.);
                let inspector = window.find("inspector").bounds();
                assert!(inspector.right() <= px(width) && inspector.bottom() <= px(height));
                assert!(inspector.size.width >= px(300.));
            })
            .unwrap();
            if !structure_only {
                cx.capture_screenshot(handle)
                    .unwrap()
                    .save(output.join(format!("{name}-overview.png")))
                    .unwrap();
            }
            cx.update_window(handle, |_, window, cx| {
                window.click(("detail-tab", 1usize), cx);
                let generation = view.read(cx).logs.generation;
                let log = runtime
                    .block_on(provider.logs(&primary.key, LogSource::Standard))
                    .unwrap();
                view.update(cx, |view, cx| {
                    view.receive(Event::Logs(generation, Ok(log)));
                    cx.notify();
                });
                window.render_frame(cx);
                window.click("log-copy", cx);
                assert!(
                    cx.read_from_clipboard()
                        .unwrap()
                        .text()
                        .unwrap()
                        .contains("已完成数据同步")
                );
                window.click("dismiss-notice", cx);
                diagnostics::log_selection(window, cx, &view);
                window.click("log-automatic", cx);
                assert!(!view.read(cx).logs.automatic);
                assert!(view.read(cx).backend.logs.borrow().is_none());
                // A completed old request must not replace the paused view.
                view.update(cx, |view, _| {
                    view.receive(Event::Logs(
                        generation,
                        Err(containerhub_core::ManagementError::new(
                            containerhub_core::ErrorKind::Timeout,
                            "stale",
                        )),
                    ))
                });
                assert!(matches!(view.read(cx).logs.result, Some(Ok(_))));
                window.click(("log-source", 1usize), cx);
                let generation = view.read(cx).logs.generation;
                let log = runtime
                    .block_on(provider.logs(&primary.key, LogSource::Boot))
                    .unwrap();
                view.update(cx, |view, cx| {
                    view.receive(Event::Logs(generation, Ok(log)));
                    cx.notify();
                });
                window.render_frame(cx);
                assert!(matches!(
                    &view.read(cx).logs.result,
                    Some(Ok(LogSnapshot {
                        source: LogSource::Boot,
                        ..
                    }))
                ));
                // Return to application logs for the artifact.
                window.click(("log-source", 0usize), cx);
                let generation = view.read(cx).logs.generation;
                let log = runtime
                    .block_on(provider.logs(&primary.key, LogSource::Standard))
                    .unwrap();
                view.update(cx, |view, cx| {
                    view.receive(Event::Logs(generation, Ok(log)));
                    cx.notify();
                });
                window.render_frame(cx);
            })
            .unwrap();
            cx.update_window(handle, |_, window, cx| {
                let input = window.find_by_label("筛选当前日志…");
                window.click(input.path().last().unwrap().clone(), cx);
                window.input("WARN", cx);
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                window.click("log-copy", cx);
                let copied = cx.read_from_clipboard().unwrap().text().unwrap();
                assert_eq!(copied.lines().count(), 1);
                assert!(copied.contains("WARN"));
                assert_ne!(window.find("log-export").disabled(), Some(true));
                let export_bounds = window.find("log-export").bounds();
                assert!(export_bounds.right() <= px(width) && export_bounds.bottom() <= px(height));
                // The headless platform returns picker cancellation; filesystem behavior has a separate test.
                window.click("log-export", cx);
                window.click("dismiss-notice", cx);
                let input = window.find_by_label("筛选当前日志…");
                window.click(input.path().last().unwrap().clone(), cx);
                window.press("secondary-a", cx);
                window.input("no-matching-log", cx);
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                assert_eq!(window.find("log-copy").disabled(), Some(true));
                assert_eq!(window.find("log-export").disabled(), Some(true));
                window.press("secondary-a", cx);
                window.press("backspace", cx);
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| window.render_frame(cx))
                .unwrap();
            if !structure_only {
                cx.capture_screenshot(handle)
                    .unwrap()
                    .save(output.join(format!("{name}-logs.png")))
                    .unwrap();
            }
            cx.update_window(handle, |_, window, cx| {
                inspector_motion::check(window, cx, &view);
                assert!(view.read(cx).selected.is_none());
                assert!(view.read(cx).backend.logs.borrow().is_none());
                window.click(("nav", 5usize), cx);
                diagnostics::runtime_settings(window, cx, &view, &requests, diagnostics.clone());
                window.click("settings", cx);
                window.click(
                    (
                        "appearance",
                        if appearance == Appearance::Dark {
                            1usize
                        } else {
                            2usize
                        },
                    ),
                    cx,
                );
                assert_ne!(view.read(cx).settings.appearance, appearance);
                assert!(matches!(requests.try_recv(), Ok(Request::SaveSettings(_))));
                window.click(("nav", 0usize), cx);
                window.click((SharedString::from("action-minio"), 0usize), cx);
                let op = submitted(writes.try_recv().unwrap());
                assert_eq!(op.command.resource_key(), Some(&primary.key));
                assert!(matches!(
                    op.command,
                    Command::Lifecycle {
                        action: LifecycleAction::Stop,
                        ..
                    }
                ));
                assert_eq!(
                    window
                        .find((SharedString::from("action-minio"), 0usize))
                        .disabled(),
                    Some(true)
                );
                assert_eq!(op.state, OperationState::Submitting);
                view.update(cx, |view, cx| {
                    let mut failed = op.clone();
                    failed.state = OperationState::Failed;
                    view.receive(Event::Operation(failed));
                    cx.notify();
                });
                window.click(("nav", 4usize), cx);
                window.click(("retry-operation", op.id as usize), cx);
                let retry = submitted(writes.try_recv().unwrap());
                assert_ne!(retry.id, op.id);
                assert_eq!(retry.command, op.command);
                view.update(cx, |view, cx| {
                    view.operations.clear();
                    cx.notify();
                });
                window.click(("nav", 0usize), cx);
                window.click(("select-container", 0usize), cx);
                window.click(("select-container", 1usize), cx);
                assert_eq!(view.read(cx).batch.len(), 2);
                window.click("batch-start", cx);
                let first = submitted(writes.try_recv().unwrap());
                let second = submitted(writes.try_recv().unwrap());
                assert_ne!(first.command.resource_key(), second.command.resource_key());
                assert!(matches!(
                    first.command,
                    Command::Lifecycle {
                        action: LifecycleAction::Start,
                        ..
                    }
                ));
                assert!(matches!(
                    second.command,
                    Command::Lifecycle {
                        action: LifecycleAction::Start,
                        ..
                    }
                ));
                assert!(view.read(cx).batch.is_empty());
                view.update(cx, |view, cx| {
                    view.remote_label = Some("远程测试 · 示例".into());
                    view.operations.clear();
                    cx.notify();
                });
                assert!(window.try_find("list-pane").is_some());
                window.click(("select-container", 0usize), cx);
                window.click(("select-container", 1usize), cx);
                window.click("batch-stop", cx);
                assert!(
                    writes.try_recv().is_err(),
                    "batch stop requires confirmation"
                );
                window.click("ok", cx);
                for _ in 0..2 {
                    assert!(matches!(
                        submitted(writes.try_recv().unwrap()).command,
                        Command::Lifecycle {
                            action: LifecycleAction::Stop,
                            ..
                        }
                    ));
                }
                view.update(cx, |view, cx| {
                    view.operations.clear();
                    cx.notify();
                });
            })
            .unwrap();
            cx.update_window(handle, |_, window, cx| {
                window.click("pull-image", cx);
                window.click("ok", cx);
                assert!(writes.try_recv().is_err(), "empty image must not submit");
                let field = window.find_by_label("例如 docker.io/library/nginx:alpine");
                window.click(field.path().last().unwrap().clone(), cx);
                window.input("example.invalid/ui-fixture:v1", cx);
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                window.click("ok", cx);
                let operation = submitted(writes.try_recv().unwrap());
                assert!(matches!(operation.command, Command::PullImage { .. }));
                window.click("create-container", cx);
                let field = window.find_by_label("容器名称，例如 my-nginx");
                window.click(field.path().last().unwrap().clone(), cx);
                window.input("ui-fixture", cx);
                let field = window.find_by_label("已拉取的镜像引用");
                window.click(field.path().last().unwrap().clone(), cx);
                window.input("example.invalid/ui-fixture:v1", cx);
                for id in ["create-name", "create-image"] {
                    let bounds = window.find(id).bounds();
                    assert!(bounds.top() >= px(0.) && bounds.bottom() <= px(height));
                    assert_eq!(bounds.size.height, px(34.));
                }
                window.scroll(
                    "create-scroll",
                    ScrollDelta::Pixels(point(px(0.), px(-160.))),
                    cx,
                );
                let field = window.find_by_label("例如 8080:80/tcp, 5353:53/udp");
                window.click(field.path().last().unwrap().clone(), cx);
                window.input("8080:80/tcp, 5353:53/udp", cx);
                for id in ["create-ports", "ok"] {
                    let bounds = window.find(id).bounds();
                    assert!(
                        bounds.left() >= px(0.)
                            && bounds.right() <= px(width)
                            && bounds.top() >= px(0.)
                            && bounds.bottom() <= px(height),
                        "{name}: {id} outside window {bounds:?}"
                    );
                }
            })
            .unwrap();
            cx.update_window(handle, |_, window, cx| {
                window.scroll(
                    "create-scroll",
                    ScrollDelta::Pixels(point(px(0.), px(-200.))),
                    cx,
                );
                window.click("create-advanced", cx);
                window.scroll(
                    "create-scroll",
                    ScrollDelta::Pixels(point(px(0.), px(-400.))),
                    cx,
                );
                window.click("create-read-only", cx);
                let field = window.find_by_label("用户，例如 1000:1000");
                window.click(field.path().last().unwrap().clone(), cx);
                window.input("1000:1000", cx);
                let bounds = window.find("ok").bounds();
                assert!(bounds.bottom() <= px(height));
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                window.scroll(
                    "create-scroll",
                    ScrollDelta::Pixels(point(px(0.), px(-800.))),
                    cx,
                );
                window.click("add-secret", cx);
                window.scroll(
                    "create-scroll",
                    ScrollDelta::Pixels(point(px(0.), px(-800.))),
                    cx,
                );
                let field = window.find_by_label("变量名，例如 API_TOKEN");
                window.click(field.path().last().unwrap().clone(), cx);
                window.input("API_TOKEN", cx);
                let field = window.find_by_label("单行值，仅用于本次创建");
                window.click(field.path().last().unwrap().clone(), cx);
                window.input("ui-secret-canary", cx);
                assert!(window.elements().iter().all(|element| {
                    element
                        .value()
                        .is_none_or(|v| !v.contains("ui-secret-canary"))
                }));
                startup::fill(window, cx, &writes);
                let bounds = window.find("ok").bounds();
                assert!(bounds.bottom() <= px(height));
                window.click("ok", cx);
                let containerhub_management::queue::OperationRequest::Submit(operation, secrets) =
                    writes.try_recv().unwrap()
                else {
                    panic!("create input");
                };
                secrets.validate_for(&operation.command).unwrap();
                assert_eq!(
                    &**secrets.stdin_bytes().unwrap(),
                    b"API_TOKEN=ui-secret-canary\n"
                );
                assert!(
                    !serde_json::to_string(&operation)
                        .unwrap()
                        .contains("ui-secret-canary")
                );
                let Command::CreateContainer { spec, .. } = operation.command else {
                    panic!("create command")
                };
                assert_eq!(
                    spec.options.startup,
                    Some(Box::new(StartupCommand {
                        entrypoint: Some("/custom app".into()),
                        arguments: vec!["two words".into(), "".into()]
                    }))
                );
                assert_eq!(spec.name, "ui-fixture");
                assert_eq!(spec.ports[0].host, "127.0.0.1");
                assert_eq!(spec.ports[0].container_port, 80);
                assert_eq!(spec.ports[1].protocol, "udp");
                assert!(spec.options.read_only);
                assert!(spec.options.start_after_create);
                assert_eq!(spec.options.user.as_deref(), Some("1000:1000"));
            })
            .unwrap();
            // Finish the create dialog's deferred close before opening another.
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                view.update(cx, |view, cx| {
                    view.select_container(snapshot.containers[5].key.clone());
                    cx.notify();
                });
                window.scroll(
                    "detail-scroll",
                    ScrollDelta::Pixels(point(px(0.), px(-1200.))),
                    cx,
                );
                window.click("delete-container", cx);
                assert!(writes.try_recv().is_err(), "delete requires confirmation");
                window.click("ok", cx);
                let operation = submitted(writes.try_recv().unwrap());
                assert_eq!(
                    operation.command,
                    Command::DeleteContainer {
                        key: snapshot.containers[5].key.clone()
                    }
                );
                view.update(cx, |view, cx| {
                    view.select_container(primary.key.clone());
                    let sample = ContainerStats {
                        key: primary.key.clone(),
                        observed_at: now(),
                        cpu_percent: None,
                        memory_bytes: Some(1234),
                        memory_limit_bytes: Some(536870912),
                    };
                    view.receive(Event::Stats(
                        view.stats.generation.saturating_sub(1),
                        Ok(sample.clone()),
                    ));
                    assert!(view.stats.sample.is_none());
                    view.receive(Event::Stats(view.stats.generation, Ok(sample)));
                    assert_eq!(view.stats.sample.as_ref().unwrap().memory_bytes, Some(1234));
                    cx.notify();
                });
            })
            .unwrap();
            cx.update_window(handle, |_, window, cx| {
                while requests.try_recv().is_ok() {}
                window.click(("nav", 1usize), cx);
                let generation = view.read(cx).resource_generation;
                view.update(cx, |view, cx| {
                    view.receive(Event::Resources(
                        ResourceKind::Image,
                        generation,
                        Ok(image_rows.clone()),
                    ));
                    cx.notify();
                });
                window.click(("inspect-image", 0usize), cx);
                let generation = view.read(cx).image.generation;
                view.update(cx, |view, cx| {
                    view.receive(Event::Image(generation - 1, Ok(unused_image.clone())));
                    cx.notify();
                });
                assert!(view.read(cx).image.result.is_none());
                view.update(cx, |view, cx| {
                    view.receive(Event::Image(generation, Ok(used_image.clone())));
                    cx.notify();
                });
                window.render_frame(cx);
                assert_eq!(window.find("delete-image").disabled(), Some(true));
                window.click("image-copy", cx);
                let copied: serde_json::Value =
                    serde_json::from_str(&cx.read_from_clipboard().unwrap().text().unwrap())
                        .unwrap();
                assert_eq!(copied["key"]["digest"], used_image.key.digest);
                window.click("dismiss-notice", cx);
                window.click("close-image", cx);
                window.scroll(
                    "resource-scroll",
                    ScrollDelta::Pixels(point(px(0.), px(-1000.))),
                    cx,
                );
                window.click(("inspect-image", image_rows.len() - 1), cx);
                let generation = view.read(cx).image.generation;
                view.update(cx, |view, cx| {
                    view.receive(Event::Image(generation, Ok(unused_image.clone())));
                    cx.notify();
                });
                window.render_frame(cx);
                assert_ne!(window.find("delete-image").disabled(), Some(true));
                for id in ["close-image", "image-copy", "delete-image"] {
                    let bounds = window.find(id).bounds();
                    assert!(
                        bounds.left() >= px(0.)
                            && bounds.right() <= px(width)
                            && bounds.top() >= px(0.)
                            && bounds.bottom() <= px(height),
                        "{name}: {id} outside window"
                    );
                }
                window.click("delete-image", cx);
                assert!(
                    writes.try_recv().is_err(),
                    "image deletion requires confirmation"
                );
                window.click("ok", cx);
                assert_eq!(
                    submitted(writes.try_recv().unwrap()).command,
                    Command::DeleteImage {
                        key: unused_image.key.clone()
                    }
                );
                window.click("close-image", cx);
                view.update(cx, |view, cx| {
                    view.receive(Event::Image(generation, Ok(used_image.clone())));
                    cx.notify();
                });
                assert!(view.read(cx).image.result.is_none());
            })
            .unwrap();
            image_batch::run(
                &mut cx,
                handle,
                &view,
                &requests,
                &writes,
                image_batch::Fixture {
                    rows: &image_rows,
                    used: &used_image,
                    unused: &unused_image,
                },
            );
            cx.update_window(handle, |_, window, cx| {
                view.update(cx, |view, cx| {
                    view.page = Page::Containers;
                    view.select_container(primary.key.clone());
                    cx.notify();
                });
                window.render_frame(cx);
                window.click(("detail-tab", 2usize), cx);
                let (generation, key) = view
                    .read(cx)
                    .selected
                    .clone()
                    .map(|key| (view.read(cx).inspect.generation, key))
                    .unwrap();
                let details = ContainerDetails {
                    container: primary.clone(),
                    observed_at: now(),
                    read_only: Some(true),
                    working_directory: Some("/app".into()),
                    user: Some("1000:1000".into()),
                    environment_names: vec!["TOKEN".into()],
                    label_names: vec!["dev.containerhub.managed".into()],
                    argument_count: 2,
                };
                view.update(cx, |view, cx| {
                    view.receive(Event::ContainerDetails(
                        generation,
                        Box::new(Ok(details.clone())),
                    ));
                    cx.notify();
                });
                window.render_frame(cx);
                window.click("inspect-copy", cx);
                assert!(
                    cx.read_from_clipboard()
                        .unwrap()
                        .text()
                        .unwrap()
                        .contains("argument_count")
                );
                assert_eq!(view.read(cx).selected.as_ref(), Some(&key));
                view.update(cx, |view, cx| {
                    view.close_details();
                    view.receive(Event::ContainerDetails(generation, Box::new(Ok(details))));
                    assert!(view.inspect.result.is_none(), "late inspect result ignored");
                    view.page = Page::Tasks;
                    view.operations = vec![Operation {
                        receipt: None,
                        id: 987,
                        command: Command::PullImage {
                            environment: "demo".into(),
                            reference: "example.invalid/progress:v1".into(),
                        },
                        result: None,
                        state: OperationState::Running,
                        progress: Some(OperationProgress::Fetching),
                        cancel_requested: false,
                        started_at: now(),
                        finished_at: None,
                        message: None,
                    }];
                    cx.notify();
                });
                window.render_frame(cx);
                window.click(("cancel-operation", 987usize), cx);
                assert!(matches!(
                    writes.try_recv().unwrap(),
                    containerhub_management::queue::OperationRequest::Cancel { id: 987, .. }
                ));
                recovery::check(window, cx, &view, &writes);
                ports::check(window, cx, &view, &writes);
                runtime_service::check(window, cx, &view, &requests, &writes, diagnostics.clone());
                create_start::check(window, cx, &view, &writes);
            })
            .unwrap();
            if !structure_only {
                visuals::capture(&mut cx, handle, &view, appearance, &output, &name);
            }
            stacks::check(
                &mut cx,
                handle,
                &view,
                &writes,
                &snapshot,
                (!structure_only).then_some((&output, name.as_str())),
            );
            println!(
                "PASS {name}: management, advanced form, inspect/stale responses, pull cancel, stable sorting, offline diagnostics/path confirmation, read-only Unicode log selection frozen across refresh"
            );
        }
    }
    if structure_only {
        println!("Structure/interaction checks passed; pixel rendering was explicitly skipped.");
    } else {
        println!("Metal offscreen captures: {}", output.display());
    }
}

fn submitted(request: containerhub_management::queue::OperationRequest) -> Operation {
    match request {
        containerhub_management::queue::OperationRequest::Submit(op, _) => *op,
        _ => panic!("expected submit"),
    }
}
