use crate::*;
use gpui_kit::component::dialog::{DialogAction, DialogClose, DialogFooter};

impl Workspace {
    fn runtime_operation(&self, environment: &str) -> Option<&Operation> {
        self.operations.iter().rev().find(|op| {
            matches!(&op.command, Command::StartRuntime { environment: e } if e == environment)
                && (op.state.is_active() || op.state == OperationState::UnknownOutcome)
        })
    }
    fn can_start_runtime(&self, report: &RuntimeDiagnostics) -> bool {
        self.remote_label.is_none()
            && !self.diagnostics_loading
            && report.supports(RuntimeFeature::StartRuntime)
            && report.service_error.as_ref().is_some_and(|e| {
                !matches!(
                    e.kind,
                    ErrorKind::PermissionDenied | ErrorKind::InvalidData | ErrorKind::NotInstalled
                )
            })
            && self.runtime_operation(&report.environment).is_none()
    }
    pub(super) fn runtime_service(
        &self,
        report: &RuntimeDiagnostics,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let operation = self.runtime_operation(&report.environment);
        let active = operation.is_some_and(|op| op.state.is_active());
        let uncertain = operation
            .filter(|op| op.state == OperationState::UnknownOutcome)
            .map(|op| op.id);
        let report = report.clone();
        observed!(div().id("runtime-service"))
            .flex()
            .items_start()
            .gap(px(12.))
            .p(px(12.))
            .border_1()
            .border_color(p.border)
            .rounded(px(7.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(12.))
                            .child(div().size(px(6.)).rounded_full().bg(
                                if report.service_error.is_none() {
                                    p.success
                                } else {
                                    p.warning
                                },
                            ))
                            .child(if active {
                                "正在启动服务"
                            } else if uncertain.is_some() {
                                "启动结果待核对"
                            } else if report.service_error.is_some() {
                                "服务未连接"
                            } else {
                                "服务可连接"
                            }),
                    )
                    .when_some(report.service_error.as_ref(), |d, error| {
                        d.child(
                            div()
                                .mt(px(6.))
                                .text_size(px(11.))
                                .text_color(p.muted)
                                .child(error.message.clone()),
                        )
                    }),
            )
            .when(
                self.remote_label.is_none()
                    && (report.service_error.is_some() || operation.is_some()),
                |d| {
                    if let Some(id) = uncertain {
                        d.child(
                            Button::new("runtime-reconcile")
                                .dense()
                                .icon(Glyph::RefreshCw)
                                .label("核对结果")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.send(Request::Reconcile(id), cx)
                                })),
                        )
                    } else {
                        let enabled = self.can_start_runtime(&report);
                        d.child(
                            Button::new("runtime-start")
                                .dense()
                                .primary()
                                .icon(Glyph::Play)
                                .label(if active {
                                    "启动中…"
                                } else {
                                    "启动服务"
                                })
                                .disabled(!enabled)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.start_runtime_form(&report, window, cx)
                                })),
                        )
                    }
                },
            )
    }
    fn start_runtime_form(
        &self,
        report: &RuntimeDiagnostics,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_start_runtime(report) {
            return;
        }
        let expected = report.clone();
        let entity = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let entity = entity.clone();
            let expected = expected.clone();
            let p = Palette::get(cx.theme().is_dark());
            dialog.title("启动运行时服务")
                .width(px(540.)).p(px(24.))
                .child(div().text_size(px(12.)).text_color(p.muted)
                    .child("启动当前环境的后台服务，完成后自动检查连接。此次不会主动启动已有容器。"))
                .child(div().mt(px(12.)).text_size(px(12.)).text_color(p.muted)
                    .child("首次安装所需的内核下载和 macOS 权限，请在终端完成。此次启动不会自动安装内核或提升权限。"))
                .footer(DialogFooter::new().gap(px(8.)).pt(px(16.)).border_t_1().border_color(p.border)
                    .child(div().w(px(64.)).h(px(32.)).child(DialogClose::new().trigger(|button| button.regular().w_full().label("取消"))))
                    .child(div().w(px(84.)).h(px(32.)).child(DialogAction::new().child(Button::new("ok").regular().w_full().primary().label("启动服务")))))
                .on_ok(move |_, _, cx| {
                    let _ = entity.update(cx, |this, cx| {
                        let current = this.diagnostics.as_ref().and_then(|r| r.as_ref().ok());
                        if current.is_some_and(|r| r.environment == expected.environment
                            && r.cli_path == expected.cli_path && r.cli_version == expected.cli_version
                            && this.can_start_runtime(r)) {
                            this.submit(Command::StartRuntime { environment: expected.environment.clone() }, cx);
                        } else {
                            this.notice = Some("运行时状态已改变，请重新检查后再启动。".into());
                            cx.notify();
                        }
                    });
                    true
                })
        });
    }
}
