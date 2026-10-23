use super::*;
use serde_json::Value;
use std::collections::HashSet;

fn invalid() -> ManagementError {
    ManagementError::new(
        ErrorKind::InvalidData,
        "镜像详情或引用关系不完整，未允许删除",
    )
}
fn text<'a>(value: &'a Value, path: &str) -> Option<&'a str> {
    value.pointer(path).and_then(Value::as_str)
}
fn parse_details(bytes: &[u8]) -> Result<ImageDetails, ManagementError> {
    let rows: Vec<Value> = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if rows.len() != 1 {
        return Err(invalid());
    }
    let row = &rows[0];
    let key = ImageKey {
        environment: LOCAL_ENVIRONMENT.into(),
        provider: ProviderKind::Apple,
        reference: text(row, "/configuration/name").ok_or_else(invalid)?.into(),
        digest: text(row, "/configuration/descriptor/digest")
            .ok_or_else(invalid)?
            .into(),
    };
    key.validate()?;
    let variants = row["variants"]
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .map(|v| {
            let digest = text(v, "/digest").ok_or_else(invalid)?;
            if !valid_digest(digest) {
                return Err(invalid());
            }
            let mut environment_names: Vec<String> = v
                .pointer("/config/config/Env")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(|s| s.split_once('=').map_or(s, |(name, _)| name).to_string())
                .collect();
            environment_names.sort();
            environment_names.dedup();
            Ok(ImageVariant {
                platform: format!(
                    "{}/{}{}",
                    text(v, "/platform/os").ok_or_else(invalid)?,
                    text(v, "/platform/architecture").ok_or_else(invalid)?,
                    text(v, "/platform/variant")
                        .map(|s| format!("/{s}"))
                        .unwrap_or_default()
                ),
                digest: digest.into(),
                size_bytes: v["size"].as_u64(),
                environment_names,
            })
        })
        .collect::<Result<Vec<_>, ManagementError>>()?;
    Ok(ImageDetails {
        key,
        observed_at: now(),
        created_at: text(row, "/configuration/creationDate").map(str::to_string),
        media_type: text(row, "/configuration/descriptor/mediaType").map(str::to_string),
        variants,
        used_by: vec![],
        other_references: vec![],
    })
}
impl AppleProvider {
    pub(super) async fn image_details(
        &self,
        environment: &str,
        reference: &str,
    ) -> Result<ImageDetails, ManagementError> {
        validate_image_query(environment, reference)?;
        if environment != LOCAL_ENVIRONMENT {
            return Err(ManagementError::new(ErrorKind::Conflict, "镜像环境不匹配"));
        }
        if self.version().await? != "1.6.0" {
            return Err(ManagementError::new(
                ErrorKind::Unsupported,
                "当前版本未验证镜像详情格式",
            ));
        }
        let mut image = parse_details(
            &self
                .run(&["image", "inspect", "--", reference], false)
                .await?,
        )?;
        let digests: HashSet<_> = std::iter::once(image.key.digest.as_str())
            .chain(image.variants.iter().map(|v| v.digest.as_str()))
            .collect();
        for container in self.list().await? {
            let digest = container
                .key
                .incarnation
                .rsplit_once('|')
                .map(|(_, d)| d)
                .filter(|d| valid_digest(d))
                .ok_or_else(invalid)?;
            if container.image == image.key.reference || digests.contains(digest) {
                image.used_by.push(container.key);
            }
        }
        // Compare full descriptors, not display names or truncated IDs, to identify alternate tags.
        let rows: Vec<Value> = serde_json::from_slice(
            &self
                .run(&["image", "list", "--format", "json"], false)
                .await?,
        )
        .map_err(|_| invalid())?;
        for row in rows {
            let name = text(&row, "/configuration/name").ok_or_else(invalid)?;
            let digest = text(&row, "/configuration/descriptor/digest")
                .filter(|d| valid_digest(d))
                .ok_or_else(invalid)?;
            if digest == image.key.digest && name != image.key.reference {
                image.other_references.push(name.to_string());
            }
        }
        image.other_references.sort();
        image.other_references.dedup();
        // Refuse a tag changed while its relationships were being inspected.
        let latest = parse_details(
            &self
                .run(&["image", "inspect", "--", &image.key.reference], false)
                .await?,
        )?;
        if latest.key != image.key {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "镜像标签已改变，请重新读取详情",
            ));
        }
        Ok(image)
    }
    pub(super) async fn delete_image(
        &self,
        key: &ImageKey,
    ) -> Result<CommandResult, ManagementError> {
        key.validate()?;
        if key.provider != ProviderKind::Apple {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "镜像运行时不匹配",
            ));
        }
        let current = self.image_details(&key.environment, &key.reference).await?;
        if current.key != *key {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "镜像标签已指向其他内容，未删除",
            ));
        }
        if !current.used_by.is_empty() {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "运行或停止的容器仍引用该镜像，请先处理这些容器",
            ));
        }
        // Explicit reference only: a digest lookup may select every tag sharing the same content.
        let command = self
            .run(&["image", "delete", "--", &key.reference], true)
            .await;
        // The CLI also performs orphan cleanup. Any failure after issuing delete can follow a successful untag.
        let rows = self.resources(ResourceKind::Image).await.map_err(|_| {
            ManagementError::new(ErrorKind::UnknownOutcome, "镜像删除已提交，但无法核对列表")
        })?;
        if rows.iter().any(|r| r.name == key.reference) {
            return Err(ManagementError::new(
                ErrorKind::UnknownOutcome,
                "镜像引用仍存在或已被重建，未继续删除",
            ));
        }
        if command.is_err() {
            return Err(ManagementError::new(
                ErrorKind::UnknownOutcome,
                "镜像引用已消失，但运行时清理未确认完成，请刷新核对",
            ));
        }
        Ok(CommandResult::Confirmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inspect_allowlist_discards_image_secrets_and_requires_digest() {
        let mut value = serde_json::json!([{"configuration":{"name":"example.invalid/a:v1","descriptor":{"digest":format!("sha256:{}","a".repeat(64)),"annotations":{"token":"private-label"}}},"variants":[{"platform":{"os":"linux","architecture":"arm64"},"digest":format!("sha256:{}","b".repeat(64)),"size":123,"config":{"config":{"Env":["API_TOKEN=private-env","PATH=/bin"],"Cmd":["--password=private-command"]}}}]}]);
        let details = parse_details(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(details.variants[0].environment_names, ["API_TOKEN", "PATH"]);
        assert!(
            !serde_json::to_string(&details)
                .unwrap()
                .contains("private-")
        );
        value[0]["configuration"]["descriptor"]["digest"] = "sha256:short".into();
        assert!(parse_details(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}
