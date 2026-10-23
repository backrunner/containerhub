use super::*;
pub(super) fn configure(
    process: &Value,
    args: &mut Vec<String>,
) -> Result<SecretEnvironment, ManagementError> {
    keys(
        process,
        &[
            "executable",
            "arguments",
            "environment",
            "workingDirectory",
            "terminal",
            "user",
            "supplementalGroups",
            "rlimits",
        ],
        "启动进程",
    )?;
    if process
        .get("supplementalGroups")
        .is_some_and(|v| v.as_array().is_none_or(|a| !a.is_empty()))
    {
        return Err(unsupported("附加用户组"));
    }
    if process["terminal"] == true {
        args.push("--tty".into());
    }
    if let Some(limits) = process.get("rlimits") {
        for limit in limits.as_array().ok_or_else(|| unsupported("ulimit"))? {
            keys(limit, &["limit", "soft", "hard"], "ulimit")?;
            let name = string(&limit["limit"], "ulimit")?.to_lowercase();
            option(
                args,
                "--ulimit",
                format!(
                    "{}={}:{}",
                    name.trim_start_matches("rlimit_"),
                    number(&limit["soft"], "ulimit")?,
                    number(&limit["hard"], "ulimit")?
                ),
            );
        }
    }
    option(
        args,
        "--workdir",
        string(&process["workingDirectory"], "工作目录")?,
    );
    let user = &process["user"];
    let user_text = if let Some(id) = user.get("id") {
        keys(id, &["uid", "gid"], "用户")?;
        format!(
            "{}:{}",
            number(&id["uid"], "用户")?,
            number(&id["gid"], "用户")?
        )
    } else if let Some(raw) = user.get("raw") {
        keys(raw, &["userString"], "用户")?;
        string(&raw["userString"], "用户")?
    } else {
        return Err(unsupported("用户"));
    };
    option(args, "--user", user_text);
    let mut environment = SecretEnvironment::default();
    for value in strings(&process["environment"], "环境变量")? {
        let (name, value) = value
            .split_once('=')
            .ok_or_else(|| unsupported("环境变量"))?;
        environment
            .insert(name.into(), value.into())
            .map_err(|_| unsupported("环境变量编码或数量"))?;
    }
    if !environment.is_empty() {
        option(args, "--env-file", "/dev/stdin".into());
    }
    Ok(environment)
}
