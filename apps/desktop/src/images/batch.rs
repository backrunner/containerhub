use super::*;

#[derive(Clone, Default)]
struct Review {
    eligible: Vec<ImageKey>,
    skipped: Vec<(String, String)>,
}
impl Review {
    fn from_results(
        environment: &str,
        results: Vec<(String, Result<ImageDetails, ManagementError>)>,
    ) -> Self {
        let mut review = Self::default();
        for (reference, result) in results {
            match result {
                Ok(image)
                    if image.key.environment != environment
                        || image.key.reference != reference
                        || image.key.validate().is_err() =>
                {
                    review
                        .skipped
                        .push((reference, "镜像身份已改变，请刷新重试".into()));
                }
                Ok(image) if !image.used_by.is_empty() => {
                    review
                        .skipped
                        .push((reference, format!("被 {} 个容器引用", image.used_by.len())));
                }
                Ok(image) => review.eligible.push(image.key),
                Err(error) => review.skipped.push((reference, error.message)),
            }
        }
        review
    }
}
impl Workspace {
    pub(crate) fn cancel_image_batch(&mut self) {
        self.image.batch_generation += 1;
        self.image.batch_loading = false;
    }
    pub(crate) fn review_image_batch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        if self.error.is_some()
            || !snapshot.capabilities.delete_image
            || !snapshot.capabilities.inspect_image
            || self.image.batch_loading
            || self.image.selected.is_empty()
            || self.image.selected.len() > 32
        {
            return;
        }
        let environment = snapshot.environment.clone();
        let mut references: Vec<_> = self.image.selected.iter().cloned().collect();
        references.sort();
        self.cancel_image_batch();
        let generation = self.image.batch_generation;
        let (reply, receive) = tokio::sync::oneshot::channel();
        if !self.backend.send(Request::InspectImageBatch {
            environment: environment.clone(),
            references,
            reply,
        }) {
            self.notice = Some("读取队列已满，请稍后重试".into());
            cx.notify();
            return;
        }
        self.image.batch_loading = true;
        cx.spawn_in(window, async move |entity, cx| {
            let result = receive.await;
            let _ = entity.update_in(cx, |this, window, cx| {
                if generation != this.image.batch_generation {
                    return;
                }
                this.image.batch_loading = false;
                if !this.is_active
                    || this.page != Page::Resources(ResourceKind::Image)
                    || this
                        .snapshot
                        .as_ref()
                        .is_none_or(|s| s.environment != environment)
                {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(results) => {
                        let mut review = Review::from_results(&environment, results);
                        review.eligible.retain(|key| {
                            if this.operations.iter().any(|o| {
                                o.state.is_active()
                                    && o.command.lock_key()
                                        == (Command::DeleteImage { key: key.clone() }).lock_key()
                            }) {
                                review
                                    .skipped
                                    .push((key.reference.clone(), "已有操作进行中".into()));
                                false
                            } else {
                                true
                            }
                        });
                        this.confirm_image_batch(review, generation, window, cx);
                    }
                    Err(_) => this.notice = Some("读取中断，请重试".into()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn confirm_image_batch(
        &self,
        review: Review,
        generation: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entity = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            let review = review.clone();
            let entity = entity.clone();
            let count = review.eligible.len();
            let p = Palette::get(cx.theme().mode.is_dark());
            dialog
                .confirm()
                .show_cancel(count > 0)
                .w(px(480.))
                .title(if count > 0 {
                    format!("删除 {count} 个镜像？")
                } else {
                    "所选镜像暂时无法删除".into()
                })
                .child(
                    div()
                        .text_size(px(12.))
                        .line_height(relative(1.5))
                        .when(!review.skipped.is_empty(), |d| {
                            d.child(
                                div()
                                    .mb(px(8.))
                                    .text_size(px(11.))
                                    .text_color(p.muted)
                                    .child(format!(
                                        "{} 项可删除 · {} 项跳过",
                                        count,
                                        review.skipped.len()
                                    )),
                            )
                        })
                        .child(
                            observed!(div().id("image-batch-review-scroll"))
                                .max_h(px(240.))
                                .overflow_y_scroll()
                                .children(
                                    review
                                        .eligible
                                        .iter()
                                        .map(|key| div().py(px(6.)).child(key.reference.clone())),
                                )
                                .children(review.skipped.iter().map(|(reference, reason)| {
                                    div().py(px(6.)).child(reference.clone()).child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(p.muted)
                                            .child(format!("跳过 · {reason}")),
                                    )
                                })),
                        )
                        .when(count > 0, |d| {
                            d.child(
                                div()
                                    .mt(px(12.))
                                    .text_size(px(11.))
                                    .text_color(p.muted)
                                    .child("删除前会再次检查容器引用。每项结果可在任务中心查看。"),
                            )
                        }),
                )
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(if count > 0 {
                            "删除镜像"
                        } else {
                            "知道了"
                        })
                        .cancel_text("取消"),
                )
                .on_ok(move |_, _, cx| {
                    let _ = entity.update(cx, |this, cx| {
                        if !this.is_active
                            || this.page != Page::Resources(ResourceKind::Image)
                            || generation != this.image.batch_generation
                        {
                            return;
                        }
                        let mut submitted = 0;
                        for key in &review.eligible {
                            let before = this.operations.len();
                            this.submit(Command::DeleteImage { key: key.clone() }, cx);
                            if this.operations.len() > before {
                                this.image.selected.remove(&key.reference);
                                submitted += 1;
                            }
                        }
                        if submitted > 0 {
                            this.notice = Some(format!("已提交 {submitted} 项删除任务"));
                        }
                        this.cancel_image_batch();
                        cx.notify();
                    });
                    true
                })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::Review;
    use containerhub_core::*;
    fn image(reference: &str) -> ImageDetails {
        ImageDetails {
            key: ImageKey {
                environment: "fixture".into(),
                provider: ProviderKind::Apple,
                reference: reference.into(),
                digest: format!("sha256:{}", "a".repeat(64)),
            },
            observed_at: now(),
            created_at: None,
            media_type: None,
            variants: vec![],
            used_by: vec![],
            other_references: vec![],
        }
    }
    #[test]
    fn review_freezes_digest_and_excludes_used_changed_or_failed_images() {
        let safe = image("alpine:3");
        let mut used = image("redis:7");
        used.used_by.push(ResourceKey {
            environment: "fixture".into(),
            provider: ProviderKind::Apple,
            namespace: None,
            id: "stopped".into(),
            incarnation: "one".into(),
        });
        let review = Review::from_results(
            "fixture",
            vec![
                ("alpine:3".into(), Ok(safe.clone())),
                ("redis:7".into(), Ok(used)),
                ("changed:1".into(), Ok(image("other:1"))),
                (
                    "timeout:1".into(),
                    Err(ManagementError::new(ErrorKind::Timeout, "超时")),
                ),
            ],
        );
        assert_eq!(review.eligible, vec![safe.key]);
        assert_eq!(review.skipped.len(), 3);
        assert!(
            Review::from_results(
                "other-environment",
                vec![("alpine:3".into(), Ok(image("alpine:3")))]
            )
            .eligible
            .is_empty()
        );
    }
}
