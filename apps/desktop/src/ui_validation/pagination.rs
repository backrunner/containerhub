use super::*;

pub(super) fn check(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    view: &Entity<Workspace>,
) {
    cx.update_window(handle, |_, window, cx| {
        window.click("container-page-size", cx);
        let option = window
            .find_by_label("5 条 / 页")
            .path()
            .last()
            .unwrap()
            .clone();
        window.click(option, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        assert_eq!(view.read(cx).pagination.size, 5);
        assert_eq!(window.find("container-page-prev").disabled(), Some(true));
        let selected = view.read(cx).selected.clone();
        let bottom = window.find("container-pagination").bounds().bottom();
        window.click("select-all-containers", cx);
        assert_eq!(view.read(cx).batch.len(), 5);
        window.click("container-page-next", cx);
        assert_eq!(view.read(cx).pagination.range(8), 5..8);
        assert_eq!(view.read(cx).batch.len(), 5);
        assert_eq!(view.read(cx).selected, selected);
        assert_eq!(window.find("container-page-next").disabled(), Some(true));
        assert!(window.try_find(("container", 3usize)).is_none());
        assert_eq!(
            window.find("container-pagination").bounds().bottom(),
            bottom
        );
        if window.viewport_size().width >= px(1280.) {
            let left = window.find("list-pane").bounds();
            let right = window.find("inspector").bounds();
            assert_eq!(left.top(), right.top());
            assert_eq!(left.bottom(), right.bottom());
            assert_eq!(bottom, right.bottom());
        }
        window.click("select-all-containers", cx);
        assert_eq!(view.read(cx).batch.len(), 8);
        window.click("select-all-containers", cx);
        assert_eq!(view.read(cx).batch.len(), 5);
        window.click("container-page-prev", cx);
        assert_eq!(view.read(cx).pagination.page, 0);
        window.click("select-all-containers", cx);
        assert!(view.read(cx).batch.is_empty());
        window.click("container-page-next", cx);
        window.click("status-filter", cx);
        let option = window
            .find_by_label("已停止")
            .path()
            .last()
            .unwrap()
            .clone();
        window.click(option, cx);
        assert_eq!(view.read(cx).pagination.page, 0);
        assert_eq!(view.read(cx).visible().len(), 3);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.click("status-filter", cx);
        let option = window
            .find_by_label("全部状态")
            .path()
            .last()
            .unwrap()
            .clone();
        window.click(option, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.click("container-page-size", cx);
        let option = window
            .find_by_label("10 条 / 页")
            .path()
            .last()
            .unwrap()
            .clone();
        window.click(option, cx);
    })
    .unwrap();
    cx.run_until_parked();
}
