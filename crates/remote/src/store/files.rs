use crate::crypto::nonce;
use anyhow::{Context, Result, bail, ensure};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let metadata = path.symlink_metadata()?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() <= 1024 * 1024,
        "invalid state file"
    );
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
pub fn private_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    ensure!(
        path.is_dir() && !path.symlink_metadata()?.file_type().is_symlink(),
        "unsafe state directory"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
pub fn create_private(path: &Path, data: &[u8]) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(data)?;
    file.sync_all()?;
    sync_directory(path.parent().context("state has no parent")?)
}
pub fn replace_private(path: &Path, data: &[u8]) -> Result<()> {
    let temporary: PathBuf = path.with_extension(format!("{}.tmp", nonce()));
    create_private(&temporary, data)?;
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        bail!(error);
    }
    sync_directory(path.parent().context("state has no parent")?)
}
pub(super) fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::File::open(path)?.sync_all()?;
    Ok(())
}
