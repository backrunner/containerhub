use crate::*;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};

pub(super) struct Pagination {
    pub page: usize,
    pub size: usize,
}

impl Default for Pagination {
    fn default() -> Self {
        Self { page: 0, size: 10 }
    }
}

impl Pagination {
    pub fn pages(&self, total: usize) -> usize {
        total.div_ceil(self.size).max(1)
    }

    pub fn range(&self, total: usize) -> std::ops::Range<usize> {
        let start = self.page.min(self.pages(total) - 1) * self.size;
        start..(start + self.size).min(total)
    }

    pub fn clamp(&mut self, total: usize) -> bool {
        let page = self.page.min(self.pages(total) - 1);
        let changed = page != self.page;
        self.page = page;
        changed
    }
}

impl Workspace {
    pub(super) fn reset_container_page(&mut self) {
        self.pagination.page = 0;
        self.container_scroll.scroll_to_item(0, ScrollStrategy::Top);
    }

    pub(super) fn pagination_bar(
        &self,
        total: usize,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pages = self.pagination.pages(total);
        let page = self.pagination.page.min(pages - 1);
        let range = self.pagination.range(total);
        observed!(div().id("container-pagination"))
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(32.))
            .flex_shrink_0()
            .text_size(px(11.))
            .text_color(p.muted)
            .child(div().flex_1().child(format!(
                "{}–{} / 共 {total} 个{}",
                if total == 0 { 0 } else { range.start + 1 },
                range.end,
                if self.error.is_some() {
                    "（上次同步）"
                } else {
                    ""
                }
            )))
            .child(
                Button::new("container-page-size")
                    .dense()
                    .label(format!("{} 条 / 页", self.pagination.size))
                    .accessibility_label("每页数量")
                    .dropdown_caret(true)
                    .dropdown_menu({
                        let entity = cx.entity().downgrade();
                        let selected = self.pagination.size;
                        move |mut menu, _, _| {
                            for size in [5, 10, 20, 50] {
                                let entity = entity.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(format!("{size} 条 / 页"))
                                        .checked(size == selected)
                                        .on_click(move |_, _, cx| {
                                            let _ = entity.update(cx, |this, cx| {
                                                this.pagination.size = size;
                                                this.reset_container_page();
                                                cx.notify();
                                            });
                                        }),
                                );
                            }
                            menu
                        }
                    }),
            )
            .child(
                Button::new("container-page-prev")
                    .square(28.)
                    .icon(Glyph::ChevronLeft)
                    .accessibility_label("上一页")
                    .tooltip("上一页")
                    .disabled(page == 0)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.pagination.page = this.pagination.page.saturating_sub(1);
                        this.container_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .min_w(px(40.))
                    .text_center()
                    .child(format!("{} / {pages}", page + 1)),
            )
            .child(
                Button::new("container-page-next")
                    .square(28.)
                    .icon(Glyph::ChevronRight)
                    .accessibility_label("下一页")
                    .tooltip("下一页")
                    .disabled(page + 1 == pages)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.pagination.page += 1;
                        this.pagination.clamp(this.visible().len());
                        this.container_scroll.scroll_to_item(0, ScrollStrategy::Top);
                        cx.notify();
                    })),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::Pagination;

    #[test]
    fn empty_full_and_partial_pages() {
        let mut pager = Pagination { page: 0, size: 5 };
        assert_eq!(pager.pages(0), 1);
        assert_eq!(pager.range(0), 0..0);
        assert_eq!(pager.pages(10), 2);
        assert_eq!(pager.range(8), 0..5);
        pager.page = 1;
        assert_eq!(pager.range(8), 5..8);
        assert_eq!(pager.range(10), 5..10);
    }

    #[test]
    fn disappearing_last_page_clamps_without_skipping_remaining_rows() {
        let mut pager = Pagination { page: 3, size: 5 };
        assert!(pager.clamp(8));
        assert_eq!(pager.page, 1);
        assert_eq!(pager.range(8), 5..8);
        assert!(!pager.clamp(8));
        assert!(pager.clamp(0));
        assert_eq!(pager.page, 0);
        assert_eq!(pager.range(0), 0..0);
    }
}
