use super::*;

impl FakeProvider {
    pub(super) fn describe_image(
        &self,
        environment: &str,
        reference: &str,
    ) -> Result<ImageDetails, ManagementError> {
        validate_image_query(environment, reference)?;
        if environment != "demo" {
            return Err(ManagementError::new(ErrorKind::Conflict, "示例环境不匹配"));
        }
        let containers = self.containers.lock().unwrap();
        let images = self.images.lock().unwrap();
        let image = images
            .iter()
            .find(|r| r.name == reference)
            .ok_or_else(|| ManagementError::new(ErrorKind::NotFound, "示例镜像不存在"))?;
        Ok(ImageDetails {
            key: ImageKey {
                environment: "demo".into(),
                provider: ProviderKind::Apple,
                reference: image.name.clone(),
                digest: format!("sha256:{}", image.id),
            },
            observed_at: now(),
            created_at: Some("2026-10-22T00:00:00Z".into()),
            media_type: Some("application/vnd.oci.image.index.v1+json".into()),
            variants: vec![ImageVariant {
                platform: "linux/arm64".into(),
                digest: format!("sha256:{}", image.id),
                size_bytes: Some(8 * 1024 * 1024),
                environment_names: vec!["PATH".into()],
            }],
            used_by: containers
                .iter()
                .filter(|c| c.image == reference)
                .map(|c| c.key.clone())
                .collect(),
            other_references: images
                .iter()
                .filter(|r| r.id == image.id && r.name != image.name)
                .map(|r| r.name.clone())
                .collect(),
        })
    }
    pub(super) fn remove_image(&self, key: &ImageKey) -> Result<CommandResult, ManagementError> {
        let current = self.describe_image(&key.environment, &key.reference)?;
        if current.key != *key {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "示例镜像身份已改变",
            ));
        }
        // Keep the relationship check and removal under the same lock order as inspect.
        let containers = self.containers.lock().unwrap();
        let mut images = self.images.lock().unwrap();
        if containers.iter().any(|c| c.image == key.reference) {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                "仍有容器引用镜像",
            ));
        }
        let index = images
            .iter()
            .position(|r| r.name == key.reference && format!("sha256:{}", r.id) == key.digest)
            .ok_or_else(|| ManagementError::new(ErrorKind::Conflict, "镜像身份已改变"))?;
        images.remove(index);
        Ok(CommandResult::Confirmed)
    }
}
