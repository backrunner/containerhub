use super::*;
use std::{
    collections::HashMap,
    path::Path,
    sync::Arc,
    time::{Instant, SystemTime},
};

#[derive(Clone, PartialEq)]
struct Fingerprint {
    path: PathBuf,
    modified: Option<SystemTime>,
    size: u64,
}
pub(super) struct Cached {
    fingerprint: Option<Fingerprint>,
    at: Instant,
    report: RuntimeDiagnostics,
}

/// The path is explicit trusted local configuration, never a remote RPC argument.
pub fn validate_binary(path: &Path) -> Result<(), ManagementError> {
    validate_path(path)?;
    let metadata = std::fs::metadata(path).map_err(|_| {
        ManagementError::new(ErrorKind::NotInstalled, "所选 CLI 文件不存在或不可访问")
    })?;
    if !metadata.is_file() {
        return Err(ManagementError::new(
            ErrorKind::InvalidData,
            "请选择 CLI 可执行文件",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(ManagementError::new(
                ErrorKind::PermissionDenied,
                "所选 CLI 没有执行权限",
            ));
        }
    }
    Ok(())
}
fn validate_path(path: &Path) -> Result<(), ManagementError> {
    if !path.is_absolute()
        || path
            .to_str()
            .is_none_or(|s| s.len() > 4096 || s.chars().any(char::is_control))
    {
        return Err(ManagementError::new(
            ErrorKind::InvalidData,
            "CLI 必须使用绝对文件路径",
        ));
    }
    Ok(())
}
impl AppleProvider {
    pub fn with_binary(path: PathBuf) -> Result<Self, ManagementError> {
        validate_path(&path)?;
        // Missing paths remain visible in diagnostics so the desktop can repair configuration.
        Ok(Self {
            binary: Some(path),
            ..Self::default()
        })
    }
    pub(super) async fn discover(&self, refresh: bool) -> RuntimeDiagnostics {
        let requested_at = Instant::now();
        let fingerprint = self.binary.as_ref().and_then(|path| {
            std::fs::metadata(path).ok().map(|m| Fingerprint {
                path: path.clone(),
                modified: m.modified().ok(),
                size: m.len(),
            })
        });
        let mut cache = self.discovery.lock().await;
        if let Some(cached) = cache.as_ref()
            && (!refresh || cached.at >= requested_at)
            && cached.fingerprint == fingerprint
            && cached.at.elapsed() < Duration::from_secs(30)
        {
            return cached.report.clone();
        }
        let version = self.version().await;
        let mut help = HashMap::new();
        if version.is_ok()
            && let Some(path) = self.binary.as_ref()
        {
            let slots = Arc::new(tokio::sync::Semaphore::new(4));
            let mut tasks = tokio::task::JoinSet::new();
            for command in [
                "list",
                "image list",
                "volume list",
                "network list",
                "logs",
                "inspect",
                "image inspect",
                "stats",
                "start",
                "stop",
                "image pull",
                "create",
                "delete",
                "image delete",
                "system status",
                "system start",
            ] {
                let path = path.clone();
                let slots = slots.clone();
                tasks.spawn(async move {
                    let _slot = slots.acquire().await.expect("discovery semaphore");
                    let mut args: Vec<_> = command.split(' ').collect();
                    args.push("--help");
                    let result =
                        process::run_with_timeout(&path, &args, false, Duration::from_secs(2))
                            .await
                            .ok()
                            .and_then(|bytes| String::from_utf8(bytes).ok())
                            .filter(|text| {
                                text.lines().any(|line| {
                                    line.starts_with(&format!("USAGE: container {command} "))
                                        || line == format!("USAGE: container {command}")
                                })
                            });
                    (command, result)
                });
            }
            while let Some(result) = tasks.join_next().await {
                if let Ok((command, Some(text))) = result {
                    help.insert(command, text);
                }
            }
        }
        let report = report(self.binary.as_deref(), version, &help);
        *cache = Some(Cached {
            fingerprint,
            at: Instant::now(),
            report: report.clone(),
        });
        report
    }
    pub(super) async fn require(&self, feature: RuntimeFeature) -> Result<(), ManagementError> {
        self.discover(false).await.require(feature)
    }
    pub(super) async fn service_status(&self) -> Result<(), ManagementError> {
        let bytes = process::run_with_timeout(
            self.binary.as_ref().ok_or_else(|| {
                ManagementError::new(ErrorKind::NotInstalled, "未找到 Apple container")
            })?,
            &["system", "status", "--format", "json"],
            false,
            Duration::from_secs(3),
        )
        .await
        .map_err(|error| {
            if error.kind == ErrorKind::NotFound {
                ManagementError::new(
                    ErrorKind::Unavailable,
                    "Apple container 服务不可访问；请在终端检查 container system status",
                )
            } else {
                error
            }
        })?;
        let status: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|_| ManagementError::new(ErrorKind::InvalidData, "运行时状态格式无法识别"))?;
        if status.get("status").and_then(|s| s.as_str()) != Some("running") {
            return Err(ManagementError::new(
                ErrorKind::Unavailable,
                "Apple container 服务未运行；可在终端运行 container system start 后刷新",
            ));
        }
        Ok(())
    }
}

fn flag(text: &str, required: &str) -> bool {
    // Only option declarations count; prose mentioning an unavailable option does not.
    text.lines()
        .filter(|line| line.trim_start().starts_with('-'))
        .any(|line| {
            line.trim_start()
                .split("  ")
                .next()
                .unwrap_or("")
                .split(|c: char| c.is_whitespace() || c == ',' || c == '/')
                .any(|token| token == required)
        })
}
fn report(
    path: Option<&Path>,
    version: Result<String, ManagementError>,
    help: &HashMap<&str, String>,
) -> RuntimeDiagnostics {
    let has = |command: &str, flags: &[&str]| {
        help.get(command)
            .is_some_and(|text| flags.iter().all(|required| flag(text, required)))
    };
    let containers = has("list", &["--all", "--format"]);
    let images = has("image list", &["--format"]);
    let inspect_image = has("image inspect", &[]);
    let create = containers
        && inspect_image
        && has(
            "create",
            &["--name", "--cpus", "--memory", "--publish", "--label"],
        );
    let pull = inspect_image
        && has("image pull", &["--progress"])
        && help
            .get("image pull")
            .is_some_and(|text| text.contains("plain"));
    let supported_version = version.as_deref().ok() == Some("1.6.0");
    let checks = RuntimeFeature::ALL
        .into_iter()
        .map(|feature| {
            let contract = match feature {
                RuntimeFeature::StartRuntime => {
                    has("system status", &["--format"])
                        && has("system start", &["--disable-kernel-install", "--timeout"])
                }
                RuntimeFeature::RebuildPorts => {
                    create
                        && containers
                        && inspect_image
                        && has("inspect", &[])
                        && has("start", &[])
                        && has("stop", &["--time"])
                        && has("delete", &[])
                        && has(
                            "create",
                            &[
                                "--env-file",
                                "--entrypoint",
                                "--mount",
                                "--network",
                                "--user",
                                "--workdir",
                                "--read-only",
                                "--platform",
                                "--runtime",
                                "--tty",
                                "--ulimit",
                                "--rosetta",
                                "--virtualization",
                                "--ssh",
                                "--init",
                                "--shm-size",
                                "--cap-add",
                                "--cap-drop",
                                "--masked-path",
                                "--read-only-path",
                                "--no-dns",
                                "--dns",
                                "--dns-option",
                                "--dns-search",
                                "--dns-domain",
                            ],
                        )
                }
                RuntimeFeature::Stacks => false,
                RuntimeFeature::Containers => containers,
                RuntimeFeature::Images => images,
                RuntimeFeature::Volumes => has("volume list", &["--format"]),
                RuntimeFeature::Networks => has("network list", &["--format"]),
                RuntimeFeature::Logs => containers && has("logs", &["-n"]),
                RuntimeFeature::BootLogs => containers && has("logs", &["-n", "--boot"]),
                RuntimeFeature::InspectContainer => containers && has("inspect", &[]),
                RuntimeFeature::InspectImage => containers && images && inspect_image,
                RuntimeFeature::Stats => containers && has("stats", &["--format", "--no-stream"]),
                RuntimeFeature::Lifecycle => {
                    containers && has("start", &[]) && has("stop", &["--time"])
                }
                RuntimeFeature::PullImage | RuntimeFeature::CancelPull => pull,
                RuntimeFeature::CreateContainer => create,
                RuntimeFeature::CreateAndStart => {
                    create && has("start", &[]) && has("stop", &["--time"])
                }
                RuntimeFeature::StartupCommand => create && has("create", &["--entrypoint"]),
                RuntimeFeature::SecretEnvironment => create && has("create", &["--env-file"]),
                RuntimeFeature::AdvancedCreate => {
                    create
                        && has(
                            "create",
                            &["--network", "--mount", "--user", "--workdir", "--read-only"],
                        )
                        && has("network list", &["--format"])
                        && has("volume list", &["--format"])
                }
                RuntimeFeature::DeleteContainer => containers && has("delete", &[]),
                RuntimeFeature::DeleteImage => {
                    containers && images && inspect_image && has("image delete", &[])
                }
            };
            let available = contract && (!feature.writes() || supported_version);
            let reason = if version.is_err() {
                Some("CLI 未安装、不可执行或版本无法识别".into())
            } else if feature.writes() && !supported_version {
                Some("此版本写操作尚未验证；当前仅开放可解析的读取，写适配版本为 1.6.0".into())
            } else if !contract {
                Some(format!("{}所需命令或参数未通过帮助检查", feature.label()))
            } else {
                None
            };
            CapabilityCheck {
                feature,
                available,
                reason,
            }
        })
        .collect();
    RuntimeDiagnostics {
        environment: LOCAL_ENVIRONMENT.into(),
        cli_path: path.map(|p| p.display().to_string()),
        cli_version: version.as_ref().ok().cloned(),
        observed_at: now(),
        service_error: version.err(),
        checks,
    }
}

#[cfg(all(test, unix))]
#[path = "discovery_tests.rs"]
mod contract_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_must_be_option_declarations_not_prose_or_prefixes() {
        assert!(!flag(
            "Description mentions --mount\n --mounts <value>  Mounts",
            "--mount"
        ));
        assert!(flag("  -w, --workdir, --cwd <dir>  Directory", "--workdir"));
        assert!(flag("  -n <count>  Number of lines", "-n"));
        assert!(flag(
            "  --enable-kernel-install/--disable-kernel-install  Install kernel",
            "--disable-kernel-install"
        ));
    }
}
