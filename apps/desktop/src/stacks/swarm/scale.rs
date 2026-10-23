use super::*;
use gpui_kit::component::dialog::{DialogAction, DialogClose, DialogFooter};
struct ScaleForm {
    input: Entity<InputState>,
    error: String,
    running: usize,
    target: u64,
    _subscription: Subscription,
}
impl Render for ScaleForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::get(cx.theme().is_dark());
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .text_size(px(12.))
            .child(
                div()
                    .flex()
                    .gap(px(24.))
                    .text_size(px(11.))
                    .text_color(p.muted)
                    .child(format!("运行中  {}", self.running))
                    .child(format!("当前目标  {}", self.target)),
            )
            .child("目标副本数")
            .child(
                Input::new(&self.input)
                    .id("swarm-replicas")
                    .small()
                    .min_h(px(32.)),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(p.muted)
                    .child("设为 0 将停止全部副本。任务由 Swarm 调度。"),
            )
            .when(!self.error.is_empty(), |d| {
                d.child(
                    div()
                        .text_size(px(11.))
                        .text_color(p.danger)
                        .child(self.error.clone()),
                )
            })
    }
}
impl Workspace {
    pub(in crate::stacks) fn swarm_scale(
        &mut self,
        service: SwarmService,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let form = cx.new(|cx| {
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(service.desired_replicas.unwrap_or(0).to_string())
            });
            let subscription =
                cx.subscribe_in(&input, window, |f: &mut ScaleForm, _, event, w, cx| {
                    if matches!(event, InputEvent::Change) {
                        f.error.clear();
                        w.refresh();
                        cx.notify();
                    }
                });
            ScaleForm {
                input,
                error: String::new(),
                running: service.running(),
                target: service.desired_replicas.unwrap_or(0),
                _subscription: subscription,
            }
        });
        let owner = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let form = form.clone();
            let owner = owner.clone();
            let service = service.clone();
            let p = Palette::get(cx.theme().is_dark());
            let replicas = form
                .read(cx)
                .input
                .read(cx)
                .value()
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|n| *n <= 10000);
            let zero = replicas == Some(0);
            dialog
                .title(format!("调整副本数：{}", service.key.id))
                .w(px(440.))
                .child(form.clone())
                .footer(
                    DialogFooter::new()
                        .gap(px(8.))
                        .child(div().w(px(64.)).child(
                            DialogClose::new().trigger(|b| b.regular().w_full().label("取消")),
                        ))
                        .child(
                            div().w(px(112.)).child(
                                DialogAction::new().child(
                                    Button::new("ok")
                                        .regular()
                                        .w_full()
                                        .when(zero, |b| b.stop_style(p, cx))
                                        .when(!zero, |b| b.primary())
                                        .label(if zero {
                                            "停止全部副本"
                                        } else {
                                            "更新副本数"
                                        })
                                        .disabled(
                                            replicas == service.desired_replicas.map(|n| n as u32),
                                        ),
                                ),
                            ),
                        ),
                )
                .on_ok(move |_, _, cx| {
                    let replicas = form
                        .read(cx)
                        .input
                        .read(cx)
                        .value()
                        .trim()
                        .parse::<u32>()
                        .ok()
                        .filter(|n| *n <= 10000);
                    let Some(replicas) = replicas else {
                        form.update(cx, |f, cx| {
                            f.error = "请输入 0–10000 的整数".into();
                            cx.notify();
                        });
                        return false;
                    };
                    let mut accepted = false;
                    let _ = owner.update(cx, |v, cx| {
                        let Ok(command) = service.command(SwarmAction::Scale { replicas }) else {
                            return;
                        };
                        if !v.supports(&command)
                            || v.stack_member_pending(&service.key)
                            || v.snapshot.as_ref().is_none_or(|s| {
                                !s.swarm
                                    .services
                                    .iter()
                                    .any(|x| x.key == service.key && x.version == service.version)
                            })
                        {
                            return;
                        }
                        v.submit(command, cx);
                        accepted = true;
                    });
                    if !accepted {
                        form.update(cx, |f, cx| {
                            f.error = "服务状态已改变，请关闭后刷新".into();
                            cx.notify();
                        });
                    }
                    accepted
                })
        });
    }
}
