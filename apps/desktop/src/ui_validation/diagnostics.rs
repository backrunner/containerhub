use super::*;

pub(super) fn sort_controls(
    window: &mut Window,
    cx: &mut App,
    view: &Entity<Workspace>,
    requests: &async_channel::Receiver<Request>,
) {
    for expected in [
        ContainerSort::State,
        ContainerSort::Image,
        ContainerSort::Name,
    ] {
        window.click("container-sort", cx);
        assert_eq!(view.read(cx).settings.sort, expected);
        assert!(
            matches!(requests.try_recv(), Ok(Request::SaveSettings(settings)) if settings.sort == expected)
        );
    }
    let ascending: Vec<_> = view.read(cx).visible().into_iter().map(|c| c.key).collect();
    window.click("container-sort-direction", cx);
    let descending: Vec<_> = view.read(cx).visible().into_iter().map(|c| c.key).collect();
    assert_eq!(descending, ascending.into_iter().rev().collect::<Vec<_>>());
    assert!(
        matches!(requests.try_recv(), Ok(Request::SaveSettings(settings)) if settings.sort_descending)
    );
    window.click("container-sort-direction", cx);
    assert!(
        matches!(requests.try_recv(), Ok(Request::SaveSettings(settings)) if !settings.sort_descending)
    );
}

pub(super) fn log_selection(window: &mut Window, cx: &mut App, view: &Entity<Workspace>) {
    let original = view
        .read(cx)
        .logs
        .result
        .as_ref()
        .unwrap()
        .as_ref()
        .unwrap()
        .clone();
    let text = view
        .read(cx)
        .logs
        .filtered()
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("已完成数据同步"));
    window.click("log-select-text", cx);
    let editor = window.find_by_label("日志快照选区");
    let bounds = editor.bounds();
    assert!(bounds.top() >= px(0.) && bounds.bottom() <= window.viewport_size().height);
    window.click(editor.path().last().unwrap().clone(), cx);
    window.press("secondary-a", cx);
    window.press("left", cx);
    window.press("shift-right", cx);
    window.press("shift-right", cx);
    window.click("log-copy-selection", cx);
    assert_eq!(
        cx.read_from_clipboard().unwrap().text().unwrap(),
        text.chars().take(2).collect::<String>()
    );
    let editor = window.find_by_label("日志快照选区");
    window.click(editor.path().last().unwrap().clone(), cx);
    window.press("secondary-a", cx);
    window.input("不允许修改", cx);
    view.update(cx, |view, cx| {
        let new = LogSnapshot::from_bytes(
            original.key.clone(),
            original.source,
            b"new content after refresh",
        );
        view.receive(Event::Logs(view.logs.generation, Ok(new)));
        cx.notify();
    });
    window.render_frame(cx);
    window.press("secondary-a", cx);
    window.press("secondary-c", cx);
    assert_eq!(
        cx.read_from_clipboard().unwrap().text().unwrap(),
        text,
        "read-only selection is frozen across refresh"
    );
    window.press("escape", cx);
    view.update(cx, |view, cx| {
        view.receive(Event::Logs(view.logs.generation, Ok(original)));
        cx.notify();
    });
    window.render_frame(cx);
}

pub(super) fn runtime_settings(
    window: &mut Window,
    cx: &mut App,
    view: &Entity<Workspace>,
    requests: &async_channel::Receiver<Request>,
    mut report: RuntimeDiagnostics,
) {
    assert!(matches!(requests.try_recv(), Ok(Request::Diagnostics)));
    report.service_error = Some(ManagementError::new(
        ErrorKind::Unavailable,
        "服务离线，CLI 能力仍可检查",
    ));
    let advanced = report
        .checks
        .iter_mut()
        .find(|c| c.feature == RuntimeFeature::AdvancedCreate)
        .unwrap();
    advanced.available = false;
    advanced.reason = Some("缺少 --mount 参数".into());
    view.update(cx, |view, cx| {
        view.demo = false;
        view.receive(Event::Diagnostics(Ok(report)));
        cx.notify();
    });
    window.render_frame(cx);
    assert_ne!(window.find("refresh").disabled(), Some(true));
    assert!(
        view.read(cx)
            .diagnostics
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .service_error
            .is_some()
    );
    window.scroll(
        "environments-scroll",
        ScrollDelta::Pixels(point(px(0.), px(-3000.))),
        cx,
    );
    window.click("runtime-path", cx);
    let input = window.find("runtime-path-input");
    window.click(input.path().last().unwrap().clone(), cx);
    window.input("/private/test/container", cx);
    window.click("ok", cx);
    assert!(
        matches!(requests.try_recv(), Ok(Request::ConfigureRuntime(settings)) if settings.apple_binary.as_deref() == Some(std::path::Path::new("/private/test/container")))
    );
    assert!(
        view.read(cx).settings.apple_binary.is_none(),
        "path changes only after worker confirmation"
    );
    view.update(cx, |view, cx| {
        view.receive(Event::RuntimeConfigured(Ok(Some(
            "/private/test/container".into(),
        ))));
        assert!(view.settings.apple_binary.is_some());
        view.demo = true;
        cx.notify();
    });
    window.render_frame(cx);
    window.click("dismiss-notice", cx);
    window.scroll(
        "environments-scroll",
        ScrollDelta::Pixels(point(px(0.), px(1000.))),
        cx,
    );
}
