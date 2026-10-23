use crate::*;

impl Workspace {
    pub(super) fn details(
        &self,
        c: &Container,
        layout: Layout,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let key = c.key.clone();
        let copy_id = c.key.id.clone();
        observed!(div().id("inspector"))
            .flex()
            .flex_col()
            .min_h_0()
            .min_w_0()
            .when(layout.split_inspector, |d| {
                d.w(px(layout.inspector)).flex_shrink_0()
            })
            .when(!layout.split_inspector, |d| d.flex_1())
            .bg(p.surface)
            .line_height(relative(1.4))
            .rounded(px(7.))
            .border_1()
            .border_color(p.border)
            .overflow_hidden()
            .child(
                div()
                    .p(px(17.))
                    .flex_shrink_0()
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap(px(9.))
                            .child(
                                observed!(div().id("inspector-glyph"))
                                    .size(px(32.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(7.))
                                    .bg(p.canvas)
                                    .child(
                                        Icon::new(Glyph::Container)
                                            .size(px(18.))
                                            .text_color(p.muted),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .truncate()
                                            .child(c.key.id.clone()),
                                    )
                                    .child(
                                        div()
                                            .mt(px(3.))
                                            .text_size(px(10.))
                                            .text_color(p.muted)
                                            .truncate()
                                            .child(c.image.clone()),
                                    ),
                            )
                            .child(
                                Button::new("close-details")
                                    .square(32.)
                                    .border_1()
                                    .border_color(p.border)
                                    .icon(if layout.split_inspector {
                                        Glyph::PanelRightClose
                                    } else {
                                        Glyph::ArrowLeft
                                    })
                                    .tooltip("收起详情")
                                    .accessibility_label("收起详情")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.collapse_details(cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .mt(px(12.))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(11.))
                            .child(
                                div()
                                    .size(px(6.))
                                    .rounded_full()
                                    .bg(if c.state.is_running() {
                                        p.success
                                    } else {
                                        p.muted
                                    }),
                            )
                            .child(c.state.label().to_owned())
                            .child(div().flex_1())
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(p.muted)
                                    .child(c.platform.clone()),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .px(px(12.))
                    .border_t_1()
                    .border_b_1()
                    .border_color(p.border)
                    .flex_shrink_0()
                    .children(
                        [
                            (DetailTab::Overview, "概览"),
                            (DetailTab::Logs, "日志"),
                            (DetailTab::Inspect, "配置"),
                        ]
                        .into_iter()
                        .enumerate()
                        .map(|(i, (tab, label))| {
                            div()
                                .flex_1()
                                .max_w(px(90.))
                                .border_b_2()
                                .border_color(if self.detail_tab == tab {
                                    p.accent
                                } else {
                                    p.surface
                                })
                                .child(
                                    Button::new(("detail-tab", i))
                                        .small()
                                        .custom(ButtonCustomVariant::new(cx))
                                        .w_full()
                                        .h(px(32.))
                                        .rounded(px(0.))
                                        .accessibility_label(label)
                                        .child(
                                            div()
                                                .text_size(px(11.))
                                                .text_color(if self.detail_tab == tab {
                                                    p.accent
                                                } else {
                                                    p.muted
                                                })
                                                .child(label),
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.detail_tab = tab;
                                            this.request_logs(false);
                                            this.request_inspect();
                                            cx.notify();
                                        })),
                                )
                        }),
                    )
                    .child(
                        div().flex_1().max_w(px(90.)).child(
                            Button::new("detail-terminal")
                                .small()
                                .ghost()
                                .w_full()
                                .h(px(36.))
                                .disabled(true)
                                .label("终端")
                                .tooltip("当前运行时尚不支持终端会话")
                                .accessibility_label("终端，当前不可用"),
                        ),
                    ),
            )
            .when(self.detail_tab == DetailTab::Inspect, |d| {
                d.child(self.inspect_panel(p, cx))
            })
            .when(self.detail_tab == DetailTab::Logs, |d| {
                d.child(self.log_panel(p, cx))
            })
            .when(self.detail_tab == DetailTab::Overview, |d| {
                d.child(
                    observed!(div().id("detail-scroll"))
                        .overflow_y_scroll()
                        .flex_1()
                        .min_h_0()
                        .p(px(16.))
                        .child(self.stats_content(p))
                        .child(
                            detail_section("连接", p)
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .mt(px(10.))
                                        .text_size(px(11.))
                                        .child(
                                            div()
                                                .w(px(66.))
                                                .flex_shrink_0()
                                                .text_color(p.muted)
                                                .child("端口映射"),
                                        )
                                        .child(
                                            div().flex_1().flex().justify_end().child(
                                                Button::new("edit-container-ports")
                                                    .ghost()
                                                    .dense()
                                                    .text_color(p.accent)
                                                    .label(match c.ports.first() {
                                                        None => "添加映射".into(),
                                                        Some(port) if c.ports.len() == 1 => {
                                                            format!(
                                                                "{} → {}",
                                                                port.host_port, port.container_port
                                                            )
                                                        }
                                                        Some(_) => {
                                                            format!("{} 条映射", c.ports.len())
                                                        }
                                                    })
                                                    .tooltip("编辑端口映射")
                                                    .accessibility_label("编辑端口映射")
                                                    .on_click({
                                                        let key = c.key.clone();
                                                        cx.listener(move |this, _, window, cx| {
                                                            this.edit_ports(key.clone(), window, cx)
                                                        })
                                                    }),
                                            ),
                                        )
                                        .when(!c.ports.is_empty(), |d| {
                                            let ports = c
                                                .ports
                                                .iter()
                                                .map(|port| port.display())
                                                .collect::<Vec<_>>()
                                                .join("\n");
                                            d.child(
                                                Button::new("copy-container-ports")
                                                    .ghost()
                                                    .square(24.)
                                                    .icon(Glyph::Copy)
                                                    .accessibility_label("复制端口映射")
                                                    .tooltip("复制完整端口映射")
                                                    .on_click(move |_, _, cx| {
                                                        cx.write_to_clipboard(
                                                            ClipboardItem::new_string(
                                                                ports.clone(),
                                                            ),
                                                        )
                                                    }),
                                            )
                                        }),
                                )
                                .child(detail_row("网络", join(c.networks.clone()), p))
                                .child(detail_row("地址", join(c.addresses.clone()), p)),
                        )
                        .child(
                            detail_section("存储", p).children(
                                if c.mounts.is_empty() {
                                    vec!["未配置挂载".to_owned()]
                                } else {
                                    c.mounts.clone()
                                }
                                .into_iter()
                                .map(|mount| {
                                    div()
                                        .mt(px(10.))
                                        .p(px(9.))
                                        .rounded(px(5.))
                                        .bg(p.canvas)
                                        .border_1()
                                        .border_color(p.border)
                                        .text_size(px(11.))
                                        .child(mount)
                                }),
                            ),
                        )
                        .child(
                            detail_section("配置", p)
                                .child(detail_row(
                                    "CPU 配置",
                                    c.cpus.map(|n| format!("{n} 核")).unwrap_or("未知".into()),
                                    p,
                                ))
                                .child(detail_row(
                                    "内存上限",
                                    c.memory_limit.map(format_bytes).unwrap_or("未知".into()),
                                    p,
                                ))
                                .child(detail_row(
                                    "环境变量名",
                                    join(c.environment_names.clone()),
                                    p,
                                ))
                                .child(detail_row("创建时间", c.created_at.clone(), p)),
                        )
                        .child(
                            div()
                                .mt(px(18.))
                                .pt(px(14.))
                                .border_t_1()
                                .border_color(p.border)
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(self.actions(c, cx))
                                .when(
                                    self.snapshot
                                        .as_ref()
                                        .is_some_and(|s| s.capabilities.delete_container),
                                    |d| {
                                        d.child(
                                            Button::new("delete-container")
                                                .ghost()
                                                .dense()
                                                .label("删除容器")
                                                .text_color(p.danger)
                                                .disabled(
                                                    !self.writable()
                                                        || c.state != ContainerState::Stopped
                                                        || self.busy(&key),
                                                )
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.delete_dialog(key.clone(), window, cx)
                                                    },
                                                )),
                                        )
                                    },
                                ),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .flex_shrink_0()
                    .px(px(14.))
                    .h(px(38.))
                    .bg(p.canvas)
                    .border_t_1()
                    .border_color(p.border)
                    .text_size(px(10.))
                    .text_color(p.muted)
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(if self.remote_label.is_some() {
                                "远程"
                            } else {
                                "本机"
                            })
                            .child(self.runtime_label().to_owned()),
                    )
                    .child(
                        Button::new("copy-container-id")
                            .ghost()
                            .dense()
                            .icon(Glyph::Copy)
                            .label("复制容器 ID")
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copy_id.clone()))
                            }),
                    ),
            )
    }
}
fn detail_section(title: &'static str, p: Palette) -> Div {
    div()
        .mt(px(18.))
        .pt(px(18.))
        .border_t_1()
        .border_color(p.border)
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .child(title),
        )
}
fn detail_row(label: &'static str, value: String, p: Palette) -> impl IntoElement {
    div()
        .flex()
        .gap(px(12.))
        .mt(px(10.))
        .text_size(px(11.))
        .child(
            div()
                .w(px(66.))
                .flex_shrink_0()
                .text_color(p.muted)
                .child(label),
        )
        .child(div().flex_1().min_w_0().text_right().child(value))
}
