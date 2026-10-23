use crate::*;

impl Workspace {
    pub(super) fn export_logs(&mut self, cx: &mut Context<Self>) {
        let Some(Ok(snapshot)) = &self.logs.result else {
            return;
        };
        let export = match snapshot.export(&self.logs.query) {
            Ok(export) => export,
            Err(error) => {
                self.notice = Some(error.message);
                cx.notify();
                return;
            }
        };
        // Freeze before showing the picker: refresh, selection and filters may change while it is open.
        let directory = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let prompt = cx.prompt_for_new_path(&directory, Some(&export.file_name));
        cx.spawn(async move |entity, cx| {
            let notice = match prompt.await {
                Ok(Ok(Some(path))) => {
                    let result =
                        cx.background_executor()
                            .spawn(async move {
                                containerhub_management::export::save_log(&path, &export)
                            })
                            .await;
                    match result {
                        Ok(()) => "已导出当前筛选日志快照。".to_string(),
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                            "目标文件已存在，未覆盖；请重新导出并选择新文件名。".into()
                        }
                        Err(_) => "导出未确认完成，请检查所选目录、磁盘空间和文件状态。".into(),
                    }
                }
                Ok(Ok(None)) => return,
                _ => "无法打开保存窗口，请稍后重试。".into(),
            };
            let _ = entity.update(cx, |this, cx| {
                this.notice = Some(notice);
                cx.notify();
            });
        })
        .detach();
    }
}
