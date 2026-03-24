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
    pub boot_include: Vec<String>,
    pub lookup_sources: Vec<String>,
}

impl Default for MandalaMemoryPolicy {
    fn default() -> Self {
        Self {
            boot_include: vec!["stable_memory".into(), "active_snapshot".into()],
            lookup_sources: vec!["past".into(), "cortex".into(), "vault".into()],
        }
    }
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
