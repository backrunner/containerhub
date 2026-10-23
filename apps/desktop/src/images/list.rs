use super::*;

impl Workspace {
    pub(crate) fn resource_page(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let images = self.page == Page::Resources(ResourceKind::Image);
        if images && self.image.reference.is_some() {
            return observed!(div().id("image-detail-scroll"))
                .flex_1()
                .min_h_0()
                .min_w_0()
                .overflow_y_scroll()
                .rounded(px(7.))
                .border_1()
                .border_color(p.border)
                .bg(p.surface)
                .child(self.image_panel(p, cx))
                .into_any_element();
        }
        let items = self.resources.as_ref().and_then(|r| r.as_ref().ok());
        let references: Vec<_> = items
            .into_iter()
            .flatten()
            .map(|r| r.name.clone())
            .collect();
        let selected = references
            .iter()
            .filter(|r| self.image.selected.contains(*r))
            .count();
        let all = !references.is_empty() && selected == references.len();
        div().flex().flex_col().flex_1().min_h_0().min_w_0()
            .when(images, |d| d.child(self.image_toolbar(p, cx)))
            .child(observed!(div().id("resource-scroll")).min_h_0().min_w_0().overflow_y_scroll()
                .rounded(px(7.)).border_1().border_color(p.border).bg(p.surface)
                .child(match &self.resources {
                    None => empty("正在读取资源…", p).into_any_element(),
                    Some(Err(e)) => empty(e.clone(), p).into_any_element(),
                    Some(Ok(items)) if items.is_empty() => empty("没有资源", p).into_any_element(),
                    Some(Ok(items)) => div().line_height(relative(1.4))
                        .child(div().flex().items_center().h(px(36.)).px(px(16.)).gap(px(12.))
                            .bg(p.canvas).border_b_1().border_color(p.border).text_size(px(11.)).text_color(p.muted)
                            .when(images, |d| d.child(Button::new("select-all-images").ghost().square(24.)
                                .icon(if all { Glyph::Check } else if selected > 0 { Glyph::Minus } else { Glyph::Square })
                                .toggled(all).text_color(if selected > 0 { p.accent } else { p.muted })
                                .accessibility_label(if all { "取消选择全部镜像" } else if selected > 0 { "部分镜像已选，全选镜像" } else { "全选镜像" })
                                .tooltip("选择全部镜像，每批最多 32 项")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let omitted = crate::selection::toggle_visible_selection(&mut this.image.selected, &references);
                                    this.cancel_image_batch();
                                    if omitted > 0 { this.notice = Some(format!("每批最多 32 项，还有 {omitted} 项未选中。")); }
                                    cx.notify();
                                }))))
                            .child(div().flex_1().child("名称 / 平台"))
                            .child(div().w(px(100.)).child("大小"))
                            .child(div().w(px(70.)).text_right().child("操作")))
                        .children(items.iter().enumerate().map(|(index, r)| {
                            let reference = r.name.clone(); let toggle = reference.clone();
                            let checked = images && self.image.selected.contains(&reference);
                            div().flex().items_center().h(px(62.)).px(px(16.)).gap(px(12.))
                                .when(index > 0, |d| d.border_t_1().border_color(p.border))
                                .when(checked, |d| d.bg(p.selected))
                                .when(images, |d| d.child(Button::new(("select-image", index)).ghost().square(24.)
                                    .icon(if checked { Glyph::Check } else { Glyph::Square }).toggled(checked)
                                    .text_color(if checked { p.accent } else { p.muted })
                                    .accessibility_label(format!("选择镜像 {}", r.name))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !this.image.selected.remove(&toggle) {
                                            if this.image.selected.len() < 32 { this.image.selected.insert(toggle.clone()); }
                                            else { this.notice = Some("每批最多选择 32 项".into()); }
                                        }
                                        this.cancel_image_batch(); cx.notify();
                                    }))))
                                .child(Icon::new(match r.kind { ResourceKind::Image => Glyph::Layers, ResourceKind::Volume => Glyph::Database, ResourceKind::Network => Glyph::Network }).size(px(18.)).text_color(p.muted))
                                .child(div().flex_1().min_w_0()
                                    .child(div().text_size(px(12.)).font_weight(FontWeight::MEDIUM).truncate().child(r.name.clone()))
                                    .child(div().mt(px(4.)).text_size(px(11.)).text_color(p.muted).truncate().child(r.detail.clone())))
                                .child(div().w(px(100.)).text_size(px(11.)).text_color(p.muted).child(r.size.map(format_bytes).unwrap_or("—".into())))
                                .child(div().w(px(70.)).flex().justify_end().when(images && self.snapshot.as_ref().is_some_and(|s| s.capabilities.inspect_image), |d| {
                                    d.child(Button::new(("inspect-image", index)).dense().label("详情")
                                        .on_click(cx.listener(move |this, _, _, cx| { this.request_image(reference.clone()); cx.notify(); })))
                                }))
                        })).into_any_element(),
                }))
            .into_any_element()
    }
    fn image_toolbar(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.image.selected.len();
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(32.))
            .mb(px(14.))
            .flex_shrink_0()
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(p.muted)
                    .child(if count > 0 {
                        format!("已选 {count} 个镜像")
                    } else {
                        format!(
                            "{} 个镜像",
                            self.resources
                                .as_ref()
                                .and_then(|r| r.as_ref().ok())
                                .map_or(0, Vec::len)
                        )
                    }),
            )
            .when(count > 0, |d| {
                d.child(
                    Button::new("clear-image-selection")
                        .ghost()
                        .dense()
                        .label("取消选择")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.image.selected.clear();
                            this.cancel_image_batch();
                            cx.notify();
                        })),
                )
                .child(div().flex_1())
                .child(
                    Button::new("copy-image-selection")
                        .dense()
                        .icon(Glyph::Copy)
                        .label("复制引用")
                        .on_click(cx.listener(|this, _, _, cx| {
                            let mut names: Vec<_> = this.image.selected.iter().cloned().collect();
                            names.sort();
                            cx.write_to_clipboard(ClipboardItem::new_string(names.join("\n")));
                        })),
                )
                .child(
                    Button::new("delete-image-selection")
                        .dense()
                        .icon(Glyph::Trash)
                        .label(if self.image.batch_loading {
                            "正在核对…"
                        } else {
                            "删除所选"
                        })
                        .disabled(
                            self.image.batch_loading
                                || self.error.is_some()
                                || self.snapshot.as_ref().is_none_or(|s| {
                                    !s.capabilities.inspect_image || !s.capabilities.delete_image
                                }),
                        )
                        .on_click(
                            cx.listener(|this, _, window, cx| this.review_image_batch(window, cx)),
                        ),
                )
            })
    }
}
