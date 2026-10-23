use super::*;
impl DockerProvider {
    pub(crate) async fn change_service(
        &self,
        key: &ResourceKey,
        stack: &str,
        version: u64,
        action: SwarmAction,
    ) -> Result<CommandResult, ManagementError> {
        let info = self.info().await?;
        if info["Swarm"]["ControlAvailable"].as_bool() != Some(true) {
            return Err(ManagementError::new(
                ErrorKind::Unsupported,
                "需要连接 Swarm manager",
            ));
        }
        let current = self.service(&key.incarnation).await?;
        if current.key != *key || current.stack != stack || current.version != version {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "Swarm 服务已变化，请刷新后重试",
            ));
        }
        current.command(action)?;
        match action {
            SwarmAction::Scale { replicas } => {
                self.run(
                    &[
                        "service",
                        "update",
                        "--detach",
                        "--replicas",
                        &replicas.to_string(),
                        "--",
                        &key.incarnation,
                    ],
                    true,
                )
                .await?;
            }
            SwarmAction::Restart => {
                self.run(
                    &[
                        "service",
                        "update",
                        "--detach",
                        "--force",
                        "--",
                        &key.incarnation,
                    ],
                    true,
                )
                .await?;
            }
            SwarmAction::Remove => {
                self.run(&["service", "rm", "--", &key.incarnation], true)
                    .await?;
            }
        }
        // Confirm the requested service specification, not replica health or rollout convergence.
        let confirmed = match action {
            SwarmAction::Remove => self
                .service_ids(&format!("id={}", key.incarnation))
                .await
                .map(|ids| ids.is_empty()),
            _ => self.service(&key.incarnation).await.map(|s| {
                s.key == *key
                    && s.stack == stack
                    && s.version > version
                    && match action {
                        SwarmAction::Scale { replicas } => {
                            s.desired_replicas == Some(replicas as u64)
                        }
                        SwarmAction::Restart => s.force_update > current.force_update,
                        _ => false,
                    }
            }),
        };
        if !matches!(confirmed, Ok(true)) {
            return Err(ManagementError::new(
                ErrorKind::UnknownOutcome,
                "服务配置更新结果待核对，不会自动重试",
            ));
        }
        Ok(CommandResult::Confirmed)
    }
}
