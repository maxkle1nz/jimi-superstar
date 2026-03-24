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
pub struct ProviderLaneRecord {
    pub provider_lane_id: String,
    pub provider: String,
    pub model: String,
    pub routing_mode: String,
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
    pub fn create_session(&mut self, title: impl Into<String>) -> SessionRecord {
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
        self.sessions
            .insert(session.session_id.0.clone(), session);
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
        self.capsules
            .insert(capsule.capsule_id.clone(), capsule);
    }
}

#[derive(Debug, Default)]
pub struct SlotRegistry {
    slots: BTreeMap<String, PersonalitySlot>,
}

impl SlotRegistry {
    pub fn define_slot(&mut self, slot_id: impl Into<String>, label: impl Into<String>) -> PersonalitySlot {
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
        status: impl Into<String>,
    ) -> ProviderLaneRecord {
        let lane = ProviderLaneRecord {
            provider_lane_id: provider_lane_id.into(),
            provider: provider.into(),
            model: model.into(),
            routing_mode: routing_mode.into(),
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

    pub fn insert_existing(&mut self, dispatch: TurnDispatchRecord) {
        self.dispatches.insert(dispatch.dispatch_id.clone(), dispatch);
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

    pub fn insert_existing(&mut self, capsule: MemoryCapsuleRecord) {
        self.capsules
            .insert(capsule.memory_capsule_id.clone(), capsule);
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
    pub slots: usize,
    pub fieldvault_artifacts: usize,
    pub provider_lanes: usize,
    pub turn_dispatches: usize,
    pub memory_capsules: usize,
}

#[derive(Debug, Default)]
pub struct HouseRuntime {
    pub events: EventStore,
    pub sessions: SessionManager,
    pub mandalas: MandalaRegistry,
    pub capsules: CapsuleRegistry,
    pub slots: SlotRegistry,
    pub fieldvault: FieldVaultRuntime,
    pub providers: ProviderLaneRegistry,
    pub dispatches: TurnDispatchRegistry,
    pub memory_capsules: MemoryCapsuleRegistry,
}

impl HouseRuntime {
    pub fn bootstrap_session(&mut self, title: impl Into<String>) -> SessionRecord {
        let session = self.sessions.create_session(title);
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
            slots: self.slots.all().len(),
            fieldvault_artifacts: self.fieldvault.all().len(),
            provider_lanes: self.providers.all().len(),
            turn_dispatches: self.dispatches.all().len(),
            memory_capsules: self.memory_capsules.all().len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::{
        DurableStore,
        MandalaActiveSnapshot, MandalaCapabilityPolicy, MandalaExecutionPolicy, MandalaManifest,
        MandalaMemoryPolicy, MandalaProjection, MandalaRefs, MandalaSelf, MandalaStableMemory,
        SealLevel,
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
        let session = runtime.bootstrap_session("JIMI bootstrap");

        assert_eq!(runtime.events.all().len(), 1);
        assert_eq!(runtime.events.all()[0].event_type, EventType::SessionCreated);
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
        let capsule = runtime
            .capsules
            .install("capsule:jimi.core", "jimi.core", 1, "marketplace:test");
        runtime.slots.define_slot("primary", "Primary");
        runtime
            .slots
            .bind_capsule("primary", capsule.capsule_id.clone(), capsule.mandala_id.clone(), false)
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
        let session = runtime.bootstrap_session("Persistent JIMI");
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
