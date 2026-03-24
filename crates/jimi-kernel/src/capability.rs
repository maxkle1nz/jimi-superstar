use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityType {
    Tool,
    Skill,
    Engine,
    Workflow,
    Channel,
    Node,
    MemorySource,
    TruthSource,
    UiSurface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportLevel {
    Native,
    Normalized,
    Degraded,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDescriptor {
    pub capability_id: String,
    pub capability_type: CapabilityType,
    pub name: String,
    pub description: String,
    pub support_level: SupportLevel,
    pub source_format: String,
    pub source_origin: String,
}
