use super::*;
use serde_json::Value;

impl AppleProvider {
    pub(super) async fn container_details(
        &self,
        key: &ResourceKey,
    ) -> Result<ContainerDetails, ManagementError> {
        self.current(key).await?;
        let bytes = self.run(&["inspect", "--", &key.id], false).await?;
        let details = parse_details(&bytes, key)?;
        self.current(key).await?;
        Ok(details)
    }
}
fn parse_details(bytes: &[u8], key: &ResourceKey) -> Result<ContainerDetails, ManagementError> {
    let mut containers = parse_containers(bytes)?;
    if containers.len() != 1 || containers[0].key != *key {
        return Err(ManagementError::new(
            ErrorKind::Conflict,
            "检查期间容器身份已改变",
        ));
    }
    let raw: Value = serde_json::from_slice(bytes)
        .map_err(|_| ManagementError::new(ErrorKind::InvalidData, "容器检查结果无效"))?;
    let config = &raw[0]["configuration"];
    let process = &config["initProcess"];
    let container = containers.remove(0);
    let user = process
        .pointer("/user/id/uid")
        .and_then(Value::as_u64)
        .map(
            |uid| match process.pointer("/user/id/gid").and_then(Value::as_u64) {
                Some(gid) => format!("{uid}:{gid}"),
                None => uid.to_string(),
            },
        );
    Ok(ContainerDetails {
        environment_names: container.environment_names.clone(),
        container,
        observed_at: now(),
        read_only: config["readOnly"].as_bool(),
        working_directory: process["workingDirectory"].as_str().map(str::to_owned),
        user,
        label_names: config["labels"]
            .as_object()
            .map(|labels| labels.keys().cloned().collect())
            .unwrap_or_default(),
        argument_count: process["arguments"].as_array().map_or(0, Vec::len),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inspect_allowlist_and_identity() {
        let mut raw: Value = serde_json::from_slice(include_bytes!(
            "../../../../tests/fixtures/apple/1.6.0/containers.json"
        ))
        .unwrap();
        raw[0]["configuration"]["initProcess"]["arguments"] =
            serde_json::json!(["--password=hidden-argument"]);
        raw[0]["configuration"]["labels"] = serde_json::json!({"TOKEN":"hidden-label"});
        raw[0]["configuration"]["initProcess"]["environment"] =
            serde_json::json!(["TOKEN=hidden-env"]);
        let bytes = serde_json::to_vec(&raw).unwrap();
        let mut key = parse_containers(&bytes).unwrap()[0].key.clone();
        let details = parse_details(&bytes, &key).unwrap();
        let json = serde_json::to_string(&details).unwrap();
        assert!(!json.contains("hidden-"));
        assert_eq!(details.argument_count, 1);
        assert_eq!(details.user.as_deref(), Some("0:0"));
        key.incarnation.push('x');
        assert_eq!(
            parse_details(&bytes, &key).unwrap_err().kind,
            ErrorKind::Conflict
        );
    }
}
