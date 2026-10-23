use super::*;
use gpui_kit::component::dialog::{DialogAction, DialogClose, DialogFooter};
impl Workspace {
    pub(super) fn stack_member_pending(&self, key: &ResourceKey) -> bool {
        self.operations.iter().any(|o| {
            o.command.environment() == key.environment
                && o.command.target() == key.id
                && (o.state.is_active() || o.state == OperationState::UnknownOutcome)
        })
    }
    pub(super) fn stack_action_available(
        &self,
        stack: &Stack,
        action: StackAction,
        scope: Option<&[String]>,
    ) -> bool {
        let commands = scope
            .map(|names| stack.commands_for_services(action, names))
            .unwrap_or_else(|| stack.commands(action));
        commands.is_ok_and(|commands| {
            !commands.is_empty()
                && commands.iter().all(|c| {
                    self.supports(c)
                        && c.resource_key()
                            .is_some_and(|k| !self.stack_member_pending(k))
                })
        })
    }
    pub(super) fn stack_scoped_action(
        &mut self,
        name: &str,
        action: StackAction,
        scope: Option<Vec<String>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(stack) = self
            .snapshot
            .as_ref()
            .map(Stack::from_snapshot)
            .and_then(|s| s.into_iter().find(|s| s.identity() == name))
        else {
            return;
        };
        let commands = match scope
            .as_ref()
            .map(|names| stack.commands_for_services(action, names))
            .unwrap_or_else(|| stack.commands(action))
        {
            Ok(c) => c,
            Err(e) => {
                self.notice = Some(e.message);
                cx.notify();
                return;
            }
        };
        if commands.is_empty() {
            return;
        }
        if commands.len() > 32 {
            self.notice = Some("每批最多 32 项，请分批操作。".into());
            cx.notify();
            return;
        }
        let entity = cx.entity().downgrade();
        let name = stack.name.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let entity = entity.clone();
            let commands = commands.clone();
            let project = stack.name.clone();
            let p = Palette::get(cx.theme().is_dark());
            dialog
                .title(format!(
                    "{}：{}",
                    if stack.kind == StackKind::Swarm && action == StackAction::Restart {
                        "滚动重启"
                    } else {
                        action.label()
                    },
                    name
                ))
                .w(px(480.))
                .child(div().text_size(px(12.)).child(format!(
                    "影响 {} 个{}",
                    commands.len(),
                    if stack.kind == StackKind::Swarm {
                        "服务"
                    } else {
                        "容器"
                    }
                )))
                .child(
                    div()
                        .id("stack-confirm-members")
                        .max_h(px(180.))
                        .overflow_y_scroll()
                        .text_size(px(11.))
                        .text_color(p.muted)
                        .children(
                            commands
                                .iter()
                                .map(|c| div().py(px(3.)).child(c.target().to_owned())),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(p.muted)
                        .child(match action {
                            StackAction::RemoveServices => {
                                "移除服务及其任务，保留网络、卷、配置和秘密。"
                            }
                            StackAction::RemoveStopped => "删除容器可写层，卷、镜像和网络保留。",
                            StackAction::Stop => "服务将中断。启用自动删除的容器可能被删除。",
                            StackAction::Restart if stack.kind == StackKind::Swarm => {
                                "按照服务的更新策略滚动重启任务。"
                            }
                            StackAction::Restart => "服务将短暂中断。",
                            StackAction::Start => "启动现有服务容器，不创建或更新部署。",
                        }),
                )
                .footer(
                    DialogFooter::new()
                        .gap(px(8.))
                        .child(div().w(px(64.)).child(
                            DialogClose::new().trigger(|b| b.regular().w_full().label("取消")),
                        ))
                        .child(
                            div()
                                .w(px(
                                    if matches!(
                                        action,
                                        StackAction::RemoveStopped | StackAction::RemoveServices
                                    ) {
                                        128.
                                    } else {
                                        72.
                                    },
                                ))
                                .child(
                                    DialogAction::new().child(
                                        Button::new("stack-confirm")
                                            .w_full()
                                            .regular()
                                            .when(action == StackAction::Start, |b| b.primary())
                                            .when(action != StackAction::Start, |b| {
                                                b.stop_style(p, cx)
                                            })
                                            .label(
                                                if stack.kind == StackKind::Swarm
                                                    && action == StackAction::Restart
                                                {
                                                    "滚动重启"
                                                } else {
                                                    action.label()
                                                },
                                            ),
                                    ),
                                ),
                        ),
                )
                .on_ok(move |_, _, cx| {
                    let mut accepted = false;
                    let _ = entity.update(cx, |v, cx| {
                        if commands.iter().any(|c| {
                            !v.supports(c)
                                || c.resource_key().is_none_or(|k| {
                                    v.stack_member_pending(k)
                                        || v.snapshot.as_ref().is_none_or(|s| {
                                            if let Command::SwarmService { version, .. } = c {
                                                return !s.swarm.services.iter().any(|x| {
                                                    x.key == *k
                                                        && x.version == *version
                                                        && x.stack == project
                                                });
                                            }
                                            !s.containers.iter().any(|x| {
                                                x.key == *k
                                                    && x.compose
                                                        .as_ref()
                                                        .is_some_and(|m| m.project == project)
                                            })
                                        })
                                })
                        }) {
                            v.notice = Some("Stack 状态已改变，请刷新后重试。".into());
                            cx.notify();
                            return;
                        }
                        for command in &commands {
                            v.submit(command.clone(), cx);
                        }
                        v.stack.checked.clear();
                        accepted = true;
                    });
                    accepted
                })
        });
    }
}
