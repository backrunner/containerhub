use super::*;
pub(super) fn check(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    view: &Entity<Workspace>,
    writes: &async_channel::Receiver<containerhub_management::queue::OperationRequest>,
    snapshot: &Snapshot,
    output: Option<(&std::path::Path, &str)>,
) {
    let mut snapshot = snapshot.clone();
    snapshot.capabilities.stacks = true;
    snapshot.environment = "docker:fixture".into();
    snapshot.cli_version = "Docker · 示例".into();
    for (i, c) in snapshot.containers.iter_mut().enumerate() {
        c.key.environment = snapshot.environment.clone();
        c.key.provider = ProviderKind::Docker;
        c.compose = Some(ComposeMembership {
            project: if i < 5 { "shop" } else { "tools" }.into(),
            service: c.key.id.clone(),
            one_off: false,
        });
    }
    while writes.try_recv().is_ok() {}
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |v, cx| {
            v.snapshot = Some(snapshot.clone());
            v.demo = true;
            v.error = None;
            v.notice = None;
            v.operations.clear();
            v.query.clear();
            v.search.update(cx, |s, cx| s.set_value("", window, cx));
            v.stack.selected = Some("Compose:shop".into());
            v.navigate(Page::Stacks, cx);
            for c in snapshot.containers.iter().filter(|c| c.state.is_running()) {
                v.usage.samples.insert(
                    c.key.clone(),
                    ContainerStats {
                        key: c.key.clone(),
                        observed_at: now(),
                        cpu_percent: Some(2.5),
                        memory_bytes: Some(48 * 1024 * 1024),
                        memory_limit_bytes: Some(512 * 1024 * 1024),
                    },
                );
            }
            cx.notify();
        });
        window.click(("stack-expand", 0usize), cx);
        window.render_frame(cx);
        let left = window.find("stack-list").bounds();
        let right = window.find("stack-detail").bounds();
        assert!((left.bottom() - right.bottom()).abs() <= px(1.));
        assert!(right.right() <= window.viewport_size().width);
        assert_ne!(window.find("stack-stop").disabled(), Some(true));
    })
    .unwrap();
    cx.run_until_parked();
    if let Some((output, name)) = output {
        cx.capture_screenshot(handle)
            .unwrap()
            .save(output.join(format!("{name}-stacks.png")))
            .unwrap();
    }
    cx.update_window(handle, |_, w, cx| {
        w.click(("stack-expand", 0usize), cx);
    })
    .unwrap();
    cx.run_until_parked();
    // Filtering and selection stay scoped; a row action ignores another checked row.
    cx.update_window(handle, |_, w, cx| {
        w.click(("stack-service-check", 1usize), cx);
        w.click("stack-attention", cx);
        w.click("stack-select-all", cx);
        view.update(cx, |v, _| {
            let stack = Stack::from_snapshot(v.snapshot.as_ref().unwrap()).remove(0);
            assert_eq!(v.stack.checked_names(&stack), vec!["minio", "nginx-proxy"]);
        });
        w.click("stack-attention", cx);
        w.click(("stack-service-lifecycle", 0usize), cx);
        w.click("stack-confirm", cx);
        let containerhub_management::queue::OperationRequest::Submit(op, _) =
            writes.try_recv().unwrap()
        else {
            panic!()
        };
        assert_eq!(
            op.command.target(),
            "minio",
            "row action must target the clicked row"
        );
        assert!(writes.try_recv().is_err());
        view.update(cx, |v, cx| {
            v.operations.clear();
            v.stack.checked.clear();
            cx.notify();
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.click(("stack-service-check", 0usize), cx);
        w.click(("stack-service-check", 1usize), cx);
        w.click("stack-stop", cx);
        w.click("stack-confirm", cx);
        let mut targets = vec![];
        while let Ok(containerhub_management::queue::OperationRequest::Submit(op, _)) =
            writes.try_recv()
        {
            targets.push(op.command.target().to_string());
        }
        targets.sort();
        assert_eq!(targets, vec!["minio", "nginx-proxy"]);
        view.update(cx, |v, cx| {
            v.operations.clear();
            cx.notify();
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.click("stack-stop", cx);
        window.render_frame(cx);
        assert!(window.find("stack-confirm").bounds().bottom() <= window.viewport_size().height);
    })
    .unwrap();
    cx.run_until_parked();
    if let Some((output, name)) = output {
        cx.capture_screenshot(handle)
            .unwrap()
            .save(output.join(format!("{name}-stack-confirm.png")))
            .unwrap();
    }
    cx.update_window(handle,|_,window,cx|{
        window.click("stack-confirm",cx);
        let mut queued=vec![];
        while let Ok(request)=writes.try_recv() {
            if let containerhub_management::queue::OperationRequest::Submit(op,_)=request {queued.push(op.command);}
        }
        assert_eq!(queued.len(),5);
        assert!(queued.iter().all(|c|matches!(c,Command::Lifecycle{key,action:LifecycleAction::Stop} if key.environment=="docker:fixture")));
        view.update(cx,|v,cx|{v.operations.clear();v.stack.selected=Some("Compose:tools".into());cx.notify();});
    }).unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.click("stack-start", cx);
        window.press("escape", cx);
        assert!(writes.try_recv().is_err(), "cancel must not submit");
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.click("stack-start", cx);
        view.update(cx, |v, _| {
            v.snapshot.as_mut().unwrap().containers[5].key.incarnation = "replacement".into()
        });
        window.click("stack-confirm", cx);
        assert!(
            writes.try_recv().is_err(),
            "stale member must block the whole planned batch"
        );
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    snapshot.swarm = SwarmInventory {
        status: SwarmStatus::Manager,
        services: vec![
            swarm_fixture("shop_web", false),
            swarm_fixture("shop_agent", true),
            extra_service(
                "shop_worker",
                "c",
                Some(2),
                "paused",
                "ghcr.io/example/worker:2.4",
            ),
            extra_service("shop_cache", "d", Some(1), "completed", "redis:7-alpine"),
            extra_service("shop_preview", "e", Some(0), "", "ghcr.io/example/api:dev"),
        ],
    };
    let mut local = snapshot.containers[0].clone();
    local.key.id = "shop_web.1.fx83pa".into();
    local.key.incarnation = format!("8a72d4f1c036{}", "a".repeat(52));
    local.compose = None;
    local.image = "nginx:alpine".into();
    local.health = ContainerHealth::Healthy;
    local.ports.clear();
    local.memory_limit = Some(512 * 1024 * 1024);
    snapshot.containers.push(local.clone());
    cx.update_window(handle, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.snapshot = Some(snapshot.clone());
            v.operations.clear();
            v.notice = None;
            v.stack.selected = Some("Swarm:shop".into());
            v.usage.samples.insert(
                local.key.clone(),
                ContainerStats {
                    key: local.key.clone(),
                    observed_at: now(),
                    cpu_percent: Some(2.4),
                    memory_bytes: Some(48 * 1024 * 1024),
                    memory_limit_bytes: local.memory_limit,
                },
            );
            cx.notify();
        });
        w.click(("stack-expand", 0usize), cx);
        w.render_frame(cx);
        let action_bounds = w.find(("stack-service-actions", 0usize)).bounds();
        assert!((action_bounds.size.width - px(112.)).abs() <= px(1.));
        assert!(action_bounds.right() <= w.find("stack-detail").bounds().right());
        assert!(
            (w.find("stack-list").bounds().bottom() - w.find("stack-detail").bounds().bottom())
                .abs()
                <= px(1.)
        );
    })
    .unwrap();
    cx.run_until_parked();
    if let Some((out, name)) = output {
        cx.capture_screenshot(handle)
            .unwrap()
            .save(out.join(format!("{name}-swarm.png")))
            .unwrap();
    }
    cx.update_window(handle, |_, w, cx| {
        assert!(w.try_find(("swarm-history", 0usize)).is_none());
        assert!(
            w.try_find("stack-container-0-2").is_none(),
            "pending and historical tasks are not containers"
        );
        assert_eq!(w.find("stack-inspect-0-1").disabled(), Some(true));
        assert_ne!(w.find("stack-inspect-0-0").disabled(), Some(true));
        let card = w.find("stack-container-0-0").bounds();
        assert!(card.right() <= w.find("stack-detail").bounds().right());
        w.click("stack-inspect-0-0", cx);
        view.update(cx, |v, cx| {
            assert!(v.page == Page::Containers);
            assert_eq!(v.selected.as_ref(), Some(&local.key));
            v.navigate(Page::Stacks, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    // Pagination contains only actual containers; a smaller fresh inventory clamps the page.
    cx.update_window(handle, |_, w, cx| {
        view.update(cx, |v, cx| {
            let s = &mut v.snapshot.as_mut().unwrap().swarm.services[0];
            for n in 20..23 {
                let mut t = s.tasks[1].clone();
                t.id = format!("additional-{n}");
                t.slot = n;
                t.container_id = Some(format!("{n:064x}"));
                s.tasks.push(t);
            }
            cx.notify();
        });
        w.render_frame(cx);
        w.click(("stack-containers-next", 0usize), cx);
        w.render_frame(cx);
        assert_eq!(
            w.find(("stack-containers-next", 0usize)).disabled(),
            Some(true)
        );
        assert_eq!(w.find("stack-inspect-0-0").disabled(), Some(true));
        assert!(w.try_find("stack-container-0-1").is_none());
        view.update(cx, |v, cx| {
            v.snapshot = Some(snapshot.clone());
            cx.notify();
        });
        w.render_frame(cx);
        assert!(w.try_find(("stack-containers-next", 0usize)).is_none());
        assert_ne!(w.find("stack-inspect-0-0").disabled(), Some(true));
        // Reveal the complete compact overview, then restore the service toolbar.
        w.scroll(
            "stack-services-scroll",
            ScrollDelta::Pixels(point(px(0.), px(-140.))),
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    if let Some((out, name)) = output {
        cx.capture_screenshot(handle)
            .unwrap()
            .save(out.join(format!("{name}-container-overview.png")))
            .unwrap();
    }
    cx.update_window(handle, |_, w, cx| {
        w.scroll(
            "stack-services-scroll",
            ScrollDelta::Pixels(point(px(0.), px(10000.))),
            cx,
        );
        w.click(("swarm-scale", 0usize), cx);
        w.render_frame(cx);
        assert!(w.find("ok").bounds().bottom() <= w.viewport_size().height);
    })
    .unwrap();
    cx.run_until_parked();
    if let Some((out, name)) = output {
        cx.capture_screenshot(handle)
            .unwrap()
            .save(out.join(format!("{name}-swarm-scale.png")))
            .unwrap();
    }
    cx.update_window(handle, |_, w, cx| {
        w.click("swarm-replicas", cx);
        w.press("cmd-a", cx);
        w.input("-1", cx);
        w.click("ok", cx);
        assert!(writes.try_recv().is_err());
        w.click("swarm-replicas", cx);
        w.press("cmd-a", cx);
        w.input("0", cx);
    })
    .unwrap();
    cx.run_until_parked();
    if let Some((out, name)) = output {
        cx.capture_screenshot(handle)
            .unwrap()
            .save(out.join(format!("{name}-swarm-scale-zero.png")))
            .unwrap();
    }
    cx.update_window(handle, |_, w, cx| {
        w.click("ok", cx);
        let containerhub_management::queue::OperationRequest::Submit(op, _) =
            writes.try_recv().unwrap()
        else {
            panic!()
        };
        assert!(matches!(
            op.command,
            Command::SwarmService {
                action: SwarmAction::Scale { replicas: 0 },
                ..
            }
        ));
        view.update(cx, |v, cx| {
            v.operations.clear();
            cx.notify();
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.click("swarm-restart", cx);
        view.update(cx, |v, _| {
            v.snapshot.as_mut().unwrap().swarm.services[0].version += 1
        });
        w.click("stack-confirm", cx);
        assert!(writes.try_recv().is_err());
        w.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.click(("stack-kind", 1usize), cx);
        assert!(
            w.try_find("stack-stop").is_some(),
            "Compose type filter must resolve the same-name Compose project"
        );
        w.click(("stack-kind", 2usize), cx);
        assert!(w.try_find("swarm-restart").is_some());
        w.click("stack-search", cx);
        w.input("REDIS", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find(("stack-service-row", 3usize)).is_some());
        assert!(w.try_find(("stack-service-row", 0usize)).is_none());
        w.click("stack-search", cx);
        w.press("cmd-a", cx);
        w.input("no-matching-stack", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("stack-detail").is_none());
        w.click("stack-search", cx);
        w.press("cmd-a", cx);
        w.press("backspace", cx);
        view.update(cx, |v, cx| {
            let snapshot = v.snapshot.as_mut().unwrap();
            snapshot.lifecycle_supported = false;
            snapshot.capabilities.create_container = false;
            cx.notify();
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(w.find("swarm-restart").disabled(), Some(true));
        assert_eq!(w.find(("swarm-scale", 0usize)).disabled(), Some(true));
        assert!(writes.try_recv().is_err());
    })
    .unwrap();
    println!(
        "PASS Stack: scoped and row operations, same-name type filters, image search, attention selection, container identity and remote availability, stale versions, read-only controls"
    );
}

fn swarm_fixture(name: &str, global: bool) -> SwarmService {
    SwarmService {
        key: ResourceKey {
            environment: "docker:fixture".into(),
            provider: ProviderKind::Docker,
            namespace: Some("swarm-service".into()),
            id: name.into(),
            incarnation: if global {
                "b".repeat(25)
            } else {
                "a".repeat(25)
            },
        },
        stack: "shop".into(),
        version: 12,
        image: if global {
            "grafana/agent:latest"
        } else {
            "nginx:alpine"
        }
        .into(),
        mode: if global {
            SwarmMode::Global
        } else {
            SwarmMode::Replicated
        },
        desired_replicas: if global { None } else { Some(3) },
        force_update: 0,
        update_state: Some("completed".into()),
        ports: if global {
            vec![]
        } else {
            vec!["8080 → 80/tcp".into()]
        },
        tasks: (1..=if global { 2 } else { 12 })
            .map(|i| SwarmTask {
                container_id: (!global && i != 3)
                    .then(|| format!("{:012x}{}", 0x8a72d4f1c035_u64 + i, "a".repeat(52))),
                id: format!("task-{i}-fixture"),
                node: if i == 3 {
                    String::new()
                } else {
                    format!("node-{}", if i > 3 { 3 } else { i })
                },
                slot: if global { 0 } else { i.min(3) },
                desired_state: if i > 3 { "shutdown" } else { "running" }.into(),
                state: if i > 3 {
                    "failed"
                } else if i == 3 {
                    "pending"
                } else {
                    "running"
                }
                .into(),
            })
            .collect(),
        tasks_truncated: false,
    }
}

fn extra_service(
    name: &str,
    id: &str,
    replicas: Option<u64>,
    update: &str,
    image: &str,
) -> SwarmService {
    let mut s = swarm_fixture(name, false);
    s.key.incarnation = id.repeat(25);
    s.image = image.into();
    s.desired_replicas = replicas;
    s.update_state = (!update.is_empty()).then(|| update.into());
    s.ports = if replicas == Some(0) {
        vec!["8081 → 3000/tcp".into()]
    } else {
        vec![]
    };
    s.tasks.truncate(replicas.unwrap_or(0) as usize);
    for task in &mut s.tasks {
        task.container_id = None;
    }
    if update == "paused" {
        s.tasks[1].state = "rejected".into();
    }
    s
}
