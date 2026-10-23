use super::*;

pub(super) fn check(
    window: &mut Window,
    cx: &mut App,
    view: &Entity<Workspace>,
    writes: &async_channel::Receiver<containerhub_management::queue::OperationRequest>,
) {
    view.update(cx, |view, cx| {
        let op = &mut view.operations[0];
        op.state = OperationState::UnknownOutcome;
        op.receipt = Some(ExecutionReceipt::Remote {
            agent: "pinned-agent".into(),
            owner: "owner".into(),
            request_id: "original-request-1234567890".into(),
            expires_at: 1,
        });
        op.message = Some("远程连接中断，结果待核对".into());
        cx.notify();
    });
    window.render_frame(cx);
    let bounds = window.find(("reconcile-operation", 987usize)).bounds();
    assert!(bounds.left() >= px(0.) && bounds.right() <= window.viewport_size().width);
    window.click(("reconcile-operation", 987usize), cx);
    assert!(matches!(
        writes.try_recv().unwrap(),
        containerhub_management::queue::OperationRequest::Reconcile { id: 987, .. }
    ));
    assert!(
        writes.try_recv().is_err(),
        "recovery must not submit a new command"
    );
    view.update(cx, |view, cx| {
        let mut op = view.operations[0].clone();
        op.state = OperationState::Reconciling;
        view.receive(Event::Operation(op));
        cx.notify();
    });
    window.render_frame(cx);
    assert!(window.try_find(("reconcile-operation", 987usize)).is_none());
    view.update(cx, |view, cx| {
        let mut op = view.operations[0].clone();
        op.state = OperationState::Succeeded;
        op.result = Some(CommandResult::Confirmed);
        view.receive(Event::Operation(op));
        cx.notify();
    });
    assert_eq!(view.read(cx).operations[0].id, 987);
    assert!(view.read(cx).operations[0].receipt.is_some());
}
