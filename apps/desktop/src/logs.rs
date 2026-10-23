use super::*;
use backend::LogRequest;
use containerhub_core::{LogSnapshot, LogSource, ManagementError};

#[derive(Clone, Copy, PartialEq)]
pub enum DetailTab {
    Overview,
    Inspect,
    Logs,
}

pub struct LogView {
    pub search: Entity<InputState>,
    pub query: String,
    pub generation: u64,
    pub source: LogSource,
    pub automatic: bool,
    pub loading: bool,
    pub result: Option<Result<LogSnapshot, String>>,
    scroll: UniformListScrollHandle,
}
impl LogView {
    pub fn new(window: &mut Window, cx: &mut Context<Workspace>) -> Self {
        Self {
            search: cx.new(|cx| InputState::new(window, cx).placeholder("筛选当前日志…")),
            query: String::new(),
            generation: 0,
            source: LogSource::Standard,
            automatic: true,
            loading: false,
            result: None,
            scroll: UniformListScrollHandle::new(),
        }
    }
    pub fn receive(
        &mut self,
        generation: u64,
        result: Result<LogSnapshot, ManagementError>,
        selected: Option<&ResourceKey>,
    ) {
        if generation != self.generation {
            return;
        }
        if let Ok(ref log) = result
            && (Some(&log.key) != selected || log.source != self.source)
        {
            return;
        }
        self.loading = false;
        self.result = Some(result.map_err(|e| e.message));
        if self.automatic {
            self.scroll.scroll_to_bottom();
        }
    }
    pub(super) fn filtered(&self) -> Vec<(usize, String)> {
        let query = self.query.trim().to_lowercase();
        match &self.result {
            Some(Ok(log)) => log
                .lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.to_lowercase().contains(&query))
                .map(|(i, line)| (i + 1, line.clone()))
                .collect(),
            _ => vec![],
        }
    }
}
impl Workspace {
    pub fn close_details(&mut self) {
        self.inspector_closing = None;
        self.selected = None;
        self.inspect.generation += 1;
        self.inspect.result = None;
        self.inspect.loading = false;
        self.request_stats();
        self.logs.generation += 1;
        self.logs.result = None;
        self.logs.loading = false;
        self.backend.logs.send_replace(None);
    }
    pub(super) fn select_container(&mut self, key: ResourceKey) {
        // A new selection, including the same row, cancels an in-flight close.
        self.inspector_closing = None;
        if self.selected.as_ref() != Some(&key) {
            self.close_details();
            self.selected = Some(key);
            self.request_stats();
            self.detail_tab = DetailTab::Overview;
        }
    }
    pub(super) fn request_logs(&mut self, clear: bool) {
        self.logs.generation += 1;
        self.logs.loading = false;
        if clear {
            self.logs.result = None;
        }
        if self.is_active
            && self.page == Page::Containers
            && self.detail_tab == DetailTab::Logs
            && let Some(key) = &self.selected
        {
            self.logs.loading = true;
            self.backend.logs.send_replace(Some(LogRequest {
                generation: self.logs.generation,
                key: key.clone(),
                source: self.logs.source,
                automatic: self.logs.automatic,
            }));
        } else {
            self.backend.logs.send_replace(None);
        }
    }
    pub(super) fn log_panel(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let lines = self.logs.filtered();
        let count = lines.len();
        // Reserve room for monospace text and the line-number gutter; the viewport
        // scrolls horizontally instead of silently hiding long application messages.
        let text_width = lines
            .iter()
            .map(|(_, line)| {
                line.chars()
                    .map(|c| if c.is_ascii() { 8 } else { 16 })
                    .sum::<usize>()
            })
            .max()
            .unwrap_or(0);
        let content_width = (text_width + 76).max(300) as f32;
        let total = match &self.logs.result {
            Some(Ok(log)) => log.lines.len(),
            _ => 0,
        };
        let subtitle = match &self.logs.result {
            Some(Ok(log)) => format!(
                "{count}/{total} 行 · 最近 500 行{}",
                if log.truncated {
                    " · 内容已截断"
                } else {
                    ""
                }
            ),
            _ => "最近 500 行 · 最多 256 KiB".into(),
        };
        div()
            .id("log-panel")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .child(
                div()
                    .px_3()
                    .py_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div().flex().gap_1().children(
                            [
                                (LogSource::Standard, "应用输出"),
                                (LogSource::Boot, "启动日志"),
                            ]
                            .into_iter()
                            .enumerate()
                            .map(|(i, (source, label))| {
                                Button::new(("log-source", i))
                                    .ghost()
                                    .dense()
                                    .label(label)
                                    .selected(self.logs.source == source)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.logs.source = source;
                                        this.request_logs(true);
                                        cx.notify();
                                    }))
                            }),
                        ),
                    )
                    .child(
                        Button::new("log-select-text")
                            .ghost()
                            .dense()
                            .label("选择日志片段")
                            .disabled(count == 0)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.select_log_text(window, cx)),
                            ),
                    )
                    .child(
                        Input::new(&self.logs.search)
                            .small()
                            .prefix(Icon::new(Glyph::Search).small()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                Button::new("log-automatic")
                                    .ghost()
                                    .dense()
                                    .label(if self.logs.automatic {
                                        "暂停自动刷新"
                                    } else {
                                        "恢复自动刷新"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.logs.automatic = !this.logs.automatic;
                                        if this.logs.automatic {
                                            this.request_logs(false);
                                        } else {
                                            this.logs.generation += 1;
                                            this.logs.loading = false;
                                            this.backend.logs.send_replace(None);
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(
                                        Button::new("log-refresh")
                                            .ghost()
                                            .square(28.)
                                            .icon(Glyph::RefreshCw)
                                            .tooltip("刷新日志")
                                            .accessibility_label("刷新日志")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.request_logs(false);
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("log-copy")
                                            .ghost()
                                            .square(28.)
                                            .icon(Glyph::Copy)
                                            .tooltip("复制筛选后的日志")
                                            .accessibility_label("复制筛选后的日志")
                                            .disabled(count == 0)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let text = this
                                                    .logs
                                                    .filtered()
                                                    .into_iter()
                                                    .map(|(_, line)| line)
                                                    .collect::<Vec<_>>()
                                                    .join("\n");
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    text,
                                                ));
                                                this.notice = Some("已复制当前筛选结果。".into());
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("log-export")
                                            .ghost()
                                            .square(28.)
                                            .icon(Glyph::Download)
                                            .tooltip("导出筛选日志到新文件")
                                            .accessibility_label("导出筛选日志到新文件")
                                            .disabled(count == 0)
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.export_logs(cx)),
                                            ),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .px_3()
                    .pb_2()
                    .text_xs()
                    .text_color(p.muted)
                    .child(subtitle),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .border_t_1()
                    .border_color(p.border)
                    .bg(rgb(0x111722))
                    .text_color(rgb(0xd4deef))
                    .when(self.logs.loading && self.logs.result.is_none(), |d| {
                        d.child(empty("正在读取日志…", p))
                    })
                    .when_some(
                        self.logs
                            .result
                            .as_ref()
                            .and_then(|r| r.as_ref().err())
                            .cloned(),
                        |d, e| d.child(empty(e, p)),
                    )
                    .when(matches!(self.logs.result, Some(Ok(_))) && count == 0, |d| {
                        d.child(empty(
                            if total == 0 {
                                "暂无日志输出"
                            } else {
                                "没有匹配的日志"
                            },
                            p,
                        ))
                    })
                    .when(count > 0, |d| {
                        d.child(
                            div()
                                .id("log-horizontal")
                                .overflow_x_scroll()
                                .size_full()
                                .child(
                                    uniform_list(
                                        "log-lines",
                                        count,
                                        move |range: std::ops::Range<usize>, _, _| {
                                            range
                                                .map(|i| {
                                                    let (n, line) = &lines[i];
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .h(px(21.))
                                                        .px_3()
                                                        .gap_3()
                                                        .font_family("Menlo")
                                                        .text_size(px(10.))
                                                        .child(
                                                            div()
                                                                .w(px(28.))
                                                                .flex_shrink_0()
                                                                .text_color(p.muted)
                                                                .child(n.to_string()),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .min_w_0()
                                                                .truncate()
                                                                .child(line.clone()),
                                                        )
                                                })
                                                .collect::<Vec<_>>()
                                        },
                                    )
                                    .track_scroll(&self.logs.scroll)
                                    .w(px(content_width))
                                    .min_w_full()
                                    .h_full(),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(p.muted)
                    .child(if self.logs.loading {
                        "正在刷新…"
                    } else if self.logs.automatic && matches!(self.logs.result, Some(Err(_))) {
                        "读取失败 · 10 秒后重试"
                    } else if self.logs.automatic {
                        "每 2 秒刷新 · 跟随最新输出"
                    } else {
                        "自动刷新已暂停"
                    }),
            )
    }
}
