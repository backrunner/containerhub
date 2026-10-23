use containerhub_core::LogExport;
use std::{io::Write, path::Path};

/// Save locally, only to a new file. Never overwrite a log, follow a destination symlink, or send a path to an Agent.
pub fn save_log(path: &Path, export: &LogExport) -> std::io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| std::io::Error::other("invalid export directory"))?;
    let mut file = tempfile::Builder::new()
        .prefix(".containerhub-export-")
        .tempfile_in(parent)?;
    file.write_all(export.content.as_bytes())?;
    file.as_file().sync_all()?;
    file.persist_noclobber(path).map_err(|e| e.error)?;
    #[cfg(unix)]
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_is_private_and_never_overwrites_an_existing_target() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("log.txt");
        let export = LogExport {
            file_name: "log.txt".into(),
            content: "frozen log\n".into(),
            line_count: 1,
            snapshot_lines: 1,
            truncated: false,
        };
        save_log(&path, &export).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), export.content);
        assert_eq!(
            save_log(&path, &export).unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            let link = directory.path().join("link.txt");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert_eq!(
                save_log(&link, &export).unwrap_err().kind(),
                std::io::ErrorKind::AlreadyExists
            );
            assert_eq!(std::fs::read_to_string(&path).unwrap(), export.content);
        }
    }
}
