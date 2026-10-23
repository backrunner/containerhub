//! Translate only understood public configuration fields. Values stay inside the adapter.
use super::*;
use serde_json::Value;
mod attachments;
mod process;

pub(super) struct Plan {
    pub args: Vec<String>,
    image_index: usize,
    pub environment: SecretEnvironment,
    pub config: Value,
    pub networks: Vec<String>,
    pub volumes: Vec<String>,
}
fn unsupported(field: &str) -> ManagementError {
    ManagementError::new(
        ErrorKind::Unsupported,
        format!("无法完整保留 {field} 配置，未停止或删除容器"),
    )
}
fn keys(value: &Value, allowed: &[&str], field: &str) -> Result<(), ManagementError> {
    let object = value.as_object().ok_or_else(|| unsupported(field))?;
    if object.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err(unsupported(field));
    }
    Ok(())
}
fn string(value: &Value, field: &str) -> Result<String, ManagementError> {
    value
        .as_str()
        .filter(|s| s.len() <= 16384 && !s.contains('\0'))
        .map(str::to_owned)
        .ok_or_else(|| unsupported(field))
}
fn strings(value: &Value, field: &str) -> Result<Vec<String>, ManagementError> {
    if value.is_null() {
        return Ok(vec![]);
    }
    value
        .as_array()
        .ok_or_else(|| unsupported(field))?
        .iter()
        .map(|v| string(v, field))
        .collect()
}
fn number(value: &Value, field: &str) -> Result<u64, ManagementError> {
    value.as_u64().ok_or_else(|| unsupported(field))
}
fn option(args: &mut Vec<String>, name: &str, value: String) {
    args.extend([name.into(), value]);
}
fn atom(value: &Value, field: &str) -> Result<String, ManagementError> {
    let value = string(value, field)?;
    if value.is_empty()
        || value
            .chars()
            .any(|c| c == ',' || c == ':' || c.is_control())
    {
        return Err(unsupported(field));
    }
    Ok(value)
}
impl Plan {
    pub fn pin_image(&mut self, reference: String) {
        self.args[self.image_index] = reference;
    }
    pub fn check_image_defaults(&self, bytes: &[u8]) -> Result<(), ManagementError> {
        let data: Vec<Value> =
            serde_json::from_slice(bytes).map_err(|_| unsupported("镜像默认配置"))?;
        let variant = data
            .first()
            .and_then(|v| v["variants"].as_array())
            .and_then(|vs| vs.iter().find(|v| v["platform"] == self.config["platform"]))
            .ok_or_else(|| unsupported("镜像平台"))?;
        if variant["config"]["config"]["StopSignal"] != self.config["stopSignal"] {
            return Err(unsupported("停止信号"));
        }
        Ok(())
    }
    pub fn verify(&self, bytes: &[u8]) -> Result<(), ManagementError> {
        let values: Vec<Value> =
            serde_json::from_slice(bytes).map_err(|_| unsupported("重建结果"))?;
        if values.len() != 1 {
            return Err(unsupported("重建结果"));
        }
        let actual = &values[0]["configuration"];
        for (key, value) in self.config.as_object().ok_or_else(|| unsupported("配置"))? {
            if ["id", "image", "creationDate", "publishedPorts"].contains(&key.as_str()) {
                continue;
            }
            let mut left = value.clone();
            let mut right = actual[key].clone();
            if key == "initProcess" {
                for v in [&mut left, &mut right] {
                    if let Some(env) = v["environment"].as_array_mut() {
                        env.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
                    }
                }
            }
            if left != right {
                return Err(unsupported("重建结果"));
            }
        }
        Ok(())
    }

    pub fn parse(
        bytes: &[u8],
        current: &Container,
        ports: &[Port],
    ) -> Result<Self, ManagementError> {
        let data: Vec<Value> =
            serde_json::from_slice(bytes).map_err(|_| unsupported("运行时响应"))?;
        if data.len() != 1 {
            return Err(unsupported("容器身份"));
        }
        let config = data[0]["configuration"].clone();
        keys(
            &config,
            &[
                "id",
                "image",
                "creationDate",
                "initProcess",
                "resources",
                "platform",
                "mounts",
                "networks",
                "dns",
                "labels",
                "publishedPorts",
                "publishedSockets",
                "readOnly",
                "rosetta",
                "runtimeHandler",
                "virtualization",
                "ssh",
                "useInit",
                "capAdd",
                "capDrop",
                "shmSize",
                "stopSignal",
                "maskedPaths",
                "readonlyPaths",
                "sysctls",
            ],
            "容器",
        )?;
        if config.get("id").is_some_and(|id| id != &current.key.id) {
            return Err(unsupported("容器身份"));
        }
        if config
            .get("publishedSockets")
            .is_some_and(|v| !v.is_null() && v.as_array().is_none_or(|a| !a.is_empty()))
            || config
                .get("sysctls")
                .is_some_and(|v| !v.is_null() && v.as_object().is_none_or(|o| !o.is_empty()))
        {
            return Err(unsupported("套接字或 sysctls"));
        }
        let resources = &config["resources"];
        keys(
            resources,
            &["cpus", "memoryInBytes", "cpuOverhead", "storage"],
            "资源",
        )?;
        if resources.get("storage").is_some_and(|v| !v.is_null())
            || resources
                .get("cpuOverhead")
                .is_some_and(|v| v.as_u64() != Some(1))
        {
            return Err(unsupported("存储配额或 CPU 开销"));
        }
        let cpus = number(&resources["cpus"], "CPU")?;
        let memory = number(&resources["memoryInBytes"], "内存")?;
        if cpus == 0 || memory == 0 || !memory.is_multiple_of(1024 * 1024) {
            return Err(unsupported("资源上限"));
        }
        let mut args = vec![
            "create".into(),
            "--name".into(),
            current.key.id.clone(),
            "--cpus".into(),
            cpus.to_string(),
            "--memory".into(),
            format!("{}M", memory / (1024 * 1024)),
        ];
        let platform = &config["platform"];
        keys(platform, &["os", "architecture", "variant"], "平台")?;
        let mut platform_name = format!(
            "{}/{}",
            atom(&platform["os"], "平台")?,
            atom(&platform["architecture"], "平台")?
        );
        if let Some(variant) = platform.get("variant").filter(|v| !v.is_null()) {
            platform_name.push('/');
            platform_name.push_str(&atom(variant, "平台")?);
        }
        option(&mut args, "--platform", platform_name);
        for (key, flag) in [
            ("readOnly", "--read-only"),
            ("rosetta", "--rosetta"),
            ("virtualization", "--virtualization"),
            ("ssh", "--ssh"),
            ("useInit", "--init"),
        ] {
            match config.get(key) {
                Some(Value::Bool(true)) => args.push(flag.into()),
                Some(Value::Bool(false)) | None => {}
                _ => return Err(unsupported(key)),
            }
        }
        if let Some(handler) = config.get("runtimeHandler") {
            option(&mut args, "--runtime", string(handler, "运行时")?);
        }
        if let Some(shm) = config.get("shmSize").filter(|v| !v.is_null()) {
            option(
                &mut args,
                "--shm-size",
                number(shm, "共享内存")?.to_string(),
            );
        }
        for (key, flag) in [
            ("capAdd", "--cap-add"),
            ("capDrop", "--cap-drop"),
            ("maskedPaths", "--masked-path"),
            ("readonlyPaths", "--read-only-path"),
        ] {
            if let Some(value) = config.get(key).filter(|v| !v.is_null()) {
                let values = strings(value, key)?;
                if values.is_empty() && ["maskedPaths", "readonlyPaths"].contains(&key) {
                    option(&mut args, flag, "NONE".into());
                }
                for value in values {
                    option(&mut args, flag, value);
                }
            }
        }
        let process = &config["initProcess"];
        let environment = process::configure(process, &mut args)?;
        let networks = attachments::networks(&config, current, &mut args)?;
        let volumes = attachments::mounts(&config, &mut args)?;
        if config["dns"].is_null() {
            args.push("--no-dns".into());
        } else {
            let dns = &config["dns"];
            keys(
                dns,
                &["nameservers", "options", "searchDomains", "domain"],
                "DNS",
            )?;
            for (key, flag) in [
                ("nameservers", "--dns"),
                ("options", "--dns-option"),
                ("searchDomains", "--dns-search"),
            ] {
                for value in strings(&dns[key], "DNS")? {
                    option(&mut args, flag, value);
                }
            }
            if let Some(domain) = dns.get("domain").filter(|v| !v.is_null()) {
                option(&mut args, "--dns-domain", string(domain, "DNS 域名")?);
            }
        }
        for (name, value) in config["labels"]
            .as_object()
            .ok_or_else(|| unsupported("标签"))?
        {
            if name.is_empty() || name.chars().any(|c| c == '=' || c.is_control()) {
                return Err(unsupported("标签名称"));
            }
            option(
                &mut args,
                "--label",
                format!("{name}={}", string(value, "标签")?),
            );
        }
        for port in ports {
            let host = if port.host.contains(':') {
                format!("[{}]", port.host)
            } else {
                port.host.clone()
            };
            option(
                &mut args,
                "--publish",
                format!(
                    "{host}:{}:{}/{}",
                    port.host_port, port.container_port, port.protocol
                ),
            );
        }
        args.push(format!(
            "--entrypoint={}",
            string(&process["executable"], "启动命令")?
        ));
        let image_index = args.len();
        args.push(current.image.clone());
        args.extend(strings(&process["arguments"], "启动参数")?);
        Ok(Self {
            args,
            image_index,
            environment,
            config,
            networks,
            volumes,
        })
    }
}
