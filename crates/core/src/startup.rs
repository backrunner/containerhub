//! Explicit argv, never a shell command line. These non-secret options are durable task data.
use crate::{ErrorKind, ManagementError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartupCommand {
    /// None preserves the image ENTRYPOINT. Some replaces it and discards the image CMD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,
    /// Replaces image CMD, with exact argument boundaries and empty values preserved.
    #[serde(default)]
    pub arguments: Vec<String>,
}
impl StartupCommand {
    pub const MAX_ARGUMENTS: usize = 64;
    pub fn validate(&self) -> Result<(), ManagementError> {
        if self.entrypoint.is_none() && self.arguments.is_empty() {
            return Err(ManagementError::new(
                ErrorKind::InvalidData,
                "请填写启动程序或至少一个参数，或恢复镜像默认命令",
            ));
        }
        if self
            .entrypoint
            .as_ref()
            .is_some_and(|value| value.trim().is_empty() || value.chars().any(char::is_control))
        {
            return Err(ManagementError::new(
                ErrorKind::InvalidData,
                "启动程序不能为空或包含控制字符",
            ));
        }
        let mut total = 0;
        for value in self.entrypoint.iter().chain(&self.arguments) {
            total += value.len() + 1;
            if value.len() > 4096 || value.contains('\0') {
                return Err(ManagementError::new(
                    ErrorKind::InvalidData,
                    "启动程序和每项参数最多 4096 字节，不能包含空字符",
                ));
            }
        }
        if self.arguments.len() > Self::MAX_ARGUMENTS || total > 32 * 1024 {
            return Err(ManagementError::new(
                ErrorKind::InvalidData,
                "启动参数最多 64 项，启动配置总计最多 32 KiB",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_boundaries_and_validates_explicit_override_and_limits() {
        let arguments = vec![
            "".into(),
            "two words".into(),
            "--name".into(),
            "$HOME; $(echo x)".into(),
            "中文\n行".into(),
        ];
        let command = StartupCommand {
            entrypoint: Some("/path with spaces/app".into()),
            arguments: arguments.clone(),
        };
        command.validate().unwrap();
        let decoded: StartupCommand =
            serde_json::from_str(&serde_json::to_string(&command).unwrap()).unwrap();
        assert_eq!(decoded.arguments, arguments);
        for bad in [
            StartupCommand {
                entrypoint: None,
                arguments: vec![],
            },
            StartupCommand {
                entrypoint: Some("".into()),
                arguments: vec!["arg".into()],
            },
            StartupCommand {
                entrypoint: Some(" \t".into()),
                arguments: vec![],
            },
            StartupCommand {
                entrypoint: Some("sh\n-c".into()),
                arguments: vec![],
            },
            StartupCommand {
                entrypoint: None,
                arguments: vec!["x\0y".into()],
            },
            StartupCommand {
                entrypoint: None,
                arguments: vec!["x".repeat(4097)],
            },
            StartupCommand {
                entrypoint: None,
                arguments: vec!["".into(); 65],
            },
            StartupCommand {
                entrypoint: None,
                arguments: vec!["x".repeat(4096); 8],
            },
        ] {
            assert!(bad.validate().is_err());
        }
        StartupCommand {
            entrypoint: Some("/app".into()),
            arguments: vec![],
        }
        .validate()
        .unwrap();
        StartupCommand {
            entrypoint: None,
            arguments: vec!["".into()],
        }
        .validate()
        .unwrap();
    }
    #[test]
    fn old_options_keep_the_same_wire_shape_and_defaults() {
        let options = crate::CreateOptions::default();
        let encoded = serde_json::to_value(&options).unwrap();
        assert!(encoded.get("startup").is_none());
        assert!(
            serde_json::from_value::<crate::CreateOptions>(encoded)
                .unwrap()
                .startup
                .is_none()
        );
        assert!(serde_json::from_str::<StartupCommand>(r#"{"arguments":"sh -c echo"}"#).is_err());
    }
}
