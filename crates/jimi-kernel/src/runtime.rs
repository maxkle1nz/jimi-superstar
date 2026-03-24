use std::collections::BTreeMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ActorRef, EventEnvelope, EventType, FieldVaultArtifact, LaneId, LaneState, MandalaManifest,
    PersonalitySlot, SealLevel, SessionId, SessionState, SlotBindingState, SubjectRef, TurnId,
    TurnState,
};

#[derive(Debug, Error)]
pub enum KernelError {
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("lane not found: {0}")]
    LaneNotFound(String),
    #[error("turn not found: {0}")]
    TurnNotFound(String),
    #[error("mandala not found: {0}")]
    MandalaNotFound(String),
    #[error("capsule not found: {0}")]
    CapsuleNotFound(String),
    #[error("slot not found: {0}")]
    SlotNotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: SessionId,
    pub room_id: String,
    pub title: String,
    pub state: SessionState,
    pub active_lane_id: LaneId,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaneRecord {
    pub lane_id: LaneId,
    pub session_id: SessionId,
    pub parent_lane_id: Option<LaneId>,
    pub state: LaneState,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnRecord {
    pub turn_id: TurnId,
    pub session_id: SessionId,
    pub lane_id: LaneId,
    pub intent_mode: String,
    pub state: TurnState,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapsuleRecord {
    pub capsule_id: String,
    pub mandala_id: String,
    pub version: u32,
    pub install_source: String,
    pub installed_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapsulePackageRecord {
    pub package_id: String,
    pub capsule_id: String,
    pub mandala_id: String,
    pub version: u32,
    pub display_name: String,
    pub creator: String,
    pub source_origin: String,
    pub package_digest: String,
    pub trust_level: String,
    pub install_status: String,
    pub installed_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapsuleImportRecord {
    pub import_id: String,
    pub package_id: String,
    pub source_origin: String,
    pub source_path: String,
    pub import_status: String,
    pub package_digest: String,
    pub imported_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapsuleExportRecord {
    pub export_id: String,
    pub package_id: String,
    pub capsule_id: String,
    pub target_path: String,
    pub export_status: String,
    pub package_digest: String,
    pub exported_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderLaneRecord {
    pub provider_lane_id: String,
    pub provider: String,
    pub model: String,
    pub routing_mode: String,
    pub fallback_group: Option<String>,
    pub priority: u32,
    pub status: String,
    pub connected_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnDispatchRecord {
    pub dispatch_id: String,
    pub turn_id: TurnId,
    pub session_id: SessionId,
    pub lane_id: LaneId,
    pub provider_lane_id: String,
    pub intent_summary: String,
    pub status: String,
    pub dispatched_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCapsuleRecord {
    pub memory_capsule_id: String,
    pub session_id: SessionId,
    pub room_id: String,
    pub lane_id: LaneId,
    pub turn_id: Option<TurnId>,
    pub role: String,
    pub content: String,
    pub intent_summary: Option<String>,
    pub relevance_score: f32,
    pub confidence_level: f32,
    pub privacy_class: String,
    pub band: String,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryCheckpointRecord {
    pub summary_checkpoint_id: String,
    pub session_id: SessionId,
    pub source_band: String,
    pub source_capsule_ids: Vec<String>,
    pub semantic_digest: String,
    pub source_capsule_count: usize,
    pub source_text_length: usize,
    pub digest_text_length: usize,
    pub compression_ratio: f32,
    pub decisions_retained: Vec<String>,
    pub unresolved_items: Vec<String>,
    pub confidence_level: f32,
    pub generated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBridgeRecord {
    pub memory_bridge_id: String,
    pub session_id: SessionId,
    pub from_capsule_id: String,
    pub to_capsule_id: String,
    pub bridge_kind: String,
    pub strength: f32,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResynthesisTriggerRecord {
    pub resynthesis_trigger_id: String,
    pub session_id: SessionId,
    pub trigger_kind: String,
    pub summary: String,
    pub confidence_level: f32,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPromotionRecord {
    pub memory_promotion_id: String,
    pub session_id: SessionId,
    pub memory_capsule_id: String,
    pub target_plane: String,
    pub promoted_value: String,
    pub confidence_level: f32,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequestRecord {
    pub approval_request_id: String,
    pub session_id: SessionId,
    pub dispatch_id: String,
    pub provider_lane_id: String,
    pub action: String,
    pub reason: String,
    pub status: String,
    pub created_at: chrono::DateTime<Utc>,
    pub resolved_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldStateWorkspaceEntry {
    pub path: String,
    pub kind: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldStateNodeRecord {
    pub node_id: String,
    pub scope: String,
    pub path: String,
    pub kind: String,
    pub size_bytes: u64,
    pub metadata_hash: String,
    pub status: String,
    pub observed_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldStateDeltaRecord {
    pub delta_id: String,
    pub scope: String,
    pub change_kind: String,
    pub path: String,
    pub summary: String,
    pub observed_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldStateRelationRecord {
    pub relation_id: String,
    pub scope: String,
    pub from_node_id: String,
    pub to_node_id: String,
    pub relation_kind: String,
    pub summary: String,
    pub observed_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldStateProcessEntry {
    pub pid: u32,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldStateSlice {
    pub scope: String,
    pub workspace_root: String,
    pub workspace_health: String,
    pub git_dirty: bool,
    pub cache_state: String,
    pub indexed_nodes: usize,
    pub relation_count: usize,
    pub workspace_entry_count: usize,
    pub workspace_entries: Vec<WorldStateWorkspaceEntry>,
    pub changed_files: Vec<String>,
    pub recent_deltas: Vec<String>,
    pub recent_relations: Vec<String>,
    pub running_processes: Vec<WorldStateProcessEntry>,
    pub observed_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Default)]
pub struct EventStore {
    events: Vec<EventEnvelope>,
    next_sequence: u64,
}

impl EventStore {
    pub fn append(
        &mut self,
        actor: ActorRef,
        subject: SubjectRef,
        event_type: EventType,
        session_id: Option<&SessionId>,
        lane_id: Option<&LaneId>,
        turn_id: Option<&TurnId>,
        payload: serde_json::Value,
    ) -> &EventEnvelope {
        self.next_sequence += 1;
        self.events.push(EventEnvelope {
            event_version: "jimi.event.v1".into(),
            event_id: format!("evt_{}", Uuid::now_v7()),
            event_type,
            emitted_at: Utc::now(),
            sequence: self.next_sequence,
            session_id: session_id.map(|id| id.0.clone()),
            lane_id: lane_id.map(|id| id.0.clone()),
            turn_id: turn_id.map(|id| id.0.clone()),
            actor,
            subject,
            payload,
        });
        self.events.last().expect("event just inserted")
    }

    pub fn all(&self) -> &[EventEnvelope] {
        &self.events
    }

    pub fn by_session(&self, session_id: &SessionId) -> Vec<&EventEnvelope> {
        self.events
            .iter()
            .filter(|event| event.session_id.as_deref() == Some(session_id.0.as_str()))
            .collect()
    }

    pub fn push_existing(&mut self, event: EventEnvelope) {
        self.events.push(event);
    }

    pub fn set_next_sequence(&mut self, sequence: u64) {
        self.next_sequence = sequence;
    }
}

#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: BTreeMap<String, SessionRecord>,
    lanes: BTreeMap<String, LaneRecord>,
    turns: BTreeMap<String, TurnRecord>,
}

impl SessionManager {
    pub fn create_session(
        &mut self,
        title: impl Into<String>,
        room_id: impl Into<String>,
    ) -> SessionRecord {
        let session_id = SessionId::new();
        let lane_id = LaneId::new();
        let now = Utc::now();
        let lane = LaneRecord {
            lane_id: lane_id.clone(),
            session_id: session_id.clone(),
            parent_lane_id: None,
            state: LaneState::Focused,
            created_at: now,
        };
        let session = SessionRecord {
            session_id: session_id.clone(),
            room_id: room_id.into(),
            title: title.into(),
            state: SessionState::Active,
            active_lane_id: lane_id.clone(),
            created_at: now,
            updated_at: now,
        };
        self.lanes.insert(lane_id.0.clone(), lane);
        self.sessions.insert(session_id.0.clone(), session.clone());
        session
    }

    pub fn create_turn(
        &mut self,
        session_id: &SessionId,
        lane_id: &LaneId,
        intent_mode: impl Into<String>,
    ) -> Result<TurnRecord, KernelError> {
        if !self.sessions.contains_key(&session_id.0) {
            return Err(KernelError::SessionNotFound(session_id.0.clone()));
        }
        if !self.lanes.contains_key(&lane_id.0) {
            return Err(KernelError::LaneNotFound(lane_id.0.clone()));
        }
        let turn = TurnRecord {
            turn_id: TurnId::new(),
            session_id: session_id.clone(),
            lane_id: lane_id.clone(),
            intent_mode: intent_mode.into(),
            state: TurnState::Queued,
            created_at: Utc::now(),
        };
        self.turns.insert(turn.turn_id.0.clone(), turn.clone());
        Ok(turn)
    }

    pub fn session(&self, session_id: &SessionId) -> Result<&SessionRecord, KernelError> {
        self.sessions
            .get(&session_id.0)
            .ok_or_else(|| KernelError::SessionNotFound(session_id.0.clone()))
    }

    pub fn lane(&self, lane_id: &LaneId) -> Result<&LaneRecord, KernelError> {
        self.lanes
            .get(&lane_id.0)
            .ok_or_else(|| KernelError::LaneNotFound(lane_id.0.clone()))
    }

    pub fn turn(&self, turn_id: &TurnId) -> Result<&TurnRecord, KernelError> {
        self.turns
            .get(&turn_id.0)
            .ok_or_else(|| KernelError::TurnNotFound(turn_id.0.clone()))
    }

    pub fn update_turn_state(
        &mut self,
        turn_id: &TurnId,
        state: TurnState,
    ) -> Result<TurnRecord, KernelError> {
        let turn = self
            .turns
            .get_mut(&turn_id.0)
            .ok_or_else(|| KernelError::TurnNotFound(turn_id.0.clone()))?;
        turn.state = state;
        Ok(turn.clone())
    }

    pub fn sessions(&self) -> Vec<&SessionRecord> {
        self.sessions.values().collect()
    }

    pub fn lanes(&self) -> Vec<&LaneRecord> {
        self.lanes.values().collect()
    }

    pub fn turns(&self) -> Vec<&TurnRecord> {
        self.turns.values().collect()
    }

    pub fn insert_session(&mut self, session: SessionRecord) {
        self.sessions.insert(session.session_id.0.clone(), session);
    }

    pub fn insert_lane(&mut self, lane: LaneRecord) {
        self.lanes.insert(lane.lane_id.0.clone(), lane);
    }

    pub fn insert_turn(&mut self, turn: TurnRecord) {
        self.turns.insert(turn.turn_id.0.clone(), turn);
    }
}

#[derive(Debug, Default)]
pub struct MandalaRegistry {
    mandalas: BTreeMap<String, MandalaManifest>,
}

impl MandalaRegistry {
    pub fn install(&mut self, mandala: MandalaManifest) {
        self.mandalas
            .insert(mandala.self_section.id.clone(), mandala);
    }

    pub fn get(&self, mandala_id: &str) -> Result<&MandalaManifest, KernelError> {
        self.mandalas
            .get(mandala_id)
            .ok_or_else(|| KernelError::MandalaNotFound(mandala_id.into()))
    }

    pub fn all(&self) -> Vec<&MandalaManifest> {
        self.mandalas.values().collect()
    }

    pub fn ids(&self) -> Vec<&str> {
        self.mandalas.keys().map(String::as_str).collect()
    }

    pub fn get_mut(&mut self, mandala_id: &str) -> Result<&mut MandalaManifest, KernelError> {
        self.mandalas
            .get_mut(mandala_id)
            .ok_or_else(|| KernelError::MandalaNotFound(mandala_id.into()))
    }
}

#[derive(Debug, Default)]
pub struct CapsuleRegistry {
    capsules: BTreeMap<String, CapsuleRecord>,
}

impl CapsuleRegistry {
    pub fn install(
        &mut self,
        capsule_id: impl Into<String>,
        mandala_id: impl Into<String>,
        version: u32,
        install_source: impl Into<String>,
    ) -> CapsuleRecord {
        let capsule = CapsuleRecord {
            capsule_id: capsule_id.into(),
            mandala_id: mandala_id.into(),
            version,
            install_source: install_source.into(),
            installed_at: Utc::now(),
        };
        self.capsules
            .insert(capsule.capsule_id.clone(), capsule.clone());
        capsule
    }

    pub fn get(&self, capsule_id: &str) -> Result<&CapsuleRecord, KernelError> {
        self.capsules
            .get(capsule_id)
            .ok_or_else(|| KernelError::CapsuleNotFound(capsule_id.into()))
    }

    pub fn all(&self) -> Vec<&CapsuleRecord> {
        self.capsules.values().collect()
    }

    pub fn insert_existing(&mut self, capsule: CapsuleRecord) {
        self.capsules.insert(capsule.capsule_id.clone(), capsule);
    }
}

#[derive(Debug, Default)]
pub struct CapsulePackageRegistry {
    packages: BTreeMap<String, CapsulePackageRecord>,
}

impl CapsulePackageRegistry {
    pub fn register(
        &mut self,
        package_id: impl Into<String>,
        capsule_id: impl Into<String>,
        mandala_id: impl Into<String>,
        version: u32,
        display_name: impl Into<String>,
        creator: impl Into<String>,
        source_origin: impl Into<String>,
        package_digest: impl Into<String>,
        trust_level: impl Into<String>,
        install_status: impl Into<String>,
    ) -> CapsulePackageRecord {
        let package = CapsulePackageRecord {
            package_id: package_id.into(),
            capsule_id: capsule_id.into(),
            mandala_id: mandala_id.into(),
            version,
            display_name: display_name.into(),
            creator: creator.into(),
            source_origin: source_origin.into(),
            package_digest: package_digest.into(),
            trust_level: trust_level.into(),
            install_status: install_status.into(),
            installed_at: Utc::now(),
        };
        self.packages
            .insert(package.package_id.clone(), package.clone());
        package
    }

    pub fn all(&self) -> Vec<&CapsulePackageRecord> {
        self.packages.values().collect()
    }

    pub fn get(&self, package_id: &str) -> Result<&CapsulePackageRecord, KernelError> {
        self.packages
            .get(package_id)
            .ok_or_else(|| KernelError::CapsuleNotFound(package_id.into()))
    }

    pub fn classify_trust(
        &mut self,
        package_id: &str,
        trust_level: impl Into<String>,
    ) -> Result<CapsulePackageRecord, KernelError> {
        let package = self
            .packages
            .get_mut(package_id)
            .ok_or_else(|| KernelError::CapsuleNotFound(package_id.into()))?;
        package.trust_level = trust_level.into();
        Ok(package.clone())
    }

    pub fn update_install_status(
        &mut self,
        package_id: &str,
        install_status: impl Into<String>,
    ) -> Result<CapsulePackageRecord, KernelError> {
        let package = self
            .packages
            .get_mut(package_id)
            .ok_or_else(|| KernelError::CapsuleNotFound(package_id.into()))?;
        package.install_status = install_status.into();
        Ok(package.clone())
    }

    pub fn insert_existing(&mut self, package: CapsulePackageRecord) {
        self.packages.insert(package.package_id.clone(), package);
    }
}

#[derive(Debug, Default)]
pub struct CapsuleImportRegistry {
    imports: BTreeMap<String, CapsuleImportRecord>,
}

impl CapsuleImportRegistry {
    pub fn record(
        &mut self,
        package_id: impl Into<String>,
        source_origin: impl Into<String>,
        source_path: impl Into<String>,
        import_status: impl Into<String>,
        package_digest: impl Into<String>,
    ) -> CapsuleImportRecord {
        let record = CapsuleImportRecord {
            import_id: format!("import_{}", Uuid::now_v7()),
            package_id: package_id.into(),
            source_origin: source_origin.into(),
            source_path: source_path.into(),
            import_status: import_status.into(),
            package_digest: package_digest.into(),
            imported_at: Utc::now(),
        };
        self.imports
            .insert(record.import_id.clone(), record.clone());
        record
    }

    pub fn all(&self) -> Vec<&CapsuleImportRecord> {
        self.imports.values().collect()
    }

    pub fn insert_existing(&mut self, record: CapsuleImportRecord) {
        self.imports.insert(record.import_id.clone(), record);
    }
}

#[derive(Debug, Default)]
pub struct CapsuleExportRegistry {
    exports: BTreeMap<String, CapsuleExportRecord>,
}

impl CapsuleExportRegistry {
    pub fn record(
        &mut self,
        package_id: impl Into<String>,
        capsule_id: impl Into<String>,
        target_path: impl Into<String>,
        export_status: impl Into<String>,
        package_digest: impl Into<String>,
    ) -> CapsuleExportRecord {
        let record = CapsuleExportRecord {
            export_id: format!("export_{}", Uuid::now_v7()),
            package_id: package_id.into(),
            capsule_id: capsule_id.into(),
            target_path: target_path.into(),
            export_status: export_status.into(),
            package_digest: package_digest.into(),
            exported_at: Utc::now(),
        };
        self.exports
            .insert(record.export_id.clone(), record.clone());
        record
    }

    pub fn all(&self) -> Vec<&CapsuleExportRecord> {
        self.exports.values().collect()
    }

    pub fn insert_existing(&mut self, record: CapsuleExportRecord) {
        self.exports.insert(record.export_id.clone(), record);
    }
}

#[derive(Debug, Default)]
pub struct SlotRegistry {
    slots: BTreeMap<String, PersonalitySlot>,
}

impl SlotRegistry {
    pub fn define_slot(
        &mut self,
        slot_id: impl Into<String>,
        label: impl Into<String>,
    ) -> PersonalitySlot {
        let slot = PersonalitySlot {
            slot_id: slot_id.into(),
            label: label.into(),
            capsule_id: None,
            principal_id: None,
            state: SlotBindingState::Empty,
            unlock_required: false,
            active_mandala_id: None,
        };
        self.slots.insert(slot.slot_id.clone(), slot.clone());
        slot
    }

    pub fn bind_capsule(
        &mut self,
        slot_id: &str,
        capsule_id: impl Into<String>,
        mandala_id: impl Into<String>,
        unlock_required: bool,
    ) -> Result<(), KernelError> {
        let slot = self
            .slots
            .get_mut(slot_id)
            .ok_or_else(|| KernelError::SlotNotFound(slot_id.into()))?;
        slot.capsule_id = Some(capsule_id.into());
        slot.active_mandala_id = Some(mandala_id.into());
        slot.unlock_required = unlock_required;
        slot.state = if unlock_required {
            SlotBindingState::Locked
        } else {
            SlotBindingState::Installed
        };
        Ok(())
    }

    pub fn activate(&mut self, slot_id: &str) -> Result<(), KernelError> {
        let slot = self
            .slots
            .get_mut(slot_id)
            .ok_or_else(|| KernelError::SlotNotFound(slot_id.into()))?;
        slot.state = if slot.unlock_required {
            SlotBindingState::Locked
        } else {
            SlotBindingState::Active
        };
        Ok(())
    }

    pub fn get(&self, slot_id: &str) -> Result<&PersonalitySlot, KernelError> {
        self.slots
            .get(slot_id)
            .ok_or_else(|| KernelError::SlotNotFound(slot_id.into()))
    }

    pub fn all(&self) -> Vec<&PersonalitySlot> {
        self.slots.values().collect()
    }

    pub fn insert_existing(&mut self, slot: PersonalitySlot) {
        self.slots.insert(slot.slot_id.clone(), slot);
    }
}

#[derive(Debug, Default)]
pub struct FieldVaultRuntime {
    artifacts: BTreeMap<String, FieldVaultArtifact>,
}

impl FieldVaultRuntime {
    pub fn register_artifact(
        &mut self,
        capsule_id: Option<String>,
        slot_id: Option<String>,
        seal_level: SealLevel,
        fld_path: impl Into<String>,
        portable: bool,
        machine_bound: bool,
    ) -> FieldVaultArtifact {
        let artifact = FieldVaultArtifact {
            artifact_id: format!("fld_{}", Uuid::now_v7()),
            capsule_id,
            slot_id,
            seal_level,
            fld_path: fld_path.into(),
            portable,
            machine_bound,
            sha256: None,
            plaintext_projection_ref: None,
        };
        self.artifacts
            .insert(artifact.artifact_id.clone(), artifact.clone());
        artifact
    }

    pub fn all(&self) -> Vec<&FieldVaultArtifact> {
        self.artifacts.values().collect()
    }

    pub fn insert_existing(&mut self, artifact: FieldVaultArtifact) {
        self.artifacts
            .insert(artifact.artifact_id.clone(), artifact);
    }
}

#[derive(Debug, Default)]
pub struct ProviderLaneRegistry {
    lanes: BTreeMap<String, ProviderLaneRecord>,
}

impl ProviderLaneRegistry {
    pub fn connect(
        &mut self,
        provider_lane_id: impl Into<String>,
        provider: impl Into<String>,
        model: impl Into<String>,
        routing_mode: impl Into<String>,
        fallback_group: Option<String>,
        priority: u32,
        status: impl Into<String>,
    ) -> ProviderLaneRecord {
        let lane = ProviderLaneRecord {
            provider_lane_id: provider_lane_id.into(),
            provider: provider.into(),
            model: model.into(),
            routing_mode: routing_mode.into(),
            fallback_group,
            priority,
            status: status.into(),
            connected_at: Utc::now(),
        };
        self.lanes
            .insert(lane.provider_lane_id.clone(), lane.clone());
        lane
    }

    pub fn all(&self) -> Vec<&ProviderLaneRecord> {
        self.lanes.values().collect()
    }

    pub fn get(&self, provider_lane_id: &str) -> Result<&ProviderLaneRecord, KernelError> {
        self.lanes
            .get(provider_lane_id)
            .ok_or_else(|| KernelError::LaneNotFound(provider_lane_id.into()))
    }

    pub fn update_lane(
        &mut self,
        provider_lane_id: &str,
        routing_mode: impl Into<String>,
        fallback_group: Option<String>,
        priority: u32,
    ) -> Result<ProviderLaneRecord, KernelError> {
        let lane = self
            .lanes
            .get_mut(provider_lane_id)
            .ok_or_else(|| KernelError::LaneNotFound(provider_lane_id.into()))?;
        lane.routing_mode = routing_mode.into();
        lane.fallback_group = fallback_group;
        lane.priority = priority;
        Ok(lane.clone())
    }

    pub fn insert_existing(&mut self, lane: ProviderLaneRecord) {
        self.lanes.insert(lane.provider_lane_id.clone(), lane);
    }
}

#[derive(Debug, Default)]
pub struct TurnDispatchRegistry {
    dispatches: BTreeMap<String, TurnDispatchRecord>,
}

impl TurnDispatchRegistry {
    pub fn dispatch(
        &mut self,
        turn_id: TurnId,
        session_id: SessionId,
        lane_id: LaneId,
        provider_lane_id: impl Into<String>,
        intent_summary: impl Into<String>,
        status: impl Into<String>,
    ) -> TurnDispatchRecord {
        let dispatch = TurnDispatchRecord {
            dispatch_id: format!("dispatch_{}", Uuid::now_v7()),
            turn_id,
            session_id,
            lane_id,
            provider_lane_id: provider_lane_id.into(),
            intent_summary: intent_summary.into(),
            status: status.into(),
            dispatched_at: Utc::now(),
        };
        self.dispatches
            .insert(dispatch.dispatch_id.clone(), dispatch.clone());
        dispatch
    }

    pub fn all(&self) -> Vec<&TurnDispatchRecord> {
        self.dispatches.values().collect()
    }

    pub fn get(&self, dispatch_id: &str) -> Result<&TurnDispatchRecord, KernelError> {
        self.dispatches
            .get(dispatch_id)
            .ok_or_else(|| KernelError::TurnNotFound(dispatch_id.into()))
    }

    pub fn update_status(
        &mut self,
        dispatch_id: &str,
        status: impl Into<String>,
    ) -> Result<TurnDispatchRecord, KernelError> {
        let dispatch = self
            .dispatches
            .get_mut(dispatch_id)
            .ok_or_else(|| KernelError::TurnNotFound(dispatch_id.into()))?;
        dispatch.status = status.into();
        Ok(dispatch.clone())
    }

    pub fn update_provider_lane(
        &mut self,
        dispatch_id: &str,
        provider_lane_id: impl Into<String>,
    ) -> Result<TurnDispatchRecord, KernelError> {
        let dispatch = self
            .dispatches
            .get_mut(dispatch_id)
            .ok_or_else(|| KernelError::TurnNotFound(dispatch_id.into()))?;
        dispatch.provider_lane_id = provider_lane_id.into();
        Ok(dispatch.clone())
    }

    pub fn insert_existing(&mut self, dispatch: TurnDispatchRecord) {
        self.dispatches
            .insert(dispatch.dispatch_id.clone(), dispatch);
    }
}

#[derive(Debug, Default)]
pub struct MemoryCapsuleRegistry {
    capsules: BTreeMap<String, MemoryCapsuleRecord>,
}

impl MemoryCapsuleRegistry {
    pub fn append(
        &mut self,
        session_id: SessionId,
        room_id: impl Into<String>,
        lane_id: LaneId,
        turn_id: Option<TurnId>,
        role: impl Into<String>,
        content: impl Into<String>,
        intent_summary: Option<String>,
        relevance_score: f32,
        confidence_level: f32,
        privacy_class: impl Into<String>,
        band: impl Into<String>,
    ) -> MemoryCapsuleRecord {
        let capsule = MemoryCapsuleRecord {
            memory_capsule_id: format!("memcap_{}", Uuid::now_v7()),
            session_id,
            room_id: room_id.into(),
            lane_id,
            turn_id,
            role: role.into(),
            content: content.into(),
            intent_summary,
            relevance_score,
            confidence_level,
            privacy_class: privacy_class.into(),
            band: band.into(),
            created_at: Utc::now(),
        };
        self.capsules
            .insert(capsule.memory_capsule_id.clone(), capsule.clone());
        capsule
    }

    pub fn all(&self) -> Vec<&MemoryCapsuleRecord> {
        self.capsules.values().collect()
    }

    pub fn by_session(&self, session_id: &SessionId) -> Vec<&MemoryCapsuleRecord> {
        self.capsules
            .values()
            .filter(|capsule| capsule.session_id.0 == session_id.0)
            .collect()
    }

    pub fn by_room(&self, room_id: &str) -> Vec<&MemoryCapsuleRecord> {
        self.capsules
            .values()
            .filter(|capsule| capsule.room_id == room_id)
            .collect()
    }

    pub fn insert_existing(&mut self, capsule: MemoryCapsuleRecord) {
        self.capsules
            .insert(capsule.memory_capsule_id.clone(), capsule);
    }

    pub fn reband_session(&mut self, session_id: &SessionId) {
        let mut capsules: Vec<_> = self
            .capsules
            .values_mut()
            .filter(|capsule| capsule.session_id.0 == session_id.0)
            .collect();
        capsules.sort_by_key(|capsule| capsule.created_at);
        let total = capsules.len();
        for (index, capsule) in capsules.into_iter().enumerate() {
            let from_end = total.saturating_sub(index + 1);
            capsule.band = if from_end < 5 {
                "hot".into()
            } else if from_end < 10 {
                "warm".into()
            } else {
                "archive".into()
            };
            let band_bonus = match capsule.band.as_str() {
                "hot" => 0.15,
                "warm" => 0.08,
                _ => 0.03,
            };
            let role_bonus = if capsule.role == "operator" {
                0.06
            } else {
                0.03
            };
            capsule.relevance_score = (capsule.confidence_level + band_bonus + role_bonus).min(1.0);
        }
    }

    pub fn query_session(
        &self,
        session_id: &SessionId,
        query: &str,
        limit: usize,
    ) -> Vec<MemoryCapsuleRecord> {
        let lowered_query = query.to_lowercase();
        let mut capsules: Vec<MemoryCapsuleRecord> = self
            .capsules
            .values()
            .filter(|capsule| capsule.session_id.0 == session_id.0)
            .cloned()
            .collect();

        capsules.sort_by(|a, b| {
            score_capsule_for_query(b, &lowered_query)
                .partial_cmp(&score_capsule_for_query(a, &lowered_query))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        capsules.truncate(limit);
        capsules
    }

    pub fn query_room(&self, room_id: &str, query: &str, limit: usize) -> Vec<MemoryCapsuleRecord> {
        let lowered_query = query.to_lowercase();
        let mut capsules: Vec<MemoryCapsuleRecord> = self
            .capsules
            .values()
            .filter(|capsule| capsule.room_id == room_id)
            .cloned()
            .collect();

        capsules.sort_by(|a, b| {
            score_capsule_for_query(b, &lowered_query)
                .partial_cmp(&score_capsule_for_query(a, &lowered_query))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        capsules.truncate(limit);
        capsules
    }

    pub fn query_global(&self, query: &str, limit: usize) -> Vec<MemoryCapsuleRecord> {
        let lowered_query = query.to_lowercase();
        let mut capsules: Vec<MemoryCapsuleRecord> = self.capsules.values().cloned().collect();

        capsules.sort_by(|a, b| {
            score_capsule_for_query(b, &lowered_query)
                .partial_cmp(&score_capsule_for_query(a, &lowered_query))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        capsules.truncate(limit);
        capsules
    }
}

#[derive(Debug, Default)]
pub struct SummaryCheckpointRegistry {
    checkpoints: BTreeMap<String, SummaryCheckpointRecord>,
}

impl SummaryCheckpointRegistry {
    pub fn create(
        &mut self,
        session_id: SessionId,
        source_band: impl Into<String>,
        source_capsule_ids: Vec<String>,
        semantic_digest: impl Into<String>,
        source_capsule_count: usize,
        source_text_length: usize,
        digest_text_length: usize,
        compression_ratio: f32,
        decisions_retained: Vec<String>,
        unresolved_items: Vec<String>,
        confidence_level: f32,
    ) -> SummaryCheckpointRecord {
        let checkpoint = SummaryCheckpointRecord {
            summary_checkpoint_id: format!("summary_{}", Uuid::now_v7()),
            session_id,
            source_band: source_band.into(),
            source_capsule_ids,
            semantic_digest: semantic_digest.into(),
            source_capsule_count,
            source_text_length,
            digest_text_length,
            compression_ratio,
            decisions_retained,
            unresolved_items,
            confidence_level,
            generated_at: Utc::now(),
        };
        self.checkpoints
            .insert(checkpoint.summary_checkpoint_id.clone(), checkpoint.clone());
        checkpoint
    }

    pub fn all(&self) -> Vec<&SummaryCheckpointRecord> {
        self.checkpoints.values().collect()
    }

    pub fn by_session(&self, session_id: &SessionId) -> Vec<&SummaryCheckpointRecord> {
        self.checkpoints
            .values()
            .filter(|checkpoint| checkpoint.session_id.0 == session_id.0)
            .collect()
    }

    pub fn insert_existing(&mut self, checkpoint: SummaryCheckpointRecord) {
        self.checkpoints
            .insert(checkpoint.summary_checkpoint_id.clone(), checkpoint);
    }
}

#[derive(Debug, Default)]
pub struct MemoryBridgeRegistry {
    bridges: BTreeMap<String, MemoryBridgeRecord>,
}

impl MemoryBridgeRegistry {
    pub fn create(
        &mut self,
        session_id: SessionId,
        from_capsule_id: impl Into<String>,
        to_capsule_id: impl Into<String>,
        bridge_kind: impl Into<String>,
        strength: f32,
    ) -> MemoryBridgeRecord {
        let bridge = MemoryBridgeRecord {
            memory_bridge_id: format!("bridge_{}", Uuid::now_v7()),
            session_id,
            from_capsule_id: from_capsule_id.into(),
            to_capsule_id: to_capsule_id.into(),
            bridge_kind: bridge_kind.into(),
            strength,
            created_at: Utc::now(),
        };
        self.bridges
            .insert(bridge.memory_bridge_id.clone(), bridge.clone());
        bridge
    }

    pub fn all(&self) -> Vec<&MemoryBridgeRecord> {
        self.bridges.values().collect()
    }

    pub fn by_session(&self, session_id: &SessionId) -> Vec<&MemoryBridgeRecord> {
        self.bridges
            .values()
            .filter(|bridge| bridge.session_id.0 == session_id.0)
            .collect()
    }

    pub fn insert_existing(&mut self, bridge: MemoryBridgeRecord) {
        self.bridges.insert(bridge.memory_bridge_id.clone(), bridge);
    }
}

#[derive(Debug, Default)]
pub struct ResynthesisTriggerRegistry {
    triggers: BTreeMap<String, ResynthesisTriggerRecord>,
}

impl ResynthesisTriggerRegistry {
    pub fn create(
        &mut self,
        session_id: SessionId,
        trigger_kind: impl Into<String>,
        summary: impl Into<String>,
        confidence_level: f32,
    ) -> ResynthesisTriggerRecord {
        let trigger = ResynthesisTriggerRecord {
            resynthesis_trigger_id: format!("resynth_{}", Uuid::now_v7()),
            session_id,
            trigger_kind: trigger_kind.into(),
            summary: summary.into(),
            confidence_level,
            created_at: Utc::now(),
        };
        self.triggers
            .insert(trigger.resynthesis_trigger_id.clone(), trigger.clone());
        trigger
    }

    pub fn all(&self) -> Vec<&ResynthesisTriggerRecord> {
        self.triggers.values().collect()
    }

    pub fn by_session(&self, session_id: &SessionId) -> Vec<&ResynthesisTriggerRecord> {
        self.triggers
            .values()
            .filter(|trigger| trigger.session_id.0 == session_id.0)
            .collect()
    }

    pub fn insert_existing(&mut self, trigger: ResynthesisTriggerRecord) {
        self.triggers
            .insert(trigger.resynthesis_trigger_id.clone(), trigger);
    }
}

#[derive(Debug, Default)]
pub struct MemoryPromotionRegistry {
    promotions: BTreeMap<String, MemoryPromotionRecord>,
}

impl MemoryPromotionRegistry {
    pub fn create(
        &mut self,
        session_id: SessionId,
        memory_capsule_id: impl Into<String>,
        target_plane: impl Into<String>,
        promoted_value: impl Into<String>,
        confidence_level: f32,
    ) -> MemoryPromotionRecord {
        let promotion = MemoryPromotionRecord {
            memory_promotion_id: format!("promote_{}", Uuid::now_v7()),
            session_id,
            memory_capsule_id: memory_capsule_id.into(),
            target_plane: target_plane.into(),
            promoted_value: promoted_value.into(),
            confidence_level,
            created_at: Utc::now(),
        };
        self.promotions
            .insert(promotion.memory_promotion_id.clone(), promotion.clone());
        promotion
    }

    pub fn all(&self) -> Vec<&MemoryPromotionRecord> {
        self.promotions.values().collect()
    }

    pub fn by_session(&self, session_id: &SessionId) -> Vec<&MemoryPromotionRecord> {
        self.promotions
            .values()
            .filter(|promotion| promotion.session_id.0 == session_id.0)
            .collect()
    }

    pub fn insert_existing(&mut self, promotion: MemoryPromotionRecord) {
        self.promotions
            .insert(promotion.memory_promotion_id.clone(), promotion);
    }
}

#[derive(Debug, Default)]
pub struct ApprovalRegistry {
    approvals: BTreeMap<String, ApprovalRequestRecord>,
}

impl ApprovalRegistry {
    pub fn request(
        &mut self,
        session_id: SessionId,
        dispatch_id: impl Into<String>,
        provider_lane_id: impl Into<String>,
        action: impl Into<String>,
        reason: impl Into<String>,
    ) -> ApprovalRequestRecord {
        let approval = ApprovalRequestRecord {
            approval_request_id: format!("approval_{}", Uuid::now_v7()),
            session_id,
            dispatch_id: dispatch_id.into(),
            provider_lane_id: provider_lane_id.into(),
            action: action.into(),
            reason: reason.into(),
            status: "pending".into(),
            created_at: Utc::now(),
            resolved_at: None,
        };
        self.approvals
            .insert(approval.approval_request_id.clone(), approval.clone());
        approval
    }

    pub fn all(&self) -> Vec<&ApprovalRequestRecord> {
        self.approvals.values().collect()
    }

    pub fn pending(&self) -> Vec<&ApprovalRequestRecord> {
        self.approvals
            .values()
            .filter(|approval| approval.status == "pending")
            .collect()
    }

    pub fn find_pending_by_dispatch(&self, dispatch_id: &str) -> Option<&ApprovalRequestRecord> {
        self.approvals
            .values()
            .find(|approval| approval.dispatch_id == dispatch_id && approval.status == "pending")
    }

    pub fn has_granted_for_dispatch(&self, dispatch_id: &str) -> bool {
        self.approvals
            .values()
            .any(|approval| approval.dispatch_id == dispatch_id && approval.status == "granted")
    }

    pub fn grant(&mut self, approval_request_id: &str) -> Result<ApprovalRequestRecord, KernelError> {
        let approval = self
            .approvals
            .get_mut(approval_request_id)
            .ok_or_else(|| KernelError::TurnNotFound(approval_request_id.into()))?;
        approval.status = "granted".into();
        approval.resolved_at = Some(Utc::now());
        Ok(approval.clone())
    }

    pub fn deny(&mut self, approval_request_id: &str) -> Result<ApprovalRequestRecord, KernelError> {
        let approval = self
            .approvals
            .get_mut(approval_request_id)
            .ok_or_else(|| KernelError::TurnNotFound(approval_request_id.into()))?;
        approval.status = "denied".into();
        approval.resolved_at = Some(Utc::now());
        Ok(approval.clone())
    }

    pub fn insert_existing(&mut self, approval: ApprovalRequestRecord) {
        self.approvals
            .insert(approval.approval_request_id.clone(), approval);
    }
}

#[derive(Debug, Default)]
pub struct WorldStateNodeRegistry {
    nodes: BTreeMap<String, WorldStateNodeRecord>,
}

impl WorldStateNodeRegistry {
    pub fn all(&self) -> Vec<&WorldStateNodeRecord> {
        self.nodes.values().collect()
    }

    pub fn by_scope(&self, scope: &str) -> Vec<&WorldStateNodeRecord> {
        self.nodes
            .values()
            .filter(|node| node.scope == scope)
            .collect()
    }

    pub fn replace_scope(&mut self, scope: &str, nodes: Vec<WorldStateNodeRecord>) {
        self.nodes.retain(|_, node| node.scope != scope);
        for node in nodes {
            self.nodes.insert(node.node_id.clone(), node);
        }
    }

    pub fn insert_existing(&mut self, node: WorldStateNodeRecord) {
        self.nodes.insert(node.node_id.clone(), node);
    }
}

#[derive(Debug, Default)]
pub struct WorldStateDeltaRegistry {
    deltas: BTreeMap<String, WorldStateDeltaRecord>,
}

impl WorldStateDeltaRegistry {
    pub fn all(&self) -> Vec<&WorldStateDeltaRecord> {
        self.deltas.values().collect()
    }

    pub fn by_scope(&self, scope: &str) -> Vec<&WorldStateDeltaRecord> {
        self.deltas
            .values()
            .filter(|delta| delta.scope == scope)
            .collect()
    }

    pub fn replace_scope(&mut self, scope: &str, deltas: Vec<WorldStateDeltaRecord>) {
        self.deltas.retain(|_, delta| delta.scope != scope);
        for delta in deltas {
            self.deltas.insert(delta.delta_id.clone(), delta);
        }
    }

    pub fn insert_existing(&mut self, delta: WorldStateDeltaRecord) {
        self.deltas.insert(delta.delta_id.clone(), delta);
    }
}

#[derive(Debug, Default)]
pub struct WorldStateRelationRegistry {
    relations: BTreeMap<String, WorldStateRelationRecord>,
}

impl WorldStateRelationRegistry {
    pub fn all(&self) -> Vec<&WorldStateRelationRecord> {
        self.relations.values().collect()
    }

    pub fn by_scope(&self, scope: &str) -> Vec<&WorldStateRelationRecord> {
        self.relations
            .values()
            .filter(|relation| relation.scope == scope)
            .collect()
    }

    pub fn replace_scope(&mut self, scope: &str, relations: Vec<WorldStateRelationRecord>) {
        self.relations.retain(|_, relation| relation.scope != scope);
        for relation in relations {
            self.relations
                .insert(relation.relation_id.clone(), relation);
        }
    }

    pub fn insert_existing(&mut self, relation: WorldStateRelationRecord) {
        self.relations
            .insert(relation.relation_id.clone(), relation);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HouseInventory {
    pub sessions: usize,
    pub lanes: usize,
    pub turns: usize,
    pub events: usize,
    pub mandalas: usize,
    pub capsules: usize,
    pub capsule_packages: usize,
    pub capsule_imports: usize,
    pub capsule_exports: usize,
    pub slots: usize,
    pub fieldvault_artifacts: usize,
    pub provider_lanes: usize,
    pub turn_dispatches: usize,
    pub memory_capsules: usize,
    pub summary_checkpoints: usize,
    pub memory_bridges: usize,
    pub resynthesis_triggers: usize,
    pub memory_promotions: usize,
    pub approval_requests: usize,
    pub world_state_nodes: usize,
    pub world_state_deltas: usize,
    pub world_state_relations: usize,
}

#[derive(Debug, Default)]
pub struct HouseRuntime {
    pub events: EventStore,
    pub sessions: SessionManager,
    pub mandalas: MandalaRegistry,
    pub capsules: CapsuleRegistry,
    pub capsule_packages: CapsulePackageRegistry,
    pub capsule_imports: CapsuleImportRegistry,
    pub capsule_exports: CapsuleExportRegistry,
    pub slots: SlotRegistry,
    pub fieldvault: FieldVaultRuntime,
    pub providers: ProviderLaneRegistry,
    pub dispatches: TurnDispatchRegistry,
    pub memory_capsules: MemoryCapsuleRegistry,
    pub summary_checkpoints: SummaryCheckpointRegistry,
    pub memory_bridges: MemoryBridgeRegistry,
    pub resynthesis_triggers: ResynthesisTriggerRegistry,
    pub memory_promotions: MemoryPromotionRegistry,
    pub approvals: ApprovalRegistry,
    pub world_state_nodes: WorldStateNodeRegistry,
    pub world_state_deltas: WorldStateDeltaRegistry,
    pub world_state_relations: WorldStateRelationRegistry,
}

impl HouseRuntime {
    pub fn bootstrap_session(
        &mut self,
        title: impl Into<String>,
        room_id: impl Into<String>,
    ) -> SessionRecord {
        let session = self.sessions.create_session(title, room_id);
        self.events.append(
            ActorRef {
                actor_type: "system".into(),
                actor_id: "house.runtime".into(),
            },
            SubjectRef {
                subject_type: "session".into(),
                subject_id: session.session_id.0.clone(),
            },
            EventType::SessionCreated,
            Some(&session.session_id),
            Some(&session.active_lane_id),
            None,
            serde_json::json!({
                "title": session.title,
                "room_id": session.room_id,
                "state": "active",
            }),
        );
        session
    }

    pub fn inventory(&self) -> HouseInventory {
        HouseInventory {
            sessions: self.sessions.sessions().len(),
            lanes: self.sessions.lanes().len(),
            turns: self.sessions.turns().len(),
            events: self.events.all().len(),
            mandalas: self.mandalas.all().len(),
            capsules: self.capsules.all().len(),
            capsule_packages: self.capsule_packages.all().len(),
            capsule_imports: self.capsule_imports.all().len(),
            capsule_exports: self.capsule_exports.all().len(),
            slots: self.slots.all().len(),
            fieldvault_artifacts: self.fieldvault.all().len(),
            provider_lanes: self.providers.all().len(),
            turn_dispatches: self.dispatches.all().len(),
            memory_capsules: self.memory_capsules.all().len(),
            summary_checkpoints: self.summary_checkpoints.all().len(),
            memory_bridges: self.memory_bridges.all().len(),
            resynthesis_triggers: self.resynthesis_triggers.all().len(),
            memory_promotions: self.memory_promotions.all().len(),
            approval_requests: self.approvals.all().len(),
            world_state_nodes: self.world_state_nodes.all().len(),
            world_state_deltas: self.world_state_deltas.all().len(),
            world_state_relations: self.world_state_relations.all().len(),
        }
    }

    pub fn refresh_memory_for_session(
        &mut self,
        session_id: &SessionId,
    ) -> Option<SummaryCheckpointRecord> {
        self.memory_capsules.reband_session(session_id);

        let hot_capsules: Vec<MemoryCapsuleRecord> = self
            .memory_capsules
            .by_session(session_id)
            .into_iter()
            .filter(|capsule| capsule.band == "hot")
            .cloned()
            .collect();
        let warm_capsules: Vec<MemoryCapsuleRecord> = self
            .memory_capsules
            .by_session(session_id)
            .into_iter()
            .filter(|capsule| capsule.band == "warm")
            .cloned()
            .collect();
        let archive_capsules: Vec<MemoryCapsuleRecord> = self
            .memory_capsules
            .by_session(session_id)
            .into_iter()
            .filter(|capsule| capsule.band == "archive")
            .cloned()
            .collect();

        if hot_capsules.is_empty() {
            return None;
        }

        let checkpoint = create_summary_for_band(
            &mut self.summary_checkpoints,
            session_id,
            "hot",
            &hot_capsules,
            0.86,
        );
        if !warm_capsules.is_empty() {
            create_summary_for_band(
                &mut self.summary_checkpoints,
                session_id,
                "warm",
                &warm_capsules,
                0.78,
            );
        }
        if !archive_capsules.is_empty() {
            create_summary_for_band(
                &mut self.summary_checkpoints,
                session_id,
                "archive",
                &archive_capsules,
                0.68,
            );
        }

        let semantic_digest = checkpoint.semantic_digest.clone();
        let decisions_retained = checkpoint.decisions_retained.clone();
        let unresolved_items = checkpoint.unresolved_items.clone();

        let latest_hot_capsule = hot_capsules.last().cloned();
        for slot in self.slots.all() {
            if let Some(mandala_id) = &slot.active_mandala_id {
                if let Ok(mandala) = self.mandalas.get_mut(mandala_id) {
                    let memory_policy = mandala.memory_policy.clone();
                    let hot_context_limit = memory_policy.hot_context_limit.max(1);
                    if memory_policy
                        .boot_include
                        .iter()
                        .any(|value| value == "active_snapshot")
                    {
                        mandala.active_snapshot.current_goal = decisions_retained
                            .last()
                            .cloned()
                            .unwrap_or_else(|| semantic_digest.clone());
                        mandala.active_snapshot.active_decisions = decisions_retained.clone();
                        mandala.active_snapshot.blockers = unresolved_items.clone();
                        mandala.active_snapshot.next_actions = hot_capsules
                            .iter()
                            .map(|capsule| capsule.content.clone())
                            .rev()
                            .take(3)
                            .collect();
                        mandala.active_snapshot.hot_context = hot_capsules
                            .iter()
                            .rev()
                            .take(hot_context_limit)
                            .map(|capsule| capsule.content.clone())
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                            .collect();
                    }

                    if let Some(capsule) = &latest_hot_capsule {
                        if memory_policy.promote_to_stable_memory
                            && memory_policy
                                .boot_include
                                .iter()
                                .any(|value| value == "stable_memory")
                            && capsule.role == "operator"
                            && capsule.confidence_level
                                >= memory_policy.promotion_confidence_threshold
                        {
                            mandala
                                .stable_memory
                                .learned_rules
                                .push(capsule.content.clone());
                            mandala.stable_memory.memory.insert(
                                format!("session.{}.latest_rule", session_id.0),
                                serde_json::json!(capsule.content),
                            );
                            let _ = self.memory_promotions.create(
                                session_id.clone(),
                                capsule.memory_capsule_id.clone(),
                                "mandala_stable_memory",
                                capsule.content.clone(),
                                capsule.confidence_level,
                            );
                        }
                    }
                }
            }
        }

        let active_policies = self
            .slots
            .all()
            .into_iter()
            .filter_map(|slot| slot.active_mandala_id.as_ref())
            .filter_map(|mandala_id| self.mandalas.get(mandala_id).ok())
            .map(|mandala| mandala.memory_policy.clone())
            .collect::<Vec<_>>();

        for capsule in hot_capsules
            .iter()
            .chain(warm_capsules.iter())
            .chain(archive_capsules.iter())
        {
            let should_seal = active_policies.iter().any(|policy| {
                policy.allow_fieldvault_sealing
                    && policy
                        .seal_privacy_classes
                        .iter()
                        .any(|class_name| class_name == &capsule.privacy_class)
            });

            if should_seal {
                let already_sealed = self
                    .memory_promotions
                    .by_session(session_id)
                    .into_iter()
                    .any(|promotion| {
                        promotion.memory_capsule_id == capsule.memory_capsule_id
                            && promotion.target_plane == "fieldvault"
                    });
                if !already_sealed {
                    let _ = self.fieldvault.register_artifact(
                        None,
                        None,
                        SealLevel::CapsulePrivate,
                        format!("./vault/{}.fld", capsule.memory_capsule_id),
                        true,
                        false,
                    );
                    let _ = self.memory_promotions.create(
                        session_id.clone(),
                        capsule.memory_capsule_id.clone(),
                        "fieldvault",
                        capsule.content.clone(),
                        capsule.confidence_level,
                    );
                }
            }
        }

        let hot_terms = hot_capsules
            .iter()
            .flat_map(|capsule| {
                capsule
                    .content
                    .to_lowercase()
                    .split_whitespace()
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        for capsule in warm_capsules.iter().chain(archive_capsules.iter()) {
            let content_lower = capsule.content.to_lowercase();
            if hot_terms
                .iter()
                .any(|term| term.len() > 4 && content_lower.contains(term))
            {
                let _ = self.memory_bridges.create(
                    session_id.clone(),
                    hot_capsules
                        .last()
                        .map(|value| value.memory_capsule_id.clone())
                        .unwrap_or_default(),
                    capsule.memory_capsule_id.clone(),
                    "topic_recurrence",
                    0.72,
                );
            }
        }

        if !warm_capsules.is_empty() || !archive_capsules.is_empty() {
            let bridge_count = self.memory_bridges.by_session(session_id).len();
            if bridge_count > 0 || unresolved_items.len() > 1 {
                let _ = self.resynthesis_triggers.create(
                    session_id.clone(),
                    "context_shift",
                    format!(
                        "Re-synthesize session memory due to {} bridges and {} unresolved items",
                        bridge_count,
                        unresolved_items.len()
                    ),
                    0.81,
                );
            }
        }

        Some(checkpoint)
    }

    pub fn query_memory(
        &self,
        session_id: &SessionId,
        query: &str,
        limit: usize,
    ) -> Vec<MemoryCapsuleRecord> {
        self.memory_capsules.query_session(session_id, query, limit)
    }

    pub fn query_memory_with_room(
        &self,
        session_id: &SessionId,
        room_id: &str,
        query: &str,
        session_limit: usize,
        room_limit: usize,
    ) -> (Vec<MemoryCapsuleRecord>, Vec<MemoryCapsuleRecord>) {
        let session_capsules = self
            .memory_capsules
            .query_session(session_id, query, session_limit);
        let mut room_capsules = self.memory_capsules.query_room(room_id, query, room_limit);
        room_capsules.retain(|capsule| capsule.session_id.0 != session_id.0);
        (session_capsules, room_capsules)
    }

    pub fn query_memory_with_fallbacks(
        &self,
        session_id: &SessionId,
        room_id: &str,
        query: &str,
        session_limit: usize,
        room_limit: usize,
        global_limit: usize,
    ) -> (
        Vec<MemoryCapsuleRecord>,
        Vec<MemoryCapsuleRecord>,
        Vec<MemoryCapsuleRecord>,
    ) {
        let session_capsules = self
            .memory_capsules
            .query_session(session_id, query, session_limit);
        let mut room_capsules = self.memory_capsules.query_room(room_id, query, room_limit);
        room_capsules.retain(|capsule| capsule.session_id.0 != session_id.0);

        let mut global_capsules = self.memory_capsules.query_global(query, global_limit);
        global_capsules
            .retain(|capsule| capsule.session_id.0 != session_id.0 && capsule.room_id != room_id);

        (session_capsules, room_capsules, global_capsules)
    }

    pub fn active_memory_policy(&self) -> Option<crate::mandala::MandalaMemoryPolicy> {
        self.slots
            .all()
            .into_iter()
            .find_map(|slot| slot.active_mandala_id.as_ref())
            .and_then(|mandala_id| self.mandalas.get(mandala_id).ok())
            .map(|mandala| mandala.memory_policy.clone())
    }
}

fn create_summary_for_band(
    registry: &mut SummaryCheckpointRegistry,
    session_id: &SessionId,
    source_band: &str,
    capsules: &[MemoryCapsuleRecord],
    confidence_level: f32,
) -> SummaryCheckpointRecord {
    let semantic_digest = capsules
        .iter()
        .map(|capsule| capsule.content.as_str())
        .collect::<Vec<_>>()
        .join(" | ");
    let source_text_length = capsules
        .iter()
        .map(|capsule| capsule.content.chars().count())
        .sum();
    let digest_text_length = semantic_digest.chars().count();
    let compression_ratio = if source_text_length == 0 {
        1.0
    } else {
        (digest_text_length as f32) / (source_text_length as f32)
    };
    let decisions_retained = capsules
        .iter()
        .filter_map(|capsule| capsule.intent_summary.clone())
        .collect::<Vec<_>>();
    let unresolved_items = capsules
        .iter()
        .filter(|capsule| capsule.role == "operator")
        .map(|capsule| capsule.content.clone())
        .collect::<Vec<_>>();

    registry.create(
        session_id.clone(),
        source_band,
        capsules
            .iter()
            .map(|capsule| capsule.memory_capsule_id.clone())
            .collect(),
        semantic_digest,
        capsules.len(),
        source_text_length,
        digest_text_length,
        compression_ratio,
        decisions_retained,
        unresolved_items,
        confidence_level,
    )
}

fn score_capsule_for_query(capsule: &MemoryCapsuleRecord, lowered_query: &str) -> f32 {
    let band_bonus = match capsule.band.as_str() {
        "hot" => 0.15,
        "warm" => 0.08,
        _ => 0.03,
    };
    let role_bonus = match capsule.role.as_str() {
        "operator" => 0.07,
        "distiller" => 0.05,
        "assistant" => 0.04,
        _ => 0.02,
    };
    let content_lower = capsule.content.to_lowercase();
    let intent_lower = capsule
        .intent_summary
        .as_ref()
        .map(|value| value.to_lowercase())
        .unwrap_or_default();
    let kind_bonus = match intent_lower.as_str() {
        "conversation/directive" => 0.12,
        "conversation/decision" => 0.11,
        "conversation/promise" => 0.1,
        "conversation/episode" => 0.08,
        "conversation/transcript" => 0.04,
        _ => 0.0,
    };
    let query_bonus = if lowered_query.is_empty() {
        0.0
    } else {
        let exact_content_match = if content_lower.contains(lowered_query) {
            0.25
        } else {
            0.0
        };
        let exact_intent_match = if intent_lower.contains(lowered_query) {
            0.22
        } else {
            0.0
        };
        let terms = lowered_query
            .split_whitespace()
            .filter(|term| !term.is_empty())
            .collect::<Vec<_>>();
        let content_term_hits = terms
            .iter()
            .filter(|term| content_lower.contains(**term))
            .count();
        let intent_term_hits = terms
            .iter()
            .filter(|term| intent_lower.contains(**term))
            .count();
        let term_overlap_bonus = if terms.is_empty() {
            0.0
        } else {
            ((content_term_hits as f32) * 0.04) + ((intent_term_hits as f32) * 0.05)
        };
        let directive_hint_bonus = if terms.iter().any(|term| {
            matches!(
                *term,
                "directive" | "decision" | "promise" | "goal" | "next" | "action"
            )
        }) && matches!(
            intent_lower.as_str(),
            "conversation/directive" | "conversation/decision" | "conversation/promise"
        ) {
            0.1
        } else {
            0.0
        };
        exact_content_match + exact_intent_match + term_overlap_bonus + directive_hint_bonus
    };

    capsule.relevance_score
        + (capsule.confidence_level * 0.2)
        + band_bonus
        + role_bonus
        + kind_bonus
        + query_bonus
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::{
        DurableStore, MandalaActiveSnapshot, MandalaCapabilityPolicy, MandalaExecutionPolicy,
        MandalaManifest, MandalaMemoryPolicy, MandalaProjection, MandalaRefs, MandalaSelf,
        MandalaStableMemory, SealLevel,
    };

    fn sample_mandala(id: &str) -> MandalaManifest {
        MandalaManifest {
            manifest_version: "mandala/v1".into(),
            kind: "mandala".into(),
            generated_at: 0.0,
            agent_version: 1,
            self_section: MandalaSelf {
                id: id.into(),
                role: "architect".into(),
                template_soul: "atlas".into(),
                execution_role: None,
                specialization: Some("systems".into()),
                tone: Some("precise".into()),
                canonical: true,
                boundaries: BTreeMap::new(),
                tags: vec!["core".into()],
            },
            execution_policy: MandalaExecutionPolicy {
                body: "contracts before code".into(),
                execution_lane: "main".into(),
                preferred_provider: "codex".into(),
                preferred_model: "gpt-5".into(),
                reasoning_effort: Some("high".into()),
                use_session_pool: false,
                allow_provider_override: false,
                capabilities: vec!["tool.exec.bash".into()],
            },
            capability_policy: MandalaCapabilityPolicy::default(),
            memory_policy: MandalaMemoryPolicy::default(),
            stable_memory: MandalaStableMemory::default(),
            active_snapshot: MandalaActiveSnapshot::default(),
            refs: MandalaRefs::default(),
            projection: MandalaProjection {
                projection_kind: "role-overlay".into(),
                requested_role: "architect".into(),
                template_soul: "atlas".into(),
                execution_role: None,
                default_body: "contracts before code".into(),
                lineage: vec!["architect".into()],
                autoevolve: true,
            },
            ownership: None,
            capsule_contract: None,
            skill_packs: Vec::new(),
            sacred_shards: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn bootstrap_session_emits_session_created_event() {
        let mut runtime = HouseRuntime::default();
        let session = runtime.bootstrap_session("JIMI bootstrap", "jimi-bootstrap-room");

        assert_eq!(runtime.events.all().len(), 1);
        assert_eq!(
            runtime.events.all()[0].event_type,
            EventType::SessionCreated
        );
        assert_eq!(
            runtime.events.all()[0].session_id.as_deref(),
            Some(session.session_id.0.as_str())
        );
    }

    #[test]
    fn can_install_capsule_and_bind_it_to_slot() {
        let mut runtime = HouseRuntime::default();
        let mandala = sample_mandala("jimi.core");
        runtime.mandalas.install(mandala);
        let capsule =
            runtime
                .capsules
                .install("capsule:jimi.core", "jimi.core", 1, "marketplace:test");
        runtime.slots.define_slot("primary", "Primary");
        runtime
            .slots
            .bind_capsule(
                "primary",
                capsule.capsule_id.clone(),
                capsule.mandala_id.clone(),
                false,
            )
            .unwrap();
        runtime.slots.activate("primary").unwrap();

        let slot = runtime.slots.get("primary").unwrap();
        assert_eq!(slot.state, SlotBindingState::Active);
        assert_eq!(slot.capsule_id.as_deref(), Some("capsule:jimi.core"));
        assert_eq!(slot.active_mandala_id.as_deref(), Some("jimi.core"));
    }

    #[test]
    fn fieldvault_runtime_registers_sealed_artifact() {
        let mut runtime = HouseRuntime::default();
        let artifact = runtime.fieldvault.register_artifact(
            Some("capsule:jimi.core".into()),
            Some("primary".into()),
            SealLevel::SacredShard,
            "/tmp/jimi-core.fld",
            true,
            false,
        );

        assert_eq!(runtime.fieldvault.all().len(), 1);
        assert_eq!(artifact.seal_level, SealLevel::SacredShard);
        assert_eq!(artifact.fld_path, "/tmp/jimi-core.fld");
    }

    #[test]
    fn durable_store_roundtrip_restores_runtime_state() {
        let mut runtime = HouseRuntime::default();
        let session = runtime.bootstrap_session("Persistent JIMI", "persistent-jimi-room");
        let turn = runtime
            .sessions
            .create_turn(&session.session_id, &session.active_lane_id, "architect")
            .unwrap();
        runtime.events.append(
            ActorRef {
                actor_type: "operator".into(),
                actor_id: "max".into(),
            },
            SubjectRef {
                subject_type: "turn".into(),
                subject_id: turn.turn_id.0.clone(),
            },
            EventType::TurnStarted,
            Some(&session.session_id),
            Some(&session.active_lane_id),
            Some(&turn.turn_id),
            serde_json::json!({ "intent_mode": "architect" }),
        );
        runtime.mandalas.install(sample_mandala("jimi.persist"));
        runtime
            .capsules
            .install("capsule:jimi.persist", "jimi.persist", 1, "test");
        runtime.slots.define_slot("primary", "Primary");
        runtime
            .slots
            .bind_capsule("primary", "capsule:jimi.persist", "jimi.persist", false)
            .unwrap();
        runtime.fieldvault.register_artifact(
            Some("capsule:jimi.persist".into()),
            Some("primary".into()),
            SealLevel::CapsulePrivate,
            "/tmp/jimi-persist.fld",
            true,
            false,
        );
        runtime.providers.connect(
            "provider.codex.primary",
            "codex",
            "gpt-5",
            "primary",
            Some("core".into()),
            100,
            "connected",
        );
        runtime.dispatches.dispatch(
            turn.turn_id.clone(),
            session.session_id.clone(),
            session.active_lane_id.clone(),
            "provider.codex.primary",
            "architect",
            "queued",
        );

        let db_path = temp_db_path();
        let mut store = DurableStore::open(&db_path).unwrap();
        store.persist_runtime(&runtime).unwrap();

        let restored = store.load_runtime().unwrap();
        assert_eq!(restored.events.all().len(), 2);
        assert_eq!(restored.sessions.sessions().len(), 1);
        assert_eq!(restored.sessions.turns().len(), 1);
        assert_eq!(restored.mandalas.all().len(), 1);
        assert_eq!(restored.capsules.all().len(), 1);
        assert_eq!(restored.slots.all().len(), 1);
        assert_eq!(restored.fieldvault.all().len(), 1);
        assert_eq!(restored.providers.all().len(), 1);
        assert_eq!(restored.dispatches.all().len(), 1);

        let _ = std::fs::remove_file(db_path);
    }

    fn temp_db_path() -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("jimi-kernel-test-{}.sqlite", uuid::Uuid::now_v7()));
        path
    }
}
