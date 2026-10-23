use super::*;

struct AddEnvironment {
    mode: usize,
    name: Entity<InputState>,
    context: Entity<InputState>,
    binary: Entity<InputState>,
    profile: Entity<InputState>,
    error: Option<String>,
}
impl AddEnvironment {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            mode: 0,
            name: cx.new(|cx| InputState::new(window, cx).placeholder("例如 开发环境")),
            context: cx.new(|cx| InputState::new(window, cx).default_value("default")),
            binary: cx.new(|cx| InputState::new(window, cx).placeholder("自动发现 Docker CLI")),
            profile: cx.new(|cx| InputState::new(window, cx).placeholder("已绑定的控制端配置目录")),
            error: None,
        }
    }
}
impl Render for AddEnvironment {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::get(cx.theme().is_dark());
        let field = |label: &str, input: Input| {
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(div().text_size(px(12.)).child(label.to_owned()))
                .child(input.small().min_h(px(32.)))
        };
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(
                div().flex().gap(px(8.)).children(
                    ["Docker", "远程 Agent"]
                        .into_iter()
                        .enumerate()
                        .map(|(i, label)| {
                            Button::new(("add-environment-mode", i))
                                .regular()
                                .selected(self.mode == i)
                                .label(label)
                                .on_click(cx.listener(move |v, _, _, cx| {
                                    v.mode = i;
                                    v.error = None;
                                    cx.notify();
                                }))
                        }),
                ),
            )
            .when(self.mode == 0, |d| {
                d.child(field(
                    "名称",
                    Input::new(&self.name).id("add-environment-name"),
                ))
                .child(field(
                    "Docker context",
                    Input::new(&self.context).id("add-docker-context"),
                ))
                .child(field(
                    "CLI 路径",
                    Input::new(&self.binary).id("add-docker-binary"),
                ))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(p.muted)
                        .child("使用已有 context 的连接与认证配置。"),
                )
            })
            .when(self.mode == 1, |d| {
                d.child(field(
                    "绑定配置",
                    Input::new(&self.profile).id("add-environment-profile"),
                ))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(p.muted)
                        .child("导入已完成配对的 Agent，保留原绑定与密钥。"),
                )
            })
            .when_some(self.error.clone(), |d, e| {
                d.child(div().text_size(px(12.)).text_color(p.danger).child(e))
            })
    }
}
impl WorkspaceHost {
    pub(super) fn add_form(&self, window: &mut Window, cx: &mut Context<Self>) {
        let form = cx.new(|cx| AddEnvironment::new(window, cx));
        let host = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let form = form.clone();
            let host = host.clone();
            dialog
                .title("添加环境")
                .w(px(460.))
                .footer(forms::footer("添加"))
                .child(form.clone())
                .on_ok(move |_, _, cx| {
                    let f = form.read(cx);
                    let mode = f.mode;
                    let name = f.name.read(cx).value().trim().to_string();
                    let context = f.context.read(cx).value().trim().to_string();
                    let binary = f.binary.read(cx).value().trim().to_string();
                    let directory = PathBuf::from(f.profile.read(cx).value().trim());
                    let result = host
                        .update(cx, |host, cx| {
                            let previous = host.targets.clone();
                            if mode == 0 {
                                anyhow::ensure!(
                                    !name.is_empty() && name.chars().count() <= 60,
                                    "请填写名称，最多 60 个字符"
                                );
                                let target = Target::Docker { context };
                                anyhow::ensure!(
                                    !host.targets.iter().any(|t| t.key() == target.key()),
                                    "该 Docker context 已添加"
                                );
                                anyhow::ensure!(
                                    !host.targets.iter().any(|t| host.label(t) == name),
                                    "环境名称已存在"
                                );
                                let binary = (!binary.is_empty()).then(|| PathBuf::from(binary));
                                if let Target::Docker { context } = &target {
                                    Runtime::docker(context, binary.as_deref())?;
                                }
                                host.remember(target.clone());
                                if let Err(e) = host.save_preference(
                                    &target,
                                    EnvironmentPreferences {
                                        name,
                                        docker_binary: binary,
                                        ..Default::default()
                                    },
                                    cx,
                                ) {
                                    host.targets = previous;
                                    return Err(e);
                                }
                                if let Err(e) = host.save_catalog() {
                                    host.targets = previous;
                                    return Err(e);
                                }
                            } else {
                                anyhow::ensure!(
                                    directory.is_absolute(),
                                    "请选择绑定配置的绝对路径"
                                );
                                let directory = directory.canonicalize()?;
                                let profile = match host.profiles.get(&directory) {
                                    Some(p) => p.clone(),
                                    None => RemoteProfile::open(&directory)?,
                                };
                                let agents = profile.agents();
                                anyhow::ensure!(!agents.is_empty(), "该配置没有已绑定的 Agent");
                                let first = agents.first().cloned().unwrap();
                                for (agent, label) in agents {
                                    host.remember(Target::Remote {
                                        directory: directory.clone(),
                                        agent,
                                        label,
                                    });
                                }
                                if let Err(e) = host.save_catalog() {
                                    host.targets = previous;
                                    return Err(e);
                                }
                                host.profiles.insert(directory.clone(), profile);
                                host.inspect_environment(
                                    Target::Remote {
                                        directory,
                                        agent: first.0,
                                        label: first.1,
                                    },
                                    cx,
                                );
                            }
                            cx.notify();
                            Ok(())
                        })
                        .and_then(|r| r);
                    if let Err(e) = result {
                        form.update(cx, |f, cx| {
                            f.error = Some(e.to_string());
                            cx.notify();
                        });
                        return false;
                    }
                    true
                })
        });
    }
}
#[cfg(feature = "ui-validation")]
pub(crate) fn fixture(mode: usize, window: &mut Window, cx: &mut App) {
    let form = cx.new(|cx| {
        let mut f = AddEnvironment::new(window, cx);
        f.mode = mode;
        f
    });
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title("添加环境")
            .w(px(460.))
            .footer(forms::footer("添加"))
            .child(form.clone())
    });
}
