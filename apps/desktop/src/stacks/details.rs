use super::*;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
impl Workspace {
    pub(super) fn stack_detail(
        &self,
        stack: &Stack,
        layout: Layout,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let visible: Vec<_> = stack
            .services
            .iter()
            .enumerate()
            .filter(|(_, s)| self.stack.service_visible(stack, s))
            .collect();
        let checked = self.stack.checked_names(stack);
        let scope = if checked.is_empty() {
            None
        } else {
            Some(checked.clone())
        };
        let attention = stack
            .services
            .iter()
            .filter(|s| s.status().needs_attention())
            .count();
        let running: usize = stack.services.iter().map(|s| s.running_count()).sum();
        let incomplete = stack
            .services
            .iter()
            .any(|s| s.swarm.as_ref().is_some_and(|s| s.tasks_truncated));
        let keys: Vec<_> = visible
            .iter()
            .map(|(_, s)| StackView::token(stack, s))
            .collect();
        let selected_count = keys
            .iter()
            .filter(|k| self.stack.checked.contains(*k))
            .count();
        let all = !keys.is_empty() && selected_count == keys.len();
        observed!(div().id("stack-detail"))
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .border_1()
            .border_color(p.border)
            .rounded(px(7.))
            .overflow_hidden()
            .child(
                div()
                    .px(px(16.))
                    .py(px(14.))
                    .flex_shrink_0()
                    .flex()
                    .gap(px(10.))
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(17.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(stack.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .px(px(6.))
                                    .py(px(2.))
                                    .border_1()
                                    .border_color(p.border)
                                    .rounded(px(4.))
                                    .text_color(p.muted)
                                    .child(stack.kind.label()),
                            ),
                    )
                    .child(self.stack_toolbar(stack, scope.clone(), p, cx)),
            )
            .child(
                div()
                    .flex()
                    .px(px(16.))
                    .pb(px(14.))
                    .gap(px(16.))
                    .border_b_1()
                    .border_color(p.border)
                    .children(
                        [
                            ("服务", stack.services.len().to_string()),
                            (
                                if stack.kind == StackKind::Swarm {
                                    "运行副本"
                                } else {
                                    "运行容器"
                                },
                                format!("{}{running}", if incomplete { "≥" } else { "" }),
                            ),
                            ("需关注", attention.to_string()),
                        ]
                        .into_iter()
                        .enumerate()
                        .map(|(i, (label, value))| {
                            div()
                                .flex_1()
                                .when(i > 0, |d| d.pl(px(16.)).border_l_1().border_color(p.border))
                                .child(div().text_size(px(10.)).text_color(p.muted).child(label))
                                .child(
                                    div()
                                        .mt(px(4.))
                                        .text_size(px(20.))
                                        .line_height(px(24.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .when(i == 2 && attention > 0, |d| d.text_color(p.warning))
                                        .child(value),
                                )
                        }),
                    ),
            )
            .child(
                div()
                    .h(px(42.))
                    .flex_shrink_0()
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(if checked.is_empty() {
                                format!("{} 个服务", visible.len())
                            } else {
                                format!("已选 {} 个服务", checked.len())
                            }),
                    )
                    .when(!checked.is_empty(), |d| {
                        d.child(
                            Button::new("stack-clear")
                                .ghost()
                                .dense()
                                .label("清除")
                                .on_click(cx.listener(|v, _, _, cx| {
                                    v.stack.checked.clear();
                                    cx.notify();
                                })),
                        )
                    })
                    .child(div().flex_1())
                    .child(
                        Button::new("stack-attention")
                            .ghost()
                            .dense()
                            .label("仅需关注")
                            .toggled(self.stack.attention_only)
                            .when(self.stack.attention_only, |b| {
                                b.bg(p.accent_surface).text_color(p.accent)
                            })
                            .on_click(cx.listener(|v, _, _, cx| {
                                v.stack.attention_only = !v.stack.attention_only;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .h(px(32.))
                    .flex_shrink_0()
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .bg(p.canvas)
                    .border_y_1()
                    .border_color(p.border)
                    .text_size(px(10.))
                    .text_color(p.muted)
                    .child(
                        Button::new("stack-select-all")
                            .ghost()
                            .square(24.)
                            .icon(if all {
                                Glyph::Check
                            } else if selected_count > 0 {
                                Glyph::Minus
                            } else {
                                Glyph::Square
                            })
                            .text_color(if selected_count > 0 {
                                p.accent
                            } else {
                                p.muted
                            })
                            .disabled(keys.is_empty())
                            .toggled(all)
                            .accessibility_label("选择当前筛选的服务")
                            .tooltip("选择当前筛选的服务")
                            .on_click(cx.listener(move |v, _, _, cx| {
                                for key in &keys {
                                    if all {
                                        v.stack.checked.remove(key);
                                    } else {
                                        v.stack.checked.insert(key.clone());
                                    }
                                }
                                cx.notify();
                            })),
                    )
                    .child(div().w(px(20.)))
                    .child(div().flex_1().min_w_0().child("服务 / 镜像"))
                    .child(
                        div()
                            .w(px(if layout.compact { 82. } else { 104. }))
                            .flex_shrink_0()
                            .child("状态 / 实例"),
                    )
                    .when(layout.show_resources, |d| {
                        d.child(div().w(px(134.)).flex_shrink_0().child("端口"))
                    })
                    .child(div().w(px(112.)).flex_shrink_0().text_right().child("操作")),
            )
            .child(
                observed!(div().id("stack-services-scroll"))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(visible.iter().map(|(i, s)| {
                        self.stack_service_row(stack, s, *i, layout, p, cx)
                            .into_any_element()
                    }))
                    .when(visible.is_empty(), |d| {
                        d.child(
                            div()
                                .p(px(32.))
                                .text_center()
                                .text_size(px(12.))
                                .text_color(p.muted)
                                .child(if self.stack.attention_only {
                                    "没有需要关注的服务"
                                } else {
                                    "没有匹配的服务"
                                }),
                        )
                    }),
            )
    }
    pub(super) fn stack_toolbar(
        &self,
        stack: &Stack,
        scope: Option<Vec<String>>,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let swarm = stack.kind == StackKind::Swarm;
        let actions = if swarm {
            vec![(
                StackAction::Restart,
                Glyph::RotateCw,
                "滚动重启",
                "swarm-restart",
            )]
        } else {
            vec![
                (StackAction::Start, Glyph::Play, "启动", "stack-start"),
                (StackAction::Stop, Glyph::Square, "停止", "stack-stop"),
                (StackAction::Restart, Glyph::RotateCw, "", "stack-restart"),
            ]
        };
        let remove = if swarm {
            StackAction::RemoveServices
        } else {
            StackAction::RemoveStopped
        };
        let allowed_remove = self.stack_action_available(stack, remove, scope.as_deref());
        let identity = stack.identity();
        let owner = cx.entity().downgrade();
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .flex_shrink_0()
            .children(actions.into_iter().map(|(action, icon, label, id)| {
                let identity = stack.identity();
                let scope = scope.clone();
                Button::new(id)
                    .regular()
                    .icon(icon)
                    .when(!label.is_empty(), |b| b.label(label))
                    .when(label.is_empty(), |b| b.square(32.).tooltip("重启"))
                    .when(action == StackAction::Stop, |b| b.stop_style(p, cx))
                    .disabled(!self.stack_action_available(stack, action, scope.as_deref()))
                    .on_click(cx.listener(move |v, _, w, cx| {
                        v.stack_scoped_action(&identity, action, scope.clone(), w, cx)
                    }))
            }))
            .child(
                Button::new("stack-more")
                    .square(32.)
                    .icon(Glyph::Ellipsis)
                    .tooltip("更多操作")
                    .dropdown_menu(move |menu, _, _| {
                        let owner = owner.clone();
                        let name = identity.clone();
                        let scope = scope.clone();
                        menu.item(
                            PopupMenuItem::new(format!("{}…", remove.label()))
                                .disabled(!allowed_remove)
                                .on_click(move |_, w, cx| {
                                    let _ = owner.update(cx, |v, cx| {
                                        v.stack_scoped_action(&name, remove, scope.clone(), w, cx)
                                    });
                                }),
                        )
                    }),
            )
    }
}
