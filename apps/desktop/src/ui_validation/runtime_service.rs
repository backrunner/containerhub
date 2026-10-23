use super::*;
use containerhub_management::queue::OperationRequest;

pub(super) fn check(
    window: &mut Window,
    cx: &mut App,
    view: &Entity<Workspace>,
    requests: &async_channel::Receiver<Request>,
    writes: &async_channel::Receiver<OperationRequest>,
    mut report: RuntimeDiagnostics,
) {
    while requests.try_recv().is_ok() {}
    assert!(writes.try_recv().is_err());
    report.service_error = Some(ManagementError::new(ErrorKind::Unavailable, "服务未启动"));
    let saved = view.update(cx, |v, cx| {
        let saved = (
            v.snapshot.take(),
            v.error.take(),
            v.diagnostics.take(),
            v.demo,
            v.remote_label.take(),
        );
        v.operations.clear();
        v.error = Some("服务未启动".into());
        v.demo = false;
        v.page = Page::Environments;
        v.receive(Event::Diagnostics(Ok(report.clone())));
        cx.notify();
        saved
    });
    window.render_frame(cx);
    window.scroll(
        "environments-scroll",
        ScrollDelta::Pixels(point(px(0.), px(3000.))),
        cx,
    );
    assert_ne!(
        window.find("runtime-start").disabled(),
        Some(true),
        "startup works without inventory"
    );
    window.click("runtime-start", cx);
    let confirm = window.find("ok").bounds();
    assert!(confirm.bottom() <= window.viewport_size().height);
    window.press("escape", cx);
    assert!(writes.try_recv().is_err(), "dismissal must not submit");
    window.click("runtime-start", cx);
    // A refreshed CLI identity invalidates the open confirmation.
    view.update(cx, |v, cx| {
        let mut changed = report.clone();
        changed.cli_path = Some("/different/container".into());
        v.receive(Event::Diagnostics(Ok(changed)));
        cx.notify();
    });
    window.click("ok", cx);
    assert!(writes.try_recv().is_err(), "do not use a changed runtime");
    view.update(cx, |v, cx| {
        v.notice = None;
        v.receive(Event::Diagnostics(Ok(report.clone())));
        cx.notify();
    });
    window.render_frame(cx);
    window.click("runtime-start", cx);
    window.click("ok", cx);
    let mut op = submitted(writes.try_recv().unwrap());
    assert_eq!(
        op.command,
        Command::StartRuntime {
            environment: report.environment.clone()
        }
    );
    window.render_frame(cx);
    assert_eq!(window.find("runtime-start").disabled(), Some(true));
    window.click("runtime-start", cx);
    assert!(writes.try_recv().is_err(), "no duplicate startup");
    op.state = OperationState::UnknownOutcome;
    op.finished_at = Some(now());
    view.update(cx, |v, cx| {
        v.receive(Event::Operation(op.clone()));
        cx.notify();
    });
    let mut refresh = false;
    let mut diagnostics = false;
    while let Ok(request) = requests.try_recv() {
        refresh |= matches!(request, Request::Refresh);
        diagnostics |= matches!(request, Request::Diagnostics);
    }
    assert!(
        refresh && diagnostics,
        "terminal startup refreshes inventory and diagnostics"
    );
    window.render_frame(cx);
    assert!(window.try_find("runtime-start").is_none());
    window.click("runtime-reconcile", cx);
    assert!(matches!(writes.try_recv(), Ok(OperationRequest::Reconcile { id, .. }) if id == op.id));
    for kind in [
        ErrorKind::PermissionDenied,
        ErrorKind::InvalidData,
        ErrorKind::NotInstalled,
    ] {
        view.update(cx, |v, cx| {
            v.operations.clear();
            let mut report = report.clone();
            report.service_error = Some(ManagementError::new(kind, "需要手动处理"));
            v.receive(Event::Diagnostics(Ok(report)));
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(window.find("runtime-start").disabled(), Some(true));
    }
    view.update(cx, |v, cx| {
        v.remote_label = Some("远程环境".into());
        v.receive(Event::Diagnostics(Ok(report)));
        cx.notify();
    });
    window.render_frame(cx);
    assert!(
        window.try_find("runtime-start").is_none(),
        "host management is local-only"
    );
    view.update(cx, |v, cx| {
        (v.snapshot, v.error, v.diagnostics, v.demo, v.remote_label) = saved;
        v.diagnostics_loading = false;
        v.operations.clear();
        v.notice = None;
        cx.notify();
    });
}
