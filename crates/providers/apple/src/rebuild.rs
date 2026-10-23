use super::*;
mod plan;
#[cfg(test)]
mod tests;

fn uncertain(message: &str) -> ManagementError {
    ManagementError::new(ErrorKind::UnknownOutcome, message)
}
impl AppleProvider {
    pub(super) async fn rebuild_ports(
        &self,
        key: &ResourceKey,
        expected_state: &ContainerState,
        expected: &[Port],
        ports: &[Port],
        control: &ExecutionControl,
    ) -> Result<CommandResult, ManagementError> {
        if !control.has_checkpoint() {
            return Err(ManagementError::new(
                ErrorKind::Unsupported,
                "端口重建需要持久化任务队列",
            ));
        }
        let original = self.current(key).await?;
        if original.ports != expected || original.state != *expected_state {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "容器状态或端口已改变，请重新打开编辑",
            ));
        }
        if ports == expected {
            return Ok(CommandResult::Container(Box::new(original)));
        }
        let raw = self.run(&["inspect", "--", &key.id], false).await?;
        let mut plan = plan::Plan::parse(&raw, &original, ports)?;
        let image = self.local_image(&original.image).await?;
        let digest = key
            .incarnation
            .rsplit_once('|')
            .map(|(_, digest)| digest)
            .filter(|digest| digest.starts_with("sha256:"))
            .ok_or_else(|| uncertain("原镜像摘要不完整，未停止或删除容器"))?;
        if image.id.trim_start_matches("sha256:") != digest.trim_start_matches("sha256:") {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "原镜像标签已改变，未停止或删除容器",
            ));
        }
        // Pin the checked image so a moving tag cannot change the recreation image.
        let repository = original.image.split('@').next().unwrap();
        let repository = repository
            .rsplit_once(':')
            .filter(|(_, suffix)| !suffix.contains('/'))
            .map_or(repository, |(base, _)| base);
        plan.pin_image(format!("{repository}@{digest}"));
        plan.check_image_defaults(
            &self
                .run(&["image", "inspect", "--", &original.image], false)
                .await?,
        )?;
        for (kind, names) in [
            (ResourceKind::Network, &plan.networks),
            (ResourceKind::Volume, &plan.volumes),
        ] {
            if !names.is_empty() {
                let available = self.resources(kind).await?;
                if names
                    .iter()
                    .any(|name| !available.iter().any(|r| &r.name == name))
                {
                    return Err(ManagementError::new(
                        ErrorKind::NotFound,
                        "原网络或挂载卷已不存在，未停止或删除容器",
                    ));
                }
            }
        }
        self.check_rebuild_ports(key, ports).await?;
        self.check_rebuild_config(key, &plan.config).await?;
        control.report(OperationProgress::Stopping);
        if original.state.is_running() {
            self.run(&["stop", "--time", "10", "--", &key.id], true)
                .await?;
        }
        // An auto-remove container can disappear on stop. Never delete a same-name replacement.
        let remaining = self
            .list()
            .await
            .map_err(|_| uncertain("停止命令已返回，状态待核对；未继续删除或创建"))?;
        if let Some(stopped) = remaining.iter().find(|c| c.key.id == key.id) {
            if stopped.key != *key || stopped.state != ContainerState::Stopped {
                return Err(uncertain(
                    "原容器未确认停止或同名容器已被替换；未继续删除或创建",
                ));
            }
            self.check_rebuild_config(key, &plan.config).await?;
            self.check_rebuild_ports(key, ports).await?;
            control.report(OperationProgress::Replacing);
            self.run(&["delete", "--", &key.id], true)
                .await
                .map_err(|_| uncertain("删除原容器的结果待核对；未创建或重复删除"))?;
        }
        if self
            .list()
            .await
            .map_err(|_| uncertain("删除后无法核对列表；未继续创建"))?
            .iter()
            .any(|c| c.key.id == key.id)
        {
            return Err(uncertain("同名资源仍存在或已被重建；未创建或继续删除"));
        }
        control.report(OperationProgress::Creating);
        let args: Vec<_> = plan.args.iter().map(String::as_str).collect();
        process::run_with_input(
            self.binary.as_ref().expect("discovered"),
            &args,
            true,
            Duration::from_secs(300),
            &plan.environment,
        )
        .await
        .map_err(|_| uncertain("原容器已删除，创建新容器未确认。请核对运行时；未自动重试重建"))?;
        let created = self
            .list()
            .await
            .map_err(|_| uncertain("新容器创建命令已返回，读取结果失败"))?
            .into_iter()
            .find(|c| {
                c.key.id == key.id && c.key != *key && c.ports == ports && !c.created_at.is_empty()
            })
            .ok_or_else(|| uncertain("新容器身份或端口尚未确认；未继续启动"))?;
        // Persist the replacement identity before any automatic start; retries can only start this identity.
        let replacement = self
            .run(&["inspect", "--", &created.key.id], false)
            .await
            .map_err(|_| uncertain("新容器已创建，但配置读取失败；未启动"))?;
        plan.verify(&replacement)
            .map_err(|_| uncertain("新容器已创建，但配置未完整匹配；未启动，请人工核对"))?;
        control.checkpoint_created(&created).await?;
        if original.state.is_running() {
            control.report(OperationProgress::Starting);
            self.lifecycle_command(&created.key, LifecycleAction::Start)
                .await
                .map_err(|_| {
                    ManagementError::new(
                        ErrorKind::Unavailable,
                        "新容器已创建且端口已确认，但启动失败。可单独重试启动，不会再次重建",
                    )
                })?;
        }
        control.report(OperationProgress::Verifying);
        let final_state = self
            .current(&created.key)
            .await
            .map_err(|_| uncertain("重建后的容器状态尚未确认"))?;
        if final_state.state != original.state || final_state.ports != ports {
            return Err(uncertain("重建后的运行状态或端口未匹配；请刷新核对"));
        }
        control.checkpoint_created(&final_state).await?;
        Ok(CommandResult::Container(Box::new(final_state)))
    }
    async fn check_rebuild_config(
        &self,
        key: &ResourceKey,
        expected: &serde_json::Value,
    ) -> Result<(), ManagementError> {
        self.current(key).await?;
        let raw = self.run(&["inspect", "--", &key.id], false).await?;
        let values: Vec<serde_json::Value> =
            serde_json::from_slice(&raw).map_err(|_| uncertain("容器配置无法核对"))?;
        if values.len() != 1 || values[0]["configuration"] != *expected {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "容器配置已改变；未继续删除或创建",
            ));
        }
        Ok(())
    }
    async fn check_rebuild_ports(
        &self,
        key: &ResourceKey,
        ports: &[Port],
    ) -> Result<(), ManagementError> {
        if self.list().await?.iter().any(|c| {
            c.key != *key
                && c.ports
                    .iter()
                    .any(|p| ports.iter().any(|new| ports_overlap(p, new)))
        }) {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "主机端口与其他容器冲突，未删除容器",
            ));
        }
        Ok(())
    }
}
