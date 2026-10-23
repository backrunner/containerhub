use containerhub_core::*;
use serde_json::Value;
use std::collections::HashSet;

fn invalid(message: &str) -> ManagementError {
    ManagementError::new(ErrorKind::InvalidData, message)
}
fn array(bytes: &[u8]) -> Result<Vec<Value>, ManagementError> {
    serde_json::from_slice::<Vec<Value>>(bytes)
        .map_err(|_| invalid("运行时 JSON 格式不兼容；未将异常响应当作空列表"))
}
fn text<'a>(v: &'a Value, path: &str) -> Option<&'a str> {
    v.pointer(path).and_then(Value::as_str)
}
fn required<'a>(v: &'a Value, path: &str) -> Result<&'a str, ManagementError> {
    text(v, path)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid(&format!("运行时响应缺少必要字段 {path}")))
}
fn entries<'a>(v: &'a Value, path: &str) -> impl Iterator<Item = &'a Value> {
    v.pointer(path)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}
pub fn parse_containers(bytes: &[u8]) -> Result<Vec<Container>, ManagementError> {
    let mut ids = HashSet::new();
    let mut result = Vec::new();
    for v in array(bytes)? {
        let id = required(&v, "/id")?;
        if !ids.insert(id.to_string()) {
            return Err(invalid("运行时返回了重复容器标识"));
        }
        let image = required(&v, "/configuration/image/reference")?.to_string();
        let created = text(&v, "/configuration/creationDate")
            .unwrap_or_default()
            .to_string();
        let digest = text(&v, "/configuration/image/descriptor/digest").unwrap_or_default();
        let state = match required(&v, "/status/state")? {
            "running" => ContainerState::Running,
            "stopped" => ContainerState::Stopped,
            other => ContainerState::Unknown(other.into()),
        };
        let ports = entries(&v, "/configuration/publishedPorts")
            .map(|p| {
                Ok(Port {
                    host: required(p, "/hostAddress")?.into(),
                    host_port: p["hostPort"]
                        .as_u64()
                        .and_then(|n| u16::try_from(n).ok())
                        .ok_or_else(|| invalid("无效主机端口"))?,
                    container_port: p["containerPort"]
                        .as_u64()
                        .and_then(|n| u16::try_from(n).ok())
                        .ok_or_else(|| invalid("无效容器端口"))?,
                    protocol: text(p, "/proto").unwrap_or("tcp").into(),
                })
            })
            .collect::<Result<Vec<_>, ManagementError>>()?;
        let environment_names = entries(&v, "/configuration/initProcess/environment")
            .filter_map(Value::as_str)
            .map(|s| s.split_once('=').map_or(s, |(name, _)| name).to_string())
            .collect();
        result.push(Container {
            key: ResourceKey {
                provider: containerhub_core::ProviderKind::Apple,
                namespace: None,
                environment: LOCAL_ENVIRONMENT.into(),
                id: id.into(),
                incarnation: format!("{created}|{digest}"),
            },
            image,
            state,
            cpus: v
                .pointer("/configuration/resources/cpus")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok()),
            memory_limit: v
                .pointer("/configuration/resources/memoryInBytes")
                .and_then(Value::as_u64),
            platform: format!(
                "{} / {}",
                text(&v, "/configuration/platform/os").unwrap_or("—"),
                text(&v, "/configuration/platform/architecture").unwrap_or("—")
            ),
            ports,
            mounts: entries(&v, "/configuration/mounts")
                .map(|m| {
                    format!(
                        "{} → {}",
                        text(m, "/type/volume/name")
                            .or_else(|| text(m, "/source"))
                            .unwrap_or("—"),
                        text(m, "/destination").unwrap_or("—")
                    )
                })
                .collect(),
            networks: entries(&v, "/configuration/networks")
                .filter_map(|n| text(n, "/network").map(str::to_string))
                .collect(),
            addresses: entries(&v, "/status/networks")
                .flat_map(|n| {
                    ["/ipv4Address", "/ipv6Address"]
                        .into_iter()
                        .filter_map(move |path| text(n, path).map(str::to_string))
                })
                .collect(),
            environment_names,
            label_names: v
                .pointer("/configuration/labels")
                .and_then(Value::as_object)
                .map(|labels| labels.keys().cloned().collect())
                .unwrap_or_default(),
            created_at: created,
            // 1.2.2's public list/inspect does not expose ContainerCreateOptions.autoRemove.
            auto_remove: None,
            compose: None,
            health: ContainerHealth::Unsupported,
        });
    }
    result.sort_by(|a, b| a.key.id.cmp(&b.key.id));
    Ok(result)
}
pub fn parse_resources(bytes: &[u8], kind: ResourceKind) -> Result<Vec<Resource>, ManagementError> {
    let mut result = Vec::new();
    for v in array(bytes)? {
        let id = required(&v, "/id")?.to_string();
        let name = required(&v, "/configuration/name")?.to_string();
        let (detail, size) = match kind {
            ResourceKind::Image => {
                let mut platforms: Vec<String> = entries(&v, "/variants")
                    .filter_map(|p| {
                        let os = text(p, "/platform/os")?;
                        let arch = text(p, "/platform/architecture")?;
                        (arch != "unknown").then(|| format!("{os}/{arch}"))
                    })
                    .collect();
                platforms.sort();
                platforms.dedup();
                (platforms.join(" · "), None)
            }
            ResourceKind::Volume => (
                format!(
                    "{} · {}",
                    text(&v, "/configuration/driver").unwrap_or("—"),
                    text(&v, "/configuration/format").unwrap_or("—")
                ),
                v.pointer("/configuration/sizeInBytes")
                    .and_then(Value::as_u64),
            ),
            ResourceKind::Network => (
                format!(
                    "{} · {}",
                    text(&v, "/configuration/mode").unwrap_or("—"),
                    text(&v, "/status/ipv4Subnet").unwrap_or("无 IPv4 子网")
                ),
                None,
            ),
        };
        result.push(Resource {
            id,
            name,
            kind,
            detail,
            size,
        });
    }
    result.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &[u8] = include_bytes!("../../../../tests/fixtures/apple/1.2.2/containers.json");
    #[test]
    fn parses_contract_and_discards_environment_values() {
        let c = parse_containers(FIXTURE).unwrap().remove(0);
        assert_eq!(c.key.id, "contract-web");
        assert_eq!(c.environment_names, vec!["PATH", "API_TOKEN"]);
        assert!(
            !serde_json::to_string(&c)
                .unwrap()
                .contains("fixture-secret")
        );
        assert_eq!(c.auto_remove, None);
        assert_eq!(c.memory_limit, Some(536870912));
        assert!(c.ports[0].display().contains("127.0.0.1:8080"));
    }
    #[test]
    fn parses_captured_1_6_0_contract() {
        let containers = parse_containers(include_bytes!(
            "../../../../tests/fixtures/apple/1.6.0/containers.json"
        ))
        .unwrap();
        assert_eq!(containers.len(), 1);
        let c = &containers[0];
        assert_eq!(c.key.id, "containerhub-smoke-20261022");
        assert_eq!(c.state, ContainerState::Stopped);
        assert_eq!(c.cpus, Some(1));
        assert!(!c.created_at.is_empty());
        assert!(!serde_json::to_string(c).unwrap().contains("REDACTED"));
    }

    #[test]
    fn inventory_search_exposes_label_names_without_values_and_reads_old_snapshots() {
        let mut value: Value = serde_json::from_slice(FIXTURE).unwrap();
        value[0]["configuration"]["labels"] =
            serde_json::json!({"org.example.team":"fixture-secret-label"});
        let c = parse_containers(&serde_json::to_vec(&value).unwrap())
            .unwrap()
            .remove(0);
        assert!(c.matches("EXAMPLE.TEAM"));
        assert!(!c.matches("fixture-secret-label"));
        assert!(
            !serde_json::to_string(&c)
                .unwrap()
                .contains("fixture-secret-label")
        );
        let mut old = serde_json::to_value(c).unwrap();
        old.as_object_mut().unwrap().remove("label_names");
        assert!(
            serde_json::from_value::<Container>(old)
                .unwrap()
                .label_names
                .is_empty()
        );
    }

    #[test]
    fn malformed_schema_is_not_an_empty_inventory() {
        for data in [b"{}".as_slice(), br#"[{"id":"x"}]"#, b"garbage"] {
            assert!(parse_containers(data).is_err());
        }
        assert!(parse_containers(b"[]").unwrap().is_empty());
    }
    #[test]
    fn unknown_states_are_preserved_and_duplicate_ids_rejected() {
        let mut v: Value = serde_json::from_slice(FIXTURE).unwrap();
        v[0]["status"]["state"] = "future-state".into();
        let c = parse_containers(&serde_json::to_vec(&v).unwrap()).unwrap();
        assert_eq!(c[0].state, ContainerState::Unknown("future-state".into()));
        let duplicate = serde_json::json!([v[0], v[0]]);
        assert!(parse_containers(&serde_json::to_vec(&duplicate).unwrap()).is_err());
    }
}
