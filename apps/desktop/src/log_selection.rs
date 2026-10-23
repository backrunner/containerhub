use crate::*;
use gpui_kit::component::{
    dialog::{DialogClose, DialogFooter},
    input::{Editor, EditorState},
};

struct LogSelection {
    editor: Entity<EditorState>,
    copied: bool,
    _observation: Subscription,
}
impl Render for LogSelection {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let empty = self.editor.read(cx).selected_text().to_string().is_empty();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                Editor::new(&self.editor)
                    .readonly(true)
                    .aria_label("日志快照选区")
                    .h(px(240.)),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Button::new("log-copy-selection")
                            .dense()
                            .label("复制选区")
                            .disabled(empty)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let text = this.editor.read(cx).selected_text().to_string();
                                if !text.is_empty() {
                                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                                    this.copied = true;
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(if self.copied {
                                "已复制"
                            } else {
                                "拖选文字，或使用 Shift + 方向键；⌘C 复制"
                            }),
                    ),
            )
    }
}
impl Workspace {
    pub(super) fn select_log_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Ok(log)) = self.logs.result.as_ref() else {
            return;
        };
        let text = self
            .logs
            .filtered()
            .into_iter()
            .map(|(_, text)| text)
            .collect::<Vec<_>>()
            .join("\n");
        if text.is_empty() {
            return;
        }
        let title = format!("{} · 日志选区", log.key.id);
        let subtitle = format!(
            "当前筛选快照 · {} · {}；选取期间内容保持不变",
            if log.source == LogSource::Boot {
                "启动日志"
            } else {
                "应用输出"
            },
            if log.truncated {
                "内容已截断"
            } else {
                "最近 500 行以内"
            }
        );
        let selection = cx.new(|cx: &mut Context<LogSelection>| {
            let editor = cx.new(|cx| {
                EditorState::new(window, cx)
                    .line_number(false)
                    .folding(false)
                    .default_value(text)
            });
            let observation = cx.observe(&editor, |this, _, cx| {
                this.copied = false;
                cx.notify();
            });
            LogSelection {
                editor,
                copied: false,
                _observation: observation,
            }
        });
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(title.clone())
                .footer(
                    DialogFooter::new()
                        .child(DialogClose::new().trigger(|button| button.label("完成"))),
                )
                .child(div().text_sm().child(subtitle.clone()))
                .child(selection.clone())
        });
    }
}
