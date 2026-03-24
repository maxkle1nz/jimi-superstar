use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorRef {
    pub actor_type: String,
    pub actor_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubjectRef {
    pub subject_type: String,
    pub subject_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub event_version: String,
    pub event_id: String,
    pub event_type: EventType,
    pub emitted_at: DateTime<Utc>,
    pub sequence: u64,
    pub session_id: Option<String>,
    pub lane_id: Option<String>,
    pub turn_id: Option<String>,
    pub actor: ActorRef,
    pub subject: SubjectRef,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    SessionCreated,
    SessionResumed,
    LaneCreated,
    TurnStarted,
    MessageDelta,
    MessageCompleted,
    ToolRequested,
    ToolStarted,
    ToolCompleted,
    ApprovalRequired,
    ApprovalGranted,
    ApprovalDenied,
    EngineSelected,
    EngineDegraded,
    TruthFusionUpdated,
    MandalaBound,
    MandalaPolicyUpdated,
    CapsuleInstalled,
    CapsuleExportCompleted,
    CapsuleMarketplaceListed,
    SlotActivated,
    ShardSealed,
    ArtifactCreated,
    SystemHealthChanged,
}
