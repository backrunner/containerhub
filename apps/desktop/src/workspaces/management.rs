use super::*;

impl Workspace {
    pub(crate) fn add_environment_button(&self, cx: &App) -> Button {
        let host = self.workspace_host.clone();
        Button::new("add-environment")
            .regular()
            .icon(Glyph::Plus)
            .label("添加环境")
            .disabled(
                host.as_ref()
                    .and_then(|h| h.upgrade())
                    .is_none_or(|h| h.read(cx).loading),
            )
            .on_click(move |_, window, cx| {
                if let Some(host) = &host {
                    let _ = host.update(cx, |host, cx| host.add_form(window, cx));
                }
            })
    }
    pub(crate) fn environments(
        &self,
        layout: Layout,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        use gpui_kit::component::menu::{ContextMenuExt, PopupMenuItem};
        let host = self.workspace_host.clone();
        let (targets, active) = host
            .as_ref()
            .and_then(|h| h.upgrade())
            .map(|h| {
                let h = h.read(cx);
                (
                    h.targets
                        .iter()
                        .map(|t| (t.clone(), h.label(t)))
                        .collect::<Vec<_>>(),
                    h.active.clone(),
                )
            })
            .unwrap_or_else(|| {
                (
                    vec![(self.environment.target.clone(), self.environment.label())],
                    self.environment.target.key(),
                )
            });
        let query = self.environment.query.trim().to_lowercase();
        let selected = self.environment.target.key();
        let filtered = targets
            .into_iter()
            .filter(|(t, label)| {
                query.is_empty()
                    || label.to_lowercase().contains(&query)
                    || t.key().to_lowercase().contains(&query)
            })
            .collect::<Vec<_>>();
        observed!(div().id("environment-columns"))
            .flex()
            .flex_1()
            .min_h_0()
            .gap(px(20.))
            .child(
                observed!(div().id("environment-list"))
                    .flex()
                    .flex_col()
                    .w(px(if layout.compact {
                        176.
                    } else if layout.sidebar < 200. {
                        192.
                    } else {
                        224.
                    }))
                    .flex_shrink_0()
                    .min_h_0()
                    .border_1()
                    .border_color(p.border)
                    .rounded(px(7.))
                    .overflow_hidden()
                    .child(
                        div().p(px(10.)).border_b_1().border_color(p.border).child(
                            Input::new(&self.environment.search)
                                .id("environment-search")
                                .small()
                                .prefix(Icon::new(Glyph::Search).size(px(13.))),
                        ),
                    )
                    .child(
                        div()
                            .id("environment-list-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .p(px(5.))
                            .children(filtered.iter().enumerate().map(|(i, (target, label))| {
                                let key = target.key();
                                let current = key == active;
                                let chosen = key == selected;
                                let inspect_target = target.clone();
                                let inspect_host = host.clone();
                                let menu_target = target.clone();
                                let menu_host = host.clone();
                                observed!(div().id(("environment-row", i)))
                                    .flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .px(px(10.))
                                    .py(px(13.))
                                    .rounded(px(5.))
                                    .cursor_pointer()
                                    .when(chosen, |d| d.bg(p.selected))
                                    .hover(|d| d.bg(p.selected))
                                    .child(
                                        Icon::new(if matches!(target, Target::Remote { .. }) {
                                            Glyph::Server
                                        } else {
                                            Glyph::Monitor
                                        })
                                        .size(px(17.))
                                        .text_color(if chosen { p.accent } else { p.muted }),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .truncate()
                                                    .child(label.clone()),
                                            )
                                            .child(
                                                div()
                                                    .mt(px(3.))
                                                    .text_size(px(10.))
                                                    .text_color(p.muted)
                                                    .truncate()
                                                    .child(match target {
                                                        Target::Local => "Apple container",
                                                        Target::Demo => "示例",
                                                        Target::Docker { .. } => "Docker Engine",
                                                        Target::Remote { .. } => "远程 Agent",
                                                    }),
                                            ),
                                    )
                                    .when(current, |d| {
                                        d.child(
                                            Icon::new(Glyph::Check)
                                                .size(px(13.))
                                                .text_color(p.accent),
                                        )
                                    })
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        if let Some(host) = &inspect_host {
                                            let _ = host.update(cx, |host, cx| {
                                                host.inspect_environment(inspect_target.clone(), cx)
                                            });
                                        }
                                    })
                                    .context_menu(move |mut menu, _, _| {
                                        for (action, label) in [
                                            (0, "查看"),
                                            (1, "切换到此环境"),
                                            (2, "编辑…"),
                                            (3, "复制环境信息"),
                                            (4, "刷新"),
                                        ] {
                                            let host = menu_host.clone();
                                            let target = menu_target.clone();
                                            menu =
                                                menu.item(PopupMenuItem::new(label).on_click(
                                                    move |_, window, cx| {
                                                        if let Some(host) = &host {
                                                            let _ =
                                                                host.update(cx, |host, cx| {
                                                                    match action {
                                                                        0 | 4 => host
                                                                            .inspect_environment(
                                                                                target.clone(),
                                                                                cx,
                                                                            ),
                                                                        1 => host.activate(
                                                                            target.clone(),
                                                                            window,
                                                                            cx,
                                                                        ),
                                                                        2 => host.edit_environment(
                                                                            target.clone(),
                                                                            window,
                                                                            cx,
                                                                        ),
                                                                        _ => {
                                                                            let text = host
                                                                        .connection(&target)
                                                                        .ok()
                                                                        .flatten()
                                                                        .map(|c| {
                                                                            format!(
                                                                                "{}\n{}\n{:?}",
                                                                                host.label(&target),
                                                                                c.agent,
                                                                                c.endpoint
                                                                            )
                                                                        })
                                                                        .unwrap_or_else(|| {
                                                                            host.label(&target)
                                                                        });
                                                                            cx.write_to_clipboard(
                                                                        ClipboardItem::new_string(
                                                                            text,
                                                                        ),
                                                                    );
                                                                        }
                                                                    }
                                                                });
                                                        }
                                                    },
                                                ));
                                        }
                                        if matches!(
                                            menu_target,
                                            Target::Remote { .. } | Target::Docker { .. }
                                        ) {
                                            let host = menu_host.clone();
                                            let target = menu_target.clone();
                                            menu = menu.separator().item(
                                                PopupMenuItem::new("移除环境…").on_click(
                                                    move |_, window, cx| {
                                                        if let Some(host) = &host {
                                                            let _ = host.update(cx, |host, cx| {
                                                                host.remove_environment(
                                                                    target.clone(),
                                                                    window,
                                                                    cx,
                                                                )
                                                            });
                                                        }
                                                    },
                                                ),
                                            );
                                        }
                                        menu
                                    })
                            }))
                            .when(filtered.is_empty(), |d| {
                                d.child(
                                    div()
                                        .p(px(14.))
                                        .text_size(px(11.))
                                        .text_color(p.muted)
                                        .child("没有匹配的环境"),
                                )
                            }),
                    ),
            )
            .child(self.environment_details(layout, selected == active, p, cx))
    }
}
