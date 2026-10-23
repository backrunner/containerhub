use super::*;

struct Entry {
    id: usize,
    name: Entity<InputState>,
    value: Entity<InputState>,
}
pub(super) struct SecretsForm {
    enabled: bool,
    entries: Vec<Entry>,
    next: usize,
}
impl SecretsForm {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            entries: vec![],
            next: 0,
        }
    }
    pub fn input(&self, cx: &App) -> Result<SecretEnvironment, String> {
        let mut input = SecretEnvironment::default();
        for entry in &self.entries {
            input
                .insert(
                    entry.name.read(cx).value().to_string(),
                    entry.value.read(cx).value().to_string(),
                )
                .map_err(|e| e.message)?;
        }
        Ok(input)
    }
}
impl Render for SecretsForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                create::disclosure("add-secret", "添加秘密环境变量", Glyph::Plus)
                    .disabled(!self.enabled || self.entries.len() >= 32)
                    .on_click(cx.listener(|this, _, window, cx| {
                        let id = this.next;
                        this.next += 1;
                        this.entries.push(Entry {
                            id,
                            name: cx.new(|cx| {
                                InputState::new(window, cx).placeholder("变量名，例如 API_TOKEN")
                            }),
                            value: cx.new(|cx| {
                                InputState::new(window, cx)
                                    .placeholder("单行值，仅用于本次创建")
                                    .masked(true)
                            }),
                        });
                        cx.notify();
                    })),
            )
            .when(!self.entries.is_empty(), |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("值不进入任务记录；容器及运行时仍能读取。关闭表单后需重新填写。"),
                )
            })
            .children(self.entries.iter().map(|entry| {
                let id = entry.id;
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div().flex_1().min_w_0().child(
                                    Input::new(&entry.name)
                                        .small()
                                        .min_h(px(34.))
                                        .text_size(px(12.))
                                        .id(("secret-name", id)),
                                ),
                            )
                            .child(
                                Button::new(("remove-secret", id))
                                    .ghost()
                                    .dense()
                                    .label("移除")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.entries.retain(|e| e.id != id);
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        Input::new(&entry.value)
                            .small()
                            .min_h(px(34.))
                            .text_size(px(12.))
                            .id(("secret-value", id)),
                    )
            }))
    }
}
