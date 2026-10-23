use super::*;

struct Argument {
    id: usize,
    value: Entity<InputState>,
}
pub(super) struct StartupForm {
    supported: bool,
    custom: bool,
    entrypoint: Entity<InputState>,
    arguments: Vec<Argument>,
    next: usize,
}
impl StartupForm {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, supported: bool) -> Self {
        Self {
            supported,
            custom: false,
            arguments: vec![],
            next: 0,
            entrypoint: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("启动程序，留空沿用镜像，例如 /usr/bin/python3")
            }),
        }
    }
    pub fn options(&self, cx: &App) -> Result<Option<Box<StartupCommand>>, String> {
        if !self.custom {
            return Ok(None);
        }
        let entrypoint = self.entrypoint.read(cx).value().to_string();
        let startup = StartupCommand {
            entrypoint: (!entrypoint.is_empty()).then_some(entrypoint),
            arguments: self
                .arguments
                .iter()
                .map(|arg| arg.value.read(cx).value().to_string())
                .collect(),
        };
        startup.validate().map_err(|e| e.message)?;
        Ok(Some(Box::new(startup)))
    }
}
impl Render for StartupForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().flex().flex_col().gap_2()
            .child(create::disclosure("create-startup", if self.custom { "恢复镜像默认命令" } else { "自定义启动命令" }, if self.custom { Glyph::ChevronDown } else { Glyph::ChevronRight }).disabled(!self.supported)
                .on_click(cx.listener(|this, _, _, cx| { this.custom = !this.custom; cx.notify(); })))
            .when(self.custom, |d| {
                d.child(Input::new(&self.entrypoint).small().min_h(px(34.)).text_size(px(12.)).id("startup-entrypoint"))
                    .child(div().text_xs().text_color(cx.theme().muted_foreground)
                        .child("指定启动程序会替换镜像的默认程序和参数。每项填写一个参数，空格和空值原样传递，无需加引号。"))
                    .children(self.arguments.iter().enumerate().map(|(index, argument)| {
                        let id = argument.id;
                        div().flex().gap_2().items_center()
                            .child(format!("{}", index + 1))
                            .child(div().flex_1().min_w_0().child(Input::new(&argument.value).small().min_h(px(34.)).text_size(px(12.)).id(("startup-argument", id))))
                            .child(Button::new(("remove-startup-argument", id)).ghost().dense().label("移除")
                                .on_click(cx.listener(move |this, _, _, cx| { this.arguments.retain(|arg| arg.id != id); cx.notify(); })))
                    }))
                    .child(Button::new("add-startup-argument").ghost().dense().label("添加参数")
                        .disabled(self.arguments.len() >= StartupCommand::MAX_ARGUMENTS)
                        .on_click(cx.listener(|this, _, window, cx| {
                            let id = this.next; this.next += 1;
                            this.arguments.push(Argument { id, value: cx.new(|cx| InputState::new(window, cx).placeholder("参数值（可为空）")) });
                            cx.notify();
                        })))
                    .child(div().text_xs().text_color(cx.theme().muted_foreground)
                        .child("启动配置会进入任务记录；敏感值请使用秘密环境变量。"))
            })
    }
}
