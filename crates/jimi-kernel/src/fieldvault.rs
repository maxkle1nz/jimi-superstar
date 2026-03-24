use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SealLevel {
    PublicProjection,
    CapsulePrivate,
    SacredShard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldVaultArtifact {
    pub artifact_id: String,
    pub capsule_id: Option<String>,
    pub slot_id: Option<String>,
    pub seal_level: SealLevel,
    pub fld_path: String,
    pub portable: bool,
    pub machine_bound: bool,
    pub sha256: Option<String>,
    pub plaintext_projection_ref: Option<String>,
}
