use containerhub_core::*;
use serde_json::Value;
pub(crate) fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}
pub(crate) fn invalid() -> ManagementError {
    ManagementError::new(ErrorKind::InvalidData, "Docker 返回了无法识别的结构")
}
pub(crate) fn json(bytes: &[u8]) -> Result<Value, ManagementError> {
    serde_json::from_slice(bytes).map_err(|_| invalid())
}
pub(crate) fn container(v: &Value, environment: &str) -> Result<Container, ManagementError> {
    let id = text(v, "Id");
    let name = text(v, "Name").trim_start_matches('/');
    if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) || name.is_empty() {
        return Err(invalid());
    }
    let config = &v["Config"];
    let host = &v["HostConfig"];
    let state = &v["State"];
    let labels = config["Labels"].as_object();
    let membership = ComposeMembership {
        project: labels
            .and_then(|l| l.get("com.docker.compose.project"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .into(),
        service: labels
            .and_then(|l| l.get("com.docker.compose.service"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .into(),
        one_off: labels
            .and_then(|l| l.get("com.docker.compose.oneoff"))
            .and_then(Value::as_str)
            .is_some_and(|s| s.eq_ignore_ascii_case("true")),
    };
    let mut ports = vec![];
    if let Some(bindings) = v["NetworkSettings"]["Ports"].as_object() {
        for (target, bindings) in bindings {
            let Some((port, protocol)) = target.split_once('/') else {
                continue;
            };
            let Ok(container_port) = port.parse::<u16>() else {
                continue;
            };
            for binding in bindings.as_array().into_iter().flatten() {
                let Ok(host_port) = text(binding, "HostPort").parse::<u16>() else {
                    continue;
                };
                let host = text(binding, "HostIp");
                if host.parse::<std::net::IpAddr>().is_ok() && ["tcp", "udp"].contains(&protocol) {
                    ports.push(Port {
                        host: host.into(),
                        host_port,
                        container_port,
                        protocol: protocol.into(),
                    });
                }
            }
        }
    }
    ports.sort_by(|a, b| {
        (&a.host, a.host_port, &a.protocol).cmp(&(&b.host, b.host_port, &b.protocol))
    });
    let network = v["NetworkSettings"]["Networks"].as_object();
    let nano = host["NanoCpus"].as_u64().unwrap_or(0);
    Ok(Container {
        key: ResourceKey {
            environment: environment.into(),
            provider: ProviderKind::Docker,
            namespace: None,
            id: name.into(),
            incarnation: id.into(),
        },
        image: text(config, "Image").into(),
        state: match text(state, "Status") {
            "running" => ContainerState::Running,
            "created" | "exited" => ContainerState::Stopped,
            s => ContainerState::Unknown(s.into()),
        },
        cpus: (nano > 0 && nano.is_multiple_of(1_000_000_000))
            .then_some((nano / 1_000_000_000) as u32),
        memory_limit: host["Memory"].as_u64().filter(|n| *n > 0),
        platform: text(v, "Platform").into(),
        ports,
        mounts: v["Mounts"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|m| format!("{} → {}", text(m, "Name"), text(m, "Destination")))
            .collect(),
        networks: network
            .map(|n| n.keys().cloned().collect())
            .unwrap_or_default(),
        addresses: network
            .into_iter()
            .flat_map(|n| n.values())
            .flat_map(|n| [text(n, "IPAddress"), text(n, "GlobalIPv6Address")])
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect(),
        environment_names: config["Env"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter_map(|s| s.split_once('=').map(|(n, _)| n.into()))
            .collect(),
        label_names: labels
            .map(|l| l.keys().cloned().collect())
            .unwrap_or_default(),
        created_at: text(v, "Created").into(),
        auto_remove: host["AutoRemove"].as_bool(),
        health: match text(&state["Health"], "Status") {
            "healthy" => ContainerHealth::Healthy,
            "unhealthy" => ContainerHealth::Unhealthy,
            "starting" => ContainerHealth::Starting,
            "" => ContainerHealth::Unconfigured,
            _ => ContainerHealth::Unknown,
        },
        compose: (membership.validate()
            && !config["Labels"]["com.docker.swarm.service.id"].is_string())
        .then_some(membership),
    })
}
