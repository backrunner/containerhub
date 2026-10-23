use crate::*;
mod row;

use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};

impl Workspace {
    pub(super) fn list(
        &self,
        available_width: f32,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Preserve name and action widths when the inspector is open.
        let show_resources = available_width >= 800.;
        let items = self.visible();
        let total = items.len();
        let items = items[self.pagination.range(total)].to_vec();
        let count = items.len();
        observed!(div().id("list-pane"))
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(32.))
                    .flex_shrink_0()
                    .child(
                        div().w(px(226.)).child(
                            Input::new(&self.search)
                                .small()
                                .min_h(px(32.))
                                .text_size(px(12.))
                                .prefix(Icon::new(Glyph::Search).size(px(14.))),
                        ),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("status-filter")
                            .regular()
                            .label(["全部状态", "运行中", "已停止"][self.filter as usize])
                            .dropdown_caret(true)
                            .dropdown_menu({
                                let entity = cx.entity().downgrade();
                                let selected = self.filter;
                                move |mut menu, _, _| {
                                    for (filter, label) in
                                        [(0, "全部状态"), (1, "运行中"), (2, "已停止")]
                                    {
                                        let entity = entity.clone();
                                        menu = menu.item(
                                            PopupMenuItem::new(label)
                                                .checked(selected == filter)
                                                .on_click(move |_, _, cx| {
                                                    let _ = entity.update(cx, |this, cx| {
                                                        this.filter = filter;
                                                        this.reset_container_page();
                                                        cx.notify();
                                                    });
                                                }),
                                        );
                                    }
                                    menu
                                }
                            }),
                    )
                    .child(
                        Button::new("refresh")
                            .square(32.)
                            .icon(Glyph::RefreshCw)
                            .tooltip("刷新（⌘R）")
                            .accessibility_label("刷新")
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    ),
            )
            .when(!self.batch.is_empty(), |d| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex_1()
                                .text_sm()
                                .child(format!("已选 {} 项（最多 32 项）", self.batch.len())),
                        )
                        .child(
                            Button::new("batch-start")
                                .dense()
                                .label("启动")
                                .disabled(
                                    !self.writable() || self.batch.iter().any(|key| self.busy(key)),
                                )
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.run_batch(LifecycleAction::Start, window, cx)
                                })),
                        )
                        .child(
                            Button::new("batch-stop")
                                .dense()
                                .stop_style(p, cx)
                                .label("停止")
                                .disabled(
                                    !self.writable() || self.batch.iter().any(|key| self.busy(key)),
                                )
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.run_batch(LifecycleAction::Stop, window, cx)
                                })),
                        )
                        .child(
                            Button::new("batch-clear")
                                .dense()
                                .ghost()
                                .label("清除")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.batch.clear();
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .rounded(px(7.))
                    .border_1()
                    .border_color(p.border)
                    .bg(p.surface)
                    .overflow_hidden()
                    .child(
                        observed!(div().id("container-header"))
                            .flex()
                            .gap(px(8.))
                            .h(px(36.))
                            .flex_shrink_0()
                            .bg(p.canvas)
                            .items_center()
                            .px(px(12.))
                            .border_b_1()
                            .border_color(p.border)
                            .text_size(px(11.))
                            .text_color(p.muted)
                            .child(self.select_all_control(&items, p, cx))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .flex_1()
                                    .min_w_0()
                                    .child(div().flex_1().child("容器"))
                                    .child(
                                        Button::new("container-sort")
                                            .ghost()
                                            .square(24.)
                                            .icon(Glyph::ArrowUpDown)
                                            .accessibility_label("切换排序字段")
                                            .tooltip(format!(
                                                "按{}排序，点击切换",
                                                self.settings.sort.label()
                                            ))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.settings.sort = this.settings.sort.next();
                                                this.reset_container_page();
                                                this.send(
                                                    Request::SaveSettings(this.settings.clone()),
                                                    cx,
                                                );
                                            })),
                                    )
                                    .child(
                                        Button::new("container-sort-direction")
                                            .ghost()
                                            .square(24.)
                                            .ml(px(4.))
                                            .icon(if self.settings.sort_descending {
                                                Glyph::ArrowDown
                                            } else {
                                                Glyph::ArrowUp
                                            })
                                            .tooltip("切换排序方向")
                                            .accessibility_label("切换排序方向")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.settings.sort_descending =
                                                    !this.settings.sort_descending;
                                                this.reset_container_page();
                                                this.send(
                                                    Request::SaveSettings(this.settings.clone()),
                                                    cx,
                                                );
                                            })),
                                    ),
                            )
                            .child(
                                observed!(div().id("container-status-header"))
                                    .w(px(68.))
                                    .flex_shrink_0()
                                    .child("状态"),
                            )
                            .when(show_resources, |d| {
                                d.child(div().w(px(54.)).flex_shrink_0().text_right().child("CPU"))
                                    .child(
                                        div().w(px(76.)).flex_shrink_0().text_right().child("内存"),
                                    )
                            })
                            .child(div().w(px(96.)).pl(px(8.)).flex_shrink_0().child("端口"))
                            .child(
                                div()
                                    .w(px(controls::ROW_ACTION_WIDTH))
                                    .flex_shrink_0()
                                    .text_right()
                                    .child("操作"),
                            ),
                    )
                    .when(total == 0, |d| {
                        d.child(empty(
                            if self.snapshot.is_none() && self.error.is_none() {
                                "正在连接运行时…"
                            } else if self.query.is_empty() && self.filter == 0 {
                                "没有可显示的容器"
                            } else {
                                "没有符合条件的容器"
                            },
                            p,
                        ))
                    })
                    .when(total > 0, |d| {
                        d.child(
                            uniform_list(
                                "container-list",
                                count,
                                cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                                    range
                                        .map(|index| {
                                            this.container_row(
                                                &items[index],
                                                index,
                                                show_resources,
                                                p,
                                                cx,
                                            )
                                        })
                                        .collect()
                                }),
                            )
                            .track_scroll(&self.container_scroll)
                            .flex_1(),
                        )
                    }),
            )
            .child(self.pagination_bar(total, p, cx))
    }
}
