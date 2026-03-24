use std::collections::BTreeMap;

use chrono::Utc;
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

#[derive(Debug, Clone)]
pub struct SessionRecord {
    pub session_id: SessionId,
    pub title: String,
    pub state: SessionState,
    pub active_lane_id: LaneId,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LaneRecord {
    pub lane_id: LaneId,
    pub session_id: SessionId,
    pub parent_lane_id: Option<LaneId>,
    pub state: LaneState,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TurnRecord {
    pub turn_id: TurnId,
    pub session_id: SessionId,
    pub lane_id: LaneId,
    pub intent_mode: String,
    pub state: TurnState,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CapsuleRecord {
    pub capsule_id: String,
    pub mandala_id: String,
    pub version: u32,
    pub install_source: String,
    pub installed_at: chrono::DateTime<Utc>,
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
}

#[derive(Debug, Default)]
pub struct HouseRuntime {
    pub events: EventStore,
    pub sessions: SessionManager,
    pub mandalas: MandalaRegistry,
    pub capsules: CapsuleRegistry,
    pub slots: SlotRegistry,
    pub fieldvault: FieldVaultRuntime,
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
}
