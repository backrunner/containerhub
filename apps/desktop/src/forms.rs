mod advanced;
mod create;
mod launch;
mod secrets;
mod startup;
use crate::*;
use gpui_kit::component::dialog::{DialogAction, DialogClose, DialogFooter};

pub(super) fn footer(label: &str) -> DialogFooter {
    DialogFooter::new()
        .gap(px(8.))
        .child(
            div().w(px(64.)).h(px(32.)).child(
                DialogClose::new().trigger(|button| button.regular().w_full().label("取消")),
            ),
        )
        .child(
            div()
                .w(px((label.chars().count() as f32 * 12. + 24.).max(64.)))
                .h(px(32.))
                .child(
                    DialogAction::new().child(
                        Button::new("ok")
                            .regular()
                            .w_full()
                            .primary()
                            .label(label.to_owned()),
                    ),
                ),
        )
}

struct FormError(String);
impl Render for FormError {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .text_sm()
            .text_color(cx.theme().danger)
            .child(self.0.clone())
    }
}
impl Workspace {
    pub(super) fn pull_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(environment) = self
            .snapshot
            .as_ref()
            .filter(|s| self.error.is_none() && s.capabilities.pull_image)
            .map(|s| s.environment.clone())
        else {
            return;
        };
        let reference = cx.new(|cx| {
            InputState::new(window, cx).placeholder("例如 docker.io/library/nginx:alpine")
        });
        let error = cx.new(|_| FormError(String::new()));
        let entity = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let reference = reference.clone();
            let error = error.clone();
            let entity = entity.clone();
            let environment = environment.clone();
            dialog
                .title("拉取镜像")
                .footer(footer("开始拉取"))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child("使用目标主机当前用户的注册表凭据。拉取期间可继续浏览其他容器。")
                        .child(Input::new(&reference).id("pull-image-reference"))
                        .child(error.clone()),
                )
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("开始拉取")
                        .cancel_text("取消"),
                )
                .on_ok(move |_, _, cx| {
                    let command = Command::PullImage {
                        environment: environment.clone(),
                        reference: reference.read(cx).value().trim().to_string(),
                    };
                    if let Err(e) = command.validate() {
                        error.update(cx, |error, cx| {
                            error.0 = e.message;
                            cx.notify();
                        });
                        return false;
                    }
                    let _ = entity.update(cx, |this, cx| this.submit(command, cx));
                    true
                })
        });
    }
    pub(super) fn delete_dialog(
        &mut self,
        key: ResourceKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entity = cx.entity().downgrade();
        window.open_alert_dialog(cx,move|dialog,_,_|{
            let entity=entity.clone();let key=key.clone();
            dialog.confirm().title(format!("删除 {}？",key.id))
                .child("删除已停止的容器及其可写根文件系统，无法撤销。不会请求删除镜像或命名卷；执行前会重新核对容器身份。")
                .button_props(DialogButtonProps::default().ok_text("删除容器").cancel_text("取消"))
                .on_ok(move|_,_,cx|{let _=entity.update(cx,|this,cx|this.submit(Command::DeleteContainer{key:key.clone()},cx));true})
        });
    }
}
fn parse_form(
    name: &str,
    image: &str,
    cpus: &str,
    memory: &str,
    ports: &str,
) -> Result<CreateContainer, String> {
    let parse = |value: &str| {
        value
            .trim()
            .parse::<u32>()
            .map_err(|_| "CPU 和内存需要填写正整数".to_string())
    };
    let mut mappings = vec![];
    for mapping in ports.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (mapping, protocol) = mapping.split_once('/').unwrap_or((mapping, "tcp"));
        let (host, container) = mapping
            .split_once(':')
            .ok_or("端口格式应为主机端口:容器端口")?;
        mappings.push(Port {
            host: "127.0.0.1".into(),
            host_port: host.parse().map_err(|_| "主机端口无效")?,
            container_port: container.parse().map_err(|_| "容器端口无效")?,
            protocol: protocol.into(),
        });
    }
    Ok(CreateContainer {
        options: Default::default(),
        name: name.trim().into(),
        image: image.trim().into(),
        cpus: parse(cpus)?,
        memory_mib: parse(memory)?,
        ports: mappings,
    })
}
