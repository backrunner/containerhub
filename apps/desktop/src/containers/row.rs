use super::*;

impl Workspace {
    pub(super) fn container_row(
        &self,
        c: &Container,
        index: usize,
        show_resources: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let key = c.key.clone();
        let batch_key = key.clone();
        observed!(div().id(("container", index)))
            .flex()
            .items_center()
            .w_full()
            .h(px(56.))
            .px(px(12.))
            .gap(px(8.))
            .border_b_1()
            .border_color(p.border)
            .cursor_pointer()
            .bg(if self.selected.as_ref() == Some(&key) {
                p.selected
            } else {
                p.surface
            })
            .hover(move |d| d.bg(p.selected))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_container(key.clone());
                cx.notify();
            }))
            .child(
                Button::new(("select-container", index))
                    .ghost()
                    .square(24.)
                    .icon(if self.batch.contains(&batch_key) {
                        Glyph::Check
                    } else {
                        Glyph::Square
                    })
                    .accessibility_label(format!("选择 {}", c.key.id))
                    .tooltip("选择容器（每批最多 32 项）")
                    .disabled(!self.batch.contains(&batch_key) && self.batch.len() >= 32)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        if !this.batch.remove(&batch_key) {
                            this.batch.insert(batch_key.clone());
                        }
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .text_size(px(12.))
                    .child(
                        div()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(c.key.id.clone()),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(11.))
                            .text_color(p.muted)
                            .child(c.image.clone()),
                    ),
            )
            .child(
                observed!(div().id(("container-status", index)))
                    .w(px(68.))
                    .flex_shrink_0()
                    .text_size(px(11.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_color(p.muted)
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
                    .child(c.state.label().to_owned()),
            )
            .when(show_resources, |d| {
                let sample = c
                    .state
                    .is_running()
                    .then(|| self.usage.sample(&c.key))
                    .flatten();
                let cpu = sample
                    .and_then(|s| s.cpu_percent)
                    .filter(|n| n.is_finite() && *n >= 0.)
                    .map(|n| format!("{n:.1}%"))
                    .unwrap_or_else(|| "—".into());
                let memory = sample
                    .and_then(|s| s.memory_bytes)
                    .map(format_bytes)
                    .unwrap_or_else(|| "—".into());
                d.child(resource_cell(54., cpu, p))
                    .child(resource_cell(76., memory, p))
            })
            .child(
                div()
                    .w(px(96.))
                    .pl(px(8.))
                    .flex_shrink_0()
                    .text_size(px(10.))
                    .text_color(p.muted)
                    .overflow_hidden()
                    .children(c.ports.iter().take(2).map(|port| {
                        div()
                            .truncate()
                            .child(format!("{} → {}", port.host_port, port.container_port))
                    }))
                    .when(c.ports.is_empty(), |d| d.child("—")),
            )
            .child(
                div()
                    .w(px(controls::ROW_ACTION_WIDTH))
                    .flex_shrink_0()
                    .child(self.actions(c, cx)),
            )
    }
}

fn resource_cell(width: f32, value: String, p: Palette) -> Div {
    div()
        .w(px(width))
        .flex_shrink_0()
        .text_right()
        .text_size(px(11.))
        .text_color(p.muted)
        .child(value)
}
