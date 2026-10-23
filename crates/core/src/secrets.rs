//! One-shot execution input. Never put this type in Command, Operation or a journal.
use crate::{Command, ErrorKind, ManagementError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecretEnvironment(BTreeMap<String, SecretValue>);
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
struct SecretValue(String);
impl Drop for SecretValue {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}
impl std::fmt::Debug for SecretEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretEnvironment")
            .field("names", &self.names())
            .finish_non_exhaustive()
    }
}
fn invalid() -> ManagementError {
    ManagementError::new(
        ErrorKind::InvalidData,
        "秘密变量需使用唯一有效名称，最多 32 项、每值 16 KiB、总计 64 KiB；值不能含空字符或换行",
    )
}
pub fn validate_secret_names(names: &[String]) -> Result<(), ManagementError> {
    let mut seen = HashSet::new();
    if names.len() > 32
        || names.iter().any(|name| {
            name.is_empty()
                || name.len() > 128
                || !name.bytes().enumerate().all(|(i, b)| {
                    b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit())
                })
                || !seen.insert(name)
        })
    {
        return Err(invalid());
    }
    Ok(())
}
impl SecretEnvironment {
    pub fn insert(&mut self, name: String, value: String) -> Result<(), ManagementError> {
        let value = SecretValue(value);
        if self.0.contains_key(&name) {
            return Err(invalid());
        }
        self.0.insert(name.clone(), value);
        if let Err(error) = self.validate() {
            self.0.remove(&name);
            return Err(error);
        }
        Ok(())
    }
    pub fn names(&self) -> Vec<String> {
        self.0.keys().cloned().collect()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn validate(&self) -> Result<(), ManagementError> {
        validate_secret_names(&self.names())?;
        let mut size = 0;
        for (name, value) in &self.0 {
            size += name.len() + value.0.len() + 2;
            // Foundation CharacterSet.newlines also splits VT, FF, NEL, LS and PS.
            if value.0.len() > 16 * 1024
                || value.0.chars().any(|c| {
                    matches!(
                        c,
                        '\0' | '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}'
                    )
                })
            {
                return Err(invalid());
            }
        }
        if size > 64 * 1024 {
            return Err(invalid());
        }
        Ok(())
    }
    pub fn validate_for(&self, command: &Command) -> Result<(), ManagementError> {
        self.validate()?;
        let names = command.secret_names();
        validate_secret_names(names)?;
        if names.len() != self.0.len() || names.iter().any(|name| !self.0.contains_key(name)) {
            return Err(ManagementError::new(
                ErrorKind::InvalidData,
                "秘密输入与声明不匹配或已丢失；请重新填写并创建新任务，未执行命令",
            ));
        }
        Ok(())
    }
    /// UTF-8 env-file stream for the child's stdin, never a temporary file or process environment.
    pub fn stdin_bytes(&self) -> Result<Zeroizing<Vec<u8>>, ManagementError> {
        self.validate()?;
        let mut bytes = Zeroizing::new(Vec::with_capacity(64 * 1024));
        for (name, value) in &self.0 {
            bytes.extend_from_slice(name.as_bytes());
            bytes.push(b'=');
            bytes.extend_from_slice(value.0.as_bytes());
            bytes.push(b'\n');
        }
        Ok(bytes)
    }
}
impl Command {
    pub fn secret_names(&self) -> &[String] {
        match self {
            Self::CreateContainer { spec, .. } => &spec.options.secret_environment,
            _ => &[],
        }
    }
}

#[cfg(test)]
mod tests;
