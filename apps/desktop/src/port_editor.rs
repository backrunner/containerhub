use crate::*;
use gpui_kit::component::{
    dialog::{DialogAction, DialogClose, DialogFooter},
    menu::{DropdownMenu, PopupMenuItem},
};

mod fields;
use fields::PortEditor;

impl Workspace {
    pub(super) fn edit_ports(
        &mut self,
        key: ResourceKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(container) = self
            .snapshot
            .as_ref()
            .and_then(|s| s.containers.iter().find(|c| c.key == key))
            .cloned()
        else {
            return;
        };
        let expected = container.ports;
        let expected_state = container.state;
        let enabled = self.error.is_none()
            && !self.busy(&key)
            && self
                .snapshot
                .as_ref()
                .is_some_and(|s| s.capabilities.rebuild_ports);
        let explanation = if self.error.is_some() {
            "环境已离线，恢复连接后再保存。"
        } else if !self
            .snapshot
            .as_ref()
            .is_some_and(|s| s.capabilities.rebuild_ports)
        {
            "当前运行时尚未支持端口重建。"
        } else if self.busy(&key) {
            "此容器有操作正在进行，请完成后再编辑。"
        } else if self.demo {
            "演示环境：重建只影响示例数据。"
        } else {
            "执行前核对身份与配置。新容器不启用退出自动删除。"
        };
        let editor = cx.new(|cx| PortEditor::new(&expected, window, cx));
        let entity = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let editor = editor.clone();
            let entity = entity.clone();
            let key = key.clone();
            let expected = expected.clone();
            let expected_state = expected_state.clone();
            let p = Palette::get(cx.theme().is_dark());
            dialog
                .title("编辑端口映射")
                .width(px(580.)).p(px(24.))
                .overlay_closable(false)
                .child(div().flex().flex_col().gap(px(12.))
                    .child(
                    div()
                        .text_size(px(12.))
                        .text_color(p.muted)
                        .child(format!("{} · 主机 → 容器", key.id)),
                )
                .child(editor.clone())
                .child(div().p(px(12.)).rounded(px(6.)).bg(p.danger.opacity(0.06)).text_size(px(12.)).text_color(p.danger)
                    .child("将停止并删除原容器，使用原镜像和新端口重建。\n可写层数据会删除，挂载卷和目录保留。\n重建后恢复原运行状态；失败不会回滚到原容器。"))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(p.muted)
                        .child(explanation),
                )
                )
                .footer(
                    DialogFooter::new()
                        .gap(px(8.)).pt(px(16.)).border_t_1().border_color(p.border)
                        .child(div().w(px(64.)).h(px(32.)).child(DialogClose::new().trigger(|button| button.regular().w_full().label("取消"))))
                        .child(div().w(px(64.)).h(px(32.)).child(
                            DialogAction::new().child(
                                Button::new("save-ports")
                                    .regular().w_full()
                                    .stop_style(p, cx)
                                    .label("重建")
                                    .disabled(!enabled),
                            ),
                        )),
                )
                .on_ok(move |_, _, cx| {
                    let result = (|| {
                        if !enabled {
                            return Err("当前环境暂不能保存端口映射".into());
                        }
                        let ports = editor.read(cx).ports(cx)?;
                        if ports == expected {return Err("端口映射未改变".into());}
                        let command = Command::RebuildPorts {
                            key: key.clone(),
                            expected: expected.clone(),
                            expected_state: expected_state.clone(),
                            ports,
                        };
                        entity
                            .update(cx, |this, cx| {
                                let current = this
                                    .snapshot
                                    .as_ref()
                                    .and_then(|s| s.containers.iter().find(|c| c.key == key));
                                if !this.supports(&command) || this.busy(&key) {
                                    return Err("环境状态已改变，请稍后重新打开".into());
                                }
                                if current.is_none_or(|c| c.ports != expected || c.state != expected_state) {
                                    return Err("容器或端口映射已改变，请重新打开后核对".into());
                                }
                                this.submit(command, cx);
                                Ok(())
                            })
                            .map_err(|_| "工作空间已关闭".to_owned())?
                    })();
                    match result {
                        Ok(()) => true,
                        Err(message) => {
                            editor.update(cx, |editor, cx| {
                                editor.error = message;
                                cx.notify();
                            });
                            false
                        }
                    }
                })
        });
    }
}
