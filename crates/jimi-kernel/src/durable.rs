use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use thiserror::Error;

use crate::{
    ApprovalRequestRecord, CapsuleExportRecord, CapsuleImportRecord, CapsulePackageRecord,
    CapsuleRecord, EventEnvelope, FieldVaultArtifact, HouseRuntime, KernelError, LaneId,
    LaneRecord, MandalaManifest, MemoryBridgeRecord, MemoryCapsuleRecord, MemoryPromotionRecord,
    PersonalitySlot, ProviderLaneRecord, ResynthesisTriggerRecord, SessionId, SessionRecord,
    SummaryCheckpointRecord, TurnDispatchRecord, TurnId, TurnRecord, WorldStateDeltaRecord,
    WorldStateNodeRecord, WorldStateRelationRecord,
};

#[derive(Debug, Error)]
pub enum DurableStoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("kernel error: {0}")]
    Kernel(#[from] KernelError),
    #[error("chrono parse error: {0}")]
    Chrono(#[from] chrono::ParseError),
}

pub struct DurableStore {
    conn: Connection,
}

impl DurableStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DurableStoreError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), DurableStoreError> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
              session_id TEXT PRIMARY KEY,
              room_id TEXT NOT NULL DEFAULT '',
              title TEXT NOT NULL,
              state TEXT NOT NULL,
              active_lane_id TEXT NOT NULL,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS lanes (
              lane_id TEXT PRIMARY KEY,
              session_id TEXT NOT NULL,
              parent_lane_id TEXT,
              state TEXT NOT NULL,
              created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS turns (
              turn_id TEXT PRIMARY KEY,
              session_id TEXT NOT NULL,
              lane_id TEXT NOT NULL,
              intent_mode TEXT NOT NULL,
              state TEXT NOT NULL,
              created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS events (
              event_id TEXT PRIMARY KEY,
              event_version TEXT NOT NULL,
              event_type TEXT NOT NULL,
              emitted_at TEXT NOT NULL,
              sequence INTEGER NOT NULL,
              session_id TEXT,
              lane_id TEXT,
              turn_id TEXT,
              actor_json TEXT NOT NULL,
              subject_json TEXT NOT NULL,
              payload_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS mandalas (
              mandala_id TEXT PRIMARY KEY,
              manifest_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS capsules (
              capsule_id TEXT PRIMARY KEY,
              mandala_id TEXT NOT NULL,
              version INTEGER NOT NULL,
              install_source TEXT NOT NULL,
              installed_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS capsule_packages (
              package_id TEXT PRIMARY KEY,
              capsule_id TEXT NOT NULL,
              mandala_id TEXT NOT NULL,
              version INTEGER NOT NULL,
              display_name TEXT NOT NULL,
              creator TEXT NOT NULL,
              source_origin TEXT NOT NULL,
              package_digest TEXT NOT NULL,
              trust_level TEXT NOT NULL,
              install_status TEXT NOT NULL,
              installed_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS capsule_imports (
              import_id TEXT PRIMARY KEY,
              package_id TEXT NOT NULL,
              source_origin TEXT NOT NULL,
              source_path TEXT NOT NULL,
              import_status TEXT NOT NULL,
              package_digest TEXT NOT NULL,
              imported_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS capsule_exports (
              export_id TEXT PRIMARY KEY,
              package_id TEXT NOT NULL,
              capsule_id TEXT NOT NULL,
              target_path TEXT NOT NULL,
              export_status TEXT NOT NULL,
              package_digest TEXT NOT NULL,
              exported_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS slots (
              slot_id TEXT PRIMARY KEY,
              label TEXT NOT NULL,
              capsule_id TEXT,
              principal_id TEXT,
              state TEXT NOT NULL,
              unlock_required INTEGER NOT NULL,
              active_mandala_id TEXT
            );

            CREATE TABLE IF NOT EXISTS fieldvault_artifacts (
              artifact_id TEXT PRIMARY KEY,
              capsule_id TEXT,
              slot_id TEXT,
              seal_level TEXT NOT NULL,
              fld_path TEXT NOT NULL,
              portable INTEGER NOT NULL,
              machine_bound INTEGER NOT NULL,
              sha256 TEXT,
              plaintext_projection_ref TEXT
            );

            CREATE TABLE IF NOT EXISTS provider_lanes (
              provider_lane_id TEXT PRIMARY KEY,
              provider TEXT NOT NULL,
              model TEXT NOT NULL,
              routing_mode TEXT NOT NULL,
              fallback_group TEXT,
              priority INTEGER NOT NULL DEFAULT 0,
              status TEXT NOT NULL,
              connected_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS turn_dispatches (
              dispatch_id TEXT PRIMARY KEY,
              turn_id TEXT NOT NULL,
              session_id TEXT NOT NULL,
              lane_id TEXT NOT NULL,
              provider_lane_id TEXT NOT NULL,
              intent_summary TEXT NOT NULL,
              status TEXT NOT NULL,
              dispatched_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS memory_capsules (
              memory_capsule_id TEXT PRIMARY KEY,
              session_id TEXT NOT NULL,
              room_id TEXT NOT NULL DEFAULT '',
              lane_id TEXT NOT NULL,
              turn_id TEXT,
              role TEXT NOT NULL,
              content TEXT NOT NULL,
              intent_summary TEXT,
              relevance_score REAL NOT NULL,
              confidence_level REAL NOT NULL,
              privacy_class TEXT NOT NULL,
              band TEXT NOT NULL,
              created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS summary_checkpoints (
              summary_checkpoint_id TEXT PRIMARY KEY,
              session_id TEXT NOT NULL,
              source_band TEXT NOT NULL,
              source_capsule_ids_json TEXT NOT NULL,
              semantic_digest TEXT NOT NULL,
              decisions_retained_json TEXT NOT NULL,
              unresolved_items_json TEXT NOT NULL,
              confidence_level REAL NOT NULL,
              generated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS memory_bridges (
              memory_bridge_id TEXT PRIMARY KEY,
              session_id TEXT NOT NULL,
              from_capsule_id TEXT NOT NULL,
              to_capsule_id TEXT NOT NULL,
              bridge_kind TEXT NOT NULL,
              strength REAL NOT NULL,
              created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS resynthesis_triggers (
              resynthesis_trigger_id TEXT PRIMARY KEY,
              session_id TEXT NOT NULL,
              trigger_kind TEXT NOT NULL,
              summary TEXT NOT NULL,
              confidence_level REAL NOT NULL,
              created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS memory_promotions (
              memory_promotion_id TEXT PRIMARY KEY,
              session_id TEXT NOT NULL,
              memory_capsule_id TEXT NOT NULL,
              target_plane TEXT NOT NULL,
              promoted_value TEXT NOT NULL,
              confidence_level REAL NOT NULL,
              created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS approval_requests (
              approval_request_id TEXT PRIMARY KEY,
              session_id TEXT NOT NULL,
              dispatch_id TEXT NOT NULL,
              provider_lane_id TEXT NOT NULL,
              action TEXT NOT NULL,
              reason TEXT NOT NULL,
              status TEXT NOT NULL,
              created_at TEXT NOT NULL,
              resolved_at TEXT
            );

            CREATE TABLE IF NOT EXISTS world_state_nodes (
              node_id TEXT PRIMARY KEY,
              scope TEXT NOT NULL,
              path TEXT NOT NULL,
              kind TEXT NOT NULL,
              size_bytes INTEGER NOT NULL,
              metadata_hash TEXT NOT NULL,
              status TEXT NOT NULL,
              observed_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS world_state_deltas (
              delta_id TEXT PRIMARY KEY,
              scope TEXT NOT NULL,
              change_kind TEXT NOT NULL,
              path TEXT NOT NULL,
              summary TEXT NOT NULL,
              observed_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS world_state_relations (
              relation_id TEXT PRIMARY KEY,
              scope TEXT NOT NULL,
              from_node_id TEXT NOT NULL,
              to_node_id TEXT NOT NULL,
              relation_kind TEXT NOT NULL,
              summary TEXT NOT NULL,
              observed_at TEXT NOT NULL
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS memory_capsules_fts USING fts5(
              memory_capsule_id UNINDEXED,
              session_id UNINDEXED,
              room_id UNINDEXED,
              content,
              intent_summary
            );
            "#,
        )?;
        self.ensure_column_exists(
            "sessions",
            "room_id",
            "ALTER TABLE sessions ADD COLUMN room_id TEXT NOT NULL DEFAULT ''",
        )?;
        self.ensure_column_exists(
            "memory_capsules",
            "room_id",
            "ALTER TABLE memory_capsules ADD COLUMN room_id TEXT NOT NULL DEFAULT ''",
        )?;
        self.ensure_column_exists(
            "summary_checkpoints",
            "source_capsule_count",
            "ALTER TABLE summary_checkpoints ADD COLUMN source_capsule_count INTEGER NOT NULL DEFAULT 0",
        )?;
        self.ensure_column_exists(
            "summary_checkpoints",
            "source_text_length",
            "ALTER TABLE summary_checkpoints ADD COLUMN source_text_length INTEGER NOT NULL DEFAULT 0",
        )?;
        self.ensure_column_exists(
            "summary_checkpoints",
            "digest_text_length",
            "ALTER TABLE summary_checkpoints ADD COLUMN digest_text_length INTEGER NOT NULL DEFAULT 0",
        )?;
        self.ensure_column_exists(
            "summary_checkpoints",
            "compression_ratio",
            "ALTER TABLE summary_checkpoints ADD COLUMN compression_ratio REAL NOT NULL DEFAULT 1.0",
        )?;
        self.ensure_column_exists(
            "provider_lanes",
            "fallback_group",
            "ALTER TABLE provider_lanes ADD COLUMN fallback_group TEXT",
        )?;
        self.ensure_column_exists(
            "provider_lanes",
            "priority",
            "ALTER TABLE provider_lanes ADD COLUMN priority INTEGER NOT NULL DEFAULT 0",
        )?;
        Ok(())
    }

    fn ensure_column_exists(
        &self,
        table: &str,
        column: &str,
        alter_sql: &str,
    ) -> Result<(), DurableStoreError> {
        let pragma = format!("PRAGMA table_info({table})");
        let mut stmt = self.conn.prepare(&pragma)?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        let mut present = false;
        for row in rows {
            if row? == column {
                present = true;
                break;
            }
        }
        if !present {
            self.conn.execute_batch(alter_sql)?;
        }
        Ok(())
    }

    pub fn persist_runtime(&mut self, runtime: &HouseRuntime) -> Result<(), DurableStoreError> {
        let tx = self.conn.transaction()?;

        for session in runtime.sessions.sessions() {
            tx.execute(
                "INSERT OR REPLACE INTO sessions (session_id, room_id, title, state, active_lane_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    session.session_id.0,
                    session.room_id,
                    session.title,
                    serde_json::to_string(&session.state)?,
                    session.active_lane_id.0,
                    session.created_at.to_rfc3339(),
                    session.updated_at.to_rfc3339(),
                ],
            )?;
        }

        for lane in runtime.sessions.lanes() {
            tx.execute(
                "INSERT OR REPLACE INTO lanes (lane_id, session_id, parent_lane_id, state, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    lane.lane_id.0,
                    lane.session_id.0,
                    lane.parent_lane_id.as_ref().map(|id| id.0.clone()),
                    serde_json::to_string(&lane.state)?,
                    lane.created_at.to_rfc3339(),
                ],
            )?;
        }

        for turn in runtime.sessions.turns() {
            tx.execute(
                "INSERT OR REPLACE INTO turns (turn_id, session_id, lane_id, intent_mode, state, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    turn.turn_id.0,
                    turn.session_id.0,
                    turn.lane_id.0,
                    turn.intent_mode,
                    serde_json::to_string(&turn.state)?,
                    turn.created_at.to_rfc3339(),
                ],
            )?;
        }

        for event in runtime.events.all() {
            tx.execute(
                "INSERT OR REPLACE INTO events (event_id, event_version, event_type, emitted_at, sequence, session_id, lane_id, turn_id, actor_json, subject_json, payload_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    event.event_id,
                    event.event_version,
                    serde_json::to_string(&event.event_type)?,
                    event.emitted_at.to_rfc3339(),
                    event.sequence as i64,
                    event.session_id,
                    event.lane_id,
                    event.turn_id,
                    serde_json::to_string(&event.actor)?,
                    serde_json::to_string(&event.subject)?,
                    serde_json::to_string(&event.payload)?,
                ],
            )?;
        }

        for mandala in runtime.mandalas.all() {
            tx.execute(
                "INSERT OR REPLACE INTO mandalas (mandala_id, manifest_json) VALUES (?1, ?2)",
                params![mandala.self_section.id, serde_json::to_string(mandala)?],
            )?;
        }

        for capsule in runtime.capsules.all() {
            tx.execute(
                "INSERT OR REPLACE INTO capsules (capsule_id, mandala_id, version, install_source, installed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    capsule.capsule_id,
                    capsule.mandala_id,
                    capsule.version as i64,
                    capsule.install_source,
                    capsule.installed_at.to_rfc3339(),
                ],
            )?;
        }

        for package in runtime.capsule_packages.all() {
            tx.execute(
                "INSERT OR REPLACE INTO capsule_packages (package_id, capsule_id, mandala_id, version, display_name, creator, source_origin, package_digest, trust_level, install_status, installed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    package.package_id,
                    package.capsule_id,
                    package.mandala_id,
                    package.version as i64,
                    package.display_name,
                    package.creator,
                    package.source_origin,
                    package.package_digest,
                    package.trust_level,
                    package.install_status,
                    package.installed_at.to_rfc3339(),
                ],
            )?;
        }

        for record in runtime.capsule_imports.all() {
            tx.execute(
                "INSERT OR REPLACE INTO capsule_imports (import_id, package_id, source_origin, source_path, import_status, package_digest, imported_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    record.import_id,
                    record.package_id,
                    record.source_origin,
                    record.source_path,
                    record.import_status,
                    record.package_digest,
                    record.imported_at.to_rfc3339(),
                ],
            )?;
        }

        for record in runtime.capsule_exports.all() {
            tx.execute(
                "INSERT OR REPLACE INTO capsule_exports (export_id, package_id, capsule_id, target_path, export_status, package_digest, exported_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    record.export_id,
                    record.package_id,
                    record.capsule_id,
                    record.target_path,
                    record.export_status,
                    record.package_digest,
                    record.exported_at.to_rfc3339(),
                ],
            )?;
        }

        for slot in runtime.slots.all() {
            tx.execute(
                "INSERT OR REPLACE INTO slots (slot_id, label, capsule_id, principal_id, state, unlock_required, active_mandala_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    slot.slot_id,
                    slot.label,
                    slot.capsule_id,
                    slot.principal_id,
                    serde_json::to_string(&slot.state)?,
                    slot.unlock_required as i64,
                    slot.active_mandala_id,
                ],
            )?;
        }

        for artifact in runtime.fieldvault.all() {
            tx.execute(
                "INSERT OR REPLACE INTO fieldvault_artifacts (artifact_id, capsule_id, slot_id, seal_level, fld_path, portable, machine_bound, sha256, plaintext_projection_ref)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    artifact.artifact_id,
                    artifact.capsule_id,
                    artifact.slot_id,
                    serde_json::to_string(&artifact.seal_level)?,
                    artifact.fld_path,
                    artifact.portable as i64,
                    artifact.machine_bound as i64,
                    artifact.sha256,
                    artifact.plaintext_projection_ref,
                ],
            )?;
        }

        for provider in runtime.providers.all() {
            tx.execute(
                "INSERT OR REPLACE INTO provider_lanes (provider_lane_id, provider, model, routing_mode, fallback_group, priority, status, connected_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    provider.provider_lane_id,
                    provider.provider,
                    provider.model,
                    provider.routing_mode,
                    provider.fallback_group,
                    provider.priority as i64,
                    provider.status,
                    provider.connected_at.to_rfc3339(),
                ],
            )?;
        }

        for dispatch in runtime.dispatches.all() {
            tx.execute(
                "INSERT OR REPLACE INTO turn_dispatches (dispatch_id, turn_id, session_id, lane_id, provider_lane_id, intent_summary, status, dispatched_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    dispatch.dispatch_id,
                    dispatch.turn_id.0,
                    dispatch.session_id.0,
                    dispatch.lane_id.0,
                    dispatch.provider_lane_id,
                    dispatch.intent_summary,
                    dispatch.status,
                    dispatch.dispatched_at.to_rfc3339(),
                ],
            )?;
        }

        for capsule in runtime.memory_capsules.all() {
            tx.execute(
                "INSERT OR REPLACE INTO memory_capsules (memory_capsule_id, session_id, room_id, lane_id, turn_id, role, content, intent_summary, relevance_score, confidence_level, privacy_class, band, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    capsule.memory_capsule_id,
                    capsule.session_id.0,
                    capsule.room_id,
                    capsule.lane_id.0,
                    capsule.turn_id.as_ref().map(|id| id.0.clone()),
                    capsule.role,
                    capsule.content,
                    capsule.intent_summary,
                    capsule.relevance_score,
                    capsule.confidence_level,
                    capsule.privacy_class,
                    capsule.band,
                    capsule.created_at.to_rfc3339(),
                ],
            )?;
        }

        tx.execute("DELETE FROM memory_capsules_fts", [])?;
        for capsule in runtime.memory_capsules.all() {
            tx.execute(
                "INSERT INTO memory_capsules_fts (memory_capsule_id, session_id, room_id, content, intent_summary)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    capsule.memory_capsule_id,
                    capsule.session_id.0,
                    capsule.room_id,
                    capsule.content,
                    capsule.intent_summary,
                ],
            )?;
        }

        for checkpoint in runtime.summary_checkpoints.all() {
            tx.execute(
                "INSERT OR REPLACE INTO summary_checkpoints (summary_checkpoint_id, session_id, source_band, source_capsule_ids_json, semantic_digest, source_capsule_count, source_text_length, digest_text_length, compression_ratio, decisions_retained_json, unresolved_items_json, confidence_level, generated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    checkpoint.summary_checkpoint_id,
                    checkpoint.session_id.0,
                    checkpoint.source_band,
                    serde_json::to_string(&checkpoint.source_capsule_ids)?,
                    checkpoint.semantic_digest,
                    checkpoint.source_capsule_count as i64,
                    checkpoint.source_text_length as i64,
                    checkpoint.digest_text_length as i64,
                    checkpoint.compression_ratio,
                    serde_json::to_string(&checkpoint.decisions_retained)?,
                    serde_json::to_string(&checkpoint.unresolved_items)?,
                    checkpoint.confidence_level,
                    checkpoint.generated_at.to_rfc3339(),
                ],
            )?;
        }

        for bridge in runtime.memory_bridges.all() {
            tx.execute(
                "INSERT OR REPLACE INTO memory_bridges (memory_bridge_id, session_id, from_capsule_id, to_capsule_id, bridge_kind, strength, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    bridge.memory_bridge_id,
                    bridge.session_id.0,
                    bridge.from_capsule_id,
                    bridge.to_capsule_id,
                    bridge.bridge_kind,
                    bridge.strength,
                    bridge.created_at.to_rfc3339(),
                ],
            )?;
        }

        for trigger in runtime.resynthesis_triggers.all() {
            tx.execute(
                "INSERT OR REPLACE INTO resynthesis_triggers (resynthesis_trigger_id, session_id, trigger_kind, summary, confidence_level, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    trigger.resynthesis_trigger_id,
                    trigger.session_id.0,
                    trigger.trigger_kind,
                    trigger.summary,
                    trigger.confidence_level,
                    trigger.created_at.to_rfc3339(),
                ],
            )?;
        }

        for promotion in runtime.memory_promotions.all() {
            tx.execute(
                "INSERT OR REPLACE INTO memory_promotions (memory_promotion_id, session_id, memory_capsule_id, target_plane, promoted_value, confidence_level, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    promotion.memory_promotion_id,
                    promotion.session_id.0,
                    promotion.memory_capsule_id,
                    promotion.target_plane,
                    promotion.promoted_value,
                    promotion.confidence_level,
                    promotion.created_at.to_rfc3339(),
                ],
            )?;
        }

        for approval in runtime.approvals.all() {
            tx.execute(
                "INSERT OR REPLACE INTO approval_requests (approval_request_id, session_id, dispatch_id, provider_lane_id, action, reason, status, created_at, resolved_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    approval.approval_request_id,
                    approval.session_id.0,
                    approval.dispatch_id,
                    approval.provider_lane_id,
                    approval.action,
                    approval.reason,
                    approval.status,
                    approval.created_at.to_rfc3339(),
                    approval.resolved_at.map(|dt| dt.to_rfc3339()),
                ],
            )?;
        }

        for node in runtime.world_state_nodes.all() {
            tx.execute(
                "INSERT OR REPLACE INTO world_state_nodes (node_id, scope, path, kind, size_bytes, metadata_hash, status, observed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    node.node_id,
                    node.scope,
                    node.path,
                    node.kind,
                    node.size_bytes as i64,
                    node.metadata_hash,
                    node.status,
                    node.observed_at.to_rfc3339(),
                ],
            )?;
        }

        for delta in runtime.world_state_deltas.all() {
            tx.execute(
                "INSERT OR REPLACE INTO world_state_deltas (delta_id, scope, change_kind, path, summary, observed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    delta.delta_id,
                    delta.scope,
                    delta.change_kind,
                    delta.path,
                    delta.summary,
                    delta.observed_at.to_rfc3339(),
                ],
            )?;
        }

        for relation in runtime.world_state_relations.all() {
            tx.execute(
                "INSERT OR REPLACE INTO world_state_relations (relation_id, scope, from_node_id, to_node_id, relation_kind, summary, observed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    relation.relation_id,
                    relation.scope,
                    relation.from_node_id,
                    relation.to_node_id,
                    relation.relation_kind,
                    relation.summary,
                    relation.observed_at.to_rfc3339(),
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn load_runtime(&self) -> Result<HouseRuntime, DurableStoreError> {
        let mut runtime = HouseRuntime::default();

        {
            let mut stmt = self.conn.prepare(
                "SELECT session_id, room_id, title, state, active_lane_id, created_at, updated_at FROM sessions ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })?;
            for row in rows {
                let (session_id, room_id, title, state, active_lane_id, created_at, updated_at) =
                    row?;
                runtime.sessions.insert_session(SessionRecord {
                    session_id: SessionId(session_id),
                    room_id,
                    title,
                    state: from_json_string(&state)?,
                    active_lane_id: LaneId(active_lane_id),
                    created_at: parse_dt(&created_at)?,
                    updated_at: parse_dt(&updated_at)?,
                });
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT lane_id, session_id, parent_lane_id, state, created_at FROM lanes ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?;
            for row in rows {
                let (lane_id, session_id, parent_lane_id, state, created_at) = row?;
                runtime.sessions.insert_lane(LaneRecord {
                    lane_id: LaneId(lane_id),
                    session_id: SessionId(session_id),
                    parent_lane_id: parent_lane_id.map(LaneId),
                    state: from_json_string(&state)?,
                    created_at: parse_dt(&created_at)?,
                });
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT turn_id, session_id, lane_id, intent_mode, state, created_at FROM turns ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })?;
            for row in rows {
                let (turn_id, session_id, lane_id, intent_mode, state, created_at) = row?;
                runtime.sessions.insert_turn(TurnRecord {
                    turn_id: TurnId(turn_id),
                    session_id: SessionId(session_id),
                    lane_id: LaneId(lane_id),
                    intent_mode,
                    state: from_json_string(&state)?,
                    created_at: parse_dt(&created_at)?,
                });
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT event_id, event_version, event_type, emitted_at, sequence, session_id, lane_id, turn_id, actor_json, subject_json, payload_json
                 FROM events ORDER BY sequence ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                ))
            })?;
            let mut max_sequence = 0;
            for row in rows {
                let (
                    event_id,
                    event_version,
                    event_type,
                    emitted_at,
                    sequence,
                    session_id,
                    lane_id,
                    turn_id,
                    actor,
                    subject,
                    payload,
                ) = row?;
                let event = EventEnvelope {
                    event_id,
                    event_version,
                    event_type: from_json_string(&event_type)?,
                    emitted_at: parse_dt(&emitted_at)?,
                    sequence: sequence as u64,
                    session_id,
                    lane_id,
                    turn_id,
                    actor: from_json_string(&actor)?,
                    subject: from_json_string(&subject)?,
                    payload: from_json_string(&payload)?,
                };
                max_sequence = max_sequence.max(event.sequence);
                runtime.events.push_existing(event);
            }
            runtime.events.set_next_sequence(max_sequence);
        }

        {
            let mut stmt = self.conn.prepare("SELECT manifest_json FROM mandalas")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            for row in rows {
                let mandala: MandalaManifest = from_json_string(&row?)?;
                runtime.mandalas.install(mandala);
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT capsule_id, mandala_id, version, install_source, installed_at FROM capsules",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?;
            for row in rows {
                let (capsule_id, mandala_id, version, install_source, installed_at) = row?;
                runtime.capsules.insert_existing(CapsuleRecord {
                    capsule_id,
                    mandala_id,
                    version: version as u32,
                    install_source,
                    installed_at: parse_dt(&installed_at)?,
                });
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT package_id, capsule_id, mandala_id, version, display_name, creator, source_origin, package_digest, trust_level, install_status, installed_at FROM capsule_packages",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                ))
            })?;
            for row in rows {
                let (
                    package_id,
                    capsule_id,
                    mandala_id,
                    version,
                    display_name,
                    creator,
                    source_origin,
                    package_digest,
                    trust_level,
                    install_status,
                    installed_at,
                ) = row?;
                runtime.capsule_packages.insert_existing(CapsulePackageRecord {
                    package_id,
                    capsule_id,
                    mandala_id,
                    version: version as u32,
                    display_name,
                    creator,
                    source_origin,
                    package_digest,
                    trust_level,
                    install_status,
                    installed_at: parse_dt(&installed_at)?,
                });
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT import_id, package_id, source_origin, source_path, import_status, package_digest, imported_at FROM capsule_imports",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })?;
            for row in rows {
                let (
                    import_id,
                    package_id,
                    source_origin,
                    source_path,
                    import_status,
                    package_digest,
                    imported_at,
                ) = row?;
                runtime.capsule_imports.insert_existing(CapsuleImportRecord {
                    import_id,
                    package_id,
                    source_origin,
                    source_path,
                    import_status,
                    package_digest,
                    imported_at: parse_dt(&imported_at)?,
                });
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT export_id, package_id, capsule_id, target_path, export_status, package_digest, exported_at FROM capsule_exports",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })?;
            for row in rows {
                let (
                    export_id,
                    package_id,
                    capsule_id,
                    target_path,
                    export_status,
                    package_digest,
                    exported_at,
                ) = row?;
                runtime.capsule_exports.insert_existing(CapsuleExportRecord {
                    export_id,
                    package_id,
                    capsule_id,
                    target_path,
                    export_status,
                    package_digest,
                    exported_at: parse_dt(&exported_at)?,
                });
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT slot_id, label, capsule_id, principal_id, state, unlock_required, active_mandala_id FROM slots",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            })?;
            for row in rows {
                let (
                    slot_id,
                    label,
                    capsule_id,
                    principal_id,
                    state,
                    unlock_required,
                    active_mandala_id,
                ) = row?;
                runtime.slots.insert_existing(PersonalitySlot {
                    slot_id,
                    label,
                    capsule_id,
                    principal_id,
                    state: from_json_string(&state)?,
                    unlock_required: unlock_required != 0,
                    active_mandala_id,
                });
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT artifact_id, capsule_id, slot_id, seal_level, fld_path, portable, machine_bound, sha256, plaintext_projection_ref FROM fieldvault_artifacts",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                ))
            })?;
            for row in rows {
                let (
                    artifact_id,
                    capsule_id,
                    slot_id,
                    seal_level,
                    fld_path,
                    portable,
                    machine_bound,
                    sha256,
                    plaintext_projection_ref,
                ) = row?;
                runtime.fieldvault.insert_existing(FieldVaultArtifact {
                    artifact_id,
                    capsule_id,
                    slot_id,
                    seal_level: from_json_string(&seal_level)?,
                    fld_path,
                    portable: portable != 0,
                    machine_bound: machine_bound != 0,
                    sha256,
                    plaintext_projection_ref,
                });
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT provider_lane_id, provider, model, routing_mode, fallback_group, priority, status, connected_at FROM provider_lanes",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })?;
            for row in rows {
                let (
                    provider_lane_id,
                    provider,
                    model,
                    routing_mode,
                    fallback_group,
                    priority,
                    status,
                    connected_at,
                ) = row?;
                runtime.providers.insert_existing(ProviderLaneRecord {
                    provider_lane_id,
                    provider,
                    model,
                    routing_mode,
                    fallback_group,
                    priority: priority as u32,
                    status,
                    connected_at: parse_dt(&connected_at)?,
                });
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT dispatch_id, turn_id, session_id, lane_id, provider_lane_id, intent_summary, status, dispatched_at FROM turn_dispatches",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })?;
            for row in rows {
                let (
                    dispatch_id,
                    turn_id,
                    session_id,
                    lane_id,
                    provider_lane_id,
                    intent_summary,
                    status,
                    dispatched_at,
                ) = row?;
                runtime.dispatches.insert_existing(TurnDispatchRecord {
                    dispatch_id,
                    turn_id: TurnId(turn_id),
                    session_id: SessionId(session_id),
                    lane_id: LaneId(lane_id),
                    provider_lane_id,
                    intent_summary,
                    status,
                    dispatched_at: parse_dt(&dispatched_at)?,
                });
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT memory_capsule_id, session_id, room_id, lane_id, turn_id, role, content, intent_summary, relevance_score, confidence_level, privacy_class, band, created_at FROM memory_capsules ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, f32>(8)?,
                    row.get::<_, f32>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, String>(12)?,
                ))
            })?;
            for row in rows {
                let (
                    memory_capsule_id,
                    session_id,
                    room_id,
                    lane_id,
                    turn_id,
                    role,
                    content,
                    intent_summary,
                    relevance_score,
                    confidence_level,
                    privacy_class,
                    band,
                    created_at,
                ) = row?;
                runtime
                    .memory_capsules
                    .insert_existing(MemoryCapsuleRecord {
                        memory_capsule_id,
                        session_id: SessionId(session_id),
                        room_id,
                        lane_id: LaneId(lane_id),
                        turn_id: turn_id.map(TurnId),
                        role,
                        content,
                        intent_summary,
                        relevance_score,
                        confidence_level,
                        privacy_class,
                        band,
                        created_at: parse_dt(&created_at)?,
                    });
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT summary_checkpoint_id, session_id, source_band, source_capsule_ids_json, semantic_digest, source_capsule_count, source_text_length, digest_text_length, compression_ratio, decisions_retained_json, unresolved_items_json, confidence_level, generated_at FROM summary_checkpoints ORDER BY generated_at ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, f32>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, f32>(11)?,
                    row.get::<_, String>(12)?,
                ))
            })?;
            for row in rows {
                let (
                    summary_checkpoint_id,
                    session_id,
                    source_band,
                    source_capsule_ids_json,
                    semantic_digest,
                    source_capsule_count,
                    source_text_length,
                    digest_text_length,
                    compression_ratio,
                    decisions_retained_json,
                    unresolved_items_json,
                    confidence_level,
                    generated_at,
                ) = row?;
                runtime
                    .summary_checkpoints
                    .insert_existing(SummaryCheckpointRecord {
                        summary_checkpoint_id,
                        session_id: SessionId(session_id),
                        source_band,
                        source_capsule_ids: from_json_string(&source_capsule_ids_json)?,
                        semantic_digest,
                        source_capsule_count: source_capsule_count as usize,
                        source_text_length: source_text_length as usize,
                        digest_text_length: digest_text_length as usize,
                        compression_ratio,
                        decisions_retained: from_json_string(&decisions_retained_json)?,
                        unresolved_items: from_json_string(&unresolved_items_json)?,
                        confidence_level,
                        generated_at: parse_dt(&generated_at)?,
                    });
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT memory_bridge_id, session_id, from_capsule_id, to_capsule_id, bridge_kind, strength, created_at FROM memory_bridges ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, f32>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })?;
            for row in rows {
                let (
                    memory_bridge_id,
                    session_id,
                    from_capsule_id,
                    to_capsule_id,
                    bridge_kind,
                    strength,
                    created_at,
                ) = row?;
                runtime.memory_bridges.insert_existing(MemoryBridgeRecord {
                    memory_bridge_id,
                    session_id: SessionId(session_id),
                    from_capsule_id,
                    to_capsule_id,
                    bridge_kind,
                    strength,
                    created_at: parse_dt(&created_at)?,
                });
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT resynthesis_trigger_id, session_id, trigger_kind, summary, confidence_level, created_at FROM resynthesis_triggers ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, f32>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })?;
            for row in rows {
                let (
                    resynthesis_trigger_id,
                    session_id,
                    trigger_kind,
                    summary,
                    confidence_level,
                    created_at,
                ) = row?;
                runtime
                    .resynthesis_triggers
                    .insert_existing(ResynthesisTriggerRecord {
                        resynthesis_trigger_id,
                        session_id: SessionId(session_id),
                        trigger_kind,
                        summary,
                        confidence_level,
                        created_at: parse_dt(&created_at)?,
                    });
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT memory_promotion_id, session_id, memory_capsule_id, target_plane, promoted_value, confidence_level, created_at FROM memory_promotions ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, f32>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })?;
            for row in rows {
                let (
                    memory_promotion_id,
                    session_id,
                    memory_capsule_id,
                    target_plane,
                    promoted_value,
                    confidence_level,
                    created_at,
                ) = row?;
                runtime
                    .memory_promotions
                    .insert_existing(MemoryPromotionRecord {
                        memory_promotion_id,
                        session_id: SessionId(session_id),
                        memory_capsule_id,
                        target_plane,
                        promoted_value,
                        confidence_level,
                        created_at: parse_dt(&created_at)?,
                    });
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT approval_request_id, session_id, dispatch_id, provider_lane_id, action, reason, status, created_at, resolved_at FROM approval_requests ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, Option<String>>(8)?,
                ))
            })?;
            for row in rows {
                let (
                    approval_request_id,
                    session_id,
                    dispatch_id,
                    provider_lane_id,
                    action,
                    reason,
                    status,
                    created_at,
                    resolved_at,
                ) = row?;
                runtime.approvals.insert_existing(ApprovalRequestRecord {
                    approval_request_id,
                    session_id: SessionId(session_id),
                    dispatch_id,
                    provider_lane_id,
                    action,
                    reason,
                    status,
                    created_at: parse_dt(&created_at)?,
                    resolved_at: resolved_at
                        .map(|value| parse_dt(&value))
                        .transpose()?,
                });
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT node_id, scope, path, kind, size_bytes, metadata_hash, status, observed_at FROM world_state_nodes ORDER BY observed_at ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })?;
            for row in rows {
                let (node_id, scope, path, kind, size_bytes, metadata_hash, status, observed_at) =
                    row?;
                runtime.world_state_nodes.insert_existing(WorldStateNodeRecord {
                    node_id,
                    scope,
                    path,
                    kind,
                    size_bytes: size_bytes as u64,
                    metadata_hash,
                    status,
                    observed_at: parse_dt(&observed_at)?,
                });
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT delta_id, scope, change_kind, path, summary, observed_at FROM world_state_deltas ORDER BY observed_at ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })?;
            for row in rows {
                let (delta_id, scope, change_kind, path, summary, observed_at) = row?;
                runtime.world_state_deltas.insert_existing(WorldStateDeltaRecord {
                    delta_id,
                    scope,
                    change_kind,
                    path,
                    summary,
                    observed_at: parse_dt(&observed_at)?,
                });
            }
        }

        {
            let mut stmt = self.conn.prepare(
                "SELECT relation_id, scope, from_node_id, to_node_id, relation_kind, summary, observed_at FROM world_state_relations ORDER BY observed_at ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })?;
            for row in rows {
                let (relation_id, scope, from_node_id, to_node_id, relation_kind, summary, observed_at) =
                    row?;
                runtime
                    .world_state_relations
                    .insert_existing(WorldStateRelationRecord {
                        relation_id,
                        scope,
                        from_node_id,
                        to_node_id,
                        relation_kind,
                        summary,
                        observed_at: parse_dt(&observed_at)?,
                    });
            }
        }

        Ok(runtime)
    }

    pub fn search_memory_capsules(
        &self,
        scope: &str,
        query: &str,
        session_id: Option<&str>,
        room_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemoryCapsuleRecord>, DurableStoreError> {
        let limit = limit.max(1).min(32) as i64;
        let trimmed_query = query.trim();
        let like_pattern = format!("%{}%", trimmed_query.to_lowercase());
        let fts_query = sanitize_fts_query(trimmed_query);
        let session_scope = session_id.unwrap_or_default().to_string();
        let room_scope = room_id.unwrap_or_default().to_string();

        let use_fts = !fts_query.is_empty();
        let (sql, params_vec): (&str, Vec<&dyn rusqlite::ToSql>) = match (scope, use_fts) {
            ("session", true) => (
                "SELECT mc.memory_capsule_id, mc.session_id, mc.room_id, mc.lane_id, mc.turn_id, mc.role, mc.content, mc.intent_summary, mc.relevance_score, mc.confidence_level, mc.privacy_class, mc.band, mc.created_at
                 FROM memory_capsules_fts
                 JOIN memory_capsules mc ON mc.memory_capsule_id = memory_capsules_fts.memory_capsule_id
                 WHERE mc.session_id = ?1
                   AND memory_capsules_fts MATCH ?2
                 ORDER BY bm25(memory_capsules_fts), mc.relevance_score DESC, mc.created_at DESC
                 LIMIT ?3",
                vec![&session_scope, &fts_query, &limit],
            ),
            ("room", true) => (
                "SELECT mc.memory_capsule_id, mc.session_id, mc.room_id, mc.lane_id, mc.turn_id, mc.role, mc.content, mc.intent_summary, mc.relevance_score, mc.confidence_level, mc.privacy_class, mc.band, mc.created_at
                 FROM memory_capsules_fts
                 JOIN memory_capsules mc ON mc.memory_capsule_id = memory_capsules_fts.memory_capsule_id
                 WHERE mc.room_id = ?1
                   AND memory_capsules_fts MATCH ?2
                 ORDER BY bm25(memory_capsules_fts), mc.relevance_score DESC, mc.created_at DESC
                 LIMIT ?3",
                vec![&room_scope, &fts_query, &limit],
            ),
            (_, true) => (
                "SELECT mc.memory_capsule_id, mc.session_id, mc.room_id, mc.lane_id, mc.turn_id, mc.role, mc.content, mc.intent_summary, mc.relevance_score, mc.confidence_level, mc.privacy_class, mc.band, mc.created_at
                 FROM memory_capsules_fts
                 JOIN memory_capsules mc ON mc.memory_capsule_id = memory_capsules_fts.memory_capsule_id
                 WHERE memory_capsules_fts MATCH ?1
                 ORDER BY bm25(memory_capsules_fts), mc.relevance_score DESC, mc.created_at DESC
                 LIMIT ?2",
                vec![&fts_query, &limit],
            ),
            ("session", false) => (
                "SELECT memory_capsule_id, session_id, room_id, lane_id, turn_id, role, content, intent_summary, relevance_score, confidence_level, privacy_class, band, created_at
                 FROM memory_capsules
                 WHERE session_id = ?1
                   AND (LOWER(content) LIKE ?2 OR LOWER(COALESCE(intent_summary, '')) LIKE ?2)
                 ORDER BY relevance_score DESC, created_at DESC
                 LIMIT ?3",
                vec![&session_scope, &like_pattern, &limit],
            ),
            ("room", false) => (
                "SELECT memory_capsule_id, session_id, room_id, lane_id, turn_id, role, content, intent_summary, relevance_score, confidence_level, privacy_class, band, created_at
                 FROM memory_capsules
                 WHERE room_id = ?1
                   AND (LOWER(content) LIKE ?2 OR LOWER(COALESCE(intent_summary, '')) LIKE ?2)
                 ORDER BY relevance_score DESC, created_at DESC
                 LIMIT ?3",
                vec![&room_scope, &like_pattern, &limit],
            ),
            _ => (
                "SELECT memory_capsule_id, session_id, room_id, lane_id, turn_id, role, content, intent_summary, relevance_score, confidence_level, privacy_class, band, created_at
                 FROM memory_capsules
                 WHERE LOWER(content) LIKE ?1 OR LOWER(COALESCE(intent_summary, '')) LIKE ?1
                 ORDER BY relevance_score DESC, created_at DESC
                 LIMIT ?2",
                vec![&like_pattern, &limit],
            ),
        };

        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, f32>(8)?,
                row.get::<_, f32>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, String>(11)?,
                row.get::<_, String>(12)?,
            ))
        })?;

        let mut results = Vec::new();
        for row in rows {
            let (
                memory_capsule_id,
                session_id,
                room_id,
                lane_id,
                turn_id,
                role,
                content,
                intent_summary,
                relevance_score,
                confidence_level,
                privacy_class,
                band,
                created_at,
            ) = row?;
            results.push(MemoryCapsuleRecord {
                memory_capsule_id,
                session_id: SessionId(session_id),
                room_id,
                lane_id: LaneId(lane_id),
                turn_id: turn_id.map(TurnId),
                role,
                content,
                intent_summary,
                relevance_score,
                confidence_level,
                privacy_class,
                band,
                created_at: parse_dt(&created_at)?,
            });
        }
        Ok(results)
    }
}

fn parse_dt(value: &str) -> Result<DateTime<Utc>, DurableStoreError> {
    Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Utc))
}

fn sanitize_fts_query(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .take(8)
        .collect::<Vec<_>>()
        .join(" ")
}

fn from_json_string<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, DurableStoreError> {
    Ok(serde_json::from_str(value)?)
}
