use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MandalaSelf {
    pub id: String,
    pub role: String,
    pub template_soul: String,
    pub execution_role: Option<String>,
    pub specialization: Option<String>,
    pub tone: Option<String>,
    pub canonical: bool,
    pub boundaries: BTreeMap<String, serde_json::Value>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MandalaExecutionPolicy {
    pub body: String,
    pub execution_lane: String,
    pub preferred_provider: String,
    pub preferred_model: String,
    pub reasoning_effort: Option<String>,
    pub use_session_pool: bool,
    pub allow_provider_override: bool,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MandalaCapabilityPolicy {
    pub declared: Vec<String>,
    pub required: Vec<String>,
    pub optional: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MandalaMemoryPolicy {
    #[serde(default = "default_boot_include")]
    pub boot_include: Vec<String>,
    #[serde(default = "default_lookup_sources")]
    pub lookup_sources: Vec<String>,
    #[serde(default = "default_hot_context_limit")]
    pub hot_context_limit: usize,
    #[serde(default = "default_relevant_context_limit")]
    pub relevant_context_limit: usize,
    #[serde(default = "default_promotion_confidence_threshold")]
    pub promotion_confidence_threshold: f32,
    #[serde(default = "default_promote_to_stable_memory")]
    pub promote_to_stable_memory: bool,
    #[serde(default = "default_allow_fieldvault_sealing")]
    pub allow_fieldvault_sealing: bool,
    #[serde(default = "default_seal_privacy_classes")]
    pub seal_privacy_classes: Vec<String>,
    #[serde(default = "default_world_state_scope")]
    pub world_state_scope: String,
    #[serde(default = "default_world_state_entry_limit")]
    pub world_state_entry_limit: usize,
    #[serde(default = "default_world_state_process_limit")]
    pub world_state_process_limit: usize,
    #[serde(default = "default_allow_world_state")]
    pub allow_world_state: bool,
}

impl Default for MandalaMemoryPolicy {
    fn default() -> Self {
        Self {
            boot_include: default_boot_include(),
            lookup_sources: default_lookup_sources(),
            hot_context_limit: default_hot_context_limit(),
            relevant_context_limit: default_relevant_context_limit(),
            promotion_confidence_threshold: default_promotion_confidence_threshold(),
            promote_to_stable_memory: default_promote_to_stable_memory(),
            allow_fieldvault_sealing: default_allow_fieldvault_sealing(),
            seal_privacy_classes: default_seal_privacy_classes(),
            world_state_scope: default_world_state_scope(),
            world_state_entry_limit: default_world_state_entry_limit(),
            world_state_process_limit: default_world_state_process_limit(),
            allow_world_state: default_allow_world_state(),
        }
    }
}

fn default_boot_include() -> Vec<String> {
    vec!["stable_memory".into(), "active_snapshot".into()]
}

fn default_lookup_sources() -> Vec<String> {
    vec!["past".into(), "cortex".into(), "vault".into()]
}

fn default_hot_context_limit() -> usize {
    5
}

fn default_relevant_context_limit() -> usize {
    5
}

fn default_promotion_confidence_threshold() -> f32 {
    0.9
}

fn default_promote_to_stable_memory() -> bool {
    true
}

fn default_allow_fieldvault_sealing() -> bool {
    true
}

fn default_seal_privacy_classes() -> Vec<String> {
    vec!["operator_private".into(), "sealed_candidate".into()]
}

fn default_world_state_scope() -> String {
    "workspace+process".into()
}

fn default_world_state_entry_limit() -> usize {
    8
}

fn default_world_state_process_limit() -> usize {
    8
}

fn default_allow_world_state() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MandalaStableMemory {
    pub operator_preferences: BTreeMap<String, serde_json::Value>,
    pub learned_rules: Vec<String>,
    pub memory: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MandalaActiveSnapshot {
    pub current_goal: String,
    pub active_decisions: Vec<String>,
    pub blockers: Vec<String>,
    pub next_actions: Vec<String>,
    pub hot_context: Vec<String>,
    pub snapshot: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MandalaRefs {
    pub past: String,
    pub cortex: String,
    pub vault: String,
    pub storms: String,
    pub projects: String,
    pub settings: String,
    pub runtime_fabric: String,
    pub personality: Option<String>,
    pub manifest: Option<String>,
}

impl Default for MandalaRefs {
    fn default() -> Self {
        Self {
            past: "/api/past/briefing/md".into(),
            cortex: "~/.roomanizer/rooms/cortex".into(),
            vault: "~/.roomanizer/secrets".into(),
            storms: "/api/storms".into(),
            projects: "/api/projects".into(),
            settings: "/api/settings".into(),
            runtime_fabric: "/api/settings/mcp".into(),
            personality: None,
            manifest: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MandalaProjection {
    pub projection_kind: String,
    pub requested_role: String,
    pub template_soul: String,
    pub execution_role: Option<String>,
    pub default_body: String,
    pub lineage: Vec<String>,
    pub autoevolve: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MandalaOwnership {
    pub principal_id: String,
    pub principal_kind: String,
    pub author_id: String,
    pub capsule_id: String,
    pub capsule_version: u32,
    pub edit_policy: String,
    pub transfer_policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MandalaCapsuleContract {
    pub supports_chat: bool,
    pub supports_stormender: bool,
    pub supports_multi_principal: bool,
    pub conductor_eligible: bool,
}

impl Default for MandalaCapsuleContract {
    fn default() -> Self {
        Self {
            supports_chat: true,
            supports_stormender: true,
            supports_multi_principal: false,
            conductor_eligible: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MandalaSkillPack {
    pub id: String,
    pub label: String,
    pub version: String,
    pub permissions: Vec<String>,
    pub entry_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SacredShard {
    pub shard_id: String,
    pub domain: String,
    pub label: String,
    pub required_unlock: bool,
    pub visibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MandalaManifest {
    pub manifest_version: String,
    pub kind: String,
    pub generated_at: f64,
    pub agent_version: u32,
    pub self_section: MandalaSelf,
    pub execution_policy: MandalaExecutionPolicy,
    pub capability_policy: MandalaCapabilityPolicy,
    pub memory_policy: MandalaMemoryPolicy,
    pub stable_memory: MandalaStableMemory,
    pub active_snapshot: MandalaActiveSnapshot,
    pub refs: MandalaRefs,
    pub projection: MandalaProjection,
    pub ownership: Option<MandalaOwnership>,
    pub capsule_contract: Option<MandalaCapsuleContract>,
    pub skill_packs: Vec<MandalaSkillPack>,
    pub sacred_shards: Vec<SacredShard>,
    pub metadata: BTreeMap<String, serde_json::Value>,
}
