use super::*;

impl Workspace {
    pub(super) fn host_charts(
        &self,
        layout: Layout,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = &self.environment;
        let host = state.host.as_ref();
        let columns = if layout.compact { 2 } else { 4 };
        let cells = (0..4)
            .map(|index| {
                let (title, value, suffix, legend) = match index {
                    0 => (
                        "CPU",
                        host.and_then(|s| s.cpu_percent)
                            .map(|v| format!("{v:.1}"))
                            .unwrap_or("—".into()),
                        "%".into(),
                        "整机 · 100%".into(),
                    ),
                    1 => (
                        "内存",
                        gib(host.and_then(|s| s.memory_used_bytes)),
                        format!("/ {} GiB", gib(host.and_then(|s| s.memory_total_bytes))),
                        "物理内存".into(),
                    ),
                    2 => (
                        "网络",
                        rate(host.and_then(|s| s.network_receive_bytes_per_second)),
                        "MB/s".into(),
                        format!(
                            "↓ 接收  ↑ {} MB/s",
                            rate(host.and_then(|s| s.network_send_bytes_per_second))
                        ),
                    ),
                    _ => (
                        "磁盘",
                        gib(host.and_then(|s| s.disk_used_bytes)),
                        format!("/ {} GiB", gib(host.and_then(|s| s.disk_total_bytes))),
                        format!(
                            "读 {} · 写 {} MB/s",
                            rate(host.and_then(|s| s.disk_read_bytes_per_second)),
                            rate(host.and_then(|s| s.disk_write_bytes_per_second))
                        ),
                    ),
                };
                let values = state
                    .history
                    .iter()
                    .map(|s| {
                        (
                            s.observed_at,
                            match index {
                                0 => s.cpu_percent,
                                1 => s.memory_used_bytes.map(|n| n as f64),
                                2 => s.network_receive_bytes_per_second,
                                _ => s.disk_read_bytes_per_second,
                            },
                            match index {
                                2 => s.network_send_bytes_per_second,
                                3 => s.disk_write_bytes_per_second,
                                _ => None,
                            },
                        )
                    })
                    .collect();
                observed!(div().id(("host-metric", index)))
                    .flex_1()
                    .min_w_0()
                    .px(px(12.))
                    .py(px(12.))
                    .when(index % columns > 0, |d| {
                        d.border_l_1().border_color(p.border)
                    })
                    .child(div().text_size(px(11.)).text_color(p.muted).child(title))
                    .child(
                        div()
                            .mt(px(9.))
                            .child(controls::metric_value(value, suffix, 23., p.muted)),
                    )
                    .child(trend(
                        values,
                        state.range,
                        if state.paused || state.host_error.is_some() {
                            p.muted
                        } else {
                            p.accent
                        },
                        p.muted,
                        index,
                        host,
                    ))
                    .child(
                        div()
                            .mt(px(7.))
                            .text_size(px(9.))
                            .text_color(p.muted)
                            .truncate()
                            .child(legend),
                    )
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let mut rows = vec![];
        let mut cells = cells.into_iter();
        for index in 0..4 / columns {
            rows.push(
                div()
                    .flex()
                    .when(index > 0, |d| d.border_t_1().border_color(p.border))
                    .children(cells.by_ref().take(columns)),
            );
        }
        div()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .mb(px(10.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .font_weight(FontWeight::MEDIUM)
                            .child("主机资源")
                            .when(host.is_some_and(|s| s.sample), |d| {
                                d.child(div().text_size(px(10.)).text_color(p.muted).child("示例"))
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                Button::new("host-range-60")
                                    .dense()
                                    .ghost()
                                    .selected(state.range == 60)
                                    .label("1 分钟")
                                    .on_click(cx.listener(|v, _, _, cx| {
                                        v.environment.range = 60;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("host-range-300")
                                    .dense()
                                    .ghost()
                                    .selected(state.range == 300)
                                    .label("5 分钟")
                                    .on_click(cx.listener(|v, _, _, cx| {
                                        v.environment.range = 300;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("host-pause")
                                    .square(28.)
                                    .icon(if state.paused {
                                        Glyph::Play
                                    } else {
                                        Glyph::Pause
                                    })
                                    .tooltip(if state.paused {
                                        "继续采样"
                                    } else {
                                        "暂停采样"
                                    })
                                    .on_click(cx.listener(|v, _, _, cx| {
                                        v.environment.paused = !v.environment.paused;
                                        v.environment.generation += 1;
                                        v.request_environment();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .border_1()
                    .border_color(p.border)
                    .rounded(px(7.))
                    .overflow_hidden()
                    .children(rows),
            )
            .when_some(state.host_error.clone(), |d, e| {
                d.child(
                    div()
                        .mt(px(8.))
                        .text_size(px(11.))
                        .text_color(p.warning)
                        .child(e),
                )
            })
    }
}
fn gib(bytes: Option<u64>) -> String {
    bytes
        .map(|n| format!("{:.1}", n as f64 / 1_073_741_824.))
        .unwrap_or("—".into())
}
fn rate(bytes: Option<f64>) -> String {
    bytes
        .map(|n| format!("{:.2}", n / 1_000_000.))
        .unwrap_or("—".into())
}
fn trend(
    values: Vec<(u64, Option<f64>, Option<f64>)>,
    range: u64,
    color: Hsla,
    secondary: Hsla,
    metric: usize,
    host: Option<&HostSnapshot>,
) -> impl IntoElement {
    let ceiling = match metric {
        0 => Some(100.),
        1 => host.and_then(|s| s.memory_total_bytes).map(|n| n as f64),
        _ => None,
    };
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let Some((end, _, _)) = values.last() else {
                return;
            };
            let max = ceiling.unwrap_or_else(|| {
                values
                    .iter()
                    .filter(|(t, _, _)| end.saturating_sub(*t) <= range)
                    .flat_map(|(_, a, b)| [*a, *b])
                    .flatten()
                    .fold(1., f64::max)
            });
            for series in 0..2 {
                let mut builder = PathBuilder::stroke(px(if series == 0 { 1.5 } else { 1. }));
                let mut connected = false;
                let mut last_time = 0;
                let mut points = 0;
                for (time, a, b) in &values {
                    if end.saturating_sub(*time) > range {
                        continue;
                    }
                    let value = if series == 0 { *a } else { *b };
                    let Some(value) = value else {
                        connected = false;
                        continue;
                    };
                    let x = bounds.right()
                        - bounds.size.width * (end.saturating_sub(*time) as f32 / range as f32);
                    let y = bounds.bottom()
                        - px(2.)
                        - (bounds.size.height - px(4.)) * (value / max).clamp(0., 1.) as f32;
                    if connected
                        && time.saturating_sub(last_time) <= 32
                        && (series == 0 || points % 2 == 1)
                    {
                        builder.line_to(point(x, y));
                    } else {
                        builder.move_to(point(x, y));
                    }
                    connected = true;
                    last_time = *time;
                    points += 1;
                }
                if points > 1
                    && let Ok(path) = builder.build()
                {
                    window.paint_path(path, if series == 0 { color } else { secondary });
                }
            }
        },
    )
    .w_full()
    .h(px(48.))
    .mt(px(12.))
}
