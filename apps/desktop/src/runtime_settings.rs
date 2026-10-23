use crate::*;

impl Workspace {
    pub(super) fn refresh_diagnostics(&mut self, cx: &mut Context<Self>) {
        if self.diagnostics_loading {
            return;
        }
        self.diagnostics_loading = self.backend.send(Request::Diagnostics);
        if !self.diagnostics_loading {
            self.notice = Some("诊断队列已满，请稍后重试".into());
        }
        cx.notify();
    }
    pub(super) fn runtime_path_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Apple container 可执行文件的绝对路径")
                .default_value(
                    self.settings
                        .apple_binary
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                )
        });
        let entity = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let path = path.clone();
            let entity = entity.clone();
            dialog
                .title("指定 Apple CLI")
                .footer(forms::footer("检查并保存"))
                .child("检查通过后保存，下次启动生效。")
                .child(Input::new(&path).id("runtime-path-input"))
                .on_ok(move |_, _, cx| {
                    let binary = PathBuf::from(path.read(cx).value().trim());
                    let _ = entity.update(cx, |this, cx| {
                        let mut settings = this.settings.clone();
                        settings.apple_binary = Some(binary);
                        this.notice = Some("正在检查 CLI 路径…".into());
                        this.send(Request::ConfigureRuntime(settings), cx);
                    });
                    true
                })
        });
    }
}
