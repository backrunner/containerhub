use crate::{crypto::*, store::*};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use zeroize::Zeroizing;
#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "dev.containerhub.remote.v1";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "storage", rename_all = "snake_case", deny_unknown_fields)]
pub enum KeyReference {
    Keychain { account: String },
    File { name: String },
}
impl KeyReference {
    pub fn store(directory: &Path, key: &SecretKey, development: bool) -> Result<Self> {
        let name = nonce();
        #[cfg(target_os = "macos")]
        if !development {
            security_framework::passwords::set_generic_password(
                KEYCHAIN_SERVICE,
                &name,
                &key.secret_bytes(),
            )?;
            return Ok(Self::Keychain { account: name });
        }
        #[cfg(not(target_os = "macos"))]
        let _ = development;
        let name = format!("{name}.key");
        create_private(&directory.join(&name), &key.secret_bytes())?;
        Ok(Self::File { name })
    }
    pub fn load(&self, directory: &Path) -> Result<SecretKey> {
        let bytes = match self {
            Self::Keychain { account } => {
                #[cfg(target_os = "macos")]
                {
                    Zeroizing::new(security_framework::passwords::get_generic_password(
                        KEYCHAIN_SERVICE,
                        account,
                    )?)
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = account;
                    anyhow::bail!("this identity belongs to a macOS Keychain");
                }
            }
            Self::File { name } => {
                ensure!(
                    !name.contains(['/', '\\']) && name.ends_with(".key"),
                    "invalid key reference"
                );
                let path = directory.join(name);
                let metadata = path.symlink_metadata()?;
                ensure!(
                    metadata.is_file()
                        && !metadata.file_type().is_symlink()
                        && metadata.len() == 32,
                    "invalid private key file"
                );
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    ensure!(
                        metadata.permissions().mode() & 0o077 == 0,
                        "private key permissions are too broad"
                    );
                }
                Zeroizing::new(fs::read(path)?)
            }
        };
        SecretKey::from_bytes(&bytes)
    }
}
