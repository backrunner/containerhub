use crate::*;

#[derive(Default)]
pub(super) struct InspectView {
    pub generation: u64,
    pub result: Option<Result<ContainerDetails, String>>,
    pub loading: bool,
}
impl Workspace {
    pub(super) fn request_inspect(&mut self) {
        self.inspect.generation += 1;
        self.inspect.result = None;
        self.inspect.loading = false;
        if self.detail_tab == DetailTab::Inspect
            && let Some(key) = self.selected.clone()
        {
            self.inspect.loading = self.backend.send(Request::InspectContainer {
                key,
                generation: self.inspect.generation,
            });
            if !self.inspect.loading {
                self.inspect.result = Some(Err("读取队列已满，请稍后刷新".into()));
            }
        }
    }
    pub(super) fn inspect_panel(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let details = self.inspect.result.as_ref().and_then(|r| r.as_ref().ok());
        let json = details.and_then(|d| serde_json::to_string_pretty(d).ok());
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .px(px(16.))
                    .h(px(42.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .flex_shrink_0()
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .child("容器配置"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(4.))
                            .child(
                                Button::new("inspect-refresh")
                                    .ghost()
                                    .square(24.)
                                    .icon(Glyph::RefreshCw)
                                    .tooltip("刷新配置")
                                    .accessibility_label("刷新配置")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request_inspect();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("inspect-copy")
                                    .small()
                                    .ghost()
                                    .h(px(24.))
                                    .icon(Glyph::Copy)
                                    .label("JSON")
                                    .tooltip("复制配置 JSON")
                                    .accessibility_label("复制配置 JSON")
                                    .disabled(json.is_none())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(json) = &json {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                json.clone(),
                                            ));
                                            this.notice = Some("已复制脱敏容器配置".into());
                                            cx.notify();
                                        }
                                    })),
                            ),
                    ),
            )
            .child(
                observed!(div().id("inspect-scroll"))
                    .overflow_y_scroll()
                    .flex_1()
                    .min_h_0()
                    .px(px(16.))
                    .pb(px(16.))
                    .text_size(px(11.))
                    .when(self.inspect.loading, |d| d.child(empty("正在读取配置…", p)))
                    .when_some(
                        self.inspect
                            .result
                            .as_ref()
                            .and_then(|r| r.as_ref().err())
                            .cloned(),
                        |d, error| d.child(empty(error, p)),
                    )
                    .when_some(details, |d, info| {
                        d.child(config_row("容器", info.container.key.id.clone(), p))
                            .child(config_row("平台", info.container.platform.clone(), p))
                            .child(config_row(
                                "文件系统",
                                info.read_only
                                    .map(|v| if v { "只读" } else { "可读写" }.to_owned())
                                    .unwrap_or("未知".into()),
                                p,
                            ))
                            .child(config_row(
                                "用户",
                                info.user.clone().unwrap_or("—".into()),
                                p,
                            ))
                            .child(config_row(
                                "工作目录",
                                info.working_directory.clone().unwrap_or("—".into()),
                                p,
                            ))
                            .child(
                                config_group("运行参数", p)
                                    .child(config_row(
                                        "环境变量",
                                        join(info.environment_names.clone()),
                                        p,
                                    ))
                                    .child(config_row("标签", join(info.label_names.clone()), p))
                                    .child(config_row(
                                        "启动参数",
                                        format!("{} 项", info.argument_count),
                                        p,
                                    ))
                                    .child(
                                        div()
                                            .mt(px(8.))
                                            .text_size(px(10.))
                                            .text_color(p.muted)
                                            .child("变量与标签仅显示名称，参数内容已隐藏。"),
                                    ),
                            )
                            .child(
                                config_group("资源", p)
                                    .child(config_row(
                                        "挂载",
                                        join(info.container.mounts.clone()),
                                        p,
                                    ))
                                    .child(config_row(
                                        "网络",
                                        join(info.container.networks.clone()),
                                        p,
                                    )),
                            )
                    }),
            )
    }
}
fn config_group(title: &'static str, p: Palette) -> Div {
    div()
        .mt(px(14.))
        .pt(px(14.))
        .border_t_1()
        .border_color(p.border)
        .child(
            div()
                .mb(px(4.))
                .font_weight(FontWeight::MEDIUM)
                .child(title),
        )
}
fn config_row(label: &'static str, value: String, p: Palette) -> impl IntoElement {
    div()
        .flex()
        .items_start()
        .gap(px(12.))
        .py(px(7.))
        .child(
            div()
                .w(px(64.))
                .flex_shrink_0()
                .text_color(p.muted)
                .child(label),
        )
        .child(div().flex_1().min_w_0().text_right().child(value))
}
