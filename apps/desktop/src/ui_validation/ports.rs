use super::*;
pub(super) fn check(
    window: &mut Window,
    cx: &mut App,
    view: &Entity<Workspace>,
    writes: &async_channel::Receiver<containerhub_management::queue::OperationRequest>,
) {
    let snapshot = view.read(cx).snapshot.clone().unwrap();
    let old = snapshot
        .containers
        .iter()
        .find(|c| c.key.id == "web-api")
        .unwrap()
        .clone();
    view.update(cx, |view, cx| {
        view.page = Page::Containers;
        view.error = None;
        view.operations.clear();
        view.select_container(old.key.clone());
        view.detail_tab = DetailTab::Overview;
        cx.notify();
    });
    window.render_frame(cx);
    open(window, cx);
    window.render_frame(cx);
    let button = window.find("save-ports");
    assert_eq!(button.label(), Some("重建"));
    assert_eq!(button.bounds().size, size(px(64.), px(32.)));
    for name in [
        "port-host",
        "port-host-number",
        "port-container-number",
        "port-protocol",
    ] {
        assert_eq!(window.find((name, 0usize)).bounds().size.height, px(32.));
    }
    assert!(button.bounds().bottom() <= window.viewport_size().height);
    window.click(("port-host-number", 0usize), cx);
    window.press("cmd-a", cx);
    window.input("0", cx);
    window.click("save-ports", cx);
    assert!(
        writes.try_recv().is_err(),
        "invalid port must not queue a rebuild"
    );
    window.click(("port-host-number", 0usize), cx);
    window.press("cmd-a", cx);
    window.input("3001", cx);
    window.click("save-ports", cx);
    let op = submitted(writes.try_recv().unwrap());
    let Command::RebuildPorts {
        key,
        expected,
        expected_state,
        ports,
    } = op.command
    else {
        panic!("expected typed rebuild");
    };
    assert_eq!(key, old.key);
    assert_eq!(expected, old.ports);
    assert_eq!(expected_state, old.state);
    assert_eq!(ports[0].host_port, 3001);
    view.update(cx, |view, cx| {
        view.operations.clear();
        cx.notify();
    });
    window.render_frame(cx);
    open(window, cx);
    window.click(("port-host-number", 0usize), cx);
    window.press("cmd-a", cx);
    window.input("3002", cx);
    view.update(cx, |view, _| {
        view.snapshot
            .as_mut()
            .unwrap()
            .containers
            .iter_mut()
            .find(|c| c.key == old.key)
            .unwrap()
            .key
            .incarnation
            .push_str("-external");
    });
    window.click("save-ports", cx);
    assert!(
        writes.try_recv().is_err(),
        "stale modal cannot rebuild a replacement"
    );
    for _ in 0..8 {
        window.click("add-port", cx);
    }
    assert!(
        window.find("save-ports").bounds().bottom() <= window.viewport_size().height,
        "multiple mapping rows must scroll without pushing the footer outside the window"
    );
    window.press("escape", cx);
    view.update(cx, |view, cx| {
        view.snapshot = Some(snapshot);
        view.close_details();
        cx.notify();
    });
}

pub(super) fn open(window: &mut Window, cx: &mut App) {
    for _ in 0..6 {
        if window.find("edit-container-ports").visible() {
            break;
        }
        window.scroll(
            "detail-scroll",
            ScrollDelta::Pixels(point(px(0.), px(-60.))),
            cx,
        );
    }
    window.click("edit-container-ports", cx);
}
