use super::*;
use containerhub_runtime::Endpoint;

struct EditEnvironment {
    name: Entity<InputState>,
    notes: Entity<InputState>,
    direct: Entity<InputState>,
    relay: Entity<InputState>,
    binary: Entity<InputState>,
    mode: usize,
    interval: u64,
    remote: bool,
    local: bool,
    docker: bool,
    context: String,
    error: Option<String>,
}
impl Render for EditEnvironment {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::get(cx.theme().is_dark());
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .text_size(px(12.))
            .child(field(
                "名称",
                Input::new(&self.name)
                    .small()
                    .min_h(px(32.))
                    .text_size(px(12.))
                    .id("environment-name"),
            ))
            .child(field(
                "备注",
                Input::new(&self.notes)
                    .small()
                    .min_h(px(32.))
                    .text_size(px(12.))
                    .id("environment-notes"),
            ))
            .when(self.remote, |d| {
                d.child(field(
                    "连接策略",
                    div().flex().gap(px(8.)).children(
                        ["直连", "中继", "自动"]
                            .into_iter()
                            .enumerate()
                            .map(|(i, label)| {
                                Button::new(("environment-mode", i))
                                    .regular()
                                    .selected(self.mode == i)
                                    .label(label)
                                    .on_click(cx.listener(move |v, _, _, cx| {
                                        v.mode = i;
                                        cx.notify();
                                    }))
                            }),
                    ),
                ))
                .when(self.mode != 1, |d| {
                    d.child(field(
                        "直连地址",
                        Input::new(&self.direct)
                            .small()
                            .min_h(px(32.))
                            .text_size(px(12.))
                            .id("environment-direct"),
                    ))
                })
                .when(self.mode != 0, |d| {
                    d.child(field(
                        "中继地址",
                        Input::new(&self.relay)
                            .small()
                            .min_h(px(32.))
                            .text_size(px(12.))
                            .id("environment-relay"),
                    ))
                })
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(p.muted)
                        .child("保存后重新验证原 Agent 身份，绑定保持不变。"),
                )
            })
            .when(self.docker, |d| {
                d.child(field(
                    "Docker context",
                    div().text_color(p.muted).child(self.context.clone()),
                ))
            })
            .when(self.local || self.docker, |d| {
                d.child(field(
                    "CLI 路径",
                    Input::new(&self.binary)
                        .small()
                        .min_h(px(32.))
                        .text_size(px(12.))
                        .id("environment-binary"),
                ))
            })
            .child(field(
                "采样间隔",
                div()
                    .flex()
                    .gap(px(8.))
                    .children([2, 5, 10].into_iter().map(|seconds| {
                        Button::new(("environment-interval", seconds as usize))
                            .regular()
                            .selected(self.interval == seconds)
                            .label(format!("{seconds} 秒"))
                            .on_click(cx.listener(move |v, _, _, cx| {
                                v.interval = seconds;
                                cx.notify();
                            }))
                    })),
            ))
            .when_some(self.error.clone(), |d, e| {
                d.child(div().text_color(p.danger).child(e))
            })
    }
}
fn field(label: &str, element: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .child(label.to_owned()),
        )
        .child(element)
}

#[cfg(feature = "ui-validation")]
pub(crate) fn edit_form_fixture(docker: bool, window: &mut Window, cx: &mut App) {
    // The production form component, with public fixtures and no profile/key access.
    let form = cx.new(|cx| EditEnvironment {
        name: cx.new(|cx| {
            InputState::new(window, cx).default_value(if docker {
                "Docker 开发"
            } else {
                "开发工作站"
            })
        }),
        notes: cx.new(|cx| InputState::new(window, cx).default_value("")),
        direct: cx.new(|cx| InputState::new(window, cx).default_value("wss://studio.example:7443")),
        relay: cx.new(|cx| InputState::new(window, cx).default_value("wss://relay.example")),
        binary: cx.new(|cx| InputState::new(window, cx).placeholder("留空以自动发现")),
        mode: 2,
        interval: 2,
        remote: !docker,
        local: false,
        docker,
        context: "desktop-linux".into(),
        error: None,
    });
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title("编辑环境")
            .w(px(460.))
            .footer(forms::footer("保存"))
            .child(form.clone())
    });
}
impl WorkspaceHost {
    pub(crate) fn activate_environment(
        &mut self,
        target: Target,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate(target, window, cx);
    }
    pub(crate) fn edit_environment(
        &mut self,
        target: Target,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Editing must remain available when the current CLI path or endpoint is broken.
        if let Target::Remote { directory, .. } = &target
            && !self.profiles.contains_key(directory)
        {
            match RemoteProfile::open(directory) {
                Ok(profile) => {
                    self.profiles.insert(directory.clone(), profile);
                }
                Err(error) => {
                    self.notice(error.to_string(), cx);
                    return;
                }
            }
        }
        let pref = self.preference(&target);
        let endpoint = self.connection(&target).ok().flatten().map(|c| c.endpoint);
        let (mode, direct, relay) = match endpoint {
            Some(Endpoint::Direct { url }) => (0, url, String::new()),
            Some(Endpoint::Relay { url, .. }) => (1, String::new(), url),
            Some(Endpoint::Auto { direct, relay, .. }) => (2, direct, relay),
            None => (0, String::new(), String::new()),
        };
        let label = self.label(&target);
        let form = cx.new(|cx| EditEnvironment {
            name: cx.new(|cx| InputState::new(window, cx).default_value(label)),
            notes: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(pref.notes)
                    .placeholder("可选")
            }),
            direct: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(direct)
                    .placeholder("wss://host.example:7443")
            }),
            relay: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(relay)
                    .placeholder("wss://relay.example")
            }),
            binary: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(
                        if matches!(target, Target::Docker { .. }) {
                            pref.docker_binary
                        } else {
                            pref.apple_binary
                        }
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                    )
                    .placeholder("留空以自动发现")
            }),
            mode,
            interval: pref.interval,
            remote: matches!(target, Target::Remote { .. }),
            local: matches!(target, Target::Local),
            docker: matches!(target, Target::Docker { .. }),
            context: if let Target::Docker { context } = &target {
                context.clone()
            } else {
                String::new()
            },
            error: None,
        });
        let host = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let form = form.clone();
            let host = host.clone();
            let target = target.clone();
            dialog
                .title("编辑环境")
                .w(px(460.))
                .footer(forms::footer("保存"))
                .child(form.clone())
                .on_ok(move |_, _, cx| {
                    let f = form.read(cx);
                    let endpoint = if let Target::Remote { agent, .. } = &target {
                        Some(match f.mode {
                            0 => Endpoint::Direct {
                                url: f.direct.read(cx).value().trim().into(),
                            },
                            1 => Endpoint::Relay {
                                url: f.relay.read(cx).value().trim().into(),
                                route: agent.clone(),
                            },
                            _ => Endpoint::Auto {
                                direct: f.direct.read(cx).value().trim().into(),
                                relay: f.relay.read(cx).value().trim().into(),
                                route: agent.clone(),
                            },
                        })
                    } else {
                        None
                    };
                    let binary = f.binary.read(cx).value().trim().to_string();
                    let pref = EnvironmentPreferences {
                        name: f.name.read(cx).value().trim().into(),
                        notes: f.notes.read(cx).value().trim().into(),
                        interval: f.interval,
                        endpoint,
                        apple_binary: (f.local && !binary.is_empty())
                            .then(|| PathBuf::from(&binary)),
                        docker_binary: (f.docker && !binary.is_empty())
                            .then(|| PathBuf::from(&binary)),
                    };
                    let result = host
                        .update(cx, |host, cx| host.save_preference(&target, pref, cx))
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
    pub(super) fn remove_environment(
        &self,
        target: Target,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(target, Target::Remote { .. } | Target::Docker { .. }) {
            return;
        }
        let host = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let host = host.clone();
            let target = target.clone();
            dialog
                .confirm()
                .title(format!("移除 {}？", target.label()))
                .child(if matches!(target, Target::Docker { .. }) {
                    "仅从此设备移除环境，容器继续运行。"
                } else {
                    "仅从此设备移除环境，容器继续运行，Agent 绑定保持不变。"
                })
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("移除")
                        .cancel_text("取消"),
                )
                .on_ok(move |_, window, cx| {
                    let _ = host.update(cx, |host, cx| {
                        if host.sessions.get(&target.key()).is_some_and(|s| {
                            s.view
                                .read(cx)
                                .operations
                                .iter()
                                .any(|o| o.state.is_active())
                        }) {
                            host.notice("该环境仍有进行中的任务".into(), cx);
                            return;
                        }
                        let saved = host.targets.clone();
                        host.targets.retain(|t| t.key() != target.key());
                        if let Err(e) = host.save_catalog() {
                            host.targets = saved;
                            host.notice(e.to_string(), cx);
                            return;
                        }
                        if host.active == target.key() {
                            if host.sessions.contains_key("local") {
                                host.select("local".into(), window, cx);
                            } else {
                                match load(&host.base, &Target::Local, None) {
                                    Ok(loaded) => host.install(Target::Local, loaded, window, cx),
                                    Err(e) => {
                                        host.targets = saved;
                                        let _ = host.save_catalog();
                                        host.notice(e.to_string(), cx);
                                        return;
                                    }
                                }
                            }
                        }
                        host.sessions.remove(&target.key());
                        let current = host
                            .targets
                            .iter()
                            .find(|t| t.key() == host.active)
                            .cloned()
                            .unwrap_or(Target::Local);
                        host.inspect_environment(current, cx);
                        cx.notify();
                    });
                    true
                })
        });
    }
}
