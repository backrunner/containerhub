use crate::*;

#[derive(Default)]
pub(super) struct StatsView {
    pub generation: u64,
    pub sample: Option<ContainerStats>,
    pub error: Option<String>,
    pub history: std::collections::VecDeque<ContainerStats>,
}
impl Workspace {
    pub(super) fn request_stats(&mut self) {
        let key = self
            .selected
            .as_ref()
            .filter(|key| {
                self.is_active
                    && self.page == Page::Containers
                    && self.snapshot.as_ref().is_some_and(|s| {
                        s.capabilities.stats
                            && s.containers
                                .iter()
                                .any(|c| c.key == **key && c.state == ContainerState::Running)
                    })
            })
            .cloned();
        let old = self.backend.stats.borrow().as_ref().map(|r| r.key.clone());
        if old == key {
            return;
        }
        self.stats.generation += 1;
        self.stats.sample = None;
        self.stats.history.clear();
        self.stats.error = None;
        self.backend
            .stats
            .send_replace(key.map(|key| backend::StatsRequest {
                key,
                generation: self.stats.generation,
            }));
    }
    pub(super) fn stats_content(&self, p: Palette) -> impl IntoElement {
        let sample = self.stats.sample.as_ref();
        let cpu = sample
            .and_then(|s| s.cpu_percent)
            .map(|v| format!("{v:.1}"))
            .unwrap_or("—".into());
        let memory = sample
            .and_then(|s| s.memory_bytes)
            .map(format_bytes)
            .unwrap_or("—".into());
        let (memory_value, memory_unit) = memory
            .split_once(' ')
            .map(|(v, u)| (v.to_owned(), u.to_owned()))
            .unwrap_or((memory.clone(), String::new()));
        let limit = sample.and_then(|s| s.memory_limit_bytes).or_else(|| {
            self.snapshot
                .as_ref()?
                .containers
                .iter()
                .find(|c| Some(&c.key) == self.selected.as_ref())?
                .memory_limit
        });
        let age = sample.map(|s| now().saturating_sub(s.observed_at));
        let stale = self.stats.error.is_some() || age.is_some_and(|age| age > 10);
        let caption = match age {
            Some(age) => format!("{age} 秒前{}", if stale { "（已陈旧）" } else { "采样" }),
            None => "等待采样".into(),
        };
        div()
            .line_height(relative(1.4))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(11.))
                    .child(div().font_weight(FontWeight::MEDIUM).child("资源用量"))
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(if stale { p.warning } else { p.muted })
                            .child(if stale {
                                caption
                            } else if self.demo {
                                "最近 60 秒（示例）".into()
                            } else {
                                "最近 60 秒".into()
                            }),
                    ),
            )
            .child(
                div().mt(px(14.)).flex().children(
                    [
                        ("CPU（单核 100%）", cpu, "%".to_owned()),
                        (
                            "内存 / 配置上限",
                            memory_value,
                            format!(
                                "{memory_unit} / {}",
                                limit.map(format_bytes).unwrap_or("—".into())
                            ),
                        ),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(i, (label, value, unit))| {
                        let values: Vec<_> = self
                            .stats
                            .history
                            .iter()
                            .map(|s| {
                                (
                                    s.observed_at,
                                    if i == 0 {
                                        s.cpu_percent
                                    } else {
                                        s.memory_bytes.map(|v| v as f64)
                                    },
                                )
                            })
                            .collect();
                        div()
                            .flex_1()
                            .min_w_0()
                            .when(i == 0, |d| d.pr(px(12.)))
                            .when(i == 1, |d| {
                                d.pl(px(16.)).border_l_1().border_color(p.border)
                            })
                            .child(div().text_size(px(10.)).text_color(p.muted).child(label))
                            .child(
                                div()
                                    .mt(px(9.))
                                    .child(controls::metric_value(value, unit, 20., p.muted)),
                            )
                            .child(sparkline(values, p.accent))
                    }),
                ),
            )
            .when_some(self.stats.error.clone(), |d, error| {
                d.child(
                    div()
                        .mt(px(8.))
                        .text_size(px(10.))
                        .text_color(p.warning)
                        .child(error),
                )
            })
    }
}
impl StatsView {
    pub fn record(&mut self, sample: ContainerStats) {
        // Only retain actual samples, bounded by both time and count. A refresh
        // at the same timestamp replaces the last point instead of inventing time.
        if self
            .history
            .back()
            .is_some_and(|last| last.observed_at >= sample.observed_at)
        {
            if self
                .history
                .back()
                .is_some_and(|last| last.observed_at == sample.observed_at)
            {
                self.history.pop_back();
            } else {
                return;
            }
        }
        self.history
            .retain(|old| sample.observed_at.saturating_sub(old.observed_at) <= 60);
        self.history.push_back(sample.clone());
        while self.history.len() > 60 {
            self.history.pop_front();
        }
        self.sample = Some(sample);
        self.error = None;
    }
}
fn sparkline(values: Vec<(u64, Option<f64>)>, color: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let Some((end, _)) = values.last() else {
                return;
            };
            let max = values
                .iter()
                .filter_map(|(_, v)| *v)
                .filter(|v| v.is_finite())
                .fold(1., f64::max);
            let mut builder = PathBuilder::stroke(px(1.3));
            let mut connected = false;
            let mut points = 0;
            for (time, value) in &values {
                let Some(value) = value.filter(|v| v.is_finite() && *v >= 0.) else {
                    connected = false;
                    continue;
                };
                let x =
                    bounds.right() - bounds.size.width * (end.saturating_sub(*time) as f32 / 60.);
                let y =
                    bounds.bottom() - px(2.) - (bounds.size.height - px(4.)) * (value / max) as f32;
                if connected {
                    builder.line_to(point(x, y));
                } else {
                    builder.move_to(point(x, y));
                }
                connected = true;
                points += 1;
            }
            if points > 1
                && let Ok(path) = builder.build()
            {
                window.paint_path(path, color);
            }
        },
    )
    .w_full()
    .h(px(30.))
    .mt(px(8.))
}

#[cfg(test)]
mod tests {
    use super::{ContainerStats, ProviderKind, ResourceKey, StatsView};
    fn sample(time: u64) -> ContainerStats {
        ContainerStats {
            key: ResourceKey {
                provider: ProviderKind::Apple,
                environment: "demo".into(),
                namespace: None,
                id: "fixture".into(),
                incarnation: "one".into(),
            },
            observed_at: time,
            cpu_percent: Some(2.4),
            memory_bytes: Some(1024),
            memory_limit_bytes: Some(4096),
        }
    }
    #[test]
    fn history_rejects_old_samples_replaces_duplicates_and_remains_bounded() {
        let mut stats = StatsView::default();
        for time in 1..=100 {
            stats.record(sample(time));
        }
        assert_eq!(stats.history.len(), 60);
        stats.record(sample(99));
        assert_eq!(stats.sample.as_ref().unwrap().observed_at, 100);
        let mut corrected = sample(100);
        corrected.cpu_percent = None;
        stats.record(corrected);
        assert_eq!(stats.history.len(), 60);
        assert_eq!(stats.history.back().unwrap().cpu_percent, None);
        stats.record(sample(200));
        assert_eq!(stats.history.len(), 1);
    }
}
