use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateOptions {
    /// Managed workflow: persist the created identity before requesting a start.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub start_after_create: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startup: Option<Box<StartupCommand>>,
    /// Names only. Values travel in one-shot execution input, never this durable DTO.
    #[serde(default)]
    pub secret_environment: Vec<String>,
    #[serde(default)]
    pub networks: Vec<String>,
    #[serde(default)]
    pub volumes: Vec<VolumeMount>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub working_directory: Option<String>,
    #[serde(default)]
    pub read_only: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolumeMount {
    pub name: String,
    pub destination: String,
    #[serde(default)]
    pub read_only: bool,
}
fn container_path(path: &str) -> bool {
    path.starts_with('/')
        && path.len() <= 1024
        && !path.chars().any(|c| c.is_control() || c == ',' || c == ':')
        && !path.split('/').any(|p| p == ".." || p == ".")
}
impl CreateOptions {
    pub fn has_advanced(&self) -> bool {
        !self.networks.is_empty()
            || !self.volumes.is_empty()
            || self.user.is_some()
            || self.working_directory.is_some()
            || self.read_only
    }
    pub fn validate(&self) -> Result<(), ManagementError> {
        if let Some(startup) = &self.startup {
            startup.validate()?;
        }
        crate::validate_secret_names(&self.secret_environment)?;
        let mut networks = std::collections::HashSet::new();
        let mut destinations = std::collections::HashSet::new();
        if self.networks.len() > 8
            || self.volumes.len() > 16
            || self
                .networks
                .iter()
                .any(|n| !valid_container_name(n) || !networks.insert(n))
            || self.volumes.iter().any(|v| {
                !valid_container_name(&v.name)
                    || !container_path(&v.destination)
                    || v.destination == "/"
                    || !destinations.insert(v.destination.trim_end_matches('/'))
            })
            || self
                .working_directory
                .as_ref()
                .is_some_and(|p| !container_path(p))
            || self.user.as_ref().is_some_and(|u| {
                u.is_empty()
                    || u.len() > 128
                    || u.split(':').count() > 2
                    || u.split(':').any(|p| {
                        p.is_empty()
                            || p.starts_with('-')
                            || !p
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
                    })
            })
        {
            return Err(ManagementError::new(
                ErrorKind::InvalidData,
                "网络、命名卷、用户或容器目录格式无效",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn advanced_options_refuse_host_paths_delimiters_and_duplicate_destinations() {
        for value in [
            serde_json::json!({"volumes":[{"name":"/etc", "destination":"/data"}]}),
            serde_json::json!({"volumes":[{"name":"data", "destination":"/x,readonly"}]}),
            serde_json::json!({"volumes":[{"name":"data", "destination":"/x"},{"name":"other","destination":"/x/"}]}),
            serde_json::json!({"working_directory":"/a/../etc"}),
            serde_json::json!({"networks":["default", "default"]}),
            serde_json::json!({"networks":["default,alias=other"]}),
            serde_json::json!({"user":"--root"}),
        ] {
            assert!(
                serde_json::from_value::<CreateOptions>(value)
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
        let options: CreateOptions = serde_json::from_value(
            serde_json::json!({"user":"1000:1000", "working_directory":"/app", "read_only":true}),
        )
        .unwrap();
        options.validate().unwrap();
        assert!(
            serde_json::from_value::<CreateOptions>(serde_json::json!({"env":{"TOKEN":"secret"}}))
                .is_err()
        );
    }
}
