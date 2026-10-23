use super::*;
use gpui_kit::component::checkbox::Checkbox;

pub(super) struct LaunchForm {
    pub enabled: bool,
    pub supported: bool,
}
impl Render for LaunchForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                Checkbox::new("create-and-start")
                    .label("创建后立即启动")
                    .checked(self.enabled)
                    .disabled(!self.supported)
                    .on_click(cx.listener(|this, checked, _, cx| {
                        this.enabled = *checked;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(if !self.supported {
                        "当前环境未提供启动权限或能力，可先创建停止的容器。"
                    } else if self.enabled {
                        "启动失败时保留容器，可在任务中心单独重试启动。"
                    } else {
                        "创建后保持停止，可稍后从容器列表启动。"
                    }),
            )
    }
}
