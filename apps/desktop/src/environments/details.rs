use super::*;
use containerhub_runtime::Endpoint;

impl Workspace {
    pub(crate) fn environment_details(
        &self,
        layout: Layout,
        active: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = &self.environment;
        let host = state.host.as_ref();
        let report = state.diagnostics.as_ref().or_else(|| {
            active
                .then(|| self.diagnostics.as_ref().and_then(|r| r.as_ref().ok()))
                .flatten()
        });
        let snapshot = state
            .containers
            .as_ref()
            .or_else(|| active.then_some(self.snapshot.as_ref()).flatten());
        let connected = state.error.is_none() && state.containers.is_some();
        let status = if state.paused {
            "已暂停"
        } else if state.error.is_some() {
            "不可用"
        } else if connected {
            "已连接"
        } else {
            "读取中…"
        };
        let target = state.target.clone();
        let edit_target = target.clone();
        let workspace_host = self.workspace_host.clone();
        let edit_host = workspace_host.clone();
        let can_activate = workspace_host.is_some();
        observed!(div().id("environments-scroll"))
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
            .text_size(px(12.))
            .line_height(relative(1.4))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(8.))
                    .pb(px(16.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .child(
                                        div()
                                            .max_w(px(if layout.compact { 240. } else { 400. }))
                                            .min_w_0()
                                            .text_size(px(17.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .truncate()
                                            .child(state.label()),
                                    )
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .text_size(px(10.))
                                            .text_color(if connected { p.success } else { p.muted })
                                            .child(status),
                                    ),
                            )
                            .child(
                                div()
                                    .mt(px(4.))
                                    .text_size(px(11.))
                                    .text_color(p.muted)
                                    .truncate()
                                    .child(format!(
                                        "{} · {}",
                                        host.and_then(|s| s.hostname.clone()).unwrap_or_else(
                                            || match target {
                                                Target::Local => "localhost".into(),
                                                Target::Demo => "示例主机".into(),
                                                Target::Docker { ref context } => context.clone(),
                                                _ => "远程主机".into(),
                                            }
                                        ),
                                        report
                                            .and_then(|r| r.cli_version.clone())
                                            .map(|v| format!(
                                                "{} {v}",
                                                if snapshot.is_some_and(|s| s.capabilities.stacks)
                                                    || matches!(state.target, Target::Docker { .. })
                                                {
                                                    "Docker"
                                                } else {
                                                    "Apple container"
                                                }
                                            ))
                                            .unwrap_or_else(|| {
                                                if matches!(state.target, Target::Docker { .. }) {
                                                    "Docker Engine".into()
                                                } else {
                                                    "Apple container".into()
                                                }
                                            })
                                    )),
                            )
                            .when(!state.preference.notes.is_empty(), |d| {
                                d.child(
                                    div()
                                        .mt(px(5.))
                                        .text_color(p.muted)
                                        .child(state.preference.notes.clone()),
                                )
                            }),
                    )
                    .when(!active, |d| {
                        d.child(
                            Button::new("environment-activate")
                                .regular()
                                .label("切换")
                                .disabled(!can_activate)
                                .on_click(move |_, window, cx| {
                                    if let Some(host) = &workspace_host {
                                        let _ = host.update(cx, |host, cx| {
                                            host.activate_environment(target.clone(), window, cx)
                                        });
                                    }
                                }),
                        )
                    })
                    .child(
                        Button::new("environment-edit")
                            .square(32.)
                            .icon(Glyph::Pencil)
                            .tooltip("编辑环境")
                            .disabled(edit_host.is_none())
                            .on_click(move |_, window, cx| {
                                if let Some(host) = &edit_host {
                                    let _ = host.update(cx, |host, cx| {
                                        host.edit_environment(edit_target.clone(), window, cx)
                                    });
                                }
                            }),
                    ),
            )
            .when_some(state.error.clone(), |d, error| {
                d.child(
                    div()
                        .mb(px(12.))
                        .text_size(px(11.))
                        .text_color(p.warning)
                        .child(error),
                )
            })
            .when(active, |d| {
                d.when_some(report.filter(|r| r.service_error.is_some()), |d, report| {
                    d.child(div().mb(px(12.)).child(self.runtime_service(report, p, cx)))
                })
            })
            .child(self.host_charts(layout, p, cx))
            .child(
                div()
                    .mt(px(16.))
                    .flex()
                    .flex_wrap()
                    .gap_x(px(20.))
                    .gap_y(px(7.))
                    .text_size(px(10.))
                    .text_color(p.muted)
                    .child(
                        host.map(|s| {
                            format!(
                                "{} / {}",
                                s.os.as_deref().unwrap_or("系统未知"),
                                s.architecture
                            )
                        })
                        .unwrap_or("系统 —".into()),
                    )
                    .child(
                        host.map(|s| {
                            format!(
                                "{} · {} 核",
                                s.processor.as_deref().unwrap_or("CPU"),
                                s.logical_cpus
                            )
                        })
                        .unwrap_or("处理器 —".into()),
                    )
                    .child(
                        host.map(|s| {
                            format!(
                                "运行 {} 天 {} 小时",
                                s.uptime_seconds / 86400,
                                s.uptime_seconds % 86400 / 3600
                            )
                        })
                        .unwrap_or("运行时间 —".into()),
                    ),
            )
            .child(self.environment_containers(snapshot, layout, p, cx))
            .child(self.environment_connection(p))
            .child(
                div()
                    .mt(px(20.))
                    .pt(px(12.))
                    .border_t_1()
                    .border_color(p.border)
                    .child(
                        Button::new("environment-capabilities")
                            .ghost()
                            .regular()
                            .w_full()
                            .justify_between()
                            .px_0()
                            .label(format!(
                                "运行时能力{}",
                                report
                                    .map(|r| format!(
                                        " · {}/{}",
                                        r.checks.iter().filter(|c| c.available).count(),
                                        r.checks.len()
                                    ))
                                    .unwrap_or_default()
                            ))
                            .icon(if state.capabilities_open {
                                Glyph::ChevronUp
                            } else {
                                Glyph::ChevronDown
                            })
                            .on_click(cx.listener(|v, _, _, cx| {
                                v.environment.capabilities_open = !v.environment.capabilities_open;
                                cx.notify();
                            })),
                    )
                    .when(state.capabilities_open, |d| {
                        d.when_some(report, |d, r| {
                            d.children(r.checks.iter().map(|c| {
                                kv(
                                    c.feature.label(),
                                    if c.available {
                                        "可用".into()
                                    } else {
                                        c.reason.clone().unwrap_or("不支持".into())
                                    },
                                    p,
                                )
                            }))
                        })
                    })
                    .when(
                        active
                            && !self.demo
                            && self.remote_label.is_none()
                            && self.workspace_host.is_none(),
                        |d| {
                            d.child(
                                Button::new("runtime-path")
                                    .dense()
                                    .label("CLI 路径…")
                                    .on_click(cx.listener(|v, _, window, cx| {
                                        v.runtime_path_form(window, cx)
                                    })),
                            )
                        },
                    ),
            )
    }
    fn environment_connection(&self, p: Palette) -> impl IntoElement {
        let state = &self.environment;
        let mut rows = vec![];
        if let Some(connection) = &state.connection {
            match &connection.endpoint {
                Endpoint::Direct { url } => {
                    rows.push(kv("连接方式", "直连 · 加密认证".into(), p));
                    rows.push(address("目标地址", url.clone(), p, 0));
                }
                Endpoint::Relay { url, .. } => {
                    rows.push(kv("连接方式", "中继 · 端到端加密".into(), p));
                    rows.push(address("中继地址", url.clone(), p, 1));
                }
                Endpoint::Auto { direct, relay, .. } => {
                    rows.push(kv("连接策略", "优先直连 · 失败后中继".into(), p));
                    rows.push(address("直连地址", direct.clone(), p, 0));
                    rows.push(address("中继地址", relay.clone(), p, 1));
                }
            }
            rows.push(kv(
                "绑定时间",
                chrono::DateTime::from_timestamp(connection.bound_at as i64, 0)
                    .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
                    .unwrap_or("—".into()),
                p,
            ));
        } else {
            rows.push(kv(
                "连接方式",
                if matches!(state.target, Target::Demo) {
                    "本机 · 示例".into()
                } else if let Target::Docker { context } = &state.target {
                    format!("Docker context · {context}")
                } else if matches!(state.target, Target::Local) {
                    "本机进程".into()
                } else {
                    "远程 · 等待读取".into()
                },
                p,
            ));
        }
        let fingerprint = state.connection.as_ref().map(|c| c.agent.clone());
        section("连接信息", p)
            .children(rows)
            .child(kv(
                "请求耗时",
                state
                    .request_ms
                    .map(|n| format!("{n} ms"))
                    .unwrap_or("—".into()),
                p,
            ))
            .child(kv(
                "最近采样",
                state
                    .host
                    .as_ref()
                    .map(|s| {
                        format!(
                            "{} 秒前{}",
                            now().saturating_sub(state.received_at.unwrap_or(s.observed_at)),
                            if state.paused { " · 已暂停" } else { "" }
                        )
                    })
                    .unwrap_or("—".into()),
                p,
            ))
            .when_some(fingerprint, |d, id| {
                let copy = id.clone();
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .py(px(7.))
                        .text_size(px(11.))
                        .child(div().w(px(76.)).text_color(p.muted).child("Agent 指纹"))
                        .child(div().flex_1().min_w_0().truncate().child(id))
                        .child(
                            Button::new("environment-copy-id")
                                .square(28.)
                                .icon(Glyph::Copy)
                                .tooltip("复制完整指纹")
                                .on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                                }),
                        ),
                )
            })
    }
}
fn section(title: &'static str, p: Palette) -> Div {
    div()
        .mt(px(20.))
        .pt(px(16.))
        .border_t_1()
        .border_color(p.border)
        .child(
            div()
                .mb(px(4.))
                .font_weight(FontWeight::MEDIUM)
                .child(title),
        )
}
fn kv(label: &str, value: String, p: Palette) -> Div {
    div()
        .flex()
        .gap(px(12.))
        .py(px(6.))
        .text_size(px(11.))
        .child(
            div()
                .w(px(76.))
                .flex_shrink_0()
                .text_color(p.muted)
                .child(label.to_owned()),
        )
        .child(div().flex_1().min_w_0().text_right().child(value))
}

fn address(label: &str, value: String, p: Palette, id: usize) -> Div {
    let copy = value.clone();
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .py(px(4.))
        .text_size(px(11.))
        .child(
            div()
                .w(px(76.))
                .flex_shrink_0()
                .text_color(p.muted)
                .child(label.to_owned()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_right()
                .child(value),
        )
        .child(
            Button::new(("environment-copy-address", id))
                .square(28.)
                .icon(Glyph::Copy)
                .tooltip("复制地址")
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                }),
        )
}
