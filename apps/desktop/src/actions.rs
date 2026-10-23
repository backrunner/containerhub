use crate::*;

impl Workspace {
    pub(super) fn busy(&self, key: &ResourceKey) -> bool {
        self.operations.iter().any(|o| {
            o.command.environment() == key.environment
                && o.command.target() == key.id
                && o.state.is_active()
        })
    }
    pub(super) fn writable(&self) -> bool {
        self.error.is_none()
            && self
                .snapshot
                .as_ref()
                .is_some_and(|s| s.lifecycle_supported)
    }
    pub(super) fn run(
        &mut self,
        key: ResourceKey,
        action: LifecycleAction,
        cx: &mut Context<Self>,
    ) {
        self.submit(Command::Lifecycle { key, action }, cx);
    }
    pub(super) fn supports(&self, command: &Command) -> bool {
        if let Command::StartRuntime { environment } = command {
            return self.remote_label.is_none()
                && self
                    .diagnostics
                    .as_ref()
                    .and_then(|r| r.as_ref().ok())
                    .is_some_and(|r| {
                        &r.environment == environment && r.supports(RuntimeFeature::StartRuntime)
                    });
        }
        let Some(snapshot) = &self.snapshot else {
            return false;
        };
        if self.error.is_some() || command.environment() != snapshot.environment {
            return false;
        }
        match command {
            Command::StartRuntime { .. } => unreachable!("handled without inventory"),
            Command::SwarmService { .. } => {
                snapshot.swarm.status == SwarmStatus::Manager
                    && snapshot.capabilities.stacks
                    && snapshot.capabilities.create_container
                    && snapshot.lifecycle_supported
            }
            Command::Lifecycle { .. } => snapshot.lifecycle_supported,
            Command::PullImage { .. } => snapshot.capabilities.pull_image,
            Command::CreateContainer { .. } => {
                snapshot.capabilities.create_container
                    && (!command.starts_after_create()
                        || (snapshot.lifecycle_supported && snapshot.capabilities.create_and_start))
            }
            Command::RebuildPorts { .. } => {
                snapshot.capabilities.rebuild_ports && snapshot.lifecycle_supported
            }
            Command::DeleteContainer { .. } => snapshot.capabilities.delete_container,
            Command::DeleteImage { .. } => snapshot.capabilities.delete_image,
        }
    }
    pub(super) fn submit(&mut self, command: Command, cx: &mut Context<Self>) {
        self.submit_with_secrets(command, SecretEnvironment::default(), cx);
    }
    pub(super) fn submit_with_secrets(
        &mut self,
        command: Command,
        secrets: SecretEnvironment,
        cx: &mut Context<Self>,
    ) {
        if !self.supports(&command)
            || self
                .operations
                .iter()
                .any(|op| op.state.is_active() && op.command.lock_key() == command.lock_key())
        {
            return;
        }
        if let Err(error) = command
            .validate()
            .and_then(|()| secrets.validate_for(&command))
        {
            self.notice = Some(error.message);
            cx.notify();
            return;
        }
        let id = self
            .operations
            .iter()
            .map(|o| o.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(now() * 1000);
        let op = Operation {
            id,
            command,
            receipt: None,
            result: None,
            progress: None,
            cancel_requested: false,
            state: OperationState::Submitting,
            started_at: now(),
            finished_at: None,
            message: None,
        };
        if self
            .backend
            .send(Request::Run(Box::new(op.clone()), secrets))
        {
            self.operations.push(op);
        } else {
            self.notice = Some("请求队列已满，操作未提交。".into());
        }
        cx.notify();
    }
    pub(super) fn run_batch(
        &mut self,
        action: LifecycleAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut keys: Vec<_> = self.batch.iter().cloned().collect();
        keys.sort_by(|a, b| a.id.cmp(&b.id));
        if keys.is_empty() || keys.len() > 32 {
            return;
        }
        if keys.iter().any(|key| self.busy(key)) {
            self.notice = Some("所选容器仍有进行中的操作，请等待完成后再提交。".into());
            cx.notify();
            return;
        }
        if action == LifecycleAction::Stop {
            let entity = cx.entity().downgrade();
            window.open_dialog(cx, move |dialog,_,_| {
                let keys = keys.clone();
                let entity = entity.clone();
                dialog.title(format!("停止选中的 {} 个容器", keys.len()))
                    .footer(crate::forms::footer("停止所选容器"))
                    .child("将逐项停止并记录结果。使用 --rm 创建的容器可能在停止后自动删除；失败项可在任务中心单独重试。")
                    .button_props(DialogButtonProps::default().ok_text("停止所选容器"))
                    .on_ok(move|_,_,cx| { let _ = entity.update(cx, |this,cx| { for key in &keys { this.run(key.clone(),action,cx); } this.batch.clear(); }); true })
            });
        } else {
            for key in keys {
                self.run(key, action, cx);
            }
            self.batch.clear();
        }
        cx.notify();
    }
    pub(super) fn retry_command(&self, operation: &Operation) -> Option<Command> {
        if (operation.command.starts_after_create()
            || matches!(operation.command, Command::RebuildPorts { .. }))
            && operation.result.is_some()
        {
            return if matches!(
                operation.state,
                OperationState::Failed | OperationState::UnknownOutcome
            ) {
                start_created_command(
                    &operation.command,
                    operation.result.as_ref(),
                    self.snapshot.as_ref()?,
                )
            } else {
                None
            };
        }
        if operation.state != OperationState::Failed || !operation.command.secret_names().is_empty()
        {
            return None;
        }
        if matches!(operation.command, Command::StartRuntime { .. }) {
            return None; // Recheck diagnostics and use the explicit environment entry point.
        }
        let snapshot = self.snapshot.as_ref()?;
        match &operation.command {
            Command::SwarmService { .. } => None, // Refresh service version and make a new explicit plan.
            Command::Lifecycle { key, action } => {
                let container = snapshot.containers.iter().find(|c| c.key == *key)?;
                match action {
                    LifecycleAction::Start | LifecycleAction::Stop => {
                        Some(operation.command.clone())
                    }
                    LifecycleAction::Restart if container.state == ContainerState::Stopped => {
                        Some(Command::Lifecycle {
                            key: key.clone(),
                            action: LifecycleAction::Start,
                        })
                    }
                    _ => None,
                }
            }
            Command::CreateContainer { spec, .. }
                if snapshot.containers.iter().any(|c| c.key.id == spec.name) =>
            {
                None
            }
            Command::RebuildPorts {
                key,
                expected,
                expected_state,
                ..
            } if !snapshot
                .containers
                .iter()
                .any(|c| c.key == *key && c.ports == *expected && c.state == *expected_state) =>
            {
                None
            }
            Command::DeleteContainer { key }
                if !snapshot
                    .containers
                    .iter()
                    .any(|c| c.key == *key && c.state == ContainerState::Stopped) =>
            {
                None
            }
            _ => Some(operation.command.clone()),
        }
    }
    pub(super) fn restart(
        &mut self,
        key: ResourceKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entity = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let key = key.clone();
            let entity = entity.clone();
            dialog
                .confirm()
                .title(format!("重启 {}？", key.id))
                .child("服务会短暂中断。启用退出自动删除的容器可能被删除，应用不会自动重建。")
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("重启容器")
                        .cancel_text("取消"),
                )
                .on_ok(move |_, _, cx| {
                    let _ = entity.update(cx, |this, cx| {
                        this.run(key.clone(), LifecycleAction::Restart, cx)
                    });
                    true
                })
        });
    }
}
