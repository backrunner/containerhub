use super::*;

pub(super) fn check(
    window: &mut Window,
    cx: &mut App,
    view: &Entity<Workspace>,
    writes: &async_channel::Receiver<containerhub_management::queue::OperationRequest>,
) {
    let mut container = view.read(cx).snapshot.as_ref().unwrap().containers[0].clone();
    container.state = ContainerState::Stopped;
    let op = Operation {
        id: 989,
        command: Command::CreateContainer {
            environment: container.key.environment.clone(),
            spec: CreateContainer {
                name: container.key.id.clone(),
                image: container.image.clone(),
                cpus: 1,
                memory_mib: 256,
                ports: vec![],
                options: CreateOptions {
                    start_after_create: true,
                    secret_environment: vec!["API_TOKEN".into()],
                    ..Default::default()
                },
            },
        },
        receipt: None,
        result: Some(CommandResult::Container(Box::new(container.clone()))),
        progress: Some(OperationProgress::Starting),
        cancel_requested: false,
        state: OperationState::Failed,
        started_at: now(),
        finished_at: Some(now()),
        message: Some("容器已创建，启动未完成".into()),
    };
    view.update(cx, |view, cx| {
        view.error = None;
        view.page = Page::Tasks;
        view.snapshot.as_mut().unwrap().containers[0] = container.clone();
        view.operations = vec![op.clone()];
        cx.notify();
    });
    window.render_frame(cx);
    let button = window.find(("retry-operation", 989usize)).bounds();
    assert!(button.left() >= px(0.) && button.right() <= window.viewport_size().width);
    window.click(("retry-operation", 989usize), cx);
    let containerhub_management::queue::OperationRequest::Submit(retry, secrets) =
        writes.try_recv().unwrap()
    else {
        panic!("retry input")
    };
    assert_eq!(
        retry.command,
        Command::Lifecycle {
            key: container.key.clone(),
            action: LifecycleAction::Start
        }
    );
    assert!(
        secrets.is_empty(),
        "starting an already created container needs no repeated secret input"
    );
    view.update(cx, |view, cx| {
        view.operations = vec![op];
        view.snapshot.as_mut().unwrap().containers[0]
            .key
            .incarnation
            .push_str("-replacement");
        cx.notify();
    });
    window.render_frame(cx);
    assert!(
        window.try_find(("retry-operation", 989usize)).is_none(),
        "do not start a same-name replacement"
    );
}
