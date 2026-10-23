use super::*;

impl Workspace {
    pub(crate) fn create_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(environment) = self
            .snapshot
            .as_ref()
            .filter(|s| self.error.is_none() && s.capabilities.create_container)
            .map(|s| s.environment.clone())
        else {
            return;
        };
        let image_value = self
            .operations
            .iter()
            .rev()
            .find_map(|op| match &op.result {
                Some(CommandResult::Image(image)) => Some(image.name.clone()),
                _ => None,
            })
            .unwrap_or_default();
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("容器名称，例如 my-nginx"));
        let image = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("已拉取的镜像引用")
                .default_value(image_value)
        });
        let cpus = cx.new(|cx| InputState::new(window, cx).default_value("2"));
        let memory = cx.new(|cx| InputState::new(window, cx).default_value("512"));
        let enabled = self
            .snapshot
            .as_ref()
            .is_some_and(|s| s.capabilities.create_advanced);
        let advanced = cx.new(|cx| advanced::AdvancedForm::new(window, cx, enabled));
        let enabled = self
            .snapshot
            .as_ref()
            .is_some_and(|s| s.capabilities.create_secrets);
        let secrets = cx.new(|_| secrets::SecretsForm::new(enabled));
        let supported = self
            .snapshot
            .as_ref()
            .is_some_and(|s| s.capabilities.create_startup);
        let startup = cx.new(|cx| startup::StartupForm::new(window, cx, supported));
        let supported = self
            .snapshot
            .as_ref()
            .is_some_and(|s| s.lifecycle_supported && s.capabilities.create_and_start);
        let launch = cx.new(|_| launch::LaunchForm {
            enabled: false,
            supported,
        });
        let ports =
            cx.new(|cx| InputState::new(window, cx).placeholder("例如 8080:80/tcp, 5353:53/udp"));
        let error = cx.new(|_| FormError(String::new()));
        let entity = cx.entity().downgrade();
        window.open_sheet_at(
            gpui_kit::component::Placement::Right,
            cx,
            move |sheet, window, cx| {
                let (name, image, cpus, memory, ports, error, entity, environment) = (
                    name.clone(),
                    image.clone(),
                    cpus.clone(),
                    memory.clone(),
                    ports.clone(),
                    error.clone(),
                    entity.clone(),
                    environment.clone(),
                );
                let advanced = advanced.clone();
                let secrets = secrets.clone();
                let startup = startup.clone();
                let launch = launch.clone();
                let p = Palette::get(cx.theme().is_dark());
                sheet
                    .size(px(520.))
                    .resizable(false)
                    .overlay_closable(false)
                    .px(px(24.))
                    .title(
                        div().py(px(14.)).pl(px(8.)).child(
                            div()
                                .text_size(px(19.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("新建容器"),
                        ),
                    )
                    .child(
                        observed!(div().id("create-scroll"))
                            .flex()
                            .flex_col()
                            .gap(px(14.))
                            .h((window.viewport_size().height - px(216.)).max(px(220.)))
                            .overflow_y_scroll()
                            .pb(px(16.))
                            .child(section("1", "基本信息", p))
                            .child(field("容器名称", Input::new(&name).id("create-name"), p))
                            .child(field("镜像", Input::new(&image).id("create-image"), p))
                            .child(section("2", "资源配置", p))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(14.))
                                    .child(div().flex_1().min_w_0().child(field(
                                        "CPU 核数",
                                        Input::new(&cpus),
                                        p,
                                    )))
                                    .child(div().flex_1().min_w_0().child(field(
                                        "内存上限 · MiB",
                                        Input::new(&memory),
                                        p,
                                    ))),
                            )
                            .child(section("3", "网络与存储", p))
                            .child(field(
                                "端口映射 · 127.0.0.1 → 容器",
                                Input::new(&ports).id("create-ports"),
                                p,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.))
                                    .child(advanced.clone())
                                    .child(secrets.clone())
                                    .child(startup.clone()),
                            )
                            .child(launch.clone())
                            .child(error.clone()),
                    )
                    .footer(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .pt(px(12.))
                            .border_t_1()
                            .border_color(p.border)
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(10.))
                                    .text_color(p.muted)
                                    .child(""),
                            )
                            .child(
                                Button::new("cancel-create")
                                    .regular()
                                    .label("取消")
                                    .on_click(|_, window, cx| window.close_sheet(cx)),
                            )
                            .child(
                                Button::new("ok")
                                    .regular()
                                    .primary()
                                    .small()
                                    .h(px(32.))
                                    .label("创建容器")
                                    .on_click(move |_, window, cx| {
                                        let input = secrets.read(cx).input(cx);
                                        let spec = parse_form(
                                            name.read(cx).value().as_ref(),
                                            image.read(cx).value().as_ref(),
                                            cpus.read(cx).value().as_ref(),
                                            memory.read(cx).value().as_ref(),
                                            ports.read(cx).value().as_ref(),
                                        )
                                        .and_then(|mut spec| {
                                            spec.options = advanced.read(cx).options(cx)?;
                                            spec.options.startup = startup.read(cx).options(cx)?;
                                            spec.options.start_after_create =
                                                launch.read(cx).enabled;
                                            Ok(spec)
                                        });
                                        let command = spec.map(|mut spec| {
                                            if let Ok(input) = &input {
                                                spec.options.secret_environment = input.names();
                                            }
                                            Command::CreateContainer {
                                                environment: environment.clone(),
                                                spec,
                                            }
                                        });
                                        let command = match command.and_then(|command| {
                                            command.validate().map_err(|e| e.message)?;
                                            let input = input?;
                                            input.validate_for(&command).map_err(|e| e.message)?;
                                            Ok((command, input))
                                        }) {
                                            Ok(command) => command,
                                            Err(message) => {
                                                error.update(cx, |error, cx| {
                                                    error.0 = message;
                                                    cx.notify();
                                                });
                                                return;
                                            }
                                        };
                                        let _ = entity.update(cx, |this, cx| {
                                            this.submit_with_secrets(command.0, command.1, cx)
                                        });
                                        window.close_sheet(cx);
                                    }),
                            ),
                    )
            },
        );
    }
}

pub(super) fn field(label: &'static str, input: Input, p: Palette) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(div().text_size(px(11.)).text_color(p.muted).child(label))
        .child(input.small().min_h(px(34.)).text_size(px(12.)))
}
fn section(number: &'static str, title: &'static str, p: Palette) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(7.))
        .mt(px(6.))
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .child(
            div()
                .size(px(19.))
                .rounded(px(4.))
                .bg(p.accent_surface)
                .text_color(p.accent)
                .flex()
                .items_center()
                .justify_center()
                .child(number),
        )
        .child(title)
}

pub(super) fn disclosure(id: &'static str, label: &'static str, icon: Glyph) -> Button {
    Button::new(id)
        .small()
        .ghost()
        .h(px(28.))
        .px(px(0.))
        .accessibility_label(label)
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap(px(7.))
                .text_size(px(11.))
                .child(Icon::new(icon).size(px(13.)))
                .child(label),
        )
}
