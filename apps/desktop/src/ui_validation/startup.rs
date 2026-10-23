use super::*;

pub(super) fn fill(
    window: &mut Window,
    cx: &mut App,
    writes: &async_channel::Receiver<containerhub_management::queue::OperationRequest>,
) {
    window.scroll(
        "create-scroll",
        ScrollDelta::Pixels(point(px(0.), px(-800.))),
        cx,
    );
    window.click("create-startup", cx);
    window.click("ok", cx);
    assert!(
        writes.try_recv().is_err(),
        "empty custom startup must not submit"
    );
    window.scroll(
        "create-scroll",
        ScrollDelta::Pixels(point(px(0.), px(-800.))),
        cx,
    );
    let field = window.find_by_label("启动程序，留空沿用镜像，例如 /usr/bin/python3");
    window.click(field.path().last().unwrap().clone(), cx);
    window.input("/custom app", cx);
    window.click("add-startup-argument", cx);
    window.scroll(
        "create-scroll",
        ScrollDelta::Pixels(point(px(0.), px(-800.))),
        cx,
    );
    window.click(("startup-argument", 0usize), cx);
    window.input("two words", cx);
    window.click("add-startup-argument", cx); // Deliberately retain an empty second argument.
    window.scroll(
        "create-scroll",
        ScrollDelta::Pixels(point(px(0.), px(-800.))),
        cx,
    );
    window.click("add-startup-argument", cx);
    window.scroll(
        "create-scroll",
        ScrollDelta::Pixels(point(px(0.), px(-800.))),
        cx,
    );
    window.click(("startup-argument", 2usize), cx);
    window.input("removed", cx);
    window.click(("remove-startup-argument", 2usize), cx);
    window.scroll(
        "create-scroll",
        ScrollDelta::Pixels(point(px(0.), px(-800.))),
        cx,
    );
    window.click("create-and-start", cx);
    window.render_frame(cx);
    assert!(window.try_find(("startup-argument", 2usize)).is_none());
    for id in ["ok", "add-startup-argument"] {
        let bounds = window.find(id).bounds();
        assert!(bounds.left() >= px(0.) && bounds.right() <= window.viewport_size().width);
        assert!(bounds.top() >= px(0.) && bounds.bottom() <= window.viewport_size().height);
    }
}
