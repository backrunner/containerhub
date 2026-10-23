mod actions;
mod details;
mod instances;
mod rows;
mod swarm;
use crate::*;
use std::collections::{HashMap, HashSet};

pub(super) struct StackView {
    pub selected: Option<String>,
    pub search: Entity<InputState>,
    pub query: String,
    pub kind: Option<StackKind>,
    pub attention_only: bool,
    pub checked: HashSet<String>,
    pub expanded: HashSet<String>,
    pub container_pages: HashMap<String, usize>,
}
impl StackView {
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            selected: None,
            query: String::new(),
            kind: None,
            attention_only: false,
            checked: HashSet::new(),
            expanded: HashSet::new(),
            container_pages: HashMap::new(),
            search: cx.new(|cx| InputState::new(window, cx).placeholder("搜索 Stack、服务或镜像…")),
        }
    }
    pub fn token(stack: &Stack, service: &StackService) -> String {
        format!(
            "{}\0{}\0{}",
            stack.environment,
            stack.identity(),
            service.name
        )
    }
    pub fn checked_names(&self, stack: &Stack) -> Vec<String> {
        stack
            .services
            .iter()
            .filter(|s| self.checked.contains(&Self::token(stack, s)))
            .map(|s| s.name.clone())
            .collect()
    }
    pub fn service_visible(&self, stack: &Stack, service: &StackService) -> bool {
        (!self.attention_only || service.status().needs_attention())
            && (stack
                .name
                .to_lowercase()
                .contains(&self.query.trim().to_lowercase())
                || service.matches(&self.query))
    }
}

fn status_color(status: StackServiceStatus, p: Palette) -> Hsla {
    match status {
        StackServiceStatus::Unhealthy | StackServiceStatus::UpdatePaused => p.danger,
        StackServiceStatus::Running => p.success,
        s if s.needs_attention() || s == StackServiceStatus::Updating => p.warning,
        _ => p.muted,
    }
}
fn status_label(status: StackServiceStatus, p: Palette) -> impl IntoElement {
    let color = status_color(status, p);
    div()
        .flex()
        .items_center()
        .gap(px(5.))
        .text_size(px(11.))
        .text_color(color)
        .child(div().size(px(5.)).rounded_full().bg(color))
        .child(status.label())
}
impl Workspace {
    pub(super) fn runtime_label(&self) -> &'static str {
        if self
            .snapshot
            .as_ref()
            .is_some_and(|s| s.capabilities.stacks)
        {
            "Docker Engine"
        } else {
            "Apple container"
        }
    }
    pub(super) fn stack_page(
        &self,
        layout: Layout,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let stacks = self
            .snapshot
            .as_ref()
            .map(Stack::from_snapshot)
            .unwrap_or_default();
        let query = self.stack.query.trim().to_lowercase();
        let items = stacks
            .iter()
            .filter(|s| {
                self.stack.kind.is_none_or(|kind| s.kind == kind)
                    && (s.name.to_lowercase().contains(&query)
                        || s.services.iter().any(|s| s.matches(&query)))
            })
            .collect::<Vec<_>>();
        let selected = self
            .stack
            .selected
            .as_ref()
            .and_then(|name| items.iter().copied().find(|s| &s.identity() == name))
            .or_else(|| items.first().copied());
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        Input::new(&self.stack.search)
                            .id("stack-search")
                            .small()
                            .min_h(px(32.))
                            .w(px(if layout.compact { 226. } else { 280. }))
                            .prefix(Icon::new(Glyph::Search).size(px(14.))),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(2.))
                            .p(px(3.))
                            .bg(p.canvas)
                            .border_1()
                            .border_color(p.border)
                            .rounded(px(6.))
                            .children(
                                [
                                    (None, "全部"),
                                    (Some(StackKind::Compose), "Compose"),
                                    (Some(StackKind::Swarm), "Swarm"),
                                ]
                                .into_iter()
                                .enumerate()
                                .map(|(i, (kind, label))| {
                                    Button::new(("stack-kind", i))
                                        .ghost()
                                        .dense()
                                        .h(px(26.))
                                        .label(label)
                                        .when(self.stack.kind == kind, |b| {
                                            b.bg(p.surface)
                                                .text_color(p.accent)
                                                .border_1()
                                                .border_color(p.border)
                                        })
                                        .on_click(cx.listener(move |v, _, _, cx| {
                                            v.stack.kind = kind;
                                            v.stack.checked.clear();
                                            cx.notify();
                                        }))
                                }),
                            ),
                    ),
            )
            .when_some(
                self.snapshot.as_ref().and_then(|s| match s.swarm.status {
                    SwarmStatus::Worker => Some("Swarm 需要连接 manager 节点"),
                    SwarmStatus::Unavailable => Some("Swarm 服务暂不可读取，请刷新"),
                    _ => None,
                }),
                |d, note| d.child(div().text_size(px(11.)).text_color(p.warning).child(note)),
            )
            .when(items.is_empty(), |d| {
                d.child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(12.))
                        .border_1()
                        .border_color(p.border)
                        .rounded(px(7.))
                        .text_color(p.muted)
                        .child(Icon::new(Glyph::Layers).size(px(28.)))
                        .child(if query.is_empty() && self.stack.kind.is_none() {
                            "暂无 Stack"
                        } else {
                            "没有匹配的 Stack"
                        }),
                )
            })
            .when_some(selected, |d, stack| {
                d.child(
                    observed!(div().id("stack-columns"))
                        .flex()
                        .flex_1()
                        .min_h_0()
                        .gap(px(12.))
                        .child(
                            observed!(div().id("stack-list"))
                                .w(px(if layout.compact { 164. } else { 208. }))
                                .flex_shrink_0()
                                .min_h_0()
                                .flex()
                                .flex_col()
                                .border_1()
                                .border_color(p.border)
                                .rounded(px(7.))
                                .overflow_hidden()
                                .child(
                                    div()
                                        .h(px(36.))
                                        .flex_shrink_0()
                                        .px(px(12.))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .border_b_1()
                                        .border_color(p.border)
                                        .text_size(px(10.))
                                        .text_color(p.muted)
                                        .child("STACKS")
                                        .child(items.len().to_string()),
                                )
                                .child(
                                    div()
                                        .id("stack-rail-scroll")
                                        .flex_1()
                                        .min_h_0()
                                        .overflow_y_scroll()
                                        .p(px(5.))
                                        .children(items.iter().enumerate().map(|(i, s)| {
                                            let identity = s.identity();
                                            let attention = s
                                                .services
                                                .iter()
                                                .filter(|s| s.status().needs_attention())
                                                .count();
                                            let running = s
                                                .services
                                                .iter()
                                                .filter(|s| s.running_count() > 0)
                                                .count();
                                            observed!(div().id(("stack-row", i)))
                                                .h(px(56.))
                                                .flex_shrink_0()
                                                .px(px(10.))
                                                .mb(px(2.))
                                                .flex()
                                                .flex_col()
                                                .justify_center()
                                                .gap(px(4.))
                                                .rounded(px(5.))
                                                .cursor_pointer()
                                                .when(stack.identity() == identity, |d| {
                                                    d.bg(p.selected)
                                                })
                                                .hover(|d| d.bg(p.selected))
                                                .child(
                                                    div()
                                                        .h(px(18.))
                                                        .flex_shrink_0()
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(8.))
                                                        .child(
                                                            Icon::new(Glyph::Layers)
                                                                .size(px(15.))
                                                                .text_color(
                                                                    if stack.identity() == identity
                                                                    {
                                                                        p.accent
                                                                    } else {
                                                                        p.muted
                                                                    },
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .min_w_0()
                                                                .text_size(px(13.))
                                                                .line_height(px(18.))
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .truncate()
                                                                .child(s.name.clone()),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_shrink_0()
                                                                .px(px(4.))
                                                                .text_size(px(10.))
                                                                .line_height(px(14.))
                                                                .text_color(p.muted)
                                                                .rounded(px(3.))
                                                                .border_1()
                                                                .border_color(p.border)
                                                                .child(s.kind.label()),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .h(px(14.))
                                                        .flex_shrink_0()
                                                        .pl(px(23.))
                                                        .text_size(px(10.))
                                                        .line_height(px(14.))
                                                        .truncate()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .gap(px(8.))
                                                        .text_color(p.muted)
                                                        .child(format!(
                                                            "{} 个服务",
                                                            s.services.len()
                                                        ))
                                                        .child(
                                                            div()
                                                                .text_color(if attention > 0 {
                                                                    p.warning
                                                                } else {
                                                                    p.muted
                                                                })
                                                                .child(if attention > 0 {
                                                                    format!("{attention} 需关注")
                                                                } else {
                                                                    format!("{running} 运行中")
                                                                }),
                                                        ),
                                                )
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(move |v, _, _, cx| {
                                                        v.stack.selected = Some(identity.clone());
                                                        v.stack.checked.clear();
                                                        v.stack.attention_only = false;
                                                        cx.notify();
                                                    }),
                                                )
                                        })),
                                ),
                        )
                        .child(self.stack_detail(stack, layout, p, cx)),
                )
            })
    }
}
