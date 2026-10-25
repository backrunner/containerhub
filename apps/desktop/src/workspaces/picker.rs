use super::*;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};

impl Workspace {
    pub(crate) fn workspace_picker(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let host = self.workspace_host.clone();
        let workspace = cx.entity().downgrade();
        let current = host.as_ref().and_then(|host| host.upgrade()).map(|host| {
            let state = host.read(cx);
            (state.targets.clone(), state.active.clone(), state.loading)
        });
        let (targets, active, loading) = current.unwrap_or_else(|| {
            let target = if self.demo {
                Target::Demo
            } else {
                Target::Local
            };
            (vec![target.clone()], target.key(), false)
        });
        let label = targets
            .iter()
            .find(|target| target.key() == active)
            .map(|t| {
                host.as_ref()
                    .and_then(|h| h.upgrade())
                    .map(|h| h.read(cx).label(t))
                    .unwrap_or_else(|| t.label())
            })
            .unwrap_or_else(|| "工作空间".into());
        let labels = targets
            .iter()
            .map(|t| {
                host.as_ref()
                    .and_then(|h| h.upgrade())
                    .map(|h| h.read(cx).label(t))
                    .unwrap_or_else(|| t.label())
            })
            .collect::<Vec<_>>();
        Button::new("workspace-switcher")
            .ghost()
            .regular()
            .px(px(6.))
            .max_w(px(330.))
            .disabled(loading)
            .accessibility_label("切换工作空间")
            .tooltip("切换本机、Docker 或已绑定的远程 Agent")
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(12.))
                    .child(
                        Icon::new(if self.remote_label.is_some() {
                            Glyph::Server
                        } else {
                            Glyph::Monitor
                        })
                        .size(px(15.))
                        .text_color(p.muted),
                    )
                    .child(div().truncate().child(if loading {
                        "正在切换…".into()
                    } else {
                        label
                    }))
                    .child(div().mx(px(2.)).text_color(p.border).child("/"))
                    .child(div().text_color(p.muted).child(self.runtime_label()))
                    .child(
                        Icon::new(Glyph::ChevronDown)
                            .size(px(12.))
                            .text_color(p.muted),
                    ),
            )
            .dropdown_menu(move |mut menu, _, _| {
                menu = menu.label("切换环境");
                for (target, label) in targets.iter().zip(&labels) {
                    let target = target.clone();
                    let host = host.clone();
                    menu = menu.item(
                        PopupMenuItem::new(label.clone())
                            .checked(target.key() == active)
                            .on_click(move |_, window, cx| {
                                if let Some(host) = &host {
                                    let _ = host.update(cx, |host, cx| {
                                        host.activate(target.clone(), window, cx)
                                    });
                                }
                            }),
                    );
                }
                if host.is_some() {
                    let workspace = workspace.clone();
                    menu = menu
                        .separator()
                        .item(PopupMenuItem::new("环境管理…").on_click(move |_, _, cx| {
                            let _ = workspace
                                .update(cx, |view, cx| view.navigate(Page::Environments, cx));
                        }));
                }
                menu
            })
    }
}
