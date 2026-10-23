use super::*;
use gpui_kit::component::checkbox::Checkbox;

pub(super) struct AdvancedForm {
    expanded: bool,
    enabled: bool,
    networks: Entity<InputState>,
    volumes: Entity<InputState>,
    user: Entity<InputState>,
    directory: Entity<InputState>,
    read_only: bool,
}
impl AdvancedForm {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, enabled: bool) -> Self {
        Self {
            expanded: false,
            enabled,
            read_only: false,
            networks: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("网络名称，多个用逗号分隔；留空使用默认网络")
            }),
            volumes: cx.new(|cx| {
                InputState::new(window, cx).placeholder("已有命名卷，例如 database-data:/data:ro")
            }),
            user: cx.new(|cx| InputState::new(window, cx).placeholder("用户，例如 1000:1000")),
            directory: cx
                .new(|cx| InputState::new(window, cx).placeholder("容器工作目录，例如 /app")),
        }
    }
    pub fn options(&self, cx: &App) -> Result<CreateOptions, String> {
        let optional = |input: &Entity<InputState>| {
            let value = input.read(cx).value().trim().to_owned();
            (!value.is_empty()).then_some(value)
        };
        let volumes = self
            .volumes
            .read(cx)
            .value()
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| {
                let parts: Vec<_> = s.trim().split(':').collect();
                if !(2..=3).contains(&parts.len())
                    || (parts.len() == 3 && !["ro", "rw"].contains(&parts[2]))
                {
                    return Err("命名卷格式：卷名:/容器目录:ro，多个用逗号分隔".to_string());
                }
                Ok(VolumeMount {
                    name: parts[0].into(),
                    destination: parts[1].into(),
                    read_only: parts.get(2) == Some(&"ro"),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let options = CreateOptions {
            start_after_create: false,
            startup: None,
            secret_environment: vec![],
            networks: self
                .networks
                .read(cx)
                .value()
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
            volumes,
            user: optional(&self.user),
            working_directory: optional(&self.directory),
            read_only: self.read_only,
        };
        options.validate().map_err(|e| e.message)?;
        Ok(options)
    }
}
impl Render for AdvancedForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::get(cx.theme().is_dark());
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                create::disclosure(
                    "create-advanced",
                    "高级选项",
                    if self.expanded {
                        Glyph::ChevronDown
                    } else {
                        Glyph::ChevronRight
                    },
                )
                .disabled(!self.enabled)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.expanded = !this.expanded;
                    cx.notify();
                })),
            )
            .when(self.expanded, |d| {
                d.child(create::field(
                    "网络",
                    Input::new(&self.networks).id("create-networks"),
                    p,
                ))
                .child(create::field(
                    "命名卷挂载",
                    Input::new(&self.volumes).id("create-volumes"),
                    p,
                ))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(div().flex_1().child(create::field(
                            "用户",
                            Input::new(&self.user).id("create-user"),
                            p,
                        )))
                        .child(div().flex_1().child(create::field(
                            "工作目录",
                            Input::new(&self.directory).id("create-directory"),
                            p,
                        ))),
                )
                .child(
                    Checkbox::new("create-read-only")
                        .label("只读根文件系统")
                        .checked(self.read_only)
                        .on_click(cx.listener(|this, checked, _, cx| {
                            this.read_only = *checked;
                            cx.notify();
                        })),
                )
            })
    }
}
