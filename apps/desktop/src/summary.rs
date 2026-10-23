use crate::*;

impl Workspace {
    pub(super) fn summary(&self, p: Palette) -> impl IntoElement {
        let containers = self
            .snapshot
            .as_ref()
            .map(|s| s.containers.as_slice())
            .unwrap_or_default();
        let running = containers.iter().filter(|c| c.state.is_running()).count();
        let stopped = containers
            .iter()
            .filter(|c| c.state == ContainerState::Stopped)
            .count();
        let other = containers.len() - running - stopped;
        let (cpu, memory) = if self.snapshot.as_ref().is_some_and(|s| s.capabilities.stats)
            && self.error.is_none()
            && self.usage.keys.len() == running
        {
            self.usage.totals()
        } else {
            (None, None)
        };
        let values = [
            (
                "运行中",
                running.to_string(),
                format!("/ {} 个容器", containers.len()),
            ),
            (
                "已停止",
                stopped.to_string(),
                if other > 0 {
                    format!("{other} 个异常 / 未知")
                } else {
                    "".into()
                },
            ),
            (
                "CPU（单核 100%）",
                cpu.map(|n| format!("{n:.1}")).unwrap_or("—".into()),
                "%".into(),
            ),
            (
                "内存用量",
                memory
                    .map(|n| format!("{:.2}", n as f64 / 1_073_741_824.))
                    .unwrap_or("—".into()),
                "GiB".into(),
            ),
        ];
        observed!(div().id("container-summary"))
            .flex()
            .w_full()
            .flex_shrink_0()
            .pb(px(20.))
            .border_b_1()
            .border_color(p.border)
            .children(
                values
                    .into_iter()
                    .enumerate()
                    .map(|(i, (label, value, suffix))| {
                        div()
                            .w(relative(0.25))
                            .min_w_0()
                            .px(px(if i == 0 { 0. } else { 24. }))
                            .when(i > 0, |d| d.border_l_1().border_color(p.border))
                            .child(div().text_size(px(11.)).text_color(p.muted).child(label))
                            .child(div().mt(px(5.)).child(controls::metric_value(
                                value,
                                suffix,
                                22.,
                                if i == 1 && other > 0 {
                                    p.danger
                                } else {
                                    p.muted
                                },
                            )))
                    }),
            )
    }
}
