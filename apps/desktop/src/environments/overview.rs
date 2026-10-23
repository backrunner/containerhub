use super::*;
use gpui_kit::component::dialog::{DialogClose, DialogFooter};

impl Workspace {
    pub(super) fn environment_containers(
        &self,
        snapshot: Option<&Snapshot>,
        layout: Layout,
        p: Palette,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overview = snapshot
            .map(|s| ContainerOverview::from_containers(&s.containers))
            .unwrap_or_default();
        let stale = self.environment.error.is_some();
        let known = snapshot.is_some() && !stale;
        let counts = [
            ("运行中", overview.running),
            ("已停止", overview.stopped),
            ("状态未知", overview.unknown),
        ];
        let health = [
            ("健康", overview.healthy, p.success),
            ("不健康", overview.unhealthy, p.danger),
            ("检查中", overview.starting, p.warning),
            ("未配置", overview.unconfigured, p.muted),
            ("不支持", overview.unsupported, p.muted),
            ("未知", overview.health_unknown, p.muted),
        ];
        let title = self.environment.label();
        let rows = snapshot.map(|s| s.containers.clone()).unwrap_or_default();
        let all = rows.clone();
        let name = title.clone();
        div()
            .mt(px(20.))
            .pt(px(16.))
            .border_t_1()
            .border_color(p.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().font_weight(FontWeight::MEDIUM).child("容器概览"))
                    .child(
                        Button::new("environment-containers")
                            .dense()
                            .ghost()
                            .label("查看容器")
                            .icon(Glyph::ArrowUpRight)
                            .disabled(snapshot.is_none())
                            .on_click(move |_, window, cx| {
                                container_dialog(name.clone(), all.clone(), stale, window, cx)
                            }),
                    ),
            )
            .child(
                div()
                    .mt(px(10.))
                    .flex()
                    .gap(px(20.))
                    .when(layout.compact, |d| d.flex_col())
                    .child(
                        div()
                            .flex()
                            .gap(px(14.))
                            .w(px(if layout.compact { 0. } else { 250. }))
                            .when(layout.compact, |d| d.w_full())
                            .flex_shrink_0()
                            .children(counts.into_iter().enumerate().map(|(index, (label, n))| {
                                let name = title.clone();
                                let rows = rows
                                    .iter()
                                    .filter(|c| match index {
                                        0 => c.state.is_running(),
                                        1 => c.state == ContainerState::Stopped,
                                        _ => matches!(c.state, ContainerState::Unknown(_)),
                                    })
                                    .cloned()
                                    .collect::<Vec<_>>();
                                Button::new(("environment-count", index))
                                    .ghost()
                                    .h(px(64.))
                                    .flex_1()
                                    .px_0()
                                    .disabled(snapshot.is_none())
                                    .child(
                                        div()
                                            .w_full()
                                            .text_left()
                                            .child(
                                                div()
                                                    .text_size(px(10.))
                                                    .text_color(p.muted)
                                                    .child(label),
                                            )
                                            .child(div().mt(px(6.)).text_size(px(24.)).child(
                                                if known { n.to_string() } else { "—".into() },
                                            )),
                                    )
                                    .on_click(move |_, window, cx| {
                                        container_dialog(
                                            name.clone(),
                                            rows.clone(),
                                            stale,
                                            window,
                                            cx,
                                        )
                                    })
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .when(!layout.compact, |d| {
                                d.pl(px(20.)).border_l_1().border_color(p.border)
                            })
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .text_size(px(10.))
                                    .text_color(p.muted)
                                    .child(if stale {
                                        "健康检查 · 待更新"
                                    } else {
                                        "健康检查"
                                    })
                                    .child(if known && overview.healthy + overview.unhealthy > 0 {
                                        format!(
                                            "{} 个已检查",
                                            overview.healthy + overview.unhealthy
                                        )
                                    } else {
                                        "—".into()
                                    }),
                            )
                            .child(
                                div()
                                    .mt(px(9.))
                                    .mb(px(8.))
                                    .h(px(5.))
                                    .flex()
                                    .gap(px(2.))
                                    .rounded(px(2.))
                                    .overflow_hidden()
                                    .children(health.iter().filter(|(_, n, _)| *n > 0).map(
                                        |(_, n, color)| {
                                            div().h_full().flex_grow(*n as f32).bg(if stale {
                                                p.muted
                                            } else {
                                                *color
                                            })
                                        },
                                    )),
                            )
                            .child(
                                div().flex().flex_wrap().gap(px(10.)).children(
                                    health
                                        .into_iter()
                                        .enumerate()
                                        .filter(|(_, (_, n, _))| *n > 0)
                                        .map(|(i, (label, n, color))| {
                                            let name = title.clone();
                                            let rows = rows
                                                .iter()
                                                .filter(|c| {
                                                    c.state.is_running()
                                                        && c.health
                                                            == [
                                                                ContainerHealth::Healthy,
                                                                ContainerHealth::Unhealthy,
                                                                ContainerHealth::Starting,
                                                                ContainerHealth::Unconfigured,
                                                                ContainerHealth::Unsupported,
                                                                ContainerHealth::Unknown,
                                                            ][i]
                                                })
                                                .cloned()
                                                .collect::<Vec<_>>();
                                            Button::new(("environment-health", i))
                                                .ghost()
                                                .h(px(22.))
                                                .px_0()
                                                .text_size(px(10.))
                                                .text_color(if stale { p.muted } else { color })
                                                .label(format!("{label} {n}"))
                                                .on_click(move |_, window, cx| {
                                                    container_dialog(
                                                        name.clone(),
                                                        rows.clone(),
                                                        stale,
                                                        window,
                                                        cx,
                                                    )
                                                })
                                        }),
                                ),
                            ),
                    ),
            )
    }
}
fn container_dialog(
    name: String,
    containers: Vec<Container>,
    stale: bool,
    window: &mut Window,
    cx: &mut App,
) {
    window.open_dialog(cx, move |dialog, _, cx| {
        let p = Palette::get(cx.theme().is_dark());
        dialog
            .title(format!("容器 · {name}"))
            .w(px(540.))
            .footer(
                DialogFooter::new()
                    .child(DialogClose::new().trigger(|b| b.regular().label("关闭"))),
            )
            .child(div().text_size(px(11.)).text_color(p.muted).child(format!(
                "{} 个容器{}",
                containers.len(),
                if stale { " · 上次采样" } else { "" }
            )))
            .child(
                div()
                    .id("environment-container-scroll")
                    .max_h(px(300.))
                    .overflow_y_scroll()
                    .children(containers.iter().map(|c| {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .py(px(10.))
                            .border_b_1()
                            .border_color(p.border)
                            .text_size(px(12.))
                            .child(
                                Icon::new(Glyph::Container)
                                    .size(px(15.))
                                    .text_color(p.muted),
                            )
                            .child(div().flex_1().min_w_0().truncate().child(c.key.id.clone()))
                            .child(div().w(px(64.)).text_color(p.muted).child(
                                if c.state.is_running() {
                                    "运行中"
                                } else if c.state == ContainerState::Stopped {
                                    "已停止"
                                } else {
                                    "未知"
                                },
                            ))
                            .child(
                                div()
                                    .w(px(60.))
                                    .text_right()
                                    .text_color(
                                        if !stale && c.health == ContainerHealth::Unhealthy {
                                            p.danger
                                        } else {
                                            p.muted
                                        },
                                    )
                                    .child(if c.state.is_running() {
                                        c.health.label()
                                    } else {
                                        "—"
                                    }),
                            )
                    })),
            )
    });
}
