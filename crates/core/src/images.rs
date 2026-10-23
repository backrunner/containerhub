use crate::*;
use serde::{Deserialize, Serialize};

/// A named image reference frozen to its content digest; deleting a tag is not deleting every alias.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageKey {
    pub environment: String,
    pub provider: ProviderKind,
    pub reference: String,
    pub digest: String,
}
impl ImageKey {
    pub fn validate(&self) -> Result<(), ManagementError> {
        validate_image_query(&self.environment, &self.reference)?;
        if !valid_digest(&self.digest) {
            return Err(ManagementError::new(
                ErrorKind::InvalidData,
                "镜像 digest 无效",
            ));
        }
        Ok(())
    }
}
pub fn valid_digest(value: &str) -> bool {
    let Some((algorithm, hash)) = value.split_once(':') else {
        return false;
    };
    let length = match algorithm {
        "sha256" => 64,
        "sha512" => 128,
        _ => return false,
    };
    hash.len() == length
        && hash
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}
pub fn validate_image_query(environment: &str, reference: &str) -> Result<(), ManagementError> {
    if environment.is_empty() || environment.len() > 256 || !valid_image_reference(reference) {
        return Err(ManagementError::new(
            ErrorKind::InvalidData,
            "镜像引用或环境无效",
        ));
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageVariant {
    pub platform: String,
    pub digest: String,
    pub size_bytes: Option<u64>,
    pub environment_names: Vec<String>,
}
/// Allowlisted inspect data; environment values, command arguments and label values never cross this boundary.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageDetails {
    pub key: ImageKey,
    pub observed_at: u64,
    pub created_at: Option<String>,
    pub media_type: Option<String>,
    pub variants: Vec<ImageVariant>,
    pub used_by: Vec<ResourceKey>,
    pub other_references: Vec<String>,
}
