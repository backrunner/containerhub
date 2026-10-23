use super::*;
const CONTAINERS_PER_PAGE: usize = 4;

impl Workspace {
    pub(super) fn container_overview(
        &self,
        stack: &Stack,
        service: &StackService,
        i: usize,
        layout: Layout,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let items = service.container_overview();
        let total = items.len();
        let pages = total.div_ceil(CONTAINERS_PER_PAGE).max(1);
        let token = StackView::token(stack, service);
        let page = self
            .stack
            .container_pages
            .get(&token)
            .copied()
            .unwrap_or(0)
            .min(pages - 1);
        let prev = token.clone();
        let published = service
            .swarm
            .as_ref()
            .filter(|_| !layout.show_resources)
            .filter(|s| !s.ports.is_empty())
            .map(|s| format!("服务端口 {}", s.ports.join(", ")));
        let incomplete = service.swarm.as_ref().is_some_and(|s| s.tasks_truncated);
        observed!(div().id(("stack-container-overview", i)))
            .bg(p.canvas)
            .pl(px(if layout.compact { 16. } else { 52. }))
            .pr(px(12.))
            .py(px(8.))
            .child(
                div()
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(10.))
                    .text_color(p.muted)
                    .child(format!("{total} 个容器"))
                    .when(incomplete, |d| d.child("采集不完整"))
                    .when_some(published, |d, ports| {
                        d.child(div().min_w_0().truncate().child(ports))
                    })
                    .child(div().flex_1())
                    .when(pages > 1, |d| {
                        d.child(format!("{} / {pages}", page + 1))
                            .child(
                                Button::new(("stack-containers-prev", i))
                                    .square(24.)
                                    .icon(Glyph::ChevronLeft)
                                    .tooltip("上一页容器")
                                    .disabled(page == 0)
                                    .on_click(cx.listener(move |v, _, _, cx| {
                                        v.stack
                                            .container_pages
                                            .insert(prev.clone(), page.saturating_sub(1));
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new(("stack-containers-next", i))
                                    .square(24.)
                                    .icon(Glyph::ChevronRight)
                                    .tooltip("下一页容器")
                                    .disabled(page + 1 >= pages)
                                    .on_click(cx.listener(move |v, _, _, cx| {
                                        v.stack
                                            .container_pages
                                            .insert(token.clone(), (page + 1).min(pages - 1));
                                        cx.notify();
                                    })),
                            )
                    }),
            )
            .when(layout.show_resources && total > 0, |d| {
                d.child(
                    div()
                        .h(px(24.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .text_size(px(10.))
                        .text_color(p.muted)
                        .child(div().flex_1().child("名称 / ID"))
                        .child(div().w(px(72.)).flex_shrink_0().child("状态"))
                        .child(
                            div()
                                .w(px(320.))
                                .flex_shrink_0()
                                .flex()
                                .gap(px(12.))
                                .child(div().w(px(62.)).child("CPU"))
                                .child(div().w(px(134.)).child("内存"))
                                .child(div().flex_1().child("端口")),
                        )
                        .child(div().w(px(56.)).flex_shrink_0()),
                )
            })
            .when(total == 0, |d| {
                d.child(
                    div()
                        .py(px(12.))
                        .text_size(px(11.))
                        .text_color(p.muted)
                        .child("暂无可查看的容器"),
                )
            })
            .children(
                items
                    .into_iter()
                    .skip(page * CONTAINERS_PER_PAGE)
                    .take(CONTAINERS_PER_PAGE)
                    .enumerate()
                    .map(|(j, item)| {
                        self.stack_container_card(item, (i, j), layout.show_resources, p, cx)
                            .into_any_element()
                    }),
            )
    }

    fn stack_container_card(
        &self,
        item: StackContainerOverview<'_>,
        index: (usize, usize),
        wide: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (i, j) = index;
        let c = item.inspected;
        let key = c.map(|c| c.key.clone());
        let sample = c
            .filter(|c| c.state.is_running())
            .and_then(|c| self.usage.sample(&c.key));
        let short_id: String = item.id.chars().take(12).collect();
        let name = c
            .map(|c| c.key.id.clone())
            .unwrap_or_else(|| short_id.clone());
        let node = item
            .node
            .map(|node| format!("节点 {}", node.chars().take(12).collect::<String>()));
        let copy = item.id.to_string();
        let state = c
            .map(|c| c.state.label().to_string())
            .unwrap_or_else(|| "未采集".into());
        let health = c
            .filter(|c| c.state.is_running())
            .map(|c| c.health.label())
            .unwrap_or("—");
        let cpu = sample
            .and_then(|s| s.cpu_percent)
            .filter(|n| n.is_finite() && *n >= 0.)
            .map(|n| format!("{n:.1}%"))
            .unwrap_or_else(|| "—".into());
        let memory = sample
            .and_then(|s| s.memory_bytes)
            .map(format_bytes)
            .unwrap_or_else(|| "—".into());
        let limit = sample
            .and_then(|s| s.memory_limit_bytes)
            .or_else(|| c.and_then(|c| c.memory_limit))
            .filter(|n| *n > 0)
            .map(format_bytes);
        let ports = c
            .filter(|c| !c.ports.is_empty())
            .map(|c| {
                c.ports
                    .iter()
                    .map(Port::display)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_else(|| "—".into());
        let status_color = c
            .map(|c| {
                if c.state.is_running() {
                    p.success
                } else {
                    p.muted
                }
            })
            .unwrap_or(p.muted);
        let metrics = || {
            div()
                .flex()
                .items_start()
                .gap(px(12.))
                .child(
                    overview_metric("CPU", div().child(cpu.clone()), wide, p)
                        .w(px(62.))
                        .flex_shrink_0(),
                )
                .child(
                    overview_metric(
                        "内存",
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(4.))
                            .child(memory.clone())
                            .when_some(limit.clone(), |d, limit| {
                                d.child(
                                    div()
                                        .text_size(px(10.))
                                        .text_color(p.muted)
                                        .child(format!("/ {limit}")),
                                )
                            }),
                        wide,
                        p,
                    )
                    .w(px(134.))
                    .flex_shrink_0(),
                )
                .child(
                    overview_metric("端口", div().truncate().child(ports.clone()), wide, p)
                        .flex_1()
                        .min_w_0(),
                )
        };
        observed!(div().id(SharedString::from(format!("stack-container-{i}-{j}"))))
            .py(px(8.))
            .border_t_1()
            .border_color(p.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_size(px(11.)).truncate().child(name))
                            .child(
                                div()
                                    .mt(px(4.))
                                    .text_size(px(10.))
                                    .text_color(p.muted)
                                    .flex()
                                    .items_center()
                                    .gap(px(12.))
                                    .when(c.is_some(), |d| {
                                        d.child(div().min_w_0().truncate().child(short_id))
                                    })
                                    .when_some(node, |d, node| {
                                        d.child(div().min_w_0().truncate().child(node))
                                    })
                                    .when(c.is_none() && item.node.is_none(), |d| {
                                        d.child("节点未提供")
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .w(px(72.))
                            .flex_shrink_0()
                            .text_size(px(10.))
                            .child(div().text_color(status_color).child(state))
                            .child(
                                div()
                                    .mt(px(4.))
                                    .text_color(
                                        if c.is_some_and(|c| c.health == ContainerHealth::Unhealthy)
                                        {
                                            p.danger
                                        } else {
                                            p.muted
                                        },
                                    )
                                    .child(if c.is_none() {
                                        "详情不可用"
                                    } else {
                                        health
                                    }),
                            ),
                    )
                    .when(wide, |d| d.child(metrics().w(px(320.)).flex_shrink_0()))
                    .child(
                        Button::new(SharedString::from(format!("stack-container-copy-{i}-{j}")))
                            .square(24.)
                            .icon(Glyph::Copy)
                            .tooltip("复制容器 ID")
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                            }),
                    )
                    .child(
                        Button::new(SharedString::from(format!("stack-inspect-{i}-{j}")))
                            .square(24.)
                            .icon(Glyph::ArrowUpRight)
                            .tooltip(if c.is_some() {
                                "查看容器详情"
                            } else {
                                "当前连接未采集此容器的详情"
                            })
                            .disabled(c.is_none())
                            .on_click(cx.listener(move |v, _, _, cx| {
                                if let Some(key) = &key {
                                    // Inventory may change while this row is visible.
                                    if v.snapshot
                                        .as_ref()
                                        .is_some_and(|s| s.containers.iter().any(|c| &c.key == key))
                                    {
                                        v.navigate(Page::Containers, cx);
                                        v.select_container(key.clone());
                                        cx.notify();
                                    }
                                }
                            })),
                    ),
            )
            .when(!wide, |d| d.child(metrics().mt(px(10.))))
    }
}
fn overview_metric(label: &'static str, value: impl IntoElement, wide: bool, p: Palette) -> Div {
    div()
        .text_size(px(11.))
        .when(!wide, |d| {
            d.child(div().text_size(px(10.)).text_color(p.muted).child(label))
        })
        .child(div().when(!wide, |d| d.mt(px(4.))).child(value))
}
