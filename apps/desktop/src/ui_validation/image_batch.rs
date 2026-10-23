use super::*;

pub(super) struct Fixture<'a> {
    pub rows: &'a [Resource],
    pub used: &'a ImageDetails,
    pub unused: &'a ImageDetails,
}
pub(super) fn run(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    view: &Entity<Workspace>,
    requests: &async_channel::Receiver<Request>,
    writes: &async_channel::Receiver<containerhub_management::queue::OperationRequest>,
    fixture: Fixture<'_>,
) {
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.operations.clear();
            view.notice = None;
            cx.notify();
        });
        window.scroll(
            "resource-scroll",
            ScrollDelta::Pixels(point(px(0.), px(2000.))),
            cx,
        );
        window.click("select-all-images", cx);
        assert_eq!(view.read(cx).image.selected.len(), fixture.rows.len());
        window.click(("select-image", 0usize), cx);
        window.find_by_label("部分镜像已选，全选镜像");
        window.click("select-all-images", cx);
        window.click("copy-image-selection", cx);
        let mut expected: Vec<_> = fixture.rows.iter().map(|r| r.name.clone()).collect();
        expected.sort();
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            expected.join("\n")
        );
        window.click("select-all-images", cx);
        assert!(view.read(cx).image.selected.is_empty());
        window.click(("select-image", 0usize), cx);
        window.scroll(
            "resource-scroll",
            ScrollDelta::Pixels(point(px(0.), px(-2000.))),
            cx,
        );
        window.click(("select-image", fixture.rows.len() - 1), cx);
        for control in [
            "copy-image-selection",
            "delete-image-selection",
            "clear-image-selection",
        ] {
            let bounds = window.find(control).bounds();
            assert!(bounds.right() <= window.viewport_size().width && bounds.top() >= px(0.));
        }
        view.update(cx, |view, cx| {
            view.error = Some("offline".into());
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(window.find("delete-image-selection").disabled(), Some(true));
        assert_ne!(window.find("copy-image-selection").disabled(), Some(true));
        view.update(cx, |view, cx| {
            view.error = None;
            cx.notify();
        });
    })
    .unwrap();
    for confirm in [false, true] {
        cx.update_window(handle, |_, window, cx| {
            while requests.try_recv().is_ok() {}
            window.click("delete-image-selection", cx);
            let Request::InspectImageBatch {
                environment,
                references,
                reply,
            } = requests.try_recv().unwrap()
            else {
                panic!("expected batch inspect")
            };
            assert_eq!(environment, "demo");
            assert_eq!(references.len(), 2);
            assert!(writes.try_recv().is_err());
            reply
                .send(vec![
                    (fixture.used.key.reference.clone(), Ok(fixture.used.clone())),
                    (
                        fixture.unused.key.reference.clone(),
                        Ok(fixture.unused.clone()),
                    ),
                ])
                .unwrap();
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("image-batch-review-scroll").is_some());
            assert!(writes.try_recv().is_err(), "review alone must never delete");
            if confirm {
                window.click("ok", cx);
                assert_eq!(
                    submitted(writes.try_recv().unwrap()).command,
                    Command::DeleteImage {
                        key: fixture.unused.key.clone()
                    }
                );
                assert!(writes.try_recv().is_err(), "used image must be skipped");
                assert_eq!(
                    view.read(cx).image.selected.len(),
                    1,
                    "skipped selection is retained"
                );
            } else {
                window.press("escape", cx);
                assert!(writes.try_recv().is_err());
            }
        })
        .unwrap();
        cx.run_until_parked();
    }
    cx.update_window(handle, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.notice = None;
            cx.notify();
        });
        window.click("delete-image-selection", cx);
        let Request::InspectImageBatch { reply, .. } = requests.try_recv().unwrap() else {
            panic!("expected batch inspect")
        };
        window.click(("nav", 0usize), cx);
        reply
            .send(vec![(
                fixture.used.key.reference.clone(),
                Ok(fixture.used.clone()),
            )])
            .unwrap();
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.try_find("image-batch-review-scroll").is_none(),
            "stale batch result after navigation must be discarded"
        );
        view.update(cx, |view, cx| {
            view.image.selected.clear();
            view.operations.clear();
            cx.notify();
        });
    })
    .unwrap();
}
