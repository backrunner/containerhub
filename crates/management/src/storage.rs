use serde::Serialize;
use std::path::Path;

pub fn save_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    use std::io::Write;
    std::fs::create_dir_all(path.parent().unwrap())?;
    let temporary = path.with_extension("json.tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.sync_all()?;
    std::fs::rename(temporary, path)?;
    #[cfg(unix)]
    std::fs::File::open(path.parent().unwrap())?.sync_all()?;
    Ok(())
}
