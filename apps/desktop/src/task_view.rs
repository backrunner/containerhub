use crate::*;

impl Workspace {
    pub(super) fn tasks(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("tasks-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .rounded_lg()
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .when(self.operations.is_empty(), |d| {
                d.child(empty("还没有应用操作。启停结果会在这里记录。", p))
            })
            .children(self.operations.iter().rev().map(|o| {
                let retry = self.retry_command(o);
                let id = o.id;
                div()
                    .p(px(18.))
                    .pl(px(54.))
                    .relative()
                    .text_size(px(12.))
                    .child(
                        div()
                            .absolute()
                            .top(px(18.))
                            .left(px(18.))
                            .size(px(24.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(p.canvas)
                            .child(
                                Icon::new(if o.state == OperationState::Succeeded {
                                    Glyph::Check
                                } else if o.state.is_active() {
                                    Glyph::Loader
                                } else {
                                    Glyph::TriangleAlert
                                })
                                .size(px(15.))
                                .text_color(
                                    if o.state == OperationState::Succeeded {
                                        p.success
                                    } else if o.state.is_active() {
                                        p.accent
                                    } else {
                                        p.warning
                                    },
                                ),
                            ),
                    )
                    .border_b_1()
                    .border_color(p.border)
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .items_center()
                            .justify_between()
                            .child(format!("{}  {}", o.command.label(), o.command.target()))
                            .when(
                                o.state.is_active()
                                    && o.command.cancellable()
                                    && self
                                        .snapshot
                                        .as_ref()
                                        .is_some_and(|s| s.capabilities.cancel_pull),
                                |d| {
                                    d.child(
                                        Button::new(("cancel-operation", id as usize))
                                            .ghost()
                                            .dense()
                                            .label(if o.cancel_requested {
                                                "取消请求中"
                                            } else {
                                                "取消拉取"
                                            })
                                            .disabled(o.cancel_requested)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.send(Request::Cancel(id), cx)
                                            })),
                                    )
                                },
                            )
                            .when_some(retry, |d, command| {
                                d.child(
                                    Button::new(("retry-operation", o.id as usize))
                                        .ghost()
                                        .dense()
                                        .label(if o.command.starts_after_create() {
                                            "启动已创建容器"
                                        } else if matches!(
                                            o.command,
                                            Command::Lifecycle {
                                                action: LifecycleAction::Restart,
                                                ..
                                            }
                                        ) {
                                            "重试启动"
                                        } else {
                                            "重试"
                                        })
                                        .disabled(
                                            !self.supports(&command)
                                                || self.operations.iter().any(|op| {
                                                    op.state.is_active()
                                                        && op.command.lock_key()
                                                            == command.lock_key()
                                                }),
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.submit(command.clone(), cx)
                                        })),
                                )
                            })
                            .when(o.state == OperationState::UnknownOutcome, |d| {
                                d.child(
                                    Button::new(("reconcile-operation", o.id as usize))
                                        .ghost()
                                        .dense()
                                        .label("核对原任务")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.send(Request::Reconcile(id), cx)
                                        })),
                                )
                            })
                            .child(
                                div()
                                    .text_color(if o.state == OperationState::Succeeded {
                                        p.success
                                    } else if o.state.is_active() {
                                        p.accent
                                    } else if o.state == OperationState::Failed {
                                        p.danger
                                    } else {
                                        p.warning
                                    })
                                    .child(o.status_label()),
                            ),
                    )
                    .when_some(o.progress, |d, progress| {
                        d.child(
                            div()
                                .mt_2()
                                .text_size(px(11.))
                                .text_color(p.accent)
                                .child(progress.label()),
                        )
                    })
                    .child(
                        div()
                            .mt_2()
                            .text_size(px(11.))
                            .text_color(p.muted)
                            .child(o.message.clone().unwrap_or("等待运行时确认…".into())),
                    )
                    .child(
                        div()
                            .mt_2()
                            .text_size(px(10.))
                            .text_color(p.muted)
                            .child(format!(
                                "任务 {} · {} 秒",
                                o.id,
                                o.finished_at
                                    .unwrap_or_else(now)
                                    .saturating_sub(o.started_at)
                            )),
                    )
                    .when_some(o.receipt.as_ref(), |d, receipt| {
                        let ExecutionReceipt::Remote { request_id, .. } = receipt;
                        d.child(
                            div()
                                .mt_1()
                                .text_size(px(10.))
                                .text_color(p.muted)
                                .child(format!("Agent 请求 {request_id}")),
                        )
                    })
            }))
    }
}
