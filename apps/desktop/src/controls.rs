use crate::*;

/// Shared dimensions with design/refinement.css. Explicit sizes override the
/// component library's 24px icon-only default when a button is marked `small`.
pub(crate) const ROW_ACTION_GAP: f32 = 8.;
pub(crate) const ROW_ACTION_WIDTH: f32 = 136.;

pub(crate) trait ControlStyle {
    fn regular(self) -> Self;
    fn dense(self) -> Self;
    fn square(self, size: f32) -> Self;
    fn stop_style(self, p: Palette, cx: &App) -> Self;
}

impl ControlStyle for Button {
    fn regular(self) -> Self {
        self.small()
            .h(px(32.))
            .px(px(10.))
            .py(px(0.))
            .flex_shrink_0()
    }

    fn dense(self) -> Self {
        self.small()
            .h(px(28.))
            .px(px(8.))
            .py(px(0.))
            .flex_shrink_0()
    }

    fn square(self, size: f32) -> Self {
        self.small()
            .size(px(size))
            .min_w(px(size))
            .p(px(0.))
            .flex_shrink_0()
    }

    fn stop_style(self, p: Palette, cx: &App) -> Self {
        self.custom(
            ButtonCustomVariant::new(cx)
                .color(p.danger.opacity(0.07))
                .foreground(p.danger)
                .hover(p.danger.opacity(0.14))
                .active(p.danger.opacity(0.2)),
        )
        .border_1()
        .border_color(p.danger.opacity(0.24))
    }
}

/// GPUI flex baselines on nested Divs use the line box bottom. The numerical
/// text and its smaller suffix therefore use explicit line boxes and a shared
/// optical baseline instead (22/28 or 20/26, with 10/14 suffixes).
pub(crate) fn metric_value(
    value: String,
    suffix: String,
    size: f32,
    suffix_color: Hsla,
) -> impl IntoElement {
    div()
        .flex()
        .items_end()
        .gap(px(6.))
        .h(px(size + 6.))
        .child(
            div()
                .text_size(px(size))
                .line_height(px(size + 6.))
                .font_weight(FontWeight::MEDIUM)
                .child(value),
        )
        .child(
            div()
                .text_size(px(10.))
                .line_height(px(14.))
                .pb(px(4.))
                .text_color(suffix_color)
                .child(suffix),
        )
}
