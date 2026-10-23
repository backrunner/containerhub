use crate::selection::toggle_visible_selection;
use crate::*;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};

impl Workspace {
    pub(super) fn actions(&self, c: &Container, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.busy(&c.key);
        let disabled = active || !self.writable() || matches!(c.state, ContainerState::Unknown(_));
        let key = c.key.clone();
        let restart_key = key.clone();
        let action = if c.state.is_running() {
            LifecycleAction::Stop
        } else {
            LifecycleAction::Start
        };
        div()
            .flex()
            .items_center()
            .gap(px(controls::ROW_ACTION_GAP))
            .child(
                Button::new((SharedString::from(format!("action-{}", c.key.id)), 0))
                    .dense()
                    .w(px(64.))
                    .min_w(px(64.))
                    .when(c.state.is_running(), |b| {
                        b.stop_style(Palette::get(cx.theme().is_dark()), cx)
                    })
                    .icon(if c.state.is_running() {
                        Glyph::Square
                    } else {
                        Glyph::Play
                    })
                    .label(if active { "处理中" } else { action.label() })
                    .disabled(disabled)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.run(key.clone(), action, cx);
                    })),
            )
            .child(
                Button::new((SharedString::from(format!("restart-{}", c.key.id)), 0))
                    .square(28.)
                    .icon(Glyph::RotateCw)
                    .tooltip("重启")
                    .accessibility_label(format!("重启 {}", c.key.id))
                    .disabled(disabled || !c.state.is_running())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.restart(restart_key.clone(), window, cx);
                    })),
            )
            .child(
                Button::new((SharedString::from(format!("more-{}", c.key.id)), 0))
                    .square(28.)
                    .icon(Glyph::Ellipsis)
                    .accessibility_label(format!("更多操作 {}", c.key.id))
                    .tooltip("更多操作")
                    .dropdown_menu({
                        let entity = cx.entity().downgrade();
                        let key = c.key.clone();
                        let can_delete = !disabled
                            && c.state == ContainerState::Stopped
                            && self
                                .snapshot
                                .as_ref()
                                .is_some_and(|s| s.capabilities.delete_container);
                        move |menu, _, _| {
                            let show_entity = entity.clone();
                            let show_key = key.clone();
                            let log_entity = entity.clone();
                            let log_key = key.clone();
                            let copy_id = key.id.clone();
                            let delete_entity = entity.clone();
                            let delete_key = key.clone();
                            menu.item(PopupMenuItem::new("查看详情").on_click(move |_, _, cx| {
                                let _ = show_entity.update(cx, |this, cx| {
                                    this.select_container(show_key.clone());
                                    cx.notify();
                                });
                            }))
                            .item(PopupMenuItem::new("查看日志").on_click(move |_, _, cx| {
                                let _ = log_entity.update(cx, |this, cx| {
                                    this.select_container(log_key.clone());
                                    this.detail_tab = DetailTab::Logs;
                                    this.request_logs(false);
                                    cx.notify();
                                });
                            }))
                            .item(PopupMenuItem::new("复制容器 ID").on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copy_id.clone()))
                            }))
                            .separator()
                            .item(
                                PopupMenuItem::new("删除容器")
                                    .disabled(!can_delete)
                                    .on_click(move |_, window, cx| {
                                        let _ = delete_entity.update(cx, |this, cx| {
                                            this.delete_dialog(delete_key.clone(), window, cx)
                                        });
                                    }),
                            )
                        }
                    }),
            )
    }
    pub(super) fn select_all_control(
        &self,
        items: &[Container],
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let keys: Vec<_> = items.iter().map(|c| c.key.clone()).collect();
        let selected = keys.iter().filter(|key| self.batch.contains(*key)).count();
        let all = !keys.is_empty() && selected == keys.len();
        Button::new("select-all-containers")
            .ghost()
            .square(24.)
            .icon(if all {
                Glyph::Check
            } else if selected > 0 {
                Glyph::Minus
            } else {
                Glyph::Square
            })
            .text_color(if selected > 0 { p.accent } else { p.muted })
            .disabled(keys.is_empty())
            .toggled(all)
            .accessibility_label(if all {
                "取消选择当前页"
            } else if selected > 0 {
                "部分已选，全选当前页"
            } else {
                "全选当前页"
            })
            .tooltip("选择当前页，每批最多 32 项")
            .on_click(cx.listener(move |this, _, _, cx| {
                let omitted = toggle_visible_selection(&mut this.batch, &keys);
                if omitted > 0 {
                    this.notice = Some(format!(
                        "每批最多 32 项，本次有 {omitted} 项未选中。可完成此批后继续选择。"
                    ));
                }
                cx.notify();
            }))
    }
}
