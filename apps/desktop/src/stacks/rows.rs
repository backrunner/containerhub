use super::*;
use gpui_kit::component::menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem};
impl Workspace {
    pub(super) fn stack_service_row(
        &self,
        stack: &Stack,
        service: &StackService,
        i: usize,
        layout: Layout,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let token = StackView::token(stack, service);
        let expanded = self.stack.expanded.contains(&token);
        let checked = self.stack.checked.contains(&token);
        let toggle = token.clone();
        let expand = token.clone();
        let identity = stack.identity();
        let scope = vec![service.name.clone()];
        let primary = if service.running_count() > 0 {
            StackAction::Stop
        } else {
            StackAction::Start
        };
        let restart_allowed =
            self.stack_action_available(stack, StackAction::Restart, Some(&scope));
        let swarm = service.swarm.clone();
        let scalable = swarm.as_ref().is_some_and(|s| {
            s.mode == SwarmMode::Replicated
                && s.command(SwarmAction::Scale {
                    replicas: s.desired_replicas.unwrap_or(0).min(10000) as u32,
                })
                .is_ok_and(|c| self.supports(&c))
                && !self.stack_member_pending(&s.key)
        });
        let owner = cx.entity().downgrade();
        let menu_identity = identity.clone();
        let menu_scope = scope.clone();
        let menu_service = service.clone();
        let menu_actions: Vec<_> = if swarm.is_some() {
            vec![StackAction::Restart, StackAction::RemoveServices]
        } else {
            vec![
                StackAction::Start,
                StackAction::Stop,
                StackAction::Restart,
                StackAction::RemoveStopped,
            ]
        }
        .into_iter()
        .map(|a| (a, self.stack_action_available(stack, a, Some(&scope))))
        .collect();
        let menu = move |mut menu: PopupMenu, _: &mut Window, _: &mut Context<PopupMenu>| {
            for (action, allowed) in &menu_actions {
                let owner = owner.clone();
                let identity = menu_identity.clone();
                let scope = menu_scope.clone();
                let action = *action;
                menu = menu.item(
                    PopupMenuItem::new(format!(
                        "{}…",
                        if menu_service.swarm.is_some() && action == StackAction::Restart {
                            "滚动重启"
                        } else {
                            action.label()
                        }
                    ))
                    .disabled(!allowed)
                    .on_click(move |_, w, cx| {
                        let _ = owner.update(cx, |v, cx| {
                            v.stack_scoped_action(&identity, action, Some(scope.clone()), w, cx)
                        });
                    }),
                );
            }
            let name = menu_service.name.clone();
            let image = menu_service.image().to_string();
            menu.separator()
                .item(
                    PopupMenuItem::new("复制服务名称").on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(name.clone()))
                    }),
                )
                .item(
                    PopupMenuItem::new("复制镜像名称").on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(image.clone()))
                    }),
                )
        };
        let ports = service
            .swarm
            .as_ref()
            .map(|s| s.ports.clone())
            .unwrap_or_else(|| {
                service
                    .containers
                    .iter()
                    .flat_map(|c| {
                        c.ports.iter().map(|p| {
                            format!("{} → {}/{}", p.host_port, p.container_port, p.protocol)
                        })
                    })
                    .collect()
            });
        let count = if let Some(s) = &service.swarm {
            let running = format!(
                "{}{}",
                if s.tasks_truncated { "≥" } else { "" },
                s.running()
            );
            s.desired_replicas
                .map(|n| format!("{running} / {n} 副本"))
                .unwrap_or_else(|| format!("{}（{running} 副本）", s.mode.label()))
        } else {
            format!(
                "{} / {} 容器",
                service.running_count(),
                service.containers.len()
            )
        };
        div()
            .id(("stack-service", i))
            .border_b_1()
            .border_color(p.border)
            .child(
                observed!(div().id(("stack-service-row", i)))
                    .h(px(68.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(12.))
                    .when(checked, |d| d.bg(p.selected))
                    .child(
                        Button::new(("stack-service-check", i))
                            .ghost()
                            .square(24.)
                            .icon(if checked { Glyph::Check } else { Glyph::Square })
                            .toggled(checked)
                            .text_color(if checked { p.accent } else { p.muted })
                            .accessibility_label(format!("选择服务 {}", service.name))
                            .on_click(cx.listener(move |v, _, _, cx| {
                                if !v.stack.checked.remove(&toggle) {
                                    v.stack.checked.insert(toggle.clone());
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new(("stack-expand", i))
                            .ghost()
                            .square(20.)
                            .icon(if expanded {
                                Glyph::ChevronDown
                            } else {
                                Glyph::ChevronRight
                            })
                            .tooltip(if expanded {
                                "收起实例"
                            } else {
                                "展开实例"
                            })
                            .on_click(cx.listener(move |v, _, _, cx| {
                                if !v.stack.expanded.remove(&expand) {
                                    v.stack.expanded.insert(expand.clone());
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new(("stack-service-name", i))
                            .ghost()
                            .h_auto()
                            .p(px(0.))
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .w_full()
                                    .text_left()
                                    .text_size(px(12.))
                                    .child(
                                        div()
                                            .truncate()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(service.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .mt(px(5.))
                                            .text_size(px(10.))
                                            .text_color(p.muted)
                                            .truncate()
                                            .child(service.image().to_string()),
                                    ),
                            )
                            .on_click(cx.listener(move |v, _, _, cx| {
                                if !v.stack.expanded.remove(&token) {
                                    v.stack.expanded.insert(token.clone());
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .w(px(if layout.compact { 82. } else { 104. }))
                            .flex_shrink_0()
                            .child(status_label(service.status(), p))
                            .child(
                                div()
                                    .mt(px(5.))
                                    .text_size(px(10.))
                                    .text_color(p.muted)
                                    .truncate()
                                    .child(count),
                            ),
                    )
                    .when(layout.show_resources, |d| {
                        d.child(
                            div()
                                .w(px(134.))
                                .flex_shrink_0()
                                .text_size(px(10.))
                                .text_color(p.muted)
                                .truncate()
                                .child(ports.first().cloned().unwrap_or_else(|| "—".into()))
                                .when(ports.len() > 1, |d| {
                                    d.child(format!(" +{}", ports.len() - 1))
                                }),
                        )
                    })
                    .child(
                        observed!(div().id(("stack-service-actions", i)))
                            .w(px(112.))
                            .flex_shrink_0()
                            .flex()
                            .gap(px(8.))
                            .child(if let Some(s) = swarm {
                                Button::new(("swarm-scale", i))
                                    .square(32.)
                                    .icon(Glyph::SlidersHorizontal)
                                    .tooltip(if s.mode == SwarmMode::Replicated {
                                        "调整副本数"
                                    } else {
                                        "此模式由 Swarm 调度"
                                    })
                                    .accessibility_label("调整副本数")
                                    .disabled(!scalable)
                                    .on_click(cx.listener(move |v, _, w, cx| {
                                        v.swarm_scale(s.clone(), w, cx)
                                    }))
                            } else {
                                let identity = identity.clone();
                                let scope = scope.clone();
                                Button::new(("stack-service-lifecycle", i))
                                    .square(32.)
                                    .icon(if primary == StackAction::Stop {
                                        Glyph::Square
                                    } else {
                                        Glyph::Play
                                    })
                                    .tooltip(primary.label())
                                    .accessibility_label(primary.label())
                                    .when(primary == StackAction::Stop, |b| b.stop_style(p, cx))
                                    .disabled(!self.stack_action_available(
                                        stack,
                                        primary,
                                        Some(&scope),
                                    ))
                                    .on_click(cx.listener(move |v, _, w, cx| {
                                        v.stack_scoped_action(
                                            &identity,
                                            primary,
                                            Some(scope.clone()),
                                            w,
                                            cx,
                                        )
                                    }))
                            })
                            .child(
                                Button::new(("stack-service-restart", i))
                                    .square(32.)
                                    .icon(Glyph::RotateCw)
                                    .tooltip(if service.swarm.is_some() {
                                        "滚动重启此服务"
                                    } else {
                                        "重启此服务"
                                    })
                                    .disabled(!restart_allowed)
                                    .on_click(cx.listener(move |v, _, w, cx| {
                                        v.stack_scoped_action(
                                            &identity,
                                            StackAction::Restart,
                                            Some(scope.clone()),
                                            w,
                                            cx,
                                        )
                                    })),
                            )
                            .child(
                                Button::new(("stack-service-more", i))
                                    .square(32.)
                                    .icon(Glyph::Ellipsis)
                                    .tooltip("服务操作")
                                    .dropdown_menu(menu.clone()),
                            ),
                    )
                    .context_menu(menu),
            )
            .when(expanded, |d| {
                d.child(self.container_overview(stack, service, i, layout, p, cx))
            })
    }
}
