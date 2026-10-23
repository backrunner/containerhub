use super::*;
struct PortInput {
    id: usize,
    host: Entity<InputState>,
    host_port: Entity<InputState>,
    container_port: Entity<InputState>,
    protocol: String,
}
pub(super) struct PortEditor {
    rows: Vec<PortInput>,
    next_id: usize,
    pub(super) error: String,
}
impl PortEditor {
    pub(super) fn new(ports: &[Port], window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut editor = Self {
            rows: vec![],
            next_id: 0,
            error: String::new(),
        };
        for port in ports {
            editor.add(port, window, cx);
        }
        editor
    }

    fn add(&mut self, port: &Port, window: &mut Window, cx: &mut Context<Self>) {
        let input = |value: String, label: &str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(label)
                    .default_value(value)
            })
        };
        self.rows.push(PortInput {
            id: self.next_id,
            host: input(port.host.clone(), "主机 IP 地址", window, cx),
            host_port: input(
                if port.host_port == 0 {
                    String::new()
                } else {
                    port.host_port.to_string()
                },
                "主机端口",
                window,
                cx,
            ),
            container_port: input(
                if port.container_port == 0 {
                    String::new()
                } else {
                    port.container_port.to_string()
                },
                "容器端口",
                window,
                cx,
            ),
            protocol: port.protocol.clone(),
        });
        self.next_id += 1;
    }
    pub(super) fn ports(&self, cx: &App) -> Result<Vec<Port>, String> {
        let mut ports = Vec::new();
        for row in &self.rows {
            let port = |input: &Entity<InputState>| {
                input
                    .read(cx)
                    .value()
                    .trim()
                    .parse::<u16>()
                    .map_err(|_| "端口需要填写 1–65535 的整数".to_owned())
            };
            ports.push(Port {
                host: row.host.read(cx).value().trim().to_owned(),
                host_port: port(&row.host_port)?,
                container_port: port(&row.container_port)?,
                protocol: row.protocol.clone(),
            });
        }
        validate_ports(&ports).map_err(|e| e.message)?;
        Ok(ports)
    }
}
impl Render for PortEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::get(cx.theme().is_dark());
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .text_size(px(11.))
                    .text_color(p.muted)
                    .child(div().flex_1().child("主机地址"))
                    .child(div().w(px(88.)).child("主机端口"))
                    .child(div().w(px(88.)).child("容器端口"))
                    .child(div().w(px(72.)).child("协议"))
                    .child(div().w(px(28.))),
            )
            .child(
                observed!(div().id("port-editor-rows"))
                    .max_h((window.viewport_size().height - px(440.)).max(px(80.)))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .children(self.rows.iter().map(|row| {
                        let id = row.id;
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(field(&row.host, ("port-host", id))),
                            )
                            .child(
                                div()
                                    .w(px(88.))
                                    .child(field(&row.host_port, ("port-host-number", id))),
                            )
                            .child(
                                div().w(px(88.)).child(field(
                                    &row.container_port,
                                    ("port-container-number", id),
                                )),
                            )
                            .child(
                                Button::new(("port-protocol", id))
                                    .regular()
                                    .w(px(72.))
                                    .label(row.protocol.to_uppercase())
                                    .dropdown_caret(true)
                                    .dropdown_menu({
                                        let entity = cx.entity().downgrade();
                                        let selected = row.protocol.clone();
                                        move |mut menu, _, _| {
                                            for protocol in ["tcp", "udp"] {
                                                let entity = entity.clone();
                                                menu = menu.item(
                                                    PopupMenuItem::new(protocol.to_uppercase())
                                                        .checked(selected == protocol)
                                                        .on_click(move |_, _, cx| {
                                                            let _ =
                                                                entity.update(cx, |this, cx| {
                                                                    if let Some(row) = this
                                                                        .rows
                                                                        .iter_mut()
                                                                        .find(|r| r.id == id)
                                                                    {
                                                                        row.protocol =
                                                                            protocol.into();
                                                                    }
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
                                Button::new(("remove-port", id))
                                    .square(28.)
                                    .icon(Glyph::Trash)
                                    .accessibility_label(format!("删除第 {} 条端口映射", id + 1))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.rows.retain(|r| r.id != id);
                                        cx.notify();
                                    })),
                            )
                    })),
            )
            .when(self.rows.is_empty(), |d| {
                d.child(
                    div()
                        .text_sm()
                        .text_color(p.muted)
                        .child("没有端口映射。保存后将移除所有已发布端口。"),
                )
            })
            .child(
                div().flex().items_start().child(
                    Button::new("add-port")
                        .dense()
                        .icon(Glyph::Plus)
                        .label("添加映射")
                        .w(px(92.))
                        .disabled(self.rows.len() >= 32)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.add(
                                &Port {
                                    host: "127.0.0.1".into(),
                                    host_port: 0,
                                    container_port: 0,
                                    protocol: "tcp".into(),
                                },
                                window,
                                cx,
                            );
                            cx.notify();
                        })),
                ),
            )
            .when(!self.error.is_empty(), |d| {
                d.child(
                    div()
                        .text_size(px(12.))
                        .text_color(p.danger)
                        .child(self.error.clone()),
                )
            })
    }
}

// Input::h is a multiline-only setter; Styled::h controls the single-line border box.
fn field(state: &Entity<InputState>, id: (&'static str, usize)) -> Input {
    Styled::h(Input::new(state).id(id).small().text_size(px(12.)), px(32.))
}
