use crate::*;

impl Workspace {
    pub(super) fn nav(
        &self,
        layout: Layout,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let workspace_name = self
            .workspace_host
            .as_ref()
            .and_then(|h| h.upgrade())
            .map(|h| h.read(cx).active_label())
            .unwrap_or_else(|| {
                if self.demo {
                    "演示工作空间".into()
                } else if self.remote_label.is_some() {
                    "远程工作空间".into()
                } else {
                    "本机工作空间".into()
                }
            });
        let pages = [
            (0usize, Page::Containers, Glyph::Container),
            (6, Page::Stacks, Glyph::Layers),
            (1, Page::Resources(ResourceKind::Image), Glyph::Layers),
            (2, Page::Resources(ResourceKind::Volume), Glyph::Database),
            (3, Page::Resources(ResourceKind::Network), Glyph::Network),
            (5, Page::Environments, Glyph::Server),
            (4, Page::Tasks, Glyph::ListChecks),
        ];
        observed!(div().id("sidebar"))
            .flex()
            .flex_col()
            .w(px(layout.sidebar))
            .flex_shrink_0()
            .bg(p.sidebar)
            .border_r_1()
            .border_color(p.border)
            .px(px(12.))
            .pt(px(24.))
            .pb(px(13.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .mb(px(30.))
                    .px(px(if layout.compact { 2. } else { 8. }))
                    .child(
                        img("brand/icon.png")
                            .size(px(36.))
                            .flex_shrink_0()
                            .border_1()
                            .border_color(p.border)
                            .rounded(px(8.)),
                    )
                    .when(!layout.compact, |d| {
                        d.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .line_height(relative(1.2))
                                .child(
                                    div()
                                        .text_size(px(15.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("ContainerHub"),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(p.muted)
                                        .child(workspace_name.clone()),
                                ),
                        )
                    }),
            )
            .when(!layout.compact, |d| d.child(nav_caption("资源", p)))
            .children(
                pages
                    .into_iter()
                    .filter(|(_, page, _)| {
                        *page != Page::Stacks
                            || self
                                .snapshot
                                .as_ref()
                                .is_some_and(|s| s.capabilities.stacks)
                    })
                    .map(|(i, page, icon)| {
                        let selected = self.page == page;
                        let count = match page {
                            Page::Containers => self.snapshot.as_ref().map(|s| s.containers.len()),
                            Page::Tasks => Some(
                                self.operations
                                    .iter()
                                    .filter(|o| o.state.is_active())
                                    .count(),
                            ),
                            _ => None,
                        };
                        div()
                            .when(i == 5, |d| {
                                d.mt(px(26.))
                                    .when(!layout.compact, |d| d.child(nav_caption("管理", p)))
                            })
                            .child(
                                Button::new(("nav", i))
                                    .ghost()
                                    .w_full()
                                    .h(px(37.))
                                    .px(px(if layout.compact { 8. } else { 12. }))
                                    .selected(selected)
                                    .tooltip(page.title())
                                    .accessibility_label(page.title())
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .w_full()
                                            .gap(px(11.))
                                            .text_size(px(13.))
                                            .text_color(if selected { p.accent } else { p.muted })
                                            .child(Icon::new(icon).size(px(17.)))
                                            .when(!layout.compact, |d| {
                                                d.child(
                                                    div()
                                                        .flex_1()
                                                        .font_weight(if selected {
                                                            FontWeight::MEDIUM
                                                        } else {
                                                            FontWeight::NORMAL
                                                        })
                                                        .child(page.title()),
                                                )
                                                .when_some(count, |d, n| {
                                                    d.child(
                                                        div()
                                                            .text_size(px(11.))
                                                            .child(n.to_string()),
                                                    )
                                                })
                                            }),
                                    )
                                    .on_click(
                                        cx.listener(move |this, _, _, cx| this.navigate(page, cx)),
                                    ),
                            )
                    }),
            )
            .child(div().flex_1())
    }
    pub(super) fn settings(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        observed!(div().id("settings-scroll"))
            .flex_1()
            .overflow_y_scroll()
            .child(
                div()
                    .max_w(px(720.))
                    .p_6()
                    .bg(p.surface)
                    .rounded_lg()
                    .border_1()
                    .border_color(p.border)
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("外观"))
                    .child(
                        div().mt_4().flex().gap_3().children(
                            [Appearance::System, Appearance::Light, Appearance::Dark]
                                .into_iter()
                                .enumerate()
                                .map(|(i, appearance)| {
                                    Button::new(("appearance", i))
                                        .label(appearance.label())
                                        .icon(match appearance {
                                            Appearance::System => Glyph::Monitor,
                                            Appearance::Light => Glyph::Sun,
                                            Appearance::Dark => Glyph::Moon,
                                        })
                                        .selected(self.settings.appearance == appearance)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.settings.appearance = appearance;
                                            appearance.apply(window, cx);
                                            this.send(
                                                Request::SaveSettings(this.settings.clone()),
                                                cx,
                                            );
                                        }))
                                }),
                        ),
                    )
                    .child(
                        div()
                            .mt_6()
                            .text_sm()
                            .text_color(p.muted)
                            .child("关闭应用后，容器仍会继续运行。"),
                    ),
            )
    }
}

fn nav_caption(label: &'static str, p: Palette) -> impl IntoElement {
    div()
        .px(px(12.))
        .mb(px(10.))
        .text_size(px(11.))
        .text_color(p.muted)
        .child(label)
}
