use super::*;
mod mutations;

impl DockerProvider {
    pub(super) async fn swarm_inventory(&self, info: &serde_json::Value) -> SwarmInventory {
        let status = if info["Swarm"]["ControlAvailable"].as_bool() == Some(true) {
            SwarmStatus::Manager
        } else if text(&info["Swarm"], "LocalNodeState") == "active" {
            SwarmStatus::Worker
        } else {
            SwarmStatus::Inactive
        };
        if status != SwarmStatus::Manager {
            return SwarmInventory {
                status,
                services: vec![],
            };
        }
        match self.swarm_services().await {
            Ok(services) => SwarmInventory { status, services },
            Err(_) => SwarmInventory {
                status: SwarmStatus::Unavailable,
                services: vec![],
            },
        }
    }
    async fn service_ids(&self, filter: &str) -> Result<Vec<String>, ManagementError> {
        let out = self
            .run(&["service", "ls", "--quiet", "--filter", filter], false)
            .await?;
        parse_ids(&out, true)
    }
    async fn swarm_services(&self) -> Result<Vec<SwarmService>, ManagementError> {
        let mut services = vec![];
        for id in self.service_ids("label=com.docker.stack.namespace").await? {
            let mut service = self.service(&id).await?;
            let id = service.key.incarnation.clone();
            let out = self
                .run(
                    &["service", "ps", "--quiet", "--no-trunc", "--", &id],
                    false,
                )
                .await?;
            let ids = parse_ids(&out, false)?;
            service.tasks_truncated = ids.len() > 256;
            for batch in ids.iter().take(256).collect::<Vec<_>>().chunks(64) {
                let mut args = vec!["inspect", "--type", "task", "--"];
                args.extend(batch.iter().map(|id| id.as_str()));
                let values = json(&self.run(&args, false).await?)?;
                for v in values.as_array().ok_or_else(invalid)? {
                    if text(v, "ServiceID") != id {
                        return Err(invalid());
                    }
                    service.tasks.push(SwarmTask {
                        id: text(v, "ID").into(),
                        node: text(v, "NodeID").into(),
                        slot: v["Slot"].as_u64().unwrap_or(0),
                        desired_state: text(v, "DesiredState").into(),
                        state: text(&v["Status"], "State").into(),
                        container_id: v["Status"]["ContainerStatus"]["ContainerID"]
                            .as_str()
                            .filter(|id| {
                                id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit())
                            })
                            .map(str::to_owned),
                    });
                }
            }
            services.push(service);
        }
        Ok(services)
    }
    async fn service(&self, id: &str) -> Result<SwarmService, ManagementError> {
        if !valid_service_reference(id) {
            return Err(invalid());
        }
        let values = json(&self.run(&["service", "inspect", "--", id], false).await?)?;
        let v = values
            .as_array()
            .filter(|a| a.len() == 1)
            .and_then(|a| a.first())
            .ok_or_else(invalid)?;
        let spec = &v["Spec"];
        let name = text(spec, "Name");
        let stack = text(&spec["Labels"], "com.docker.stack.namespace");
        let full_id = text(v, "ID");
        if !valid_swarm_id(full_id)
            || !full_id.starts_with(id)
            || !valid_container_name(name)
            || !valid_container_name(stack)
        {
            return Err(invalid());
        }
        let mode = if spec["Mode"]["Replicated"].is_object() {
            SwarmMode::Replicated
        } else if spec["Mode"]["Global"].is_object() {
            SwarmMode::Global
        } else if spec["Mode"]["ReplicatedJob"].is_object() || spec["Mode"]["GlobalJob"].is_object()
        {
            SwarmMode::Job
        } else {
            SwarmMode::Unknown
        };
        Ok(SwarmService {
            key: ResourceKey {
                environment: self.environment.clone(),
                provider: ProviderKind::Docker,
                namespace: Some("swarm-service".into()),
                id: name.into(),
                incarnation: full_id.into(),
            },
            stack: stack.into(),
            version: v["Version"]["Index"].as_u64().ok_or_else(invalid)?,
            image: text(&spec["TaskTemplate"]["ContainerSpec"], "Image").into(),
            mode,
            desired_replicas: spec["Mode"]["Replicated"]["Replicas"].as_u64(),
            force_update: spec["TaskTemplate"]["ForceUpdate"].as_u64().unwrap_or(0),
            update_state: v["UpdateStatus"]["State"].as_str().map(str::to_owned),
            ports: v["Endpoint"]["Ports"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|p| {
                    format!(
                        "{} → {}/{}",
                        p["PublishedPort"].as_u64().unwrap_or(0),
                        p["TargetPort"].as_u64().unwrap_or(0),
                        text(p, "Protocol")
                    )
                })
                .collect(),
            tasks: vec![],
            tasks_truncated: false,
        })
    }
}
fn parse_ids(out: &[u8], allow_short: bool) -> Result<Vec<String>, ManagementError> {
    std::str::from_utf8(out)
        .map_err(|_| invalid())?
        .lines()
        .filter(|l| !l.is_empty())
        .map(|s| {
            if valid_swarm_id(s) || (allow_short && valid_service_reference(s)) {
                Ok(s.into())
            } else {
                Err(invalid())
            }
        })
        .collect()
}

// Docker service ls (including --quiet) always formats 12-character IDs.
// Resolve list entries with inspect; mutations only receive full, validated keys.
fn valid_service_reference(id: &str) -> bool {
    valid_swarm_id(id)
        || (id.len() == 12
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
}
