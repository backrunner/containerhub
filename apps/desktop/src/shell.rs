use crate::*;

pub(super) const TITLE_BAR_HEIGHT: Pixels = px(54.);

pub(super) fn window_options() -> WindowOptions {
    WindowOptions {
        titlebar: Some(TitlebarOptions {
            // AppKit's 14-point buttons share the toolbar's vertical center.
            traffic_light_position: Some(point(px(20.), (TITLE_BAR_HEIGHT - px(14.)) / 2.)),
            ..TitleBar::title_bar_options()
        }),
        ..TitleBar::window_options()
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(f32::from(window.viewport_size().width));
        let p = Palette::get(cx.theme().is_dark());
        let inspector_visibility = self.inspector_visibility(window, cx);
        if self.initial_selection_pending && self.snapshot.is_some() {
            self.initial_selection_pending = false;
            if layout.split_inspector
                && self.page == Page::Containers
                && self.selected.is_none()
                && let Some(container) = self.snapshot.as_ref().and_then(|s| {
                    s.containers
                        .iter()
                        .find(|c| c.state.is_running())
                        .or(s.containers.first())
                })
            {
                self.select_container(container.key.clone());
            }
        }
        let selected = self
            .selected
            .as_ref()
            .and_then(|key| {
                self.snapshot
                    .as_ref()?
                    .containers
                    .iter()
                    .find(|c| &c.key == key)
            })
            .cloned();
        let count = self.snapshot.as_ref().map(|s| s.containers.len());
        let content = match self.page {
            Page::Containers => div()
                .flex()
                .flex_1()
                .min_h_0()
                .gap(px(20. * inspector_visibility))
                .when(selected.is_none() || layout.split_inspector, |d| {
                    let available_width = f32::from(window.viewport_size().width)
                        - layout.sidebar
                        - 2. * layout.padding
                        - if selected.is_some() {
                            (layout.inspector + 20.) * inspector_visibility
                        } else {
                            0.
                        };
                    d.child(self.list(available_width, p, cx))
                })
                .when_some(selected, |d, c| {
                    d.child(
                        observed!(div().id("inspector-slot"))
                            .flex()
                            .min_h_0()
                            .min_w_0()
                            .overflow_hidden()
                            .when(layout.split_inspector, |d| {
                                d.w(px(layout.inspector * inspector_visibility))
                                    .flex_shrink_0()
                            })
                            .when(!layout.split_inspector, |d| d.flex_1())
                            .opacity(inspector_visibility)
                            .child(self.details(&c, layout, p, cx)),
                    )
                })
                .into_any_element(),
            Page::Resources(_) => self.resource_page(p, cx).into_any_element(),
            Page::Stacks => self.stack_page(layout, p, cx).into_any_element(),
            Page::Tasks => self.tasks(p, cx).into_any_element(),
            Page::Settings => self.settings(p, cx).into_any_element(),
            Page::Environments => self.environments(layout, p, cx).into_any_element(),
        };
        observed!(div().id("workspace"))
            .key_context("ContainerHub")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(p.surface)
            .text_color(p.text)
            .text_size(px(13.))
            .on_action(cx.listener(|this, _: &Refresh, _, cx| this.refresh(cx)))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.navigate(Page::Containers, cx);
                this.close_details();
                this.search.update(cx, |input, cx| input.focus(window, cx));
            }))
            .on_action(cx.listener(|this, _: &CloseDetails, _, cx| {
                this.collapse_details(cx);
            }))
            .child(
                TitleBar::new()
                    .h(TITLE_BAR_HEIGHT)
                    .bg(p.surface)
                    .border_b_1()
                    .border_color(p.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .w_full()
                            .h_full()
                            .pr(px(20.))
                            .pl(px((layout.sidebar - 80. + layout.padding).max(16.)))
                            .gap(px(8.))
                            .text_size(px(12.))
                            .child(self.workspace_picker(p, cx))
                            .child(div().flex_1())
                            .child(
                                Button::new("command-search")
                                    .regular()
                                    .w(px(if layout.compact { 120. } else { 197. }))
                                    .border_1()
                                    .border_color(p.border)
                                    .bg(p.canvas)
                                    .accessibility_label("搜索容器（⌘K）")
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .w_full()
                                            .gap(px(8.))
                                            .text_size(px(11.))
                                            .text_color(p.muted)
                                            .child(Icon::new(Glyph::Search).size(px(14.)))
                                            .child("搜索容器")
                                            .child(div().flex_1())
                                            .child("⌘ K"),
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.navigate(Page::Containers, cx);
                                        this.close_details();
                                        this.search.update(cx, |input, cx| input.focus(window, cx));
                                    })),
                            )
                            .child(
                                Button::new("settings")
                                    .square(32.)
                                    .icon(Glyph::Settings)
                                    .tooltip("设置与外观")
                                    .accessibility_label("设置与外观")
                                    .selected(self.page == Page::Settings)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.navigate(Page::Settings, cx)
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.nav(layout, p, cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .px(px(layout.padding))
                            .pt(px(26.))
                            .pb(px(22.))
                            .gap(px(20.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_4()
                                    .child(
                                        div().flex_1().min_w_0().flex().flex_col().gap_2().child(
                                            div()
                                                .flex()
                                                .gap_3()
                                                .items_center()
                                                .child(
                                                    div()
                                                        .text_size(px(24.))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .child(self.page.title()),
                                                )
                                                .when(self.page == Page::Containers, |d| {
                                                    d.child(
                                                        div()
                                                            .text_size(px(12.))
                                                            .px(px(6.))
                                                            .py(px(1.))
                                                            .rounded(px(4.))
                                                            .border_1()
                                                            .border_color(p.border)
                                                            .text_color(p.muted)
                                                            .child(
                                                                count
                                                                    .map(|n| n.to_string())
                                                                    .unwrap_or("—".into()),
                                                            ),
                                                    )
                                                }),
                                        ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .flex_shrink_0()
                                            .when(
                                                self.page == Page::Containers
                                                    || self.page
                                                        == Page::Resources(ResourceKind::Image),
                                                |d| {
                                                    d.child(
                                                        Button::new("pull-image")
                                                            .regular()
                                                            .icon(Glyph::Download)
                                                            .label("拉取镜像")
                                                            .disabled(
                                                                self.error.is_some()
                                                                    || !self
                                                                        .snapshot
                                                                        .as_ref()
                                                                        .is_some_and(|s| {
                                                                            s.capabilities
                                                                                .pull_image
                                                                        }),
                                                            )
                                                            .on_click(cx.listener(
                                                                |this, _, window, cx| {
                                                                    this.pull_form(window, cx)
                                                                },
                                                            )),
                                                    )
                                                    .child(
                                                        Button::new("create-container")
                                                            .regular()
                                                            .icon(Glyph::Plus)
                                                            .primary()
                                                            .label("新建容器")
                                                            .disabled(
                                                                self.error.is_some()
                                                                    || !self
                                                                        .snapshot
                                                                        .as_ref()
                                                                        .is_some_and(|s| {
                                                                            s.capabilities
                                                                                .create_container
                                                                        }),
                                                            )
                                                            .on_click(cx.listener(
                                                                |this, _, window, cx| {
                                                                    this.create_form(window, cx)
                                                                },
                                                            )),
                                                    )
                                                },
                                            )
                                            .when(self.page == Page::Environments, |d| {
                                                d.child(self.add_environment_button(cx))
                                            })
                                            .when(self.page != Page::Containers, |d| {
                                                d.child(
                                                    Button::new("refresh")
                                                        .square(32.)
                                                        .icon(Glyph::RefreshCw)
                                                        .tooltip("刷新 · ⌘R")
                                                        .accessibility_label("刷新")
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            if this.page == Page::Environments {
                                                                this.refresh_environment(cx);
                                                            } else {
                                                                this.refresh(cx);
                                                            }
                                                        })),
                                                )
                                            }),
                                    ),
                            )
                            .when_some(
                                (self.page != Page::Environments)
                                    .then(|| {
                                        self.error.clone().or_else(|| {
                                            self.snapshot
                                                .as_ref()
                                                .and_then(|s| s.capability_note.clone())
                                        })
                                    })
                                    .flatten(),
                                |d, e| {
                                    d.child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(12.))
                                            .p_3()
                                            .rounded_lg()
                                            .bg(p.accent_surface)
                                            .text_color(p.warning)
                                            .child(div().flex_1().min_w_0().child(e))
                                            .when(
                                                self.error.is_some()
                                                    && self.page != Page::Environments,
                                                |d| {
                                                    d.child(
                                                        Button::new("open-runtime")
                                                            .dense()
                                                            .label("检查环境")
                                                            .on_click(cx.listener(
                                                                |this, _, _, cx| {
                                                                    this.navigate(
                                                                        Page::Environments,
                                                                        cx,
                                                                    )
                                                                },
                                                            )),
                                                    )
                                                },
                                            ),
                                    )
                                },
                            )
                            .when_some(self.notice.clone(), |d, n| {
                                d.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_3()
                                        .p_3()
                                        .bg(p.accent_surface)
                                        .child(div().flex_1().child(n))
                                        .child(
                                            Button::new("dismiss-notice")
                                                .ghost()
                                                .square(28.)
                                                .icon(Glyph::X)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.notice = None;
                                                    cx.notify();
                                                })),
                                        ),
                                )
                            })
                            .when(
                                self.page == Page::Containers
                                    && (self.selected.is_none() || layout.split_inspector),
                                |d| d.child(self.summary(p)),
                            )
                            .child(content),
                    ),
            )
    }
}
