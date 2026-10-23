use super::*;
use std::time::{Duration, Instant};

pub(super) fn check(window: &mut Window, cx: &mut App, view: &Entity<Workspace>) {
    let key = view.read(cx).selected.clone().unwrap();
    let button = window.find("close-details").bounds();
    let glyph = window.find("inspector-glyph").bounds();
    let panel = window.find("inspector").bounds();
    assert_eq!(button.size, size(px(32.), px(32.)));
    assert_eq!(button.top(), glyph.top());
    assert_eq!(panel.right() - button.right(), glyph.left() - panel.left());

    cx.set_reduce_motion(false);
    window.click("close-details", cx);
    assert_eq!(view.read(cx).selected.as_ref(), Some(&key));
    assert!(view.read(cx).inspector_closing.is_some());

    // Sample an actual intermediate layout: the content stays at its original
    // width, while its clipping column and the inter-column gap shrink.
    view.update(cx, |view, cx| {
        view.inspector_closing = Some(Instant::now() - Duration::from_millis(100));
        cx.notify();
    });
    window.render_frame(cx);
    let slot = window.find("inspector-slot").bounds();
    assert_eq!(
        window.find("inspector").bounds().size.width,
        panel.size.width
    );
    if Layout::for_width(f32::from(window.viewport_size().width)).split_inspector {
        assert!(slot.size.width > px(0.) && slot.size.width < panel.size.width);
        let list = window.find("list-pane").bounds();
        assert!(slot.left() - list.right() < px(20.));
        assert_eq!(slot.bottom(), list.bottom());
    }

    view.update(cx, |view, cx| {
        view.inspector_closing = Some(Instant::now() - Duration::from_millis(250));
        cx.notify();
    });
    window.render_frame(cx);
    assert!(view.read(cx).selected.is_none());
    assert!(window.try_find("inspector").is_none());
    assert!(view.read(cx).backend.logs.borrow().is_none());

    // Reopening a row while it is closing must cancel the close permanently.
    view.update(cx, |view, cx| {
        view.select_container(key.clone());
        view.collapse_details(cx);
        view.select_container(key.clone());
    });
    window.render_frame(cx);
    assert_eq!(view.read(cx).selected.as_ref(), Some(&key));
    assert!(view.read(cx).inspector_closing.is_none());
    assert_eq!(
        window.find("inspector").bounds().size.width,
        panel.size.width
    );

    // Reduce motion bypasses the transition and releases subscriptions at once.
    cx.set_reduce_motion(true);
    window.click("close-details", cx);
    assert!(view.read(cx).selected.is_none());
    assert!(view.read(cx).inspector_closing.is_none());
}
