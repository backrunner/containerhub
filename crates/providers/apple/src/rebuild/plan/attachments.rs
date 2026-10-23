use super::*;
pub(super) fn networks(
    config: &Value,
    current: &Container,
    args: &mut Vec<String>,
) -> Result<Vec<String>, ManagementError> {
    if config["networks"].as_array().is_none_or(|v| v.is_empty()) {
        return Err(unsupported("未连接网络"));
    }
    let mut networks = vec![];
    for network in config["networks"]
        .as_array()
        .ok_or_else(|| unsupported("网络"))?
    {
        keys(network, &["network", "options"], "网络")?;
        let name = atom(&network["network"], "网络")?;
        networks.push(name.clone());
        let mut spec = name;
        if let Some(options) = network.get("options") {
            keys(
                options,
                &["hostname", "mtu", "macAddress", "aliases"],
                "网络选项",
            )?;
            if options
                .get("hostname")
                .is_some_and(|v| v != &current.key.id)
            {
                return Err(unsupported("主机名"));
            }
            if let Some(mtu) = options.get("mtu") {
                spec.push_str(&format!(",mtu={}", number(mtu, "MTU")?));
            }
            if let Some(mac) = options.get("macAddress") {
                let mac = string(mac, "MAC")?;
                let parts: Vec<_> = mac.split(':').collect();
                if parts.len() != 6
                    || parts
                        .iter()
                        .any(|p| p.len() != 2 || !p.chars().all(|c| c.is_ascii_hexdigit()))
                {
                    return Err(unsupported("MAC 地址"));
                }
                spec.push_str(&format!(",mac={mac}"));
            }
            for alias in strings(&options["aliases"], "网络别名")? {
                if alias.is_empty()
                    || !alias
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || ['.', '-', '_'].contains(&c))
                {
                    return Err(unsupported("网络别名"));
                }
                spec.push_str(&format!(",alias={alias}"));
            }
        }
        option(args, "--network", spec);
    }
    Ok(networks)
}
pub(super) fn mounts(
    config: &Value,
    args: &mut Vec<String>,
) -> Result<Vec<String>, ManagementError> {
    let mut volumes = vec![];
    for mount in config["mounts"]
        .as_array()
        .ok_or_else(|| unsupported("挂载"))?
    {
        keys(mount, &["type", "source", "destination", "options"], "挂载")?;
        let options = strings(&mount["options"], "挂载选项")?;
        if options.iter().any(|o| !["ro", "rw"].contains(&o.as_str())) {
            return Err(unsupported("挂载选项"));
        }
        let destination = atom(&mount["destination"], "挂载目标")?;
        let spec = if let Some(volume) = mount["type"].get("volume") {
            keys(volume, &["name", "format", "cache", "sync"], "命名卷")?;
            if volume.get("cache").is_some_and(|v| v != "on")
                || volume.get("sync").is_some_and(|v| v != "fsync")
            {
                return Err(unsupported("命名卷缓存"));
            }
            let name = atom(&volume["name"], "卷名")?;
            volumes.push(name.clone());
            format!("type=volume,source={name},target={destination}")
        } else if mount["type"].get("virtiofs").is_some() {
            let source = atom(&mount["source"], "绑定目录")?;
            if !std::path::Path::new(&source).is_absolute() {
                return Err(unsupported("绑定目录"));
            }
            format!("type=bind,source={source},target={destination}")
        } else {
            return Err(unsupported("挂载类型"));
        };
        option(
            args,
            "--mount",
            format!(
                "{spec}{}",
                if options.iter().any(|o| o == "ro") {
                    ",readonly"
                } else {
                    ""
                }
            ),
        );
    }
    Ok(volumes)
}
