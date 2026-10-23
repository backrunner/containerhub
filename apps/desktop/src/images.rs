use crate::*;
mod batch;
mod list;

#[derive(Default)]
pub(super) struct ImageView {
    pub generation: u64,
    pub reference: Option<String>,
    pub result: Option<Result<ImageDetails, String>>,
    pub selected: std::collections::HashSet<String>,
    pub batch_generation: u64,
    pub batch_loading: bool,
}
impl Workspace {
    pub(super) fn close_image(&mut self) {
        self.image.generation += 1;
        self.image.reference = None;
        self.image.result = None;
    }
    pub(super) fn request_image(&mut self, reference: String) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        self.image.generation += 1;
        self.image.reference = Some(reference.clone());
        self.image.result = None;
        if !self.backend.send(Request::InspectImage {
            environment: snapshot.environment.clone(),
            reference,
            generation: self.image.generation,
        }) {
            self.image.result = Some(Err("请求队列已满，请刷新后重试".into()));
        }
    }
    pub(super) fn image_panel(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .p_4()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .mb_4()
                    .child(
                        Button::new("close-image")
                            .ghost()
                            .dense()
                            .icon(Glyph::ArrowLeft)
                            .label("镜像列表")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.close_image();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(self.image.reference.clone().unwrap_or_default()),
                    ),
            )
            .child(match &self.image.result {
                None => empty("正在核对镜像与容器引用…", p).into_any_element(),
                Some(Err(error)) => empty(error.clone(), p).into_any_element(),
                Some(Ok(image)) => {
                    let command = Command::DeleteImage {
                        key: image.key.clone(),
                    };
                    let busy = self.operations.iter().any(|op| {
                        op.state.is_active() && op.command.lock_key() == command.lock_key()
                    });
                    let frozen = image.clone();
                    let copy = serde_json::to_string_pretty(image).unwrap_or_default();
                    div()
                        .min_w_0()
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .mb_4()
                                .child(
                                    Button::new("image-copy")
                                        .dense()
                                        .label("复制详情")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                copy.clone(),
                                            ));
                                            this.notice =
                                                Some("已复制镜像详情（不包含环境变量值）。".into());
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("delete-image")
                                        .dense()
                                        .label("删除镜像")
                                        .disabled(
                                            !self.supports(&command)
                                                || busy
                                                || !image.used_by.is_empty(),
                                        )
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.delete_image_dialog(frozen.clone(), window, cx)
                                        })),
                                ),
                        )
                        .child(div().text_xs().text_color(p.muted).child(format!(
                            "引用信息：{} 秒前核对",
                            now().saturating_sub(image.observed_at)
                        )))
                        .child(
                            div()
                                .my_3()
                                .text_sm()
                                .truncate()
                                .child(format!("Digest：{}", image.key.digest)),
                        )
                        .child(div().mb_3().text_sm().child(format!(
                            "创建时间：{}",
                            image.created_at.as_deref().unwrap_or("未知")
                        )))
                        .child(div().mb_3().text_sm().child(format!(
                            "容器引用：{}",
                            if image.used_by.is_empty() {
                                "无；执行前再次检查".into()
                            } else {
                                image
                                    .used_by
                                    .iter()
                                    .map(|k| k.id.clone())
                                    .collect::<Vec<_>>()
                                    .join("、")
                            }
                        )))
                        .child(div().mb_3().text_sm().child(format!(
                            "同内容的其他标签：{}",
                            if image.other_references.is_empty() {
                                "无".into()
                            } else {
                                image.other_references.join("、")
                            }
                        )))
                        .children(image.variants.iter().map(|v| {
                            div()
                                .mb_4()
                                .p_3()
                                .rounded_md()
                                .bg(p.canvas)
                                .child(div().text_sm().child(format!(
                                    "{} · {}",
                                    v.platform,
                                    v.size_bytes.map(format_bytes).unwrap_or("大小未知".into())
                                )))
                                .child(div().mt_2().text_xs().truncate().child(v.digest.clone()))
                                .child(div().mt_2().text_xs().child(format!(
                                    "环境变量名：{}",
                                    join(v.environment_names.clone())
                                )))
                        }))
                        .into_any_element()
                }
            })
    }
    fn delete_image_dialog(
        &mut self,
        image: ImageDetails,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entity = cx.entity().downgrade();
        window.open_alert_dialog(cx,move|dialog,_,_|{
            let key=image.key.clone();let entity=entity.clone();
            dialog.confirm().title("删除这个镜像引用？")
                .child(format!("{}\n{}\n执行前会再次核对 digest 和所有容器（包括已停止容器）的引用。运行时还会清理不再引用的镜像数据；其他标签保留。",key.reference,key.digest))
                .button_props(DialogButtonProps::default().ok_text("删除镜像").cancel_text("取消"))
                .on_ok(move|_,_,cx|{let _=entity.update(cx,|this,cx|this.submit(Command::DeleteImage{key:key.clone()},cx));true})
        });
    }
}
