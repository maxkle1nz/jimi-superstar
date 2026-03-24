use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    net::SocketAddr,
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex},
    time::Duration,
};

mod provider_adapter;
mod provider_auth;

use axum::{
    Json, Router,
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::Html,
    routing::{get, post},
};
use jimi_kernel::{
    ActorRef, ApprovalRequestRecord, CapsuleExportRecord, CapsuleImportRecord, DurableStore,
    EventEnvelope, EventType, FieldVaultArtifact, HouseInventory, HouseRuntime,
    MandalaActiveSnapshot, MandalaCapabilityPolicy,
    MandalaCapsuleContract, MandalaExecutionPolicy, MandalaManifest, MandalaMemoryPolicy,
    MandalaProjection, MandalaRefs, MandalaSelf, MandalaStableMemory, MemoryBridgeRecord,
    CapsulePackageRecord, MemoryCapsuleRecord, MemoryPromotionRecord, ProviderLaneRecord, ResynthesisTriggerRecord, SealLevel,
    SessionId, SessionRecord, SlotBindingState, SubjectRef, SummaryCheckpointRecord, TurnDispatchRecord,
    TurnRecord, WorldStateDeltaRecord, WorldStateNodeRecord, WorldStateRelationRecord,
    WorldStateProcessEntry, WorldStateSlice, WorldStateWorkspaceEntry,
};
use provider_adapter::{
    fallback_candidates, provider_adapter_label, resolve_provider_adapter, run_provider_adapter,
    ProviderExecutionError,
};
use provider_auth::{detect_provider_credentials, ProviderCredentialStatus};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc};

#[derive(Clone)]
struct AppState {
    runtime: Arc<Mutex<HouseRuntime>>,
    store: Arc<Mutex<DurableStore>>,
    events_tx: broadcast::Sender<EventEnvelope>,
    house_root: PathBuf,
    distiller_tx: mpsc::UnboundedSender<String>,
    distiller_status: Arc<Mutex<TranscriptDistillerStatus>>,
    distiller_pending: Arc<Mutex<BTreeSet<String>>>,
    autonomy_status: Arc<Mutex<AutonomyStatus>>,
}

#[derive(Debug, Default, Clone, Serialize)]
struct TranscriptDistillerStatus {
    running: bool,
    pending: usize,
    processed: u64,
    failed: u64,
    last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct AutonomyStatus {
    enabled: bool,
    running: bool,
    cycles: u64,
    completed_dispatches: u64,
    transition_count: u64,
    last_transition: Option<String>,
    last_dispatch_id: Option<String>,
    last_intent_summary: Option<String>,
    last_selection_reason: Option<String>,
    last_error: Option<String>,
}

impl Default for AutonomyStatus {
    fn default() -> Self {
        Self {
            enabled: true,
            running: false,
            cycles: 0,
            completed_dispatches: 0,
            transition_count: 0,
            last_transition: None,
            last_dispatch_id: None,
            last_intent_summary: None,
            last_selection_reason: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    inventory: HouseInventory,
}

#[derive(Debug, Deserialize)]
struct CreateSessionRequest {
    title: String,
    room_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateTurnRequest {
    session_id: String,
    intent_mode: String,
    intent_summary: String,
}

#[derive(Debug, Deserialize)]
struct UpdateMemoryPolicyRequest {
    hot_context_limit: usize,
    relevant_context_limit: usize,
    promotion_confidence_threshold: f32,
    promote_to_stable_memory: bool,
    allow_fieldvault_sealing: bool,
    world_state_scope: String,
    world_state_entry_limit: usize,
    world_state_process_limit: usize,
    allow_world_state: bool,
}

#[derive(Debug, Serialize)]
struct InventoryResponse {
    inventory: HouseInventory,
    mandalas: Vec<String>,
    slots: Vec<String>,
}

#[derive(Debug, Serialize)]
struct MacroAxisStatus {
    label: &'static str,
    percent: u8,
    phase: &'static str,
    summary: &'static str,
    steps_remaining: u8,
}

#[derive(Debug, Serialize)]
struct MacroMenuItem {
    key: &'static str,
    prompt: &'static str,
}

#[derive(Debug, Serialize)]
struct MacroStatusResponse {
    retrobuilder: MacroAxisStatus,
    l1ght: MacroAxisStatus,
    project: MacroAxisStatus,
    current_phase_steps_remaining: u8,
    project_steps_remaining: u8,
    subsystems: Vec<MacroAxisStatus>,
    done: Vec<&'static str>,
    doing: Vec<&'static str>,
    next: Vec<&'static str>,
    later: Vec<&'static str>,
    menu: Vec<MacroMenuItem>,
}

#[derive(Debug, Serialize)]
struct DistillerStatusResponse {
    running: bool,
    pending: usize,
    processed: u64,
    failed: u64,
    last_error: Option<String>,
}

#[derive(Debug, Serialize)]
struct SecurityStatusResponse {
    active_mandala_id: Option<String>,
    preferred_provider: Option<String>,
    preferred_model: Option<String>,
    sealing_enabled: bool,
    seal_privacy_classes: Vec<String>,
    capability_declared: usize,
    capability_required: usize,
    capability_optional: usize,
    sacred_shards: usize,
    skill_packs: usize,
    artifacts_total: usize,
    artifact_seal_levels: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CapsuleTrustLevelCount {
    trust_level: String,
    packages: usize,
}

#[derive(Debug, Serialize)]
struct CapsuleTrustStatusResponse {
    total_packages: usize,
    internal_packages: usize,
    external_packages: usize,
    trust_levels: Vec<CapsuleTrustLevelCount>,
    packages: Vec<CapsulePackageRecord>,
}

#[derive(Debug, Serialize)]
struct MarketplaceStatusResponse {
    total_entries: usize,
    installable_entries: usize,
    trusted_entries: usize,
    entries: Vec<CapsulePackageRecord>,
}

#[derive(Debug, Serialize)]
struct CapsuleImportsResponse {
    imports: Vec<CapsuleImportRecord>,
}

#[derive(Debug, Serialize)]
struct CapsuleExportsResponse {
    exports: Vec<CapsuleExportRecord>,
}

#[derive(Debug, Serialize)]
struct CapsuleImportDemoResponse {
    package: CapsulePackageRecord,
    import_record: CapsuleImportRecord,
}

#[derive(Debug, Serialize)]
struct CapsuleExportResponse {
    package: CapsulePackageRecord,
    export_record: CapsuleExportRecord,
}

#[derive(Debug, Serialize)]
struct ProviderReadinessResponse {
    providers: Vec<ProviderCredentialStatus>,
}

#[derive(Debug, Serialize)]
struct WorldStateStatusResponse {
    active_mandala_id: Option<String>,
    preferred_scope: String,
    lookup_sources: Vec<String>,
    slice: WorldStateSlice,
}

#[derive(Debug, Serialize)]
struct AutonomyStatusResponse {
    enabled: bool,
    running: bool,
    cycles: u64,
    completed_dispatches: u64,
    transition_count: u64,
    last_transition: Option<String>,
    mission_phase: String,
    current_goal: Option<String>,
    next_actions: Vec<String>,
    queued_dispatch_count: usize,
    awaiting_approval_count: usize,
    recommended_next_step: String,
    transition_guidance: String,
    last_dispatch_id: Option<String>,
    last_intent_summary: Option<String>,
    last_selection_reason: Option<String>,
    last_error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ApprovalsStatusResponse {
    pending: usize,
    granted: usize,
    denied: usize,
    approvals: Vec<ApprovalRequestRecord>,
}

#[derive(Debug, Serialize)]
struct ApprovalActionResponse {
    approval: ApprovalRequestRecord,
}

#[derive(Debug, Deserialize)]
struct SetAutonomyRequest {
    enabled: bool,
}

#[derive(Debug, Serialize)]
struct MemorySearchResponse {
    scope: String,
    query: String,
    session_id: Option<String>,
    room_id: Option<String>,
    results: Vec<MemoryCapsuleRecord>,
}

#[derive(Debug, Serialize)]
struct ControlPlaneSection {
    label: &'static str,
    status: &'static str,
    priority: &'static str,
    summary: &'static str,
}

#[derive(Debug, Serialize)]
struct ControlPlaneResponse {
    title: &'static str,
    sections: Vec<ControlPlaneSection>,
}

#[derive(Debug, Serialize)]
struct CapsuleBootstrapResponse {
    mandala_id: String,
    capsule_id: String,
    package_id: String,
    slot_id: String,
    slot_state: SlotBindingState,
}

#[derive(Debug, Serialize)]
struct CapsuleInstallPreviewResponse {
    package: CapsulePackageRecord,
    slot_recommendation: String,
    required_capabilities: Vec<String>,
    optional_capabilities: Vec<String>,
    fieldvault_requirements: usize,
    install_preview_summary: String,
}

#[derive(Debug, Deserialize)]
struct UpdateCapsuleTrustRequest {
    trust_level: String,
}

#[derive(Debug, Serialize)]
struct CapsuleInstallRequestResponse {
    package: CapsulePackageRecord,
    approval: ApprovalRequestRecord,
}

#[derive(Debug, Serialize)]
struct CapsuleActivationResponse {
    package_id: String,
    capsule_id: String,
    mandala_id: String,
    slot_id: String,
    slot_state: SlotBindingState,
}

#[derive(Debug, Serialize)]
struct MemoryPolicyUpdateResponse {
    mandala_id: String,
    memory_policy: MandalaMemoryPolicy,
}

#[derive(Debug, Serialize)]
struct ArtifactBootstrapResponse {
    artifact_id: String,
    capsule_id: Option<String>,
    slot_id: Option<String>,
    seal_level: SealLevel,
}

#[derive(Debug, Serialize)]
struct ProviderBootstrapResponse {
    provider_lane_id: String,
    provider: String,
    model: String,
    routing_mode: String,
    fallback_group: Option<String>,
    priority: u32,
    status: String,
}

#[derive(Debug, Deserialize)]
struct UpdateProviderLaneRequest {
    routing_mode: String,
    fallback_group: Option<String>,
    priority: u32,
}

#[derive(Debug, Serialize)]
struct ProviderLaneUpdateResponse {
    provider: ProviderLaneRecord,
}

#[derive(Debug, Serialize)]
struct TurnBootstrapResponse {
    turn: TurnRecord,
    dispatch: TurnDispatchRecord,
}

#[derive(Debug, Serialize)]
struct DispatchExecutionResponse {
    dispatch: TurnDispatchRecord,
    turn: TurnRecord,
    output_text: String,
}

#[derive(Debug)]
struct CompactedProviderResponse {
    memory_text: String,
    confidence_level: f32,
    raw_length: usize,
    compacted_length: usize,
}

#[derive(Debug, Serialize)]
struct ContextPacketResponse {
    session_id: String,
    room_id: String,
    active_mandala_id: Option<String>,
    memory_policy: MandalaMemoryPolicy,
    hot_capsules: Vec<MemoryCapsuleRecord>,
    relevant_capsules: Vec<MemoryCapsuleRecord>,
    room_relevant_capsules: Vec<MemoryCapsuleRecord>,
    global_relevant_capsules: Vec<MemoryCapsuleRecord>,
    summary_checkpoints: Vec<SummaryCheckpointRecord>,
    memory_bridges: Vec<MemoryBridgeRecord>,
    resynthesis_triggers: Vec<ResynthesisTriggerRecord>,
    memory_promotions: Vec<MemoryPromotionRecord>,
    stable_memory: Option<MandalaStableMemory>,
    active_snapshot: Option<MandalaActiveSnapshot>,
    query_seed: String,
    providers: Vec<String>,
    world_state_slice: WorldStateSlice,
}

#[tokio::main]
async fn main() {
    let db_path = std::env::var("JIMI_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./data/jimi.sqlite"));
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let store = DurableStore::open(&db_path).expect("failed to open durable store");
    let runtime = store.load_runtime().unwrap_or_default();
    let (distiller_tx, distiller_rx) = mpsc::unbounded_channel();
    let distiller_status_state = Arc::new(Mutex::new(TranscriptDistillerStatus::default()));
    let distiller_pending = Arc::new(Mutex::new(BTreeSet::new()));
    let autonomy_status_state = Arc::new(Mutex::new(AutonomyStatus::default()));

    let state = AppState {
        runtime: Arc::new(Mutex::new(runtime)),
        store: Arc::new(Mutex::new(store)),
        events_tx: broadcast::channel(256).0,
        house_root: std::env::current_dir().expect("failed to resolve current dir"),
        distiller_tx,
        distiller_status: distiller_status_state.clone(),
        distiller_pending: distiller_pending.clone(),
        autonomy_status: autonomy_status_state.clone(),
    };

    spawn_transcript_distiller(state.clone(), distiller_rx);
    spawn_autonomy_loop(state.clone());

    let app = Router::new()
        .route("/", get(cockpit))
        .route("/health", get(health))
        .route("/sessions", get(list_sessions).post(create_session))
        .route("/turns", get(list_turns).post(create_turn))
        .route("/dispatches", get(list_dispatches))
        .route("/dispatches/execute-latest", post(execute_latest_dispatch))
        .route(
            "/dispatches/execute-latest-live",
            post(execute_latest_dispatch_live),
        )
        .route("/memory/capsules", get(list_memory_capsules))
        .route("/memory/summaries", get(list_summary_checkpoints))
        .route("/memory/bridges", get(list_memory_bridges))
        .route(
            "/memory/resynthesis-triggers",
            get(list_resynthesis_triggers),
        )
        .route("/memory/promotions", get(list_memory_promotions))
        .route("/memory/query/{session_id}", get(query_memory))
        .route("/memory/search", get(search_memory))
        .route("/context-packet/{session_id}", get(context_packet))
        .route("/mandalas", get(list_mandalas))
        .route(
            "/mandalas/{mandala_id}/memory-policy",
            post(update_mandala_memory_policy),
        )
        .route("/capsules", get(list_capsules))
        .route("/capsule-packages", get(list_capsule_packages))
        .route("/capsule-imports", get(list_capsule_imports))
        .route("/capsule-exports", get(list_capsule_exports))
        .route(
            "/capsule-packages/{package_id}/preview",
            get(capsule_package_preview),
        )
        .route(
            "/capsule-packages/{package_id}/trust",
            post(update_capsule_trust),
        )
        .route(
            "/capsule-packages/{package_id}/install-request",
            post(request_capsule_install),
        )
        .route(
            "/capsule-packages/{package_id}/activate",
            post(activate_capsule_package),
        )
        .route(
            "/capsule-packages/{package_id}/export",
            post(export_capsule_package),
        )
        .route("/slots", get(list_slots))
        .route("/artifacts", get(list_artifacts))
        .route("/providers", get(list_providers))
        .route("/providers/{provider_lane_id}", post(update_provider_lane))
        .route("/status/macro", get(macro_status))
        .route("/status/control-plane", get(control_plane_status))
        .route("/status/security", get(security_status))
        .route("/status/capsule-trust", get(capsule_trust_status))
        .route("/status/marketplace", get(marketplace_status))
        .route("/status/approvals", get(approvals_status))
        .route("/status/provider-readiness", get(provider_readiness_status))
        .route("/status/world-state", get(world_state_status))
        .route("/status/distiller", get(distiller_status))
        .route("/status/autonomy", get(autonomy_status))
        .route("/autonomy", post(set_autonomy))
        .route("/approvals", get(list_approvals))
        .route("/approvals/{approval_id}/grant", post(grant_approval))
        .route("/approvals/{approval_id}/deny", post(deny_approval))
        .route("/bootstrap/core-capsule", post(bootstrap_core_capsule))
        .route("/bootstrap/import-demo-capsule", post(import_demo_capsule))
        .route("/bootstrap/core-artifact", post(bootstrap_core_artifact))
        .route("/bootstrap/provider-lane", post(bootstrap_provider_lane))
        .route(
            "/bootstrap/provider-lane-anthropic",
            post(bootstrap_provider_lane_anthropic),
        )
        .route("/events", get(list_events))
        .route("/ws/events", get(ws_events))
        .route("/inventory", get(inventory))
        .with_state(state);

    let addr: SocketAddr = std::env::var("JIMI_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".into())
        .parse()
        .expect("invalid JIMI_ADDR");

    println!("JIMI server listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind listener");
    axum::serve(listener, app).await.expect("server crashed");
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(HealthResponse {
        status: "ok",
        inventory: runtime.inventory(),
    })
}

async fn cockpit() -> Html<&'static str> {
    Html(COCKPIT_HTML)
}

async fn list_sessions(State(state): State<AppState>) -> Json<Vec<SessionRecord>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(runtime.sessions.sessions().into_iter().cloned().collect())
}

async fn create_session(
    State(state): State<AppState>,
    Json(request): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<SessionRecord>), (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let room_id = request
        .room_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| slugify_room_id(&request.title));
    let session = runtime.bootstrap_session(request.title, room_id);
    let new_event = runtime.events.all().last().cloned();
    persist_runtime(&state, &runtime)?;
    drop(runtime);
    if let Some(event) = new_event {
        let _ = state.events_tx.send(event);
    }
    Ok((StatusCode::CREATED, Json(session)))
}

async fn create_turn(
    State(state): State<AppState>,
    Json(request): Json<CreateTurnRequest>,
) -> Result<(StatusCode, Json<TurnBootstrapResponse>), (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let session_id = jimi_kernel::SessionId(request.session_id.clone());
    let session = runtime
        .sessions
        .session(&session_id)
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?
        .clone();

    let provider_candidates = runtime.providers.all();
    let provider_lane = provider_candidates
        .iter()
        .find(|provider| provider.routing_mode == "primary" && provider.status == "connected")
        .or_else(|| {
            provider_candidates
                .iter()
                .find(|provider| provider.status == "connected")
        })
        .cloned()
        .cloned()
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "connect a provider lane before dispatching a turn".to_string(),
            )
        })?;

    let (turn, dispatch) = queue_turn_for_lane(
        &mut runtime,
        &session,
        &provider_lane,
        request.intent_mode.clone(),
        request.intent_summary.clone(),
        "operator",
        "cockpit.turns",
    )?;

    let new_events: Vec<EventEnvelope> =
        runtime.events.all().iter().rev().take(2).cloned().collect();
    persist_runtime(&state, &runtime)?;
    drop(runtime);

    for event in new_events.into_iter().rev() {
        let _ = state.events_tx.send(event);
    }

    Ok((
        StatusCode::CREATED,
        Json(TurnBootstrapResponse { turn, dispatch }),
    ))
}

fn queue_turn_for_lane(
    runtime: &mut HouseRuntime,
    session: &SessionRecord,
    provider_lane: &ProviderLaneRecord,
    intent_mode: String,
    intent_summary: String,
    actor_type: &str,
    actor_id: &str,
) -> Result<(TurnRecord, TurnDispatchRecord), (StatusCode, String)> {
    let turn = runtime
        .sessions
        .create_turn(&session.session_id, &session.active_lane_id, intent_mode.clone())
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;

    runtime.events.append(
        ActorRef {
            actor_type: actor_type.into(),
            actor_id: actor_id.into(),
        },
        SubjectRef {
            subject_type: "turn".into(),
            subject_id: turn.turn_id.0.clone(),
        },
        EventType::TurnStarted,
        Some(&turn.session_id),
        Some(&turn.lane_id),
        Some(&turn.turn_id),
        serde_json::json!({
            "intent_mode": intent_mode,
            "intent_summary": intent_summary.clone(),
        }),
    );

    let dispatch = runtime.dispatches.dispatch(
        turn.turn_id.clone(),
        turn.session_id.clone(),
        turn.lane_id.clone(),
        provider_lane.provider_lane_id.clone(),
        intent_summary.clone(),
        "queued",
    );

    runtime.memory_capsules.append(
        turn.session_id.clone(),
        session.room_id.clone(),
        turn.lane_id.clone(),
        Some(turn.turn_id.clone()),
        if actor_type == "house" { "distiller" } else { "operator" },
        intent_summary.clone(),
        Some(intent_summary.clone()),
        0.95,
        0.95,
        "session_open",
        "hot",
    );
    runtime.refresh_memory_for_session(&turn.session_id);

    runtime.events.append(
        ActorRef {
            actor_type: "router".into(),
            actor_id: "house.dispatch".into(),
        },
        SubjectRef {
            subject_type: "provider_lane".into(),
            subject_id: provider_lane.provider_lane_id.clone(),
        },
        EventType::EngineSelected,
        Some(&turn.session_id),
        Some(&turn.lane_id),
        Some(&turn.turn_id),
        serde_json::json!({
            "dispatch_id": dispatch.dispatch_id.clone(),
            "provider_lane_id": provider_lane.provider_lane_id.clone(),
            "provider": provider_lane.provider.clone(),
            "model": provider_lane.model.clone(),
            "intent_summary": intent_summary,
        }),
    );

    Ok((turn, dispatch))
}

async fn list_events(State(state): State<AppState>) -> Json<Vec<EventEnvelope>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(runtime.events.all().to_vec())
}

async fn list_turns(State(state): State<AppState>) -> Json<Vec<TurnRecord>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(runtime.sessions.turns().into_iter().cloned().collect())
}

async fn list_dispatches(State(state): State<AppState>) -> Json<Vec<TurnDispatchRecord>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(runtime.dispatches.all().into_iter().cloned().collect())
}

async fn list_memory_capsules(State(state): State<AppState>) -> Json<Vec<MemoryCapsuleRecord>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(runtime.memory_capsules.all().into_iter().cloned().collect())
}

async fn list_summary_checkpoints(
    State(state): State<AppState>,
) -> Json<Vec<SummaryCheckpointRecord>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(
        runtime
            .summary_checkpoints
            .all()
            .into_iter()
            .cloned()
            .collect(),
    )
}

async fn list_memory_bridges(State(state): State<AppState>) -> Json<Vec<MemoryBridgeRecord>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(runtime.memory_bridges.all().into_iter().cloned().collect())
}

async fn list_resynthesis_triggers(
    State(state): State<AppState>,
) -> Json<Vec<ResynthesisTriggerRecord>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(
        runtime
            .resynthesis_triggers
            .all()
            .into_iter()
            .cloned()
            .collect(),
    )
}

async fn list_memory_promotions(State(state): State<AppState>) -> Json<Vec<MemoryPromotionRecord>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(
        runtime
            .memory_promotions
            .all()
            .into_iter()
            .cloned()
            .collect(),
    )
}

async fn context_packet(
    axum::extract::Path(session_id): axum::extract::Path<String>,
    State(state): State<AppState>,
) -> Result<Json<ContextPacketResponse>, (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let session_id = jimi_kernel::SessionId(session_id);
    let memory_policy = runtime.active_memory_policy().unwrap_or_default();
    refresh_world_state_cache(&mut runtime, &state.house_root, &memory_policy);
    persist_runtime(&state, &runtime)?;
    Ok(Json(assemble_context_packet(
        &runtime,
        &session_id,
        &state.house_root,
    )))
}

fn assemble_context_packet(
    runtime: &HouseRuntime,
    session_id: &jimi_kernel::SessionId,
    house_root: &PathBuf,
) -> ContextPacketResponse {
    let memory_policy = runtime.active_memory_policy().unwrap_or_default();
    let active_mandala_id = runtime
        .slots
        .all()
        .into_iter()
        .find_map(|slot| slot.active_mandala_id.clone());

    let hot_capsules: Vec<MemoryCapsuleRecord> = runtime
        .memory_capsules
        .by_session(session_id)
        .into_iter()
        .rev()
        .take(memory_policy.hot_context_limit.max(1))
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let active_mandala = active_mandala_id
        .as_ref()
        .and_then(|mandala_id| runtime.mandalas.get(mandala_id).ok());

    let active_snapshot = if memory_policy
        .boot_include
        .iter()
        .any(|value| value == "active_snapshot")
    {
        active_mandala.map(|mandala| mandala.active_snapshot.clone())
    } else {
        None
    };

    let stable_memory = if memory_policy
        .boot_include
        .iter()
        .any(|value| value == "stable_memory")
    {
        active_mandala.map(|mandala| mandala.stable_memory.clone())
    } else {
        None
    };

    let query_seed = active_snapshot
        .as_ref()
        .map(|snapshot| snapshot.current_goal.clone())
        .or_else(|| hot_capsules.last().map(|capsule| capsule.content.clone()))
        .unwrap_or_default();

    let room_id = runtime
        .sessions
        .session(session_id)
        .map(|session| session.room_id.clone())
        .unwrap_or_default();
    let (relevant_capsules, room_relevant_capsules, global_relevant_capsules) = runtime
        .query_memory_with_fallbacks(
            session_id,
            &room_id,
            &query_seed,
            memory_policy.relevant_context_limit.max(1),
            4,
            3,
        );

    let summary_checkpoints = runtime
        .summary_checkpoints
        .by_session(session_id)
        .into_iter()
        .rev()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let memory_bridges = runtime
        .memory_bridges
        .by_session(session_id)
        .into_iter()
        .rev()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let resynthesis_triggers = runtime
        .resynthesis_triggers
        .by_session(session_id)
        .into_iter()
        .rev()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let memory_promotions = runtime
        .memory_promotions
        .by_session(session_id)
        .into_iter()
        .rev()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let providers = runtime
        .providers
        .all()
        .into_iter()
        .map(|provider| format!("{}:{}", provider.provider, provider.model))
        .collect();
    let world_state_slice = build_world_state_slice_from_cache(runtime, house_root, &memory_policy);

    ContextPacketResponse {
        session_id: session_id.0.clone(),
        room_id,
        active_mandala_id,
        memory_policy,
        hot_capsules,
        relevant_capsules,
        room_relevant_capsules,
        global_relevant_capsules,
        summary_checkpoints,
        memory_bridges,
        resynthesis_triggers,
        memory_promotions,
        stable_memory,
        active_snapshot,
        query_seed,
        providers,
        world_state_slice,
    }
}

fn build_provider_prompt(
    provider_lane_id: &str,
    packet: &ContextPacketResponse,
    intent_summary: &str,
) -> String {
    let hot_context = packet
        .hot_capsules
        .iter()
        .map(|capsule| format!("- [{}:{}] {}", capsule.role, capsule.band, capsule.content))
        .collect::<Vec<_>>()
        .join("\n");
    let relevant_context = packet
        .relevant_capsules
        .iter()
        .map(|capsule| format!("- score={:.2} {}", capsule.relevance_score, capsule.content))
        .collect::<Vec<_>>()
        .join("\n");
    let room_relevant_context = packet
        .room_relevant_capsules
        .iter()
        .map(|capsule| format!("- [{}] {}", capsule.room_id, capsule.content))
        .collect::<Vec<_>>()
        .join("\n");
    let global_relevant_context = packet
        .global_relevant_capsules
        .iter()
        .map(|capsule| {
            format!(
                "- [{}:{}] {}",
                capsule.room_id, capsule.session_id.0, capsule.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let world_state_context = packet
        .world_state_slice
        .workspace_entries
        .iter()
        .map(|entry| format!("- [{}:{}] {}", entry.kind, entry.size_bytes, entry.path))
        .collect::<Vec<_>>()
        .join("\n");
    let process_context = packet
        .world_state_slice
        .running_processes
        .iter()
        .map(|process| format!("- pid={} {}", process.pid, process.command))
        .collect::<Vec<_>>()
        .join("\n");
    let changed_files_context = packet
        .world_state_slice
        .changed_files
        .iter()
        .map(|path| format!("- {}", path))
        .collect::<Vec<_>>()
        .join("\n");
    let summaries = packet
        .summary_checkpoints
        .iter()
        .map(|summary| format!("- [{}] {}", summary.source_band, summary.semantic_digest))
        .collect::<Vec<_>>()
        .join("\n");
    let stable_rules = packet
        .stable_memory
        .as_ref()
        .map(|memory| {
            memory
                .learned_rules
                .iter()
                .take(6)
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .join(" | ");
    let blockers = packet
        .active_snapshot
        .as_ref()
        .map(|snapshot| snapshot.blockers.join(" | "))
        .unwrap_or_default();
    let next_actions = packet
        .active_snapshot
        .as_ref()
        .map(|snapshot| snapshot.next_actions.join(" | "))
        .unwrap_or_default();

    format!(
        concat!(
            "You are the live provider lane for JIMI SUPERSTAR.\n",
            "Respond briefly, concretely, and act as a sovereign house lane.\n\n",
            "Lane: {provider_lane_id}\n",
            "Session: {session_id}\n",
            "Active mandala: {active_mandala}\n",
            "Intent: {intent_summary}\n",
            "Query seed: {query_seed}\n",
            "Boot include: {boot_include}\n",
            "Lookup sources: {lookup_sources}\n",
            "Stable promotion: {stable_promotion}\n",
            "FieldVault sealing: {fieldvault_sealing}\n",
            "Promotion threshold: {promotion_threshold:.2}\n\n",
            "Goal: {goal}\n",
            "Blockers: {blockers}\n",
            "Next actions: {next_actions}\n",
            "Stable rules: {stable_rules}\n\n",
            "Hot capsules:\n{hot_context}\n\n",
            "Relevant memory:\n{relevant_context}\n\n",
            "Room memory:\n{room_relevant_context}\n\n",
            "House memory:\n{global_relevant_context}\n\n",
            "World state root: {workspace_root}\n",
            "Workspace health: {workspace_health}\n",
            "Git dirty: {git_dirty}\n",
            "World state entries:\n{world_state_context}\n\n",
            "Changed files:\n{changed_files_context}\n\n",
            "Running processes:\n{process_context}\n\n",
            "Summaries:\n{summaries}\n\n",
            "Return only the answer for this turn. Do not explain the packet."
        ),
        provider_lane_id = provider_lane_id,
        session_id = packet.session_id,
        active_mandala = packet
            .active_mandala_id
            .clone()
            .unwrap_or_else(|| "none".into()),
        intent_summary = intent_summary,
        query_seed = packet.query_seed,
        boot_include = packet.memory_policy.boot_include.join(", "),
        lookup_sources = packet.memory_policy.lookup_sources.join(", "),
        stable_promotion = if packet.memory_policy.promote_to_stable_memory {
            "on"
        } else {
            "off"
        },
        fieldvault_sealing = if packet.memory_policy.allow_fieldvault_sealing {
            "on"
        } else {
            "off"
        },
        promotion_threshold = packet.memory_policy.promotion_confidence_threshold,
        goal = packet
            .active_snapshot
            .as_ref()
            .map(|snapshot| snapshot.current_goal.clone())
            .unwrap_or_else(|| "none".into()),
        blockers = if blockers.is_empty() {
            "none".into()
        } else {
            blockers
        },
        next_actions = if next_actions.is_empty() {
            "none".into()
        } else {
            next_actions
        },
        stable_rules = if stable_rules.is_empty() {
            "none".into()
        } else {
            stable_rules
        },
        hot_context = if hot_context.is_empty() {
            "- none".into()
        } else {
            hot_context
        },
        relevant_context = if relevant_context.is_empty() {
            "- none".into()
        } else {
            relevant_context
        },
        room_relevant_context = if room_relevant_context.is_empty() {
            "- none".into()
        } else {
            room_relevant_context
        },
        global_relevant_context = if global_relevant_context.is_empty() {
            "- none".into()
        } else {
            global_relevant_context
        },
        workspace_root = packet.world_state_slice.workspace_root,
        workspace_health = packet.world_state_slice.workspace_health,
        git_dirty = if packet.world_state_slice.git_dirty {
            "yes"
        } else {
            "no"
        },
        world_state_context = if world_state_context.is_empty() {
            "- none".into()
        } else {
            world_state_context
        },
        changed_files_context = if changed_files_context.is_empty() {
            "- none".into()
        } else {
            changed_files_context
        },
        process_context = if process_context.is_empty() {
            "- none".into()
        } else {
            process_context
        },
        summaries = if summaries.is_empty() {
            "- none".into()
        } else {
            summaries
        },
    )
}

fn compact_provider_response(output_text: &str) -> CompactedProviderResponse {
    let normalized_lines = output_text
        .split('\n')
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let joined = normalized_lines.join(" ");
    let raw_length = joined.chars().count();

    let mut chunks = joined
        .split_terminator(['.', '!', '?'])
        .map(str::trim)
        .filter(|chunk| !chunk.is_empty())
        .take(2)
        .map(str::to_string)
        .collect::<Vec<_>>();

    if chunks.is_empty() && !joined.is_empty() {
        chunks.push(joined.clone());
    }

    let mut memory_text = chunks.join(". ");
    if !memory_text.is_empty() && !memory_text.ends_with('.') {
        memory_text.push('.');
    }

    let limit = 280;
    if memory_text.chars().count() > limit {
        memory_text = memory_text
            .chars()
            .take(limit.saturating_sub(1))
            .collect::<String>();
        if !memory_text.ends_with('…') {
            memory_text.push('…');
        }
    }

    let compacted_length = memory_text.chars().count();
    let confidence_level = if raw_length > 700 {
        0.84
    } else if raw_length > 280 {
        0.88
    } else {
        0.92
    };

    CompactedProviderResponse {
        memory_text,
        confidence_level,
        raw_length,
        compacted_length,
    }
}

fn refresh_world_state_cache(
    runtime: &mut HouseRuntime,
    house_root: &PathBuf,
    memory_policy: &MandalaMemoryPolicy,
) {
    if !memory_policy.allow_world_state {
        runtime.world_state_nodes.replace_scope("disabled", Vec::new());
        runtime
            .world_state_deltas
            .replace_scope("disabled", Vec::new());
        runtime
            .world_state_relations
            .replace_scope("disabled", Vec::new());
        return;
    }

    let scope = memory_policy.world_state_scope.trim().to_string();
    let include_workspace = matches!(
        scope.as_str(),
        "workspace" | "workspace+process" | "workspace+health"
    );
    let mut workspace_entries = std::fs::read_dir(house_root)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            let file_type = metadata.file_type();
            let kind = if file_type.is_dir() {
                "dir"
            } else if file_type.is_file() {
                "file"
            } else if file_type.is_symlink() {
                "link"
            } else {
                "other"
            };
            Some(WorldStateWorkspaceEntry {
                path: entry.path().display().to_string(),
                kind: kind.to_string(),
                size_bytes: metadata.len(),
            })
        })
        .collect::<Vec<_>>();
    workspace_entries.sort_by(|a, b| a.path.cmp(&b.path));
    let observed_at = chrono::Utc::now();
    let workspace_nodes = if include_workspace {
        workspace_entries
            .iter()
            .map(|entry| WorldStateNodeRecord {
                node_id: format!("wsnode::{scope}::{}", entry.path),
                scope: scope.clone(),
                path: entry.path.clone(),
                kind: entry.kind.clone(),
                size_bytes: entry.size_bytes,
                metadata_hash: format!("{}:{}:{}", entry.kind, entry.size_bytes, entry.path),
                status: "active".into(),
                observed_at,
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let changed_files = Command::new("git")
        .arg("-C")
        .arg(house_root)
        .args(["status", "--short"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|stdout| {
            stdout
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToOwned::to_owned)
                .take(memory_policy.world_state_entry_limit.max(1))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let changed_paths = changed_files
        .iter()
        .filter_map(|line| {
            let trimmed = line.trim();
            let (_, path) = trimmed
                .split_once(' ')
                .map(|(kind, rest)| (kind.trim().to_string(), rest.trim().to_string()))
                .unwrap_or_else(|| ("unknown".into(), trimmed.to_string()));
            if path.is_empty() {
                None
            } else {
                Some(path)
            }
        })
        .collect::<Vec<_>>();
    let recent_deltas = changed_files
        .iter()
        .map(|line| {
            let trimmed = line.trim();
            let (change_kind, path) = trimmed
                .split_once(' ')
                .map(|(kind, rest)| (kind.trim().to_string(), rest.trim().to_string()))
                .unwrap_or_else(|| ("unknown".into(), trimmed.to_string()));
            WorldStateDeltaRecord {
                delta_id: format!("wsdelta::{scope}::{trimmed}"),
                scope: scope.clone(),
                change_kind: change_kind.clone(),
                path: path.clone(),
                summary: format!("{change_kind} {path}"),
                observed_at,
            }
        })
        .collect::<Vec<_>>();
    let workspace_relations = if include_workspace {
        let mut relations = workspace_entries
            .iter()
            .filter_map(|entry| {
                let path = std::path::Path::new(&entry.path);
                let parent = path.parent()?.display().to_string();
                Some(WorldStateRelationRecord {
                    relation_id: format!("wsrel::{scope}::{parent}=>{}", entry.path),
                    scope: scope.clone(),
                    from_node_id: format!("wsnode::{scope}::{parent}"),
                    to_node_id: format!("wsnode::{scope}::{}", entry.path),
                    relation_kind: "contains".into(),
                    summary: format!("{parent} contains {}", entry.path),
                    observed_at,
                })
            })
            .collect::<Vec<_>>();

        let mut sibling_pairs = workspace_entries
            .windows(2)
            .filter_map(|window| {
                let left = &window[0];
                let right = &window[1];
                let left_parent = std::path::Path::new(&left.path).parent()?.display().to_string();
                let right_parent = std::path::Path::new(&right.path).parent()?.display().to_string();
                if left_parent != right_parent || left_parent.is_empty() {
                    return None;
                }
                Some(WorldStateRelationRecord {
                    relation_id: format!("wsrel::{scope}::sibling::{}<>{}", left.path, right.path),
                    scope: scope.clone(),
                    from_node_id: format!("wsnode::{scope}::{}", left.path),
                    to_node_id: format!("wsnode::{scope}::{}", right.path),
                    relation_kind: "sibling_in_scope".into(),
                    summary: format!("{} and {} share parent {}", left.path, right.path, left_parent),
                    observed_at,
                })
            })
            .take(memory_policy.world_state_entry_limit.max(1))
            .collect::<Vec<_>>();

        let mut changed_relations = changed_paths
            .iter()
            .map(|path| WorldStateRelationRecord {
                relation_id: format!("wsrel::{scope}::changed::{path}"),
                scope: scope.clone(),
                from_node_id: format!("wsnode::{scope}::{}", house_root.display()),
                to_node_id: format!("wsnode::{scope}::{path}"),
                relation_kind: "changed_in_workspace".into(),
                summary: format!("{path} changed in active workspace"),
                observed_at,
            })
            .collect::<Vec<_>>();

        relations.append(&mut sibling_pairs);
        relations.append(&mut changed_relations);
        relations
    } else {
        Vec::new()
    };

    runtime
        .world_state_nodes
        .replace_scope(&scope, workspace_nodes);
    runtime
        .world_state_deltas
        .replace_scope(&scope, recent_deltas);
    runtime
        .world_state_relations
        .replace_scope(&scope, workspace_relations);
}

fn build_world_state_slice_from_cache(
    runtime: &HouseRuntime,
    house_root: &PathBuf,
    memory_policy: &MandalaMemoryPolicy,
) -> WorldStateSlice {
    if !memory_policy.allow_world_state {
        return WorldStateSlice {
            scope: "disabled".into(),
            workspace_root: house_root.display().to_string(),
            workspace_health: "disabled".into(),
            git_dirty: false,
            cache_state: "disabled".into(),
            indexed_nodes: 0,
            relation_count: 0,
            workspace_entry_count: 0,
            workspace_entries: Vec::new(),
            changed_files: Vec::new(),
            recent_deltas: Vec::new(),
            recent_relations: Vec::new(),
            running_processes: Vec::new(),
            observed_at: chrono::Utc::now(),
        };
    }

    let scope = memory_policy.world_state_scope.trim().to_string();
    let include_processes = matches!(scope.as_str(), "process" | "workspace+process");
    let nodes = runtime.world_state_nodes.by_scope(&scope);
    let deltas = runtime.world_state_deltas.by_scope(&scope);
    let relations = runtime.world_state_relations.by_scope(&scope);
    let indexed_nodes = nodes.len();
    let relation_count = relations.len();
    let mut workspace_entries = nodes
        .into_iter()
        .map(|node| WorldStateWorkspaceEntry {
            path: node.path.clone(),
            kind: node.kind.clone(),
            size_bytes: node.size_bytes,
        })
        .collect::<Vec<_>>();
    workspace_entries.sort_by(|a, b| a.path.cmp(&b.path));
    let workspace_entry_count = workspace_entries.len();
    workspace_entries.truncate(memory_policy.world_state_entry_limit.max(1));

    let recent_deltas = deltas
        .into_iter()
        .rev()
        .take(memory_policy.world_state_entry_limit.max(1))
        .map(|delta| delta.summary.clone())
        .collect::<Vec<_>>();
    let recent_relations = relations
        .into_iter()
        .rev()
        .take(memory_policy.world_state_entry_limit.max(1))
        .map(|relation| format!("[{}] {}", relation.relation_kind, relation.summary))
        .collect::<Vec<_>>();
    let changed_files = recent_deltas.clone();
    let git_dirty = !recent_deltas.is_empty();
    let workspace_health = if !house_root.join(".git").exists() {
        "ungit".to_string()
    } else if git_dirty {
        "dirty".to_string()
    } else {
        "clean".to_string()
    };
    let cache_state = if indexed_nodes == 0 {
        "cold".to_string()
    } else if git_dirty {
        "fresh".to_string()
    } else {
        "warm".to_string()
    };
    let running_processes = if include_processes {
        Command::new("ps")
            .args(["-axo", "pid=,comm="])
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|stdout| {
                stdout
                    .lines()
                    .filter_map(|line| {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            return None;
                        }
                        let mut parts = trimmed.split_whitespace();
                        let pid = parts.next()?.parse::<u32>().ok()?;
                        let command = parts.collect::<Vec<_>>().join(" ");
                        if command.is_empty() {
                            return None;
                        }
                        Some(WorldStateProcessEntry { pid, command })
                    })
                    .take(memory_policy.world_state_process_limit)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    WorldStateSlice {
        scope,
        workspace_root: house_root.display().to_string(),
        workspace_health,
        git_dirty,
        cache_state,
        indexed_nodes,
        relation_count,
        workspace_entry_count,
        workspace_entries,
        changed_files,
        recent_deltas,
        recent_relations,
        running_processes,
        observed_at: chrono::Utc::now(),
    }
}

async fn query_memory(
    axum::extract::Path(session_id): axum::extract::Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    State(state): State<AppState>,
) -> Result<Json<Vec<MemoryCapsuleRecord>>, (StatusCode, String)> {
    let runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let session_id = jimi_kernel::SessionId(session_id);
    let query = params.get("q").cloned().unwrap_or_default();
    Ok(Json(runtime.query_memory(&session_id, &query, 8)))
}

async fn search_memory(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    State(state): State<AppState>,
) -> Result<Json<MemorySearchResponse>, (StatusCode, String)> {
    let scope = params
        .get("scope")
        .cloned()
        .unwrap_or_else(|| "house".into());
    let query = params.get("q").cloned().unwrap_or_default();
    let session_id = params.get("session_id").cloned();
    let explicit_room_id = params.get("room_id").cloned();
    let limit = params
        .get("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(8);

    let derived_room_id = if explicit_room_id.is_none() {
        if let Some(session_id) = session_id.as_ref() {
            let runtime = state.runtime.lock().map_err(internal_lock_error)?;
            runtime
                .sessions
                .session(&jimi_kernel::SessionId(session_id.clone()))
                .ok()
                .map(|session| session.room_id.clone())
        } else {
            None
        }
    } else {
        explicit_room_id.clone()
    };

    let store = state.store.lock().map_err(internal_lock_error)?;
    let results = store
        .search_memory_capsules(
            &scope,
            &query,
            session_id.as_deref(),
            derived_room_id.as_deref(),
            limit,
        )
        .map_err(internal_store_error)?;

    Ok(Json(MemorySearchResponse {
        scope,
        query,
        session_id,
        room_id: derived_room_id,
        results,
    }))
}

async fn execute_latest_dispatch(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<DispatchExecutionResponse>), (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;

    let latest_dispatch = runtime
        .dispatches
        .all()
        .into_iter()
        .rev()
        .find(|dispatch| dispatch.status == "queued")
        .cloned()
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "no queued dispatches available".to_string(),
            )
        })?;

    let running_dispatch = runtime
        .dispatches
        .update_status(&latest_dispatch.dispatch_id, "running")
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    let _running_turn = runtime
        .sessions
        .update_turn_state(&latest_dispatch.turn_id, jimi_kernel::TurnState::Executing)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;

    runtime.events.append(
        ActorRef {
            actor_type: "provider".into(),
            actor_id: running_dispatch.provider_lane_id.clone(),
        },
        SubjectRef {
            subject_type: "dispatch".into(),
            subject_id: running_dispatch.dispatch_id.clone(),
        },
        EventType::ToolStarted,
        Some(&running_dispatch.session_id),
        Some(&running_dispatch.lane_id),
        Some(&running_dispatch.turn_id),
        serde_json::json!({
            "dispatch_id": running_dispatch.dispatch_id.clone(),
            "provider_lane_id": running_dispatch.provider_lane_id.clone(),
            "status": "running",
        }),
    );

    let output_text = format!(
        "JIMI routed '{}' through {} and completed the first simulated execution loop.",
        running_dispatch.intent_summary, running_dispatch.provider_lane_id
    );
    let compacted_response = compact_provider_response(&output_text);

    let completed_dispatch = runtime
        .dispatches
        .update_status(&running_dispatch.dispatch_id, "completed")
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    let completed_turn = runtime
        .sessions
        .update_turn_state(&running_dispatch.turn_id, jimi_kernel::TurnState::Completed)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    let completed_room_id = runtime
        .sessions
        .session(&completed_turn.session_id)
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?
        .room_id
        .clone();

    runtime.memory_capsules.append(
        completed_turn.session_id.clone(),
        completed_room_id,
        completed_turn.lane_id.clone(),
        Some(completed_turn.turn_id.clone()),
        "assistant",
        compacted_response.memory_text.clone(),
        Some(running_dispatch.intent_summary.clone()),
        0.91,
        compacted_response.confidence_level,
        "session_open",
        "hot",
    );
    runtime.refresh_memory_for_session(&completed_turn.session_id);

    runtime.events.append(
        ActorRef {
            actor_type: "provider".into(),
            actor_id: completed_dispatch.provider_lane_id.clone(),
        },
        SubjectRef {
            subject_type: "turn".into(),
            subject_id: completed_turn.turn_id.0.clone(),
        },
        EventType::MessageCompleted,
        Some(&completed_turn.session_id),
        Some(&completed_turn.lane_id),
        Some(&completed_turn.turn_id),
        serde_json::json!({
            "dispatch_id": completed_dispatch.dispatch_id.clone(),
            "provider_lane_id": completed_dispatch.provider_lane_id.clone(),
            "output_text": output_text.clone(),
            "memory_text": compacted_response.memory_text.clone(),
            "raw_output_length": compacted_response.raw_length,
            "compacted_output_length": compacted_response.compacted_length,
            "status": "completed",
        }),
    );

    let new_events: Vec<EventEnvelope> =
        runtime.events.all().iter().rev().take(2).cloned().collect();
    persist_runtime(&state, &runtime)?;
    drop(runtime);

    for event in new_events.into_iter().rev() {
        let _ = state.events_tx.send(event);
    }

    Ok((
        StatusCode::CREATED,
        Json(DispatchExecutionResponse {
            dispatch: completed_dispatch,
            turn: completed_turn,
            output_text,
        }),
    ))
}

async fn execute_latest_dispatch_live(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<DispatchExecutionResponse>), (StatusCode, String)> {
    let result = run_latest_dispatch_live_once(&state).await?;
    Ok((StatusCode::CREATED, Json(result)))
}

fn approval_reason_for_dispatch(
    provider_lane_id: &str,
    provider: &str,
    packet: &ContextPacketResponse,
) -> Option<String> {
    if provider != "codex" {
        return Some(format!(
            "provider lane {provider_lane_id} uses non-default engine {provider}"
        ));
    }

    if packet.world_state_slice.git_dirty {
        return Some(format!(
            "workspace is dirty with {} changed files",
            packet.world_state_slice.changed_files.len()
        ));
    }

    None
}

fn mission_alignment_score(intent_summary: &str, mission_terms: &[String]) -> f32 {
    let lowered_intent = intent_summary.to_lowercase();
    mission_terms.iter().fold(0.0, |acc, term| {
        if term.is_empty() {
            acc
        } else if lowered_intent.contains(term) {
            acc + 0.45
        } else {
            let overlap = term
                .split_whitespace()
                .filter(|piece| piece.len() > 3 && lowered_intent.contains(piece))
                .count();
            acc + (overlap as f32 * 0.08)
        }
    })
}

fn select_next_live_dispatch(
    runtime: &HouseRuntime,
) -> Option<(TurnDispatchRecord, f32, String)> {
    let active_mandala = runtime
        .slots
        .all()
        .into_iter()
        .find_map(|slot| slot.active_mandala_id.as_ref())
        .and_then(|mandala_id| runtime.mandalas.get(mandala_id).ok());

    let mut mission_terms = Vec::new();
    if let Some(mandala) = active_mandala {
        mission_terms.push(mandala.active_snapshot.current_goal.to_lowercase());
        mission_terms.extend(
            mandala
                .active_snapshot
                .next_actions
                .iter()
                .map(|value| value.to_lowercase()),
        );
    }

    runtime
        .dispatches
        .all()
        .into_iter()
        .filter(|dispatch| dispatch.status == "queued")
        .map(|dispatch| {
            let provider_bonus = runtime
                .providers
                .get(&dispatch.provider_lane_id)
                .ok()
                .map(|provider| if provider.routing_mode == "primary" { 0.2 } else { 0.0 })
                .unwrap_or(0.0);
            let mission_bonus = mission_alignment_score(&dispatch.intent_summary, &mission_terms);
            let score = 1.0 + provider_bonus + mission_bonus;
            let reason = if mission_bonus > 0.0 {
                "mission_alignment"
            } else if provider_bonus > 0.0 {
                "primary_lane_priority"
            } else {
                "queued_fallback"
            };
            (dispatch.clone(), score, reason.to_string())
        })
        .max_by(|left, right| {
            left.1
                .partial_cmp(&right.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.0.dispatched_at.cmp(&right.0.dispatched_at))
        })
}

async fn run_latest_dispatch_live_once(
    state: &AppState,
) -> Result<DispatchExecutionResponse, (StatusCode, String)> {
    let (dispatch, provider_lane, provider_prompt, house_root, selection_score, selection_reason) = {
        let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;

        let (latest_dispatch, selection_score, selection_reason) =
            select_next_live_dispatch(&runtime).ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    "no queued dispatches available".to_string(),
                )
            })?;

        let running_dispatch = runtime
            .dispatches
            .update_status(&latest_dispatch.dispatch_id, "running")
            .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
        let _ = runtime
            .sessions
            .update_turn_state(&latest_dispatch.turn_id, jimi_kernel::TurnState::Executing)
            .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;

        let provider_lane = runtime
            .providers
            .all()
            .into_iter()
            .find(|provider| provider.provider_lane_id == running_dispatch.provider_lane_id)
            .cloned()
            .ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    "provider lane metadata missing for dispatch".to_string(),
                )
            })?;
        let adapter = resolve_provider_adapter(&provider_lane);

        runtime.events.append(
            ActorRef {
                actor_type: "provider".into(),
                actor_id: running_dispatch.provider_lane_id.clone(),
            },
            SubjectRef {
                subject_type: "dispatch".into(),
                subject_id: running_dispatch.dispatch_id.clone(),
            },
            EventType::ToolStarted,
            Some(&running_dispatch.session_id),
            Some(&running_dispatch.lane_id),
            Some(&running_dispatch.turn_id),
            serde_json::json!({
                "dispatch_id": running_dispatch.dispatch_id.clone(),
                "provider_lane_id": running_dispatch.provider_lane_id.clone(),
                "status": "running",
                "mode": provider_adapter_label(&adapter),
                "context_packet_ready": true,
                "provider": provider_lane.provider.clone(),
                "model": provider_lane.model.clone(),
                "selection_score": selection_score,
                "selection_reason": selection_reason,
            }),
        );

        let world_state_policy = runtime.active_memory_policy().unwrap_or_default();
        refresh_world_state_cache(&mut runtime, &state.house_root, &world_state_policy);
        let packet = assemble_context_packet(&runtime, &running_dispatch.session_id, &state.house_root);
        let provider_prompt = build_provider_prompt(
            &running_dispatch.provider_lane_id,
            &packet,
            &running_dispatch.intent_summary,
        );

        if let Ok(mut autonomy_status) = state.autonomy_status.lock() {
            autonomy_status.last_dispatch_id = Some(running_dispatch.dispatch_id.clone());
            autonomy_status.last_intent_summary = Some(running_dispatch.intent_summary.clone());
            autonomy_status.last_selection_reason = Some(selection_reason.clone());
            mark_autonomy_transition(
                &mut autonomy_status,
                format!("selected:{}:{}", running_dispatch.dispatch_id, selection_reason),
            );
        }

        if let Some(reason) = approval_reason_for_dispatch(
            &running_dispatch.provider_lane_id,
            &provider_lane.provider,
            &packet,
        ) {
            if runtime
                .approvals
                .find_pending_by_dispatch(&running_dispatch.dispatch_id)
                .is_none()
                && !runtime
                    .approvals
                    .has_granted_for_dispatch(&running_dispatch.dispatch_id)
            {
                let approval = runtime.approvals.request(
                    running_dispatch.session_id.clone(),
                    running_dispatch.dispatch_id.clone(),
                    running_dispatch.provider_lane_id.clone(),
                    "execute_dispatch_live",
                    reason.clone(),
                );

                runtime
                    .dispatches
                    .update_status(&running_dispatch.dispatch_id, "awaiting_approval")
                    .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
                let _ = runtime.sessions.update_turn_state(
                    &running_dispatch.turn_id,
                    jimi_kernel::TurnState::AwaitingApproval,
                );

                runtime.events.append(
                    ActorRef {
                        actor_type: "policy".into(),
                        actor_id: "house.approvals".into(),
                    },
                    SubjectRef {
                        subject_type: "approval".into(),
                        subject_id: approval.approval_request_id.clone(),
                    },
                    EventType::ApprovalRequired,
                    Some(&running_dispatch.session_id),
                    Some(&running_dispatch.lane_id),
                    Some(&running_dispatch.turn_id),
                    serde_json::json!({
                        "approval_request_id": approval.approval_request_id,
                        "dispatch_id": running_dispatch.dispatch_id,
                        "provider_lane_id": running_dispatch.provider_lane_id,
                        "action": "execute_dispatch_live",
                        "reason": reason,
                        "status": "pending",
                    }),
                );

                let new_event = runtime.events.all().last().cloned();
                persist_runtime(&state, &runtime)?;
                drop(runtime);

                if let Some(event) = new_event {
                    let _ = state.events_tx.send(event);
                }

                if let Ok(mut autonomy_status) = state.autonomy_status.lock() {
                    mark_autonomy_transition(
                        &mut autonomy_status,
                        format!("awaiting_approval:{}", running_dispatch.dispatch_id),
                    );
                }

                return Err((
                    StatusCode::CONFLICT,
                    "approval required before live execution".to_string(),
                ));
            }
        }

        let new_event = runtime.events.all().last().cloned();
        persist_runtime(&state, &runtime)?;
        drop(runtime);

        if let Some(event) = new_event {
            let _ = state.events_tx.send(event);
        }

        (
            running_dispatch.clone(),
            provider_lane,
            provider_prompt,
            state.house_root.clone(),
            selection_score,
            selection_reason,
        )
    };

    let ready_providers = ready_provider_names();
    let all_provider_lanes = {
        let runtime = state.runtime.lock().map_err(internal_lock_error)?;
        runtime
            .providers
            .all()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>()
    };
    let mut provider_attempts = vec![provider_lane.clone()];
    provider_attempts.extend(fallback_candidates(
        &provider_lane,
        &all_provider_lanes,
        &ready_providers,
    ));

    let mut executing_lane = provider_lane.clone();
    let mut output_text = None;
    let mut last_error: Option<ProviderExecutionError> = None;
    let mut fallback_from: Option<String> = None;

    for (attempt_index, candidate_lane) in provider_attempts.into_iter().enumerate() {
        let adapter = resolve_provider_adapter(&candidate_lane);
        let adapter_label = provider_adapter_label(&adapter).to_string();
        let provider_lane_for_execution = candidate_lane.clone();
        let adapter_for_execution = adapter.clone();
        let prompt_for_execution = provider_prompt.clone();
        let root_for_execution = house_root.clone();
        let output_result = tokio::task::spawn_blocking(move || {
            run_provider_adapter(
                &provider_lane_for_execution,
                &adapter_for_execution,
                &root_for_execution,
                &prompt_for_execution,
            )
        })
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

        match output_result {
            Ok(output) => {
                if candidate_lane.provider_lane_id != dispatch.provider_lane_id {
                    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
                    let _ = runtime.dispatches.update_provider_lane(
                        &dispatch.dispatch_id,
                        candidate_lane.provider_lane_id.clone(),
                    );
                    runtime.events.append(
                        ActorRef {
                            actor_type: "provider".into(),
                            actor_id: candidate_lane.provider_lane_id.clone(),
                        },
                        SubjectRef {
                            subject_type: "dispatch".into(),
                            subject_id: dispatch.dispatch_id.clone(),
                        },
                        EventType::EngineSelected,
                        Some(&dispatch.session_id),
                        Some(&dispatch.lane_id),
                        Some(&dispatch.turn_id),
                        serde_json::json!({
                            "dispatch_id": dispatch.dispatch_id,
                            "attempt_index": attempt_index,
                            "fallback_from": fallback_from,
                            "fallback_to": candidate_lane.provider_lane_id,
                            "provider": candidate_lane.provider,
                            "model": candidate_lane.model,
                            "adapter": adapter_label,
                            "selection_score": selection_score,
                            "selection_reason": selection_reason,
                        }),
                    );
                    let new_event = runtime.events.all().last().cloned();
                    persist_runtime(&state, &runtime)?;
                    drop(runtime);
                    if let Some(event) = new_event {
                        let _ = state.events_tx.send(event);
                    }
                    if let Ok(mut autonomy_status) = state.autonomy_status.lock() {
                        mark_autonomy_transition(
                            &mut autonomy_status,
                            format!(
                                "fallback:{}->{}",
                                fallback_from.clone().unwrap_or_else(|| dispatch.provider_lane_id.clone()),
                                candidate_lane.provider_lane_id
                            ),
                        );
                    }
                }
                executing_lane = candidate_lane;
                output_text = Some(output);
                break;
            }
            Err(error) => {
                let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
                runtime.events.append(
                    ActorRef {
                        actor_type: "provider".into(),
                        actor_id: candidate_lane.provider_lane_id.clone(),
                    },
                    SubjectRef {
                        subject_type: "dispatch".into(),
                        subject_id: dispatch.dispatch_id.clone(),
                    },
                    EventType::EngineDegraded,
                    Some(&dispatch.session_id),
                    Some(&dispatch.lane_id),
                    Some(&dispatch.turn_id),
                    serde_json::json!({
                        "dispatch_id": dispatch.dispatch_id,
                        "provider_lane_id": candidate_lane.provider_lane_id,
                        "provider": candidate_lane.provider,
                        "model": candidate_lane.model,
                        "adapter": adapter_label,
                        "failure_class": error.class(),
                        "attempt_index": attempt_index,
                        "fallback_from": fallback_from,
                        "fallback_to": if error.should_fallback() {
                            Some("next_ready_lane".to_string())
                        } else {
                            None::<String>
                        },
                        "fallback_exhausted": !error.should_fallback(),
                        "selection_score": selection_score,
                        "selection_reason": selection_reason,
                        "reason": error.message(),
                    }),
                );
                let new_event = runtime.events.all().last().cloned();
                persist_runtime(&state, &runtime)?;
                drop(runtime);
                if let Some(event) = new_event {
                    let _ = state.events_tx.send(event);
                }

                fallback_from = Some(candidate_lane.provider_lane_id.clone());
                last_error = Some(error.clone());
                if let Ok(mut autonomy_status) = state.autonomy_status.lock() {
                    mark_autonomy_transition(
                        &mut autonomy_status,
                        format!(
                            "degraded:{}:{}",
                            candidate_lane.provider_lane_id,
                            error.class()
                        ),
                    );
                }
                if !error.should_fallback() {
                    break;
                }
            }
        }
    }
    let output_text = match output_text {
        Some(output) => output,
        None => {
            let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
            let _failed_dispatch = runtime
                .dispatches
                .update_status(&dispatch.dispatch_id, "failed")
                .map_err(|update_error| (StatusCode::BAD_REQUEST, update_error.to_string()))?;
            let _failed_turn = runtime
                .sessions
                .update_turn_state(&dispatch.turn_id, jimi_kernel::TurnState::Failed)
                .map_err(|update_error| (StatusCode::BAD_REQUEST, update_error.to_string()))?;
            persist_runtime(&state, &runtime)?;
            drop(runtime);
            if let Ok(mut autonomy_status) = state.autonomy_status.lock() {
                mark_autonomy_transition(
                    &mut autonomy_status,
                    format!("failed:{}", dispatch.dispatch_id),
                );
            }
            return Err((
                StatusCode::BAD_GATEWAY,
                last_error
                    .map(|error| error.message().to_string())
                    .unwrap_or_else(|| "all provider attempts failed".to_string()),
            ));
        }
    };
    let compacted_response = compact_provider_response(&output_text);

    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let completed_dispatch = runtime
        .dispatches
        .update_status(&dispatch.dispatch_id, "completed")
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    let completed_turn = runtime
        .sessions
        .update_turn_state(&dispatch.turn_id, jimi_kernel::TurnState::Completed)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    let completed_room_id = runtime
        .sessions
        .session(&completed_turn.session_id)
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?
        .room_id
        .clone();

    runtime.memory_capsules.append(
        completed_turn.session_id.clone(),
        completed_room_id,
        completed_turn.lane_id.clone(),
        Some(completed_turn.turn_id.clone()),
        "assistant",
        compacted_response.memory_text.clone(),
        Some(dispatch.intent_summary.clone()),
        0.94,
        compacted_response.confidence_level,
        "session_open",
        "hot",
    );
    runtime.refresh_memory_for_session(&completed_turn.session_id);

    runtime.events.append(
        ActorRef {
            actor_type: "provider".into(),
            actor_id: executing_lane.provider_lane_id.clone(),
        },
        SubjectRef {
            subject_type: "turn".into(),
            subject_id: completed_turn.turn_id.0.clone(),
        },
        EventType::MessageCompleted,
        Some(&completed_turn.session_id),
        Some(&completed_turn.lane_id),
        Some(&completed_turn.turn_id),
        serde_json::json!({
            "dispatch_id": completed_dispatch.dispatch_id.clone(),
            "provider_lane_id": completed_dispatch.provider_lane_id.clone(),
            "provider": executing_lane.provider.clone(),
            "model": executing_lane.model.clone(),
            "output_text": output_text.clone(),
            "memory_text": compacted_response.memory_text.clone(),
            "raw_output_length": compacted_response.raw_length,
            "compacted_output_length": compacted_response.compacted_length,
            "status": "completed",
            "mode": provider_adapter_label(&resolve_provider_adapter(&executing_lane)),
        }),
    );

    let queued_dispatches_after_completion = runtime
        .dispatches
        .all()
        .into_iter()
        .filter(|candidate| {
            candidate.session_id == completed_turn.session_id && candidate.status == "queued"
        })
        .count();
    let active_next_action = runtime
        .slots
        .all()
        .into_iter()
        .find_map(|slot| slot.active_mandala_id.as_ref())
        .and_then(|mandala_id| runtime.mandalas.get(mandala_id).ok())
        .and_then(|mandala| mandala.active_snapshot.next_actions.first().cloned());

    if queued_dispatches_after_completion == 0 {
        if let Some(next_action) = active_next_action {
            if !next_action.trim().is_empty()
                && next_action.trim() != dispatch.intent_summary.trim()
            {
                if let Ok(session) = runtime.sessions.session(&completed_turn.session_id).cloned() {
                    let _ = queue_turn_for_lane(
                        &mut runtime,
                        &session,
                        &executing_lane,
                        "autonomy_followup".into(),
                        next_action.clone(),
                        "house",
                        "autonomy.followup",
                    );
                    if let Ok(mut autonomy_status) = state.autonomy_status.lock() {
                        mark_autonomy_transition(
                            &mut autonomy_status,
                            format!("next_action_queued:{}", next_action),
                        );
                    }
                }
            }
        }
    }

    let new_event = runtime.events.all().last().cloned();
    persist_runtime(&state, &runtime)?;
    drop(runtime);

    if let Some(event) = new_event {
        let _ = state.events_tx.send(event);
    }
    if let Ok(mut autonomy_status) = state.autonomy_status.lock() {
        mark_autonomy_transition(
            &mut autonomy_status,
            format!("completed:{}", completed_dispatch.dispatch_id),
        );
    }
    enqueue_transcript_distillation(&state, &completed_turn.session_id);

    Ok(DispatchExecutionResponse {
        dispatch: completed_dispatch,
        turn: completed_turn,
        output_text,
    })
}

async fn list_mandalas(State(state): State<AppState>) -> Json<Vec<MandalaManifest>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(runtime.mandalas.all().into_iter().cloned().collect())
}

async fn update_mandala_memory_policy(
    axum::extract::Path(mandala_id): axum::extract::Path<String>,
    State(state): State<AppState>,
    Json(request): Json<UpdateMemoryPolicyRequest>,
) -> Result<(StatusCode, Json<MemoryPolicyUpdateResponse>), (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let session_id = runtime
        .sessions
        .sessions()
        .last()
        .map(|session| session.session_id.clone());

    let updated_policy = {
        let mandala = runtime
            .mandalas
            .get_mut(&mandala_id)
            .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?;
        mandala.memory_policy.hot_context_limit = request.hot_context_limit.max(1);
        mandala.memory_policy.relevant_context_limit = request.relevant_context_limit.max(1);
        mandala.memory_policy.promotion_confidence_threshold =
            request.promotion_confidence_threshold.clamp(0.0, 1.0);
        mandala.memory_policy.promote_to_stable_memory = request.promote_to_stable_memory;
        mandala.memory_policy.allow_fieldvault_sealing = request.allow_fieldvault_sealing;
        mandala.memory_policy.world_state_scope = request.world_state_scope.trim().to_string();
        mandala.memory_policy.world_state_entry_limit = request.world_state_entry_limit.max(1);
        mandala.memory_policy.world_state_process_limit = request.world_state_process_limit.max(0);
        mandala.memory_policy.allow_world_state = request.allow_world_state;
        mandala.memory_policy.clone()
    };

    runtime.events.append(
        ActorRef {
            actor_type: "operator".into(),
            actor_id: "cockpit".into(),
        },
        SubjectRef {
            subject_type: "mandala".into(),
            subject_id: mandala_id.clone(),
        },
        EventType::MandalaPolicyUpdated,
        session_id.as_ref(),
        None,
        None,
        serde_json::json!({
            "mandala_id": mandala_id.clone(),
            "hot_context_limit": updated_policy.hot_context_limit,
            "relevant_context_limit": updated_policy.relevant_context_limit,
            "promotion_confidence_threshold": updated_policy.promotion_confidence_threshold,
            "promote_to_stable_memory": updated_policy.promote_to_stable_memory,
            "allow_fieldvault_sealing": updated_policy.allow_fieldvault_sealing,
            "world_state_scope": updated_policy.world_state_scope,
            "world_state_entry_limit": updated_policy.world_state_entry_limit,
            "world_state_process_limit": updated_policy.world_state_process_limit,
            "allow_world_state": updated_policy.allow_world_state,
        }),
    );

    let new_event = runtime.events.all().last().cloned();
    persist_runtime(&state, &runtime)?;
    drop(runtime);

    if let Some(event) = new_event {
        let _ = state.events_tx.send(event);
    }

    Ok((
        StatusCode::CREATED,
        Json(MemoryPolicyUpdateResponse {
            mandala_id,
            memory_policy: updated_policy,
        }),
    ))
}

async fn list_capsules(State(state): State<AppState>) -> Json<Vec<jimi_kernel::CapsuleRecord>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(runtime.capsules.all().into_iter().cloned().collect())
}

async fn list_capsule_packages(
    State(state): State<AppState>,
) -> Json<Vec<CapsulePackageRecord>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(runtime.capsule_packages.all().into_iter().cloned().collect())
}

async fn list_capsule_imports(State(state): State<AppState>) -> Json<CapsuleImportsResponse> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(CapsuleImportsResponse {
        imports: runtime
            .capsule_imports
            .all()
            .into_iter()
            .cloned()
            .collect(),
    })
}

async fn list_capsule_exports(State(state): State<AppState>) -> Json<CapsuleExportsResponse> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(CapsuleExportsResponse {
        exports: runtime
            .capsule_exports
            .all()
            .into_iter()
            .cloned()
            .collect(),
    })
}

async fn capsule_package_preview(
    Path(package_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<CapsuleInstallPreviewResponse>, (StatusCode, String)> {
    let runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let package = runtime
        .capsule_packages
        .get(&package_id)
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?
        .clone();
    let mandala = runtime
        .mandalas
        .get(&package.mandala_id)
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?;
    let slot_recommendation = runtime
        .slots
        .all()
        .into_iter()
        .find(|slot| slot.active_mandala_id.is_none())
        .map(|slot| slot.slot_id.clone())
        .unwrap_or_else(|| "slot.jimi.superstar.primary".to_string());
    let required_capabilities = mandala.capability_policy.required.clone();
    let optional_capabilities = mandala.capability_policy.optional.clone();
    let fieldvault_requirements = mandala.sacred_shards.len();
    let install_preview_summary = format!(
        "{} from {} requests {} required capabilities and {} sealed shard bindings",
        package.display_name,
        package.source_origin,
        required_capabilities.len(),
        fieldvault_requirements
    );
    Ok(Json(CapsuleInstallPreviewResponse {
        package,
        slot_recommendation,
        required_capabilities,
        optional_capabilities,
        fieldvault_requirements,
        install_preview_summary,
    }))
}

async fn update_capsule_trust(
    Path(package_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<UpdateCapsuleTrustRequest>,
) -> Result<Json<CapsulePackageRecord>, (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let updated = runtime
        .capsule_packages
        .classify_trust(&package_id, request.trust_level.trim())
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?;
    runtime.events.append(
        ActorRef {
            actor_type: "operator".into(),
            actor_id: "cockpit.capsules".into(),
        },
        SubjectRef {
            subject_type: "capsule_package".into(),
            subject_id: updated.package_id.clone(),
        },
        EventType::CapsuleInstalled,
        None,
        None,
        None,
        serde_json::json!({
            "package_id": updated.package_id,
            "capsule_id": updated.capsule_id,
            "trust_level": updated.trust_level,
            "change_kind": "trust_classified",
        }),
    );
    let new_event = runtime.events.all().last().cloned();
    persist_runtime(&state, &runtime)?;
    drop(runtime);
    if let Some(event) = new_event {
        let _ = state.events_tx.send(event);
    }
    Ok(Json(updated))
}

async fn request_capsule_install(
    Path(package_id): Path<String>,
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<CapsuleInstallRequestResponse>), (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let package = runtime
        .capsule_packages
        .get(&package_id)
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?
        .clone();

    if package.install_status == "installed" {
        return Err((
            StatusCode::CONFLICT,
            "package is already installed in the house".to_string(),
        ));
    }

    let synthetic_session_id = SessionId(format!("session.package.{}", package.package_id));
    let synthetic_dispatch_id = format!("capsule_install::{}", package.package_id);
    let synthetic_lane_id = "provider.none.marketplace".to_string();

    if let Some(existing) = runtime
        .approvals
        .find_pending_by_dispatch(&synthetic_dispatch_id)
        .cloned()
    {
        return Ok((
            StatusCode::OK,
            Json(CapsuleInstallRequestResponse {
                package,
                approval: existing,
            }),
        ));
    }

    let status = if package.trust_level.eq_ignore_ascii_case("internal") {
        "install_ready"
    } else {
        "awaiting_approval"
    };
    let updated_package = runtime
        .capsule_packages
        .update_install_status(&package_id, status)
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?;

    let approval = runtime.approvals.request(
        synthetic_session_id.clone(),
        synthetic_dispatch_id.clone(),
        synthetic_lane_id,
        format!("install_capsule_package:{}", package.package_id),
        format!(
            "install {} from {} with trust {}",
            package.display_name, package.source_origin, package.trust_level
        ),
    );

    runtime.events.append(
        ActorRef {
            actor_type: "operator".into(),
            actor_id: "cockpit.capsules".into(),
        },
        SubjectRef {
            subject_type: "capsule_package".into(),
            subject_id: package.package_id.clone(),
        },
        EventType::ApprovalRequired,
        Some(&synthetic_session_id),
        None,
        None,
        serde_json::json!({
            "approval_request_id": approval.approval_request_id,
            "package_id": package.package_id,
            "capsule_id": package.capsule_id,
            "trust_level": package.trust_level,
            "change_kind": "install_requested",
            "install_status": updated_package.install_status,
        }),
    );

    let new_event = runtime.events.all().last().cloned();
    persist_runtime(&state, &runtime)?;
    drop(runtime);
    if let Some(event) = new_event {
        let _ = state.events_tx.send(event);
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(CapsuleInstallRequestResponse {
            package: updated_package,
            approval,
        }),
    ))
}

async fn export_capsule_package(
    Path(package_id): Path<String>,
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<CapsuleExportResponse>), (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let package = runtime
        .capsule_packages
        .get(&package_id)
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?
        .clone();

    let export_dir = state.house_root.join("exports");
    fs::create_dir_all(&export_dir).map_err(internal_fs_error)?;
    let export_path = export_dir.join(format!("{}.capsule.json", package.package_id));
    let export_payload = serde_json::json!({
        "package_id": package.package_id,
        "capsule_id": package.capsule_id,
        "mandala_id": package.mandala_id,
        "display_name": package.display_name,
        "creator": package.creator,
        "source_origin": package.source_origin,
        "package_digest": package.package_digest,
        "trust_level": package.trust_level,
        "install_status": package.install_status,
    });
    fs::write(
        &export_path,
        serde_json::to_string_pretty(&export_payload).map_err(internal_serde_error)?,
    )
    .map_err(internal_fs_error)?;

    let export_record = runtime.capsule_exports.record(
        package.package_id.clone(),
        package.capsule_id.clone(),
        export_path.display().to_string(),
        "completed",
        package.package_digest.clone(),
    );

    runtime.events.append(
        ActorRef {
            actor_type: "operator".into(),
            actor_id: "cockpit.marketplace".into(),
        },
        SubjectRef {
            subject_type: "capsule_package".into(),
            subject_id: package.package_id.clone(),
        },
        EventType::CapsuleExportCompleted,
        None,
        None,
        None,
        serde_json::json!({
            "package_id": package.package_id,
            "capsule_id": package.capsule_id,
            "export_id": export_record.export_id,
            "target_path": export_record.target_path,
            "export_status": export_record.export_status,
        }),
    );

    let new_event = runtime.events.all().last().cloned();
    persist_runtime(&state, &runtime)?;
    drop(runtime);
    if let Some(event) = new_event {
        let _ = state.events_tx.send(event);
    }
    Ok((
        StatusCode::OK,
        Json(CapsuleExportResponse {
            package,
            export_record,
        }),
    ))
}

async fn activate_capsule_package(
    Path(package_id): Path<String>,
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<CapsuleActivationResponse>), (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let package = runtime
        .capsule_packages
        .get(&package_id)
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?
        .clone();

    let allowed = matches!(
        package.install_status.as_str(),
        "approved_for_install" | "install_ready" | "installed"
    );
    if !allowed {
        return Err((
            StatusCode::CONFLICT,
            format!(
                "package {} is not ready for activation (status: {})",
                package.package_id, package.install_status
            ),
        ));
    }

    let slot_id = runtime
        .slots
        .all()
        .into_iter()
        .find(|slot| slot.active_mandala_id.is_none())
        .map(|slot| slot.slot_id.clone())
        .unwrap_or_else(|| "slot.jimi.superstar.primary".to_string());

    if runtime.slots.get(&slot_id).is_err() {
        runtime
            .slots
            .define_slot(slot_id.clone(), "Primary Personality Slot");
    }

    runtime
        .slots
        .bind_capsule(&slot_id, package.capsule_id.clone(), package.mandala_id.clone(), false)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    runtime
        .slots
        .activate(&slot_id)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    let updated_package = runtime
        .capsule_packages
        .update_install_status(&package_id, "installed")
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?;
    let slot = runtime
        .slots
        .get(&slot_id)
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?
        .clone();

    runtime.events.append(
        ActorRef {
            actor_type: "operator".into(),
            actor_id: "cockpit.marketplace".into(),
        },
        SubjectRef {
            subject_type: "slot".into(),
            subject_id: slot_id.clone(),
        },
        EventType::SlotActivated,
        None,
        None,
        None,
        serde_json::json!({
            "package_id": updated_package.package_id,
            "capsule_id": updated_package.capsule_id,
            "mandala_id": updated_package.mandala_id,
            "slot_id": slot_id,
            "slot_state": slot.state,
        }),
    );

    let new_event = runtime.events.all().last().cloned();
    persist_runtime(&state, &runtime)?;
    drop(runtime);
    if let Some(event) = new_event {
        let _ = state.events_tx.send(event);
    }

    Ok((
        StatusCode::OK,
        Json(CapsuleActivationResponse {
            package_id: updated_package.package_id,
            capsule_id: updated_package.capsule_id,
            mandala_id: updated_package.mandala_id,
            slot_id: slot.slot_id,
            slot_state: slot.state,
        }),
    ))
}

async fn list_slots(State(state): State<AppState>) -> Json<Vec<jimi_kernel::PersonalitySlot>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(runtime.slots.all().into_iter().cloned().collect())
}

async fn list_artifacts(State(state): State<AppState>) -> Json<Vec<FieldVaultArtifact>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(runtime.fieldvault.all().into_iter().cloned().collect())
}

async fn list_providers(
    State(state): State<AppState>,
) -> Json<Vec<jimi_kernel::ProviderLaneRecord>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(runtime.providers.all().into_iter().cloned().collect())
}

async fn update_provider_lane(
    Path(provider_lane_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<UpdateProviderLaneRequest>,
) -> Result<Json<ProviderLaneUpdateResponse>, (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let fallback_group = request
        .fallback_group
        .and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });
    let updated = runtime
        .providers
        .update_lane(
            &provider_lane_id,
            request.routing_mode.trim(),
            fallback_group,
            request.priority,
        )
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?;

    runtime.events.append(
        ActorRef {
            actor_type: "operator".into(),
            actor_id: "cockpit.providers".into(),
        },
        SubjectRef {
            subject_type: "provider_lane".into(),
            subject_id: updated.provider_lane_id.clone(),
        },
        EventType::EngineSelected,
        None,
        None,
        None,
        serde_json::json!({
            "provider_lane_id": updated.provider_lane_id,
            "routing_mode": updated.routing_mode,
            "fallback_group": updated.fallback_group,
            "priority": updated.priority,
            "status": updated.status,
            "change_kind": "lane_policy_updated",
        }),
    );

    let new_event = runtime.events.all().last().cloned();
    persist_runtime(&state, &runtime)?;
    drop(runtime);
    if let Some(event) = new_event {
        let _ = state.events_tx.send(event);
    }

    Ok(Json(ProviderLaneUpdateResponse { provider: updated }))
}

async fn inventory(State(state): State<AppState>) -> Json<InventoryResponse> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(InventoryResponse {
        inventory: runtime.inventory(),
        mandalas: runtime
            .mandalas
            .ids()
            .into_iter()
            .map(ToOwned::to_owned)
            .collect(),
        slots: runtime
            .slots
            .all()
            .into_iter()
            .map(|slot| slot.slot_id.clone())
            .collect(),
    })
}

async fn macro_status(State(state): State<AppState>) -> Json<MacroStatusResponse> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(build_macro_status(&runtime.inventory()))
}

async fn control_plane_status(State(state): State<AppState>) -> Json<ControlPlaneResponse> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(build_control_plane_status(&runtime.inventory()))
}

async fn world_state_status(
    State(state): State<AppState>,
) -> Result<Json<WorldStateStatusResponse>, (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let active_mandala_id = runtime
        .slots
        .all()
        .into_iter()
        .find_map(|slot| slot.active_mandala_id.as_ref())
        .and_then(|mandala_id| runtime.mandalas.get(mandala_id).ok())
        .map(|mandala| mandala.self_section.id.clone());
    let memory_policy = runtime.active_memory_policy().unwrap_or_default();
    refresh_world_state_cache(&mut runtime, &state.house_root, &memory_policy);
    let response = WorldStateStatusResponse {
        active_mandala_id,
        preferred_scope: memory_policy.world_state_scope.clone(),
        lookup_sources: memory_policy.lookup_sources.clone(),
        slice: build_world_state_slice_from_cache(&runtime, &state.house_root, &memory_policy),
    };
    persist_runtime(&state, &runtime)?;
    Ok(Json(response))
}

async fn distiller_status(State(state): State<AppState>) -> Json<DistillerStatusResponse> {
    let status = state
        .distiller_status
        .lock()
        .expect("distiller status lock poisoned")
        .clone();
    Json(DistillerStatusResponse {
        running: status.running,
        pending: status.pending,
        processed: status.processed,
        failed: status.failed,
        last_error: status.last_error,
    })
}

fn guidance_from_transition(
    last_transition: Option<&str>,
    next_actions: &[String],
    awaiting_approval_count: usize,
    queued_dispatch_count: usize,
) -> String {
    let next_action = next_actions
        .first()
        .cloned()
        .unwrap_or_else(|| "define the next mission".to_string());

    match last_transition.unwrap_or_default() {
        transition if transition.starts_with("approval_denied:") => {
            "revise the blocked turn or reroute it through a safer lane".to_string()
        }
        transition if transition.starts_with("awaiting_approval:") => {
            "resolve the pending approval so the mission can continue".to_string()
        }
        transition if transition.starts_with("degraded:") => {
            "inspect the degraded lane and prefer a ready fallback in the same group".to_string()
        }
        transition if transition.starts_with("fallback:") => {
            "observe the fallback lane and let autonomy continue if the output looks healthy"
                .to_string()
        }
        transition if transition.starts_with("failed:") => {
            "rebuild the failed dispatch with a narrower intent or a different provider rail"
                .to_string()
        }
        transition if transition.starts_with("completed:") && !next_actions.is_empty() => {
            format!("promote the mission by dispatching the next action: {next_action}")
        }
        _ if awaiting_approval_count > 0 => "review and resolve pending approvals".to_string(),
        _ if queued_dispatch_count > 0 => {
            "let autonomy continue through queued dispatches".to_string()
        }
        _ if !next_actions.is_empty() => {
            format!("spawn the next turn toward: {next_action}")
        }
        _ => "ground the next mission move explicitly".to_string(),
    }
}

fn current_mission_state(
    runtime: &HouseRuntime,
    autonomy_status: Option<&AutonomyStatus>,
) -> (String, Option<String>, Vec<String>, usize, usize, String, String) {
    let active_mandala = runtime
        .slots
        .all()
        .into_iter()
        .find_map(|slot| slot.active_mandala_id.as_ref())
        .and_then(|mandala_id| runtime.mandalas.get(mandala_id).ok());
    let current_goal = active_mandala.map(|mandala| mandala.active_snapshot.current_goal.clone());
    let next_actions = active_mandala
        .map(|mandala| mandala.active_snapshot.next_actions.clone())
        .unwrap_or_default();
    let queued_dispatch_count = runtime
        .dispatches
        .all()
        .into_iter()
        .filter(|dispatch| dispatch.status == "queued")
        .count();
    let awaiting_approval_count = runtime.approvals.pending().len();
    let mission_phase = if awaiting_approval_count > 0 {
        "awaiting_approval"
    } else if queued_dispatch_count > 0 {
        "advancing"
    } else if current_goal.as_deref().unwrap_or_default().is_empty() {
        "unassigned"
    } else if !next_actions.is_empty() {
        "ready_for_next_turn"
    } else {
        "steady"
    }
    .to_string();
    let recommended_next_step = if awaiting_approval_count > 0 {
        "review and resolve pending approvals".to_string()
    } else if queued_dispatch_count > 0 {
        "let autonomy continue through queued dispatches".to_string()
    } else if !next_actions.is_empty() {
        next_actions[0].clone()
    } else if let Some(goal) = &current_goal {
        format!("ground the next turn toward: {goal}")
    } else {
        "define the next mission".to_string()
    };
    let transition_guidance = guidance_from_transition(
        autonomy_status.and_then(|status| status.last_transition.as_deref()),
        &next_actions,
        awaiting_approval_count,
        queued_dispatch_count,
    );
    (
        mission_phase,
        current_goal,
        next_actions,
        queued_dispatch_count,
        awaiting_approval_count,
        recommended_next_step,
        transition_guidance,
    )
}

fn mark_autonomy_transition(status: &mut AutonomyStatus, transition: impl Into<String>) {
    status.transition_count += 1;
    status.last_transition = Some(transition.into());
}

async fn autonomy_status(State(state): State<AppState>) -> Json<AutonomyStatusResponse> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    let status = state
        .autonomy_status
        .lock()
        .expect("autonomy status lock poisoned")
        .clone();
    let (
        mission_phase,
        current_goal,
        next_actions,
        queued_dispatch_count,
        awaiting_approval_count,
        recommended_next_step,
        transition_guidance,
    ) = current_mission_state(&runtime, Some(&status));
    drop(runtime);
    Json(AutonomyStatusResponse {
        enabled: status.enabled,
        running: status.running,
        cycles: status.cycles,
        completed_dispatches: status.completed_dispatches,
        transition_count: status.transition_count,
        last_transition: status.last_transition,
        mission_phase,
        current_goal,
        next_actions,
        queued_dispatch_count,
        awaiting_approval_count,
        recommended_next_step,
        transition_guidance,
        last_dispatch_id: status.last_dispatch_id,
        last_intent_summary: status.last_intent_summary,
        last_selection_reason: status.last_selection_reason,
        last_error: status.last_error,
    })
}

async fn set_autonomy(
    State(state): State<AppState>,
    Json(request): Json<SetAutonomyRequest>,
) -> Result<Json<AutonomyStatusResponse>, (StatusCode, String)> {
    let runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let (
        mission_phase,
        current_goal,
        next_actions,
        queued_dispatch_count,
        awaiting_approval_count,
        recommended_next_step,
        transition_guidance,
    ) = current_mission_state(&runtime, None);
    drop(runtime);
    let mut status = state.autonomy_status.lock().map_err(internal_lock_error)?;
    status.enabled = request.enabled;
    mark_autonomy_transition(
        &mut status,
        if request.enabled {
            "autonomy_enabled"
        } else {
            "autonomy_paused"
        },
    );
    Ok(Json(AutonomyStatusResponse {
        enabled: status.enabled,
        running: status.running,
        cycles: status.cycles,
        completed_dispatches: status.completed_dispatches,
        transition_count: status.transition_count,
        last_transition: status.last_transition.clone(),
        mission_phase,
        current_goal,
        next_actions,
        queued_dispatch_count,
        awaiting_approval_count,
        recommended_next_step,
        transition_guidance,
        last_dispatch_id: status.last_dispatch_id.clone(),
        last_intent_summary: status.last_intent_summary.clone(),
        last_selection_reason: status.last_selection_reason.clone(),
        last_error: status.last_error.clone(),
    }))
}

async fn list_approvals(State(state): State<AppState>) -> Json<Vec<ApprovalRequestRecord>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(runtime.approvals.all().into_iter().cloned().collect())
}

async fn approvals_status(State(state): State<AppState>) -> Json<ApprovalsStatusResponse> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    let approvals: Vec<ApprovalRequestRecord> = runtime.approvals.all().into_iter().cloned().collect();
    let pending = approvals.iter().filter(|approval| approval.status == "pending").count();
    let granted = approvals.iter().filter(|approval| approval.status == "granted").count();
    let denied = approvals.iter().filter(|approval| approval.status == "denied").count();
    Json(ApprovalsStatusResponse {
        pending,
        granted,
        denied,
        approvals,
    })
}

async fn grant_approval(
    Path(approval_id): Path<String>,
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<ApprovalActionResponse>), (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let approval = runtime
        .approvals
        .grant(&approval_id)
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?;

    if approval.action.starts_with("install_capsule_package:") {
        if let Some(package_id) = approval.action.split(':').nth(1) {
            let _ = runtime
                .capsule_packages
                .update_install_status(package_id, "approved_for_install");
        }
    } else if let Ok(dispatch) = runtime.dispatches.update_status(&approval.dispatch_id, "queued") {
        let _ = runtime
            .sessions
            .update_turn_state(&dispatch.turn_id, jimi_kernel::TurnState::Queued);
    }

    runtime.events.append(
        ActorRef {
            actor_type: "operator".into(),
            actor_id: "cockpit.approvals".into(),
        },
        SubjectRef {
            subject_type: "approval".into(),
            subject_id: approval.approval_request_id.clone(),
        },
        EventType::ApprovalGranted,
        Some(&approval.session_id),
        None,
        None,
        serde_json::json!({
            "approval_request_id": approval.approval_request_id,
            "dispatch_id": approval.dispatch_id,
            "provider_lane_id": approval.provider_lane_id,
            "action": approval.action,
            "status": approval.status,
        }),
    );

    let new_event = runtime.events.all().last().cloned();
    persist_runtime(&state, &runtime)?;
    drop(runtime);

    if let Some(event) = new_event {
        let _ = state.events_tx.send(event);
    }
    if let Ok(mut autonomy_status) = state.autonomy_status.lock() {
        mark_autonomy_transition(
            &mut autonomy_status,
            format!("approval_granted:{}", approval.dispatch_id),
        );
    }

    Ok((StatusCode::OK, Json(ApprovalActionResponse { approval })))
}

async fn deny_approval(
    Path(approval_id): Path<String>,
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<ApprovalActionResponse>), (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let approval = runtime
        .approvals
        .deny(&approval_id)
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?;

    if approval.action.starts_with("install_capsule_package:") {
        if let Some(package_id) = approval.action.split(':').nth(1) {
            let _ = runtime
                .capsule_packages
                .update_install_status(package_id, "install_denied");
        }
    } else if let Ok(dispatch) = runtime.dispatches.update_status(&approval.dispatch_id, "denied") {
        let _ = runtime
            .sessions
            .update_turn_state(&dispatch.turn_id, jimi_kernel::TurnState::Interrupted);
    }

    runtime.events.append(
        ActorRef {
            actor_type: "operator".into(),
            actor_id: "cockpit.approvals".into(),
        },
        SubjectRef {
            subject_type: "approval".into(),
            subject_id: approval.approval_request_id.clone(),
        },
        EventType::ApprovalDenied,
        Some(&approval.session_id),
        None,
        None,
        serde_json::json!({
            "approval_request_id": approval.approval_request_id,
            "dispatch_id": approval.dispatch_id,
            "provider_lane_id": approval.provider_lane_id,
            "action": approval.action,
            "status": approval.status,
        }),
    );

    let new_event = runtime.events.all().last().cloned();
    persist_runtime(&state, &runtime)?;
    drop(runtime);

    if let Some(event) = new_event {
        let _ = state.events_tx.send(event);
    }
    if let Ok(mut autonomy_status) = state.autonomy_status.lock() {
        mark_autonomy_transition(
            &mut autonomy_status,
            format!("approval_denied:{}", approval.dispatch_id),
        );
    }

    Ok((StatusCode::OK, Json(ApprovalActionResponse { approval })))
}

async fn security_status(State(state): State<AppState>) -> Json<SecurityStatusResponse> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    let active_mandala = runtime
        .slots
        .all()
        .into_iter()
        .find_map(|slot| slot.active_mandala_id.as_ref())
        .and_then(|mandala_id| runtime.mandalas.get(mandala_id).ok());
    let artifact_seal_levels = runtime
        .fieldvault
        .all()
        .into_iter()
        .map(|artifact| format!("{:?}", artifact.seal_level))
        .collect::<Vec<_>>();

    Json(SecurityStatusResponse {
        active_mandala_id: active_mandala.map(|mandala| mandala.self_section.id.clone()),
        preferred_provider: active_mandala
            .map(|mandala| mandala.execution_policy.preferred_provider.clone()),
        preferred_model: active_mandala
            .map(|mandala| mandala.execution_policy.preferred_model.clone()),
        sealing_enabled: active_mandala
            .map(|mandala| mandala.memory_policy.allow_fieldvault_sealing)
            .unwrap_or(false),
        seal_privacy_classes: active_mandala
            .map(|mandala| mandala.memory_policy.seal_privacy_classes.clone())
            .unwrap_or_default(),
        capability_declared: active_mandala
            .map(|mandala| mandala.capability_policy.declared.len())
            .unwrap_or(0),
        capability_required: active_mandala
            .map(|mandala| mandala.capability_policy.required.len())
            .unwrap_or(0),
        capability_optional: active_mandala
            .map(|mandala| mandala.capability_policy.optional.len())
            .unwrap_or(0),
        sacred_shards: active_mandala
            .map(|mandala| mandala.sacred_shards.len())
            .unwrap_or(0),
        skill_packs: active_mandala
            .map(|mandala| mandala.skill_packs.len())
            .unwrap_or(0),
        artifacts_total: runtime.fieldvault.all().len(),
        artifact_seal_levels,
    })
}

async fn capsule_trust_status(State(state): State<AppState>) -> Json<CapsuleTrustStatusResponse> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    let packages = runtime
        .capsule_packages
        .all()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let mut trust_counts = BTreeMap::<String, usize>::new();
    let mut internal_packages = 0usize;
    let mut external_packages = 0usize;

    for package in &packages {
        *trust_counts.entry(package.trust_level.clone()).or_default() += 1;
        if package.trust_level.eq_ignore_ascii_case("internal") {
            internal_packages += 1;
        } else {
            external_packages += 1;
        }
    }

    Json(CapsuleTrustStatusResponse {
        total_packages: packages.len(),
        internal_packages,
        external_packages,
        trust_levels: trust_counts
            .into_iter()
            .map(|(trust_level, packages)| CapsuleTrustLevelCount {
                trust_level,
                packages,
            })
            .collect(),
        packages,
    })
}

async fn marketplace_status(State(state): State<AppState>) -> Json<MarketplaceStatusResponse> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    let entries = runtime
        .capsule_packages
        .all()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let installable_entries = entries
        .iter()
        .filter(|package| package.install_status != "installed")
        .count();
    let trusted_entries = entries
        .iter()
        .filter(|package| {
            package.trust_level.eq_ignore_ascii_case("internal")
                || package.trust_level.eq_ignore_ascii_case("trusted")
        })
        .count();

    Json(MarketplaceStatusResponse {
        total_entries: entries.len(),
        installable_entries,
        trusted_entries,
        entries,
    })
}

async fn provider_readiness_status() -> Json<ProviderReadinessResponse> {
    Json(ProviderReadinessResponse {
        providers: detect_provider_credentials(),
    })
}

fn provider_credential_status(provider: &str) -> Option<ProviderCredentialStatus> {
    detect_provider_credentials()
        .into_iter()
        .find(|status| status.provider == provider)
}

fn ready_provider_names() -> Vec<String> {
    detect_provider_credentials()
        .into_iter()
        .filter(|status| status.ready)
        .map(|status| status.provider)
        .collect()
}

fn build_macro_status(inventory: &HouseInventory) -> MacroStatusResponse {
    let runtime_percent = if inventory.sessions > 0 && inventory.events > 0 {
        82
    } else {
        36
    };
    let memory_percent = if inventory.memory_capsules > 0 && inventory.summary_checkpoints > 0 {
        84
    } else {
        32
    };
    let cockpit_percent = if inventory.events > 0 && inventory.turn_dispatches > 0 {
        85
    } else {
        40
    };
    let world_state_percent = if inventory.sessions > 0 && inventory.provider_lanes > 0 {
        52
    } else {
        24
    };
    let providers_percent = if inventory.provider_lanes > 0 { 68 } else { 24 };
    let capsules_percent = if inventory.capsules > 0 && inventory.slots > 0 {
        88
    } else {
        28
    };

    MacroStatusResponse {
        retrobuilder: MacroAxisStatus {
            label: "RETROBUILDER",
            percent: 96,
            phase: "R4",
            summary: "live loops materializing",
            steps_remaining: 3,
        },
        l1ght: MacroAxisStatus {
            label: "L1GHT",
            percent: 93,
            phase: "L4",
            summary: "live surfaces with approvals and world state",
            steps_remaining: 3,
        },
        project: MacroAxisStatus {
            label: "PROJECT",
            percent: 96,
            phase: "operational core",
            summary: "multi-engine house online, hardening and ecosystem ahead",
            steps_remaining: 12,
        },
        current_phase_steps_remaining: 3,
        project_steps_remaining: 12,
        subsystems: vec![
            MacroAxisStatus {
                label: "RUNTIME",
                percent: runtime_percent,
                phase: "core",
                summary: "session, dispatch, persistence",
                steps_remaining: 4,
            },
            MacroAxisStatus {
                label: "MEMORY",
                percent: memory_percent,
                phase: "capsule",
                summary: "capture, summaries, compaction",
                steps_remaining: 5,
            },
            MacroAxisStatus {
                label: "COCKPIT",
                percent: cockpit_percent,
                phase: "surface",
                summary: "inventory, memory, pulse",
                steps_remaining: 4,
            },
            MacroAxisStatus {
                label: "WORLD STATE",
                percent: world_state_percent,
                phase: "substrate",
                summary: "bounded workspace and process slices are starting to come online",
                steps_remaining: 6,
            },
            MacroAxisStatus {
                label: "PROVIDERS",
                percent: providers_percent + 24,
                phase: "lane",
                summary: "codex and anthropic paths are online through the adapter spine",
                steps_remaining: 4,
            },
            MacroAxisStatus {
                label: "CAPSULES",
                percent: capsules_percent,
                phase: "house identity",
                summary: "mandala, slot, fieldvault",
                steps_remaining: 5,
            },
            MacroAxisStatus {
                label: "MARKETPLACE",
                percent: 18,
                phase: "future",
                summary: "capsule marketplace not started",
                steps_remaining: 12,
            },
            MacroAxisStatus {
                label: "SECURITY",
                percent: 72,
                phase: "guardrails",
                summary: "policy baseline and approval gate are live; deeper controls still ahead",
                steps_remaining: 4,
            },
            MacroAxisStatus {
                label: "AUTONOMY",
                percent: 72,
                phase: "self-propelled",
                summary: "background house loop is online with approval guardrails; mission autonomy still evolving",
                steps_remaining: 4,
            },
        ],
        done: vec![
            "grounded constitutional spec stack",
            "sovereign runtime core with durable sqlite memory",
            "live cockpit with event pulse and inventory",
            "mandala capsules, slots, and fieldvault artifacts",
            "provider lane, dispatch flow, and live codex execution",
            "capsule memory with summaries, bridges, promotions, and compaction",
            "approval guardrails and human interruptability for live execution",
        ],
        doing: vec![
            "turning memory orchestration into a first-class runtime discipline",
            "improving macro visibility for chat and cockpit",
            "tightening the provider lane around sovereign context packets",
            "turning explicit operator continuation into mission-aware autonomy",
        ],
        next: vec![
            "expose adapter readiness and failures as stronger house events",
            "deepen autonomy from baseline loop into mission-aware execution",
            "grow world state from bounded slices into incremental host graphing",
        ],
        later: vec![
            "multi-engine adapters",
            "capsule marketplace and personality slots for external creators",
            "deeper security and policy enforcement surfaces",
        ],
        menu: vec![
            MacroMenuItem {
                key: "1",
                prompt: "what was completed",
            },
            MacroMenuItem {
                key: "2",
                prompt: "what is being built now",
            },
            MacroMenuItem {
                key: "3",
                prompt: "what remains",
            },
            MacroMenuItem {
                key: "4",
                prompt: "show full panorama",
            },
            MacroMenuItem {
                key: "5",
                prompt: "show subsystem status",
            },
            MacroMenuItem {
                key: "6",
                prompt: "show latest milestones",
            },
            MacroMenuItem {
                key: "7",
                prompt: "show risks and blocks",
            },
            MacroMenuItem {
                key: "8",
                prompt: "show recommended next step",
            },
        ],
    }
}

fn build_control_plane_status(inventory: &HouseInventory) -> ControlPlaneResponse {
    ControlPlaneResponse {
        title: "Integrated control plane",
        sections: vec![
            ControlPlaneSection {
                label: "House Control",
                status: "partial",
                priority: "high",
                summary: "macro steering and method visibility are online; mutable runtime modes still ahead",
            },
            ControlPlaneSection {
                label: "Mandala Studio",
                status: if inventory.mandalas > 0 {
                    "visible"
                } else {
                    "pending"
                },
                priority: "high",
                summary: "mandalas and snapshots are visible; editing policies in cockpit is next",
            },
            ControlPlaneSection {
                label: "Capsule Manager",
                status: if inventory.capsules > 0 {
                    "visible"
                } else {
                    "pending"
                },
                priority: "medium",
                summary: "capsules, slots, and artifacts are visible; install graph and dependency checks are future work",
            },
            ControlPlaneSection {
                label: "Provider Matrix",
                status: if inventory.provider_lanes > 0 {
                    "partial"
                } else {
                    "pending"
                },
                priority: "high",
                summary: "connected lanes are shown; adapter matrix and fallback routing are not extracted yet",
            },
            ControlPlaneSection {
                label: "Memory Console",
                status: if inventory.memory_capsules > 0 {
                    "partial"
                } else {
                    "pending"
                },
                priority: "high",
                summary: "capsules, summaries, bridges, triggers, promotions, and compaction metrics are live",
            },
            ControlPlaneSection {
                label: "World State Console",
                status: if inventory.sessions > 0 {
                    "baseline"
                } else {
                    "planned"
                },
                priority: "high",
                summary: "bounded workspace and process snapshots are visible; incremental ingest and host graphing are future work",
            },
            ControlPlaneSection {
                label: "Security Surface",
                status: if inventory.approval_requests > 0 || inventory.fieldvault_artifacts > 0 {
                    "partial"
                } else {
                    "baseline"
                },
                priority: "medium",
                summary: "policy baseline, sealing posture, capability counts, and approval guardrails are now visible",
            },
            ControlPlaneSection {
                label: "Autonomy Surface",
                status: "partial",
                priority: "high",
                summary: "background autonomy loop is online; mission-aware execution and deeper guardrails are next",
            },
            ControlPlaneSection {
                label: "Workflow Composer",
                status: "planned",
                priority: "medium",
                summary: "turns and dispatches exist; higher-order flow composition is still ahead",
            },
            ControlPlaneSection {
                label: "Macro Planner",
                status: "online",
                priority: "high",
                summary: "project panorama, subsystem bars, and menu are now visible in the cockpit",
            },
            ControlPlaneSection {
                label: "Marketplace Shell",
                status: "future",
                priority: "later",
                summary: "capsule marketplace remains a constitutional target, not an implemented surface yet",
            },
        ],
    }
}

async fn bootstrap_core_capsule(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<CapsuleBootstrapResponse>), (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;

    let mandala_id = "jimi.superstar.core".to_string();
    let capsule_id = "capsule.jimi.superstar.core.v1".to_string();
    let package_id = "package.jimi.superstar.core.v1".to_string();
    let slot_id = "slot.jimi.superstar.primary".to_string();

    if runtime.mandalas.get(&mandala_id).is_err() {
        runtime.mandalas.install(sample_core_mandala());
        runtime.events.append(
            ActorRef {
                actor_type: "operator".into(),
                actor_id: "cockpit.bootstrap".into(),
            },
            SubjectRef {
                subject_type: "mandala".into(),
                subject_id: mandala_id.clone(),
            },
            EventType::MandalaBound,
            None,
            None,
            None,
            serde_json::json!({
                "mandala_id": mandala_id.clone(),
                "projection_kind": "role-overlay",
                "source": "cockpit_bootstrap",
            }),
        );
    }

    if runtime.capsules.get(&capsule_id).is_err() {
        runtime
            .capsules
            .install(capsule_id.clone(), mandala_id.clone(), 1, "house.bootstrap");
        runtime.events.append(
            ActorRef {
                actor_type: "operator".into(),
                actor_id: "cockpit.bootstrap".into(),
            },
            SubjectRef {
                subject_type: "capsule".into(),
                subject_id: capsule_id.clone(),
            },
            EventType::CapsuleInstalled,
            None,
            None,
            None,
            serde_json::json!({
                "capsule_id": capsule_id.clone(),
                "mandala_id": mandala_id.clone(),
                "version": 1,
                "install_source": "house.bootstrap",
            }),
        );
    }

    let package_exists = runtime
        .capsule_packages
        .all()
        .into_iter()
        .any(|package| package.package_id == package_id);
    if !package_exists {
        runtime.capsule_packages.register(
            package_id.clone(),
            capsule_id.clone(),
            mandala_id.clone(),
            1,
            "JIMI Core Capsule",
            "max kle1nz",
            "house.bootstrap",
            "digest::jimi.superstar.core.v1",
            "internal",
            "installed",
        );
    }

    if runtime.slots.get(&slot_id).is_err() {
        runtime
            .slots
            .define_slot(slot_id.clone(), "Primary Guardian Slot");
    }

    runtime
        .slots
        .bind_capsule(&slot_id, capsule_id.clone(), mandala_id.clone(), false)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    runtime
        .slots
        .activate(&slot_id)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    runtime.events.append(
        ActorRef {
            actor_type: "operator".into(),
            actor_id: "cockpit.bootstrap".into(),
        },
        SubjectRef {
            subject_type: "slot".into(),
            subject_id: slot_id.clone(),
        },
        EventType::SlotActivated,
        None,
        None,
        None,
        serde_json::json!({
            "slot_id": slot_id.clone(),
            "capsule_id": capsule_id.clone(),
            "mandala_id": mandala_id.clone(),
            "state": "active",
        }),
    );

    let slot = runtime
        .slots
        .get(&slot_id)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .clone();
    let new_events: Vec<EventEnvelope> =
        runtime.events.all().iter().rev().take(4).cloned().collect();
    persist_runtime(&state, &runtime)?;
    drop(runtime);

    for event in new_events.into_iter().rev() {
        let _ = state.events_tx.send(event);
    }

    Ok((
        StatusCode::CREATED,
        Json(CapsuleBootstrapResponse {
            mandala_id,
            capsule_id,
            package_id,
            slot_id,
            slot_state: slot.state,
        }),
    ))
}

async fn import_demo_capsule(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<CapsuleImportDemoResponse>), (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;

    let mandala_id = "mandala.echo.scribe".to_string();
    let capsule_id = "capsule.echo.scribe.v1".to_string();
    let package_id = "package.echo.scribe.v1".to_string();

    if runtime.mandalas.get(&mandala_id).is_err() {
        runtime.mandalas.install(sample_external_mandala());
    }

    if runtime.capsules.get(&capsule_id).is_err() {
        runtime
            .capsules
            .install(capsule_id.clone(), mandala_id.clone(), 1, "marketplace.demo");
    }

    let package = if let Ok(existing) = runtime.capsule_packages.get(&package_id) {
        existing.clone()
    } else {
        runtime.capsule_packages.register(
            package_id.clone(),
            capsule_id.clone(),
            mandala_id.clone(),
            1,
            "Echo Scribe",
            "jimi.market.demo",
            "marketplace.demo",
            "digest.echo.scribe.v1",
            "unverified",
            "imported",
        )
    };

    let import_record = runtime.capsule_imports.record(
        package.package_id.clone(),
        "marketplace.demo",
        state
            .house_root
            .join("imports/echo-scribe-demo.capsule.json")
            .display()
            .to_string(),
        "imported",
        package.package_digest.clone(),
    );

    runtime.events.append(
        ActorRef {
            actor_type: "operator".into(),
            actor_id: "cockpit.marketplace".into(),
        },
        SubjectRef {
            subject_type: "capsule_package".into(),
            subject_id: package.package_id.clone(),
        },
        EventType::CapsuleMarketplaceListed,
        None,
        None,
        None,
        serde_json::json!({
            "package_id": package.package_id,
            "capsule_id": package.capsule_id,
            "import_id": import_record.import_id,
            "source_origin": import_record.source_origin,
            "trust_level": package.trust_level,
            "install_status": package.install_status,
        }),
    );

    let new_event = runtime.events.all().last().cloned();
    persist_runtime(&state, &runtime)?;
    drop(runtime);
    if let Some(event) = new_event {
        let _ = state.events_tx.send(event);
    }

    Ok((
        StatusCode::CREATED,
        Json(CapsuleImportDemoResponse {
            package,
            import_record,
        }),
    ))
}

async fn bootstrap_core_artifact(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<ArtifactBootstrapResponse>), (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;

    let capsule_id = "capsule.jimi.superstar.core.v1".to_string();
    let slot_id = "slot.jimi.superstar.primary".to_string();

    if runtime.capsules.get(&capsule_id).is_err() || runtime.slots.get(&slot_id).is_err() {
        return Err((
            StatusCode::BAD_REQUEST,
            "core capsule and primary slot must exist before sealing an artifact".into(),
        ));
    }

    let artifact = runtime.fieldvault.register_artifact(
        Some(capsule_id.clone()),
        Some(slot_id.clone()),
        SealLevel::CapsulePrivate,
        "./vault/jimi-superstar-core.fld",
        true,
        false,
    );

    runtime.events.append(
        ActorRef {
            actor_type: "operator".into(),
            actor_id: "cockpit.bootstrap".into(),
        },
        SubjectRef {
            subject_type: "artifact".into(),
            subject_id: artifact.artifact_id.clone(),
        },
        EventType::ArtifactCreated,
        None,
        None,
        None,
        serde_json::json!({
            "artifact_id": artifact.artifact_id.clone(),
            "capsule_id": capsule_id.clone(),
            "slot_id": slot_id.clone(),
            "fld_path": artifact.fld_path.clone(),
            "seal_level": "capsule_private",
        }),
    );

    let new_event = runtime.events.all().last().cloned();
    persist_runtime(&state, &runtime)?;
    drop(runtime);

    if let Some(event) = new_event {
        let _ = state.events_tx.send(event);
    }

    Ok((
        StatusCode::CREATED,
        Json(ArtifactBootstrapResponse {
            artifact_id: artifact.artifact_id,
            capsule_id: artifact.capsule_id,
            slot_id: artifact.slot_id,
            seal_level: artifact.seal_level,
        }),
    ))
}

async fn bootstrap_provider_lane(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<ProviderBootstrapResponse>), (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let readiness = provider_credential_status("codex");

    let provider = runtime.providers.connect(
        "provider.codex.primary",
        "codex",
        "gpt-5",
        "primary",
        Some("core".into()),
        100,
        "connected",
    );

    runtime.events.append(
        ActorRef {
            actor_type: "operator".into(),
            actor_id: "cockpit.bootstrap".into(),
        },
        SubjectRef {
            subject_type: "provider_lane".into(),
            subject_id: provider.provider_lane_id.clone(),
        },
        EventType::EngineSelected,
        None,
        None,
        None,
        serde_json::json!({
            "provider_lane_id": provider.provider_lane_id.clone(),
            "provider": provider.provider.clone(),
            "model": provider.model.clone(),
            "routing_mode": provider.routing_mode.clone(),
            "fallback_group": provider.fallback_group.clone(),
            "priority": provider.priority,
            "status": provider.status.clone(),
        }),
    );

    if let Some(readiness) = readiness {
        runtime.events.append(
            ActorRef {
                actor_type: "house.auth".into(),
                actor_id: "provider.readiness".into(),
            },
            SubjectRef {
                subject_type: "provider_lane".into(),
                subject_id: provider.provider_lane_id.clone(),
            },
            if readiness.ready {
                EventType::EngineSelected
            } else {
                EventType::EngineDegraded
            },
            None,
            None,
            None,
            serde_json::json!({
                "provider_lane_id": provider.provider_lane_id.clone(),
                "provider": provider.provider.clone(),
                "status": if readiness.ready { "ready" } else { "degraded" },
                "credential_source": readiness.source,
                "details": readiness.details,
            }),
        );
    }

    let new_events: Vec<EventEnvelope> = runtime.events.all().iter().rev().take(2).cloned().collect();
    persist_runtime(&state, &runtime)?;
    drop(runtime);

    for event in new_events.into_iter().rev() {
        let _ = state.events_tx.send(event);
    }

    Ok((
        StatusCode::CREATED,
        Json(ProviderBootstrapResponse {
            provider_lane_id: provider.provider_lane_id,
            provider: provider.provider,
            model: provider.model,
            routing_mode: provider.routing_mode,
            fallback_group: provider.fallback_group,
            priority: provider.priority,
            status: provider.status,
        }),
    ))
}

async fn bootstrap_provider_lane_anthropic(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<ProviderBootstrapResponse>), (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let readiness = provider_credential_status("anthropic");

    let provider = runtime.providers.connect(
        "provider.anthropic.secondary",
        "anthropic",
        "claude-sonnet-4.5",
        "secondary",
        Some("core".into()),
        80,
        "connected",
    );

    runtime.events.append(
        ActorRef {
            actor_type: "operator".into(),
            actor_id: "cockpit.bootstrap".into(),
        },
        SubjectRef {
            subject_type: "provider_lane".into(),
            subject_id: provider.provider_lane_id.clone(),
        },
        EventType::EngineSelected,
        None,
        None,
        None,
        serde_json::json!({
            "provider_lane_id": provider.provider_lane_id.clone(),
            "provider": provider.provider.clone(),
            "model": provider.model.clone(),
            "routing_mode": provider.routing_mode.clone(),
            "fallback_group": provider.fallback_group.clone(),
            "priority": provider.priority,
            "status": provider.status.clone(),
        }),
    );

    if let Some(readiness) = readiness {
        runtime.events.append(
            ActorRef {
                actor_type: "house.auth".into(),
                actor_id: "provider.readiness".into(),
            },
            SubjectRef {
                subject_type: "provider_lane".into(),
                subject_id: provider.provider_lane_id.clone(),
            },
            if readiness.ready {
                EventType::EngineSelected
            } else {
                EventType::EngineDegraded
            },
            None,
            None,
            None,
            serde_json::json!({
                "provider_lane_id": provider.provider_lane_id.clone(),
                "provider": provider.provider.clone(),
                "status": if readiness.ready { "ready" } else { "degraded" },
                "credential_source": readiness.source,
                "details": readiness.details,
            }),
        );
    }

    let new_events: Vec<EventEnvelope> = runtime.events.all().iter().rev().take(2).cloned().collect();
    persist_runtime(&state, &runtime)?;
    drop(runtime);

    for event in new_events.into_iter().rev() {
        let _ = state.events_tx.send(event);
    }

    Ok((
        StatusCode::CREATED,
        Json(ProviderBootstrapResponse {
            provider_lane_id: provider.provider_lane_id,
            provider: provider.provider,
            model: provider.model,
            routing_mode: provider.routing_mode,
            fallback_group: provider.fallback_group,
            priority: provider.priority,
            status: provider.status,
        }),
    ))
}

async fn ws_events(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_events(socket, state))
}

fn persist_runtime(state: &AppState, runtime: &HouseRuntime) -> Result<(), (StatusCode, String)> {
    let mut store = state.store.lock().map_err(internal_lock_error)?;
    store
        .persist_runtime(runtime)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
}

fn internal_lock_error<T>(_error: T) -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal state lock poisoned".into(),
    )
}

fn internal_store_error(error: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

fn internal_fs_error(error: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

fn internal_serde_error(error: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

fn slugify_room_id(title: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for ch in title.chars() {
        let lowered = ch.to_ascii_lowercase();
        if lowered.is_ascii_alphanumeric() {
            slug.push(lowered);
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }
    let cleaned = slug.trim_matches('-').to_string();
    if cleaned.is_empty() {
        "jimi-room".into()
    } else {
        cleaned
    }
}

fn enqueue_transcript_distillation(state: &AppState, session_id: &jimi_kernel::SessionId) {
    let mut pending = match state.distiller_pending.lock() {
        Ok(pending) => pending,
        Err(_) => return,
    };
    if !pending.insert(session_id.0.clone()) {
        return;
    }
    let pending_len = pending.len();
    drop(pending);

    if let Ok(mut status) = state.distiller_status.lock() {
        status.pending = pending_len;
    }

    let _ = state.distiller_tx.send(session_id.0.clone());
}

fn spawn_autonomy_loop(state: AppState) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;

            let enabled = match state.autonomy_status.lock() {
                Ok(status) => status.enabled && !status.running,
                Err(_) => false,
            };
            if !enabled {
                continue;
            }

            if let Ok(mut status) = state.autonomy_status.lock() {
                status.running = true;
                status.cycles += 1;
            }

            let outcome = run_latest_dispatch_live_once(&state).await;
            if let Ok(mut status) = state.autonomy_status.lock() {
                status.running = false;
                match outcome {
                    Ok(_) => {
                        status.completed_dispatches += 1;
                        status.last_error = None;
                        if status.last_transition.is_none() {
                            mark_autonomy_transition(&mut status, "cycle_completed");
                        }
                    }
                    Err((code, message)) => {
                        if code == StatusCode::CONFLICT
                            && message.contains("approval required before live execution")
                        {
                            mark_autonomy_transition(&mut status, "cycle_waiting_for_approval");
                        } else if code == StatusCode::BAD_REQUEST
                            && message.contains("no queued dispatches available")
                        {
                            mark_autonomy_transition(&mut status, "cycle_idle");
                        }
                        if code != StatusCode::BAD_REQUEST
                            || !message.contains("no queued dispatches available")
                        {
                            status.last_error = Some(message);
                        }
                    }
                }
            }
        }
    });
}

fn spawn_transcript_distiller(state: AppState, mut rx: mpsc::UnboundedReceiver<String>) {
    tokio::spawn(async move {
        if let Ok(mut status) = state.distiller_status.lock() {
            status.running = true;
        }

        while let Some(session_id) = rx.recv().await {
            let result =
                distill_session_transcript(&state, &jimi_kernel::SessionId(session_id.clone()));
            let pending_len = {
                let mut pending = match state.distiller_pending.lock() {
                    Ok(pending) => pending,
                    Err(_) => continue,
                };
                pending.remove(&session_id);
                pending.len()
            };

            if let Ok(mut status) = state.distiller_status.lock() {
                status.pending = pending_len;
                match result {
                    Ok(processed) => {
                        if processed {
                            status.processed += 1;
                        }
                    }
                    Err(error) => {
                        status.failed += 1;
                        status.last_error = Some(error.clone());
                    }
                }
            }
        }
    });
}

fn distill_session_transcript(
    state: &AppState,
    session_id: &jimi_kernel::SessionId,
) -> Result<bool, String> {
    let mut runtime = state
        .runtime
        .lock()
        .map_err(|_| "runtime lock poisoned".to_string())?;
    let session = runtime
        .sessions
        .session(session_id)
        .map_err(|error| error.to_string())?
        .clone();
    let latest_turn = runtime
        .sessions
        .turns()
        .into_iter()
        .filter(|turn| turn.session_id.0 == session_id.0)
        .max_by_key(|turn| turn.created_at)
        .cloned();

    let Some(latest_turn) = latest_turn else {
        return Ok(false);
    };

    let has_transcript_capsule = runtime
        .memory_capsules
        .by_session(session_id)
        .into_iter()
        .any(|capsule| {
            capsule.role == "distiller"
                && capsule.turn_id.as_ref().map(|turn| turn.0.as_str())
                    == Some(latest_turn.turn_id.0.as_str())
                && capsule.intent_summary.as_deref() == Some("conversation/transcript")
        });
    if has_transcript_capsule {
        return Ok(false);
    }

    let recent_capsules = runtime
        .memory_capsules
        .by_session(session_id)
        .into_iter()
        .filter(|capsule| capsule.role == "operator" || capsule.role == "assistant")
        .rev()
        .take(6)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();

    if recent_capsules.is_empty() {
        return Ok(false);
    }

    let transcript_summary = recent_capsules
        .iter()
        .map(|capsule| format!("{}: {}", capsule.role, capsule.content))
        .collect::<Vec<_>>()
        .join(" | ");

    let latest_operator = recent_capsules
        .iter()
        .rev()
        .find(|capsule| capsule.role == "operator")
        .cloned();
    let latest_assistant = recent_capsules
        .iter()
        .rev()
        .find(|capsule| capsule.role == "assistant")
        .cloned();

    let mut distilled_capsules = 1;
    let mut distilled_kinds = vec!["conversation/transcript".to_string()];
    runtime.memory_capsules.append(
        session_id.clone(),
        session.room_id.clone(),
        latest_turn.lane_id.clone(),
        Some(latest_turn.turn_id.clone()),
        "distiller",
        transcript_summary,
        Some("conversation/transcript".into()),
        0.82,
        0.86,
        "session_open",
        "warm",
    );

    if recent_capsules.len() >= 2 {
        let episode_summary = recent_capsules
            .iter()
            .map(|capsule| {
                capsule
                    .intent_summary
                    .clone()
                    .unwrap_or_else(|| capsule.content.clone())
            })
            .collect::<Vec<_>>()
            .join(" -> ");
        distilled_capsules += 1;
        distilled_kinds.push("conversation/episode".into());
        runtime.memory_capsules.append(
            session_id.clone(),
            session.room_id.clone(),
            latest_turn.lane_id.clone(),
            Some(latest_turn.turn_id.clone()),
            "distiller",
            format!("Episode arc: {}", episode_summary),
            Some("conversation/episode".into()),
            0.8,
            0.85,
            "session_open",
            "warm",
        );
    }

    if let Some(operator_capsule) = latest_operator.clone() {
        distilled_capsules += 1;
        distilled_kinds.push("conversation/directive".into());
        runtime.memory_capsules.append(
            session_id.clone(),
            session.room_id.clone(),
            latest_turn.lane_id.clone(),
            Some(latest_turn.turn_id.clone()),
            "distiller",
            format!("Operator directive focus: {}", operator_capsule.content),
            Some("conversation/directive".into()),
            0.79,
            0.84,
            "session_open",
            "warm",
        );
    }

    if let Some(assistant_capsule) = latest_assistant.clone() {
        let assistant_lower = assistant_capsule.content.to_lowercase();
        let operator_text = latest_operator
            .as_ref()
            .map(|capsule| capsule.content.as_str())
            .unwrap_or("current session");

        if assistant_lower.contains("will ")
            || assistant_lower.contains("i'll ")
            || assistant_lower.contains("next ")
            || assistant_lower.contains("going to")
        {
            distilled_capsules += 1;
            distilled_kinds.push("conversation/promise".into());
            runtime.memory_capsules.append(
                session_id.clone(),
                session.room_id.clone(),
                latest_turn.lane_id.clone(),
                Some(latest_turn.turn_id.clone()),
                "distiller",
                format!(
                    "Promise made for {}: {}",
                    operator_text, assistant_capsule.content
                ),
                Some("conversation/promise".into()),
                0.8,
                0.86,
                "session_open",
                "warm",
            );
        }

        if assistant_lower.contains("decided")
            || assistant_lower.contains("we should")
            || assistant_lower.contains("the plan is")
            || assistant_lower.contains("we will use")
            || assistant_lower.contains("i recommend")
        {
            distilled_capsules += 1;
            distilled_kinds.push("conversation/decision".into());
            runtime.memory_capsules.append(
                session_id.clone(),
                session.room_id.clone(),
                latest_turn.lane_id.clone(),
                Some(latest_turn.turn_id.clone()),
                "distiller",
                format!(
                    "Decision taken for {}: {}",
                    operator_text, assistant_capsule.content
                ),
                Some("conversation/decision".into()),
                0.82,
                0.88,
                "session_open",
                "warm",
            );
        }
    }

    runtime.refresh_memory_for_session(session_id);
    runtime.events.append(
        ActorRef {
            actor_type: "worker".into(),
            actor_id: "transcript.distiller".into(),
        },
        SubjectRef {
            subject_type: "session".into(),
            subject_id: session_id.0.clone(),
        },
        EventType::TruthFusionUpdated,
        Some(session_id),
        Some(&latest_turn.lane_id),
        Some(&latest_turn.turn_id),
        serde_json::json!({
            "distilled_capsules": distilled_capsules,
            "room_id": session.room_id,
            "kinds": distilled_kinds,
        }),
    );
    let new_event = runtime.events.all().last().cloned();
    persist_runtime(state, &runtime).map_err(|(_, error)| error)?;
    drop(runtime);

    if let Some(event) = new_event {
        let _ = state.events_tx.send(event);
    }

    Ok(true)
}

async fn handle_ws_events(mut socket: WebSocket, state: AppState) {
    let snapshot = {
        let runtime = match state.runtime.lock() {
            Ok(runtime) => runtime,
            Err(_) => return,
        };
        runtime.events.all().to_vec()
    };

    for event in snapshot {
        if send_event(&mut socket, &event).await.is_err() {
            return;
        }
    }

    let mut rx = state.events_tx.subscribe();
    while let Ok(event) = rx.recv().await {
        if send_event(&mut socket, &event).await.is_err() {
            break;
        }
    }
}

async fn send_event(socket: &mut WebSocket, event: &EventEnvelope) -> Result<(), ()> {
    let payload = serde_json::to_string(event).map_err(|_| ())?;
    socket
        .send(Message::Text(payload.into()))
        .await
        .map_err(|_| ())
}

fn sample_core_mandala() -> MandalaManifest {
    MandalaManifest {
        manifest_version: "mandala/v1".into(),
        kind: "mandala".into(),
        generated_at: 1_774_771_200.0,
        agent_version: 1,
        self_section: MandalaSelf {
            id: "jimi.superstar.core".into(),
            role: "guardian".into(),
            template_soul: "jimi-superstar".into(),
            execution_role: Some("house-conductor".into()),
            specialization: Some("sovereign-agent-house".into()),
            tone: Some("warm-precise".into()),
            canonical: true,
            boundaries: Default::default(),
            tags: vec!["core".into(), "guardian".into(), "retrobuilder".into()],
        },
        execution_policy: MandalaExecutionPolicy {
            body:
                "Protect the house, narrate the build, and keep contracts ahead of improvisation."
                    .into(),
            execution_lane: "main".into(),
            preferred_provider: "codex".into(),
            preferred_model: "gpt-5".into(),
            reasoning_effort: Some("high".into()),
            use_session_pool: false,
            allow_provider_override: true,
            capabilities: vec![
                "tool.exec.bash".into(),
                "tool.http".into(),
                "tool.ws".into(),
                "memory.runtime".into(),
            ],
        },
        capability_policy: MandalaCapabilityPolicy {
            declared: vec![
                "skill.import".into(),
                "capsule.install".into(),
                "slot.activate".into(),
                "event.stream".into(),
            ],
            required: vec!["tool.exec.bash".into(), "memory.runtime".into()],
            optional: vec!["provider.codex".into(), "provider.claude".into()],
        },
        memory_policy: MandalaMemoryPolicy {
            boot_include: vec!["stable_memory".into(), "active_snapshot".into()],
            lookup_sources: vec![
                "past".into(),
                "cortex".into(),
                "vault".into(),
                "m1nd".into(),
            ],
            hot_context_limit: 5,
            relevant_context_limit: 6,
            promotion_confidence_threshold: 0.88,
            promote_to_stable_memory: true,
            allow_fieldvault_sealing: true,
            seal_privacy_classes: vec!["operator_private".into(), "sealed_candidate".into()],
            world_state_scope: "workspace+process".into(),
            world_state_entry_limit: 8,
            world_state_process_limit: 8,
            allow_world_state: true,
        },
        stable_memory: MandalaStableMemory::default(),
        active_snapshot: MandalaActiveSnapshot {
            current_goal: "Bootstrap the first sovereign cockpit loop.".into(),
            active_decisions: vec!["Mandalas are canonical saves.".into()],
            blockers: Vec::new(),
            next_actions: vec![
                "Expose capsule installation in the cockpit.".into(),
                "Add provider lanes after slot management.".into(),
            ],
            hot_context: vec!["RETROBUILDER".into(), "FieldVault".into()],
            snapshot: Default::default(),
        },
        refs: MandalaRefs::default(),
        projection: MandalaProjection {
            projection_kind: "role-overlay".into(),
            requested_role: "guardian".into(),
            template_soul: "jimi-superstar".into(),
            execution_role: Some("house-conductor".into()),
            default_body: "Protect the sovereign agent house and keep the build grounded.".into(),
            lineage: vec!["jimi".into(), "guardian".into()],
            autoevolve: true,
        },
        ownership: None,
        capsule_contract: Some(MandalaCapsuleContract::default()),
        skill_packs: Vec::new(),
        sacred_shards: Vec::new(),
        metadata: Default::default(),
    }
}

fn sample_external_mandala() -> MandalaManifest {
    MandalaManifest {
        manifest_version: "mandala/v1".into(),
        kind: "mandala".into(),
        generated_at: 1_774_771_200.0,
        agent_version: 1,
        self_section: MandalaSelf {
            id: "mandala.echo.scribe".into(),
            role: "scribe".into(),
            template_soul: "echo-scribe".into(),
            execution_role: Some("capsule-guest".into()),
            specialization: Some("memory-capture-and-closure".into()),
            tone: Some("calm-structured".into()),
            canonical: false,
            boundaries: Default::default(),
            tags: vec!["external".into(), "marketplace".into(), "scribe".into()],
        },
        execution_policy: MandalaExecutionPolicy {
            body: "Capture useful decisions and shape them into portable memory artifacts.".into(),
            execution_lane: "guest".into(),
            preferred_provider: "anthropic".into(),
            preferred_model: "claude-sonnet".into(),
            reasoning_effort: Some("medium".into()),
            use_session_pool: false,
            allow_provider_override: true,
            capabilities: vec!["memory.runtime".into(), "event.stream".into()],
        },
        capability_policy: MandalaCapabilityPolicy {
            declared: vec!["capsule.install".into(), "memory.capture".into()],
            required: vec!["memory.runtime".into()],
            optional: vec!["provider.claude".into(), "fieldvault.bind".into()],
        },
        memory_policy: MandalaMemoryPolicy {
            boot_include: vec!["stable_memory".into(), "active_snapshot".into()],
            lookup_sources: vec!["past".into(), "vault".into()],
            hot_context_limit: 4,
            relevant_context_limit: 5,
            promotion_confidence_threshold: 0.9,
            promote_to_stable_memory: true,
            allow_fieldvault_sealing: true,
            seal_privacy_classes: vec!["sealed_candidate".into()],
            world_state_scope: "workspace".into(),
            world_state_entry_limit: 4,
            world_state_process_limit: 2,
            allow_world_state: false,
        },
        stable_memory: MandalaStableMemory::default(),
        active_snapshot: MandalaActiveSnapshot {
            current_goal: "Offer a portable capsule example for marketplace install and export.".into(),
            active_decisions: vec!["Marketplace capsules remain under house sovereignty.".into()],
            blockers: Vec::new(),
            next_actions: vec![
                "Preview install impact before activation.".into(),
                "Export a portable capsule bundle when trust is acceptable.".into(),
            ],
            hot_context: vec!["marketplace".into(), "capsule".into()],
            snapshot: Default::default(),
        },
        refs: MandalaRefs::default(),
        projection: MandalaProjection {
            projection_kind: "capsule-guest".into(),
            requested_role: "scribe".into(),
            template_soul: "echo-scribe".into(),
            execution_role: Some("capsule-guest".into()),
            default_body: "Act as a portable marketplace capsule under house policy.".into(),
            lineage: vec!["marketplace".into(), "guest".into()],
            autoevolve: false,
        },
        ownership: None,
        capsule_contract: Some(MandalaCapsuleContract::default()),
        skill_packs: Vec::new(),
        sacred_shards: Vec::new(),
        metadata: Default::default(),
    }
}

const COCKPIT_HTML: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>JIMI SUPERSTAR</title>
    <style>
      :root {
        --bg: #07111f;
        --panel: #0e1b2f;
        --panel-2: #10253f;
        --line: rgba(117, 156, 214, 0.22);
        --text: #eef5ff;
        --muted: #9eb0cb;
        --accent: #67d4ff;
        --accent-2: #87ffb0;
        --danger: #ff8e8e;
      }
      * { box-sizing: border-box; }
      body {
        margin: 0;
        font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
        background:
          radial-gradient(circle at top left, rgba(103, 212, 255, 0.16), transparent 28%),
          radial-gradient(circle at bottom right, rgba(135, 255, 176, 0.12), transparent 22%),
          var(--bg);
        color: var(--text);
      }
      .shell {
        max-width: 1280px;
        margin: 0 auto;
        padding: 24px;
      }
      .hero {
        display: flex;
        justify-content: space-between;
        align-items: end;
        gap: 24px;
        margin-bottom: 24px;
      }
      .tag {
        display: inline-block;
        color: var(--accent);
        border: 1px solid var(--line);
        padding: 6px 10px;
        margin-bottom: 12px;
        letter-spacing: 0.08em;
        text-transform: uppercase;
        font-size: 12px;
      }
      h1 {
        margin: 0;
        font-size: clamp(32px, 6vw, 64px);
        line-height: 0.94;
      }
      .sub {
        margin-top: 10px;
        color: var(--muted);
        max-width: 700px;
      }
      .grid {
        display: grid;
        grid-template-columns: 1.2fr 0.8fr;
        gap: 20px;
      }
      .stack {
        display: grid;
        gap: 20px;
      }
      .panel {
        background: linear-gradient(180deg, rgba(255,255,255,0.03), rgba(255,255,255,0.01)), var(--panel);
        border: 1px solid var(--line);
        border-radius: 18px;
        padding: 18px;
        box-shadow: 0 14px 34px rgba(0,0,0,0.22);
      }
      .panel h2 {
        margin: 0 0 12px;
        font-size: 14px;
        letter-spacing: 0.08em;
        text-transform: uppercase;
        color: var(--accent);
      }
      .stats {
        display: grid;
        grid-template-columns: repeat(4, minmax(0, 1fr));
        gap: 12px;
      }
      .stat {
        background: var(--panel-2);
        border: 1px solid var(--line);
        border-radius: 14px;
        padding: 12px;
      }
      .stat .value {
        font-size: 28px;
        font-weight: 700;
      }
      .stat .label {
        color: var(--muted);
        font-size: 12px;
        margin-top: 6px;
      }
      .session-form {
        display: flex;
        gap: 10px;
        margin: 16px 0 8px;
      }
      .action-row {
        display: flex;
        gap: 10px;
        margin: 12px 0 4px;
        flex-wrap: wrap;
      }
      input, button {
        font: inherit;
      }
      input {
        flex: 1;
        padding: 12px 14px;
        border-radius: 12px;
        border: 1px solid var(--line);
        background: rgba(255,255,255,0.02);
        color: var(--text);
      }
      button {
        padding: 12px 16px;
        border-radius: 12px;
        border: 1px solid rgba(103, 212, 255, 0.35);
        background: linear-gradient(180deg, rgba(103,212,255,0.22), rgba(103,212,255,0.12));
        color: var(--text);
        cursor: pointer;
      }
      button:hover {
        border-color: rgba(103, 212, 255, 0.6);
      }
      .list {
        display: grid;
        gap: 10px;
        margin-top: 14px;
      }
      .card {
        border: 1px solid var(--line);
        border-radius: 14px;
        padding: 12px;
        background: rgba(255,255,255,0.015);
      }
      .card strong {
        display: block;
        margin-bottom: 4px;
      }
      .meta {
        color: var(--muted);
        font-size: 12px;
        word-break: break-word;
      }
      .feed {
        max-height: 560px;
        overflow: auto;
      }
      .event {
        padding: 10px 0;
        border-top: 1px solid rgba(117, 156, 214, 0.12);
      }
      .event:first-child {
        border-top: 0;
        padding-top: 0;
      }
      .event-type {
        color: var(--accent-2);
        font-weight: 700;
      }
      .empty {
        color: var(--muted);
        padding: 12px 0;
      }
      .small {
        font-size: 12px;
        color: var(--muted);
      }
      .status-ok { color: var(--accent-2); }
      .status-error { color: var(--danger); }
      @media (max-width: 980px) {
        .grid { grid-template-columns: 1fr; }
        .stats { grid-template-columns: repeat(2, minmax(0, 1fr)); }
      }
    </style>
  </head>
  <body>
    <div class="shell">
      <div class="hero">
        <div>
          <div class="tag">JIMI SUPERSTAR / House Cockpit</div>
          <h1>House Runtime Live</h1>
          <div class="sub">
            Minimal sovereign cockpit for the JIMI house. Inventory, sessions, and realtime event pulse share one live runtime.
          </div>
        </div>
        <div class="small" id="ws-status">connecting…</div>
      </div>

      <div class="panel" style="margin-bottom: 20px;">
        <h2>Inventory</h2>
        <div class="stats" id="stats"></div>
      </div>

      <div class="grid">
        <div class="stack">
          <div class="panel">
            <h2>Sessions</h2>
            <form class="session-form" id="session-form">
              <input id="session-title" placeholder="Name a new room or mission" value="Boot the next JIMI lane" />
              <button type="submit">Create Session</button>
            </form>
            <div class="list" id="session-list"></div>
            <form class="session-form" id="turn-form">
              <input id="turn-intent" placeholder="Turn intent summary" value="Ground the next implementation slice" />
              <button type="submit">Dispatch Turn</button>
            </form>
            <div class="small" id="turn-status">awaiting routed turn</div>
            <div class="action-row">
              <button id="execute-dispatch">Execute Latest Dispatch</button>
              <div class="small" id="execute-status">awaiting execution</div>
            </div>
            <div class="action-row">
              <button id="execute-dispatch-live">Execute Latest Dispatch Live</button>
              <div class="small" id="execute-live-status">awaiting codex lane</div>
            </div>
            <div class="list" id="turn-list"></div>
            <div class="list" id="dispatch-list"></div>
          </div>

          <div class="panel">
            <h2>Capsule Core</h2>
            <div class="small">Install the first canonical JIMI capsule and bind it to the primary personality slot.</div>
            <div class="action-row">
              <button id="bootstrap-core">Install Core Capsule</button>
              <div class="small" id="bootstrap-status">awaiting bootstrap</div>
            </div>
            <div class="action-row">
              <button id="seal-core">Seal Core Artifact</button>
              <div class="small" id="seal-status">awaiting fieldvault</div>
            </div>
            <div class="action-row">
              <button id="bootstrap-provider">Connect Provider Lane</button>
              <div class="small" id="provider-status">awaiting engine lane</div>
            </div>
            <div class="action-row">
              <button id="bootstrap-provider-anthropic">Connect Anthropic Lane</button>
              <div class="small" id="provider-secondary-status">awaiting secondary lane</div>
            </div>
            <div class="list" id="mandala-list"></div>
            <div class="list" id="capsule-list"></div>
            <div class="list" id="capsule-preview-view"></div>
            <div class="list" id="slot-list"></div>
            <div class="list" id="artifact-list"></div>
            <div class="list" id="provider-list"></div>
          </div>

          <div class="panel">
            <h2>Macro Status</h2>
            <div class="small">Panoramic RETROBUILDER and L1GHT view for the whole project.</div>
            <div class="list" id="macro-status-view"></div>
            <div class="list" id="macro-menu-view"></div>
          </div>

          <div class="panel">
            <h2>Control Plane</h2>
            <div class="small">Integrated navigator for the house as cockpit and configurator.</div>
            <form class="session-form" id="memory-policy-form">
              <input id="hot-limit" type="number" min="1" step="1" placeholder="hot" value="5" />
              <input id="relevant-limit" type="number" min="1" step="1" placeholder="relevant" value="6" />
              <input id="promotion-threshold" type="number" min="0" max="1" step="0.01" placeholder="threshold" value="0.88" />
              <select id="world-state-scope">
                <option value="workspace+process">world: workspace+process</option>
                <option value="workspace">world: workspace</option>
                <option value="process">world: process</option>
              </select>
              <input id="world-entry-limit" type="number" min="1" step="1" placeholder="world entries" value="8" />
              <input id="world-process-limit" type="number" min="0" step="1" placeholder="world processes" value="8" />
              <button type="submit">Update Memory Policy</button>
            </form>
            <div class="action-row">
              <label class="small"><input id="stable-promotion-toggle" type="checkbox" checked /> stable promotion</label>
              <label class="small"><input id="fieldvault-sealing-toggle" type="checkbox" checked /> fieldvault sealing</label>
              <label class="small"><input id="world-state-toggle" type="checkbox" checked /> world state</label>
              <div class="small" id="memory-policy-status">awaiting mandala policy edit</div>
            </div>
            <div class="list" id="control-plane-view"></div>
          </div>

          <div class="panel">
            <h2>Provider Readiness</h2>
            <div class="small">Credential and adapter readiness transplanted from Maestro Auth Truth.</div>
            <div class="list" id="provider-readiness-view"></div>
          </div>

          <div class="panel">
            <h2>Transcript Distiller</h2>
            <div class="small">Background worker for transcript-to-capsule distillation.</div>
            <div class="list" id="distiller-status-view"></div>
          </div>

          <div class="panel">
            <h2>Security Surface</h2>
            <div class="small">Active policy, sealing posture, and capability baseline for the house.</div>
            <div class="list" id="security-status-view"></div>
          </div>

          <div class="panel">
            <h2>Capsule Trust Surface</h2>
            <div class="small">Trust posture and package provenance inside the sovereign capsule house.</div>
            <div class="list" id="capsule-trust-view"></div>
          </div>

          <div class="panel">
            <h2>Marketplace Shell</h2>
            <div class="small">Local capsule catalog for discovery, trust comparison, and install intent.</div>
            <div class="action-row">
              <button onclick="importDemoCapsule()">Import Demo Capsule</button>
            </div>
            <div class="list" id="marketplace-view"></div>
          </div>

          <div class="panel">
            <h2>Approval Surface</h2>
            <div class="small">Human interruptability and live execution guardrails for the house.</div>
            <div class="list" id="approvals-view"></div>
          </div>

          <div class="panel">
            <h2>World State Console</h2>
            <div class="small">Bounded host-state slices for on-demand computer context.</div>
            <div class="list" id="world-state-view"></div>
          </div>

          <div class="panel">
            <h2>Autonomy Surface</h2>
            <div class="small">House self-propulsion after the mission is defined.</div>
            <div class="action-row">
              <button id="toggle-autonomy">Toggle Autonomy</button>
              <div class="small" id="autonomy-status-line">awaiting autonomy state</div>
            </div>
            <div class="list" id="autonomy-status-view"></div>
          </div>

          <div class="panel">
            <h2>Capsule Memory</h2>
            <div class="small" id="context-status">awaiting context packet</div>
            <form class="session-form" id="memory-search-form">
              <select id="memory-search-scope">
                <option value="session">session</option>
                <option value="room">room</option>
                <option value="house" selected>house</option>
              </select>
              <input id="memory-search-query" placeholder="search memory..." />
              <button type="submit">Search Memory</button>
            </form>
            <div class="small" id="memory-search-status">awaiting memory search</div>
            <div class="list" id="memory-capsule-list"></div>
            <div class="list" id="memory-search-results"></div>
            <div class="list" id="memory-relevance-list"></div>
            <div class="list" id="room-memory-relevance-list"></div>
            <div class="list" id="global-memory-relevance-list"></div>
            <div class="list" id="summary-list"></div>
            <div class="list" id="bridge-list"></div>
            <div class="list" id="trigger-list"></div>
            <div class="list" id="promotion-list"></div>
            <div class="list" id="context-packet-view"></div>
          </div>
        </div>

        <div class="panel">
          <h2>Event Pulse</h2>
          <div class="feed" id="event-feed"></div>
        </div>
      </div>
    </div>

    <script>
      const statsEl = document.getElementById('stats');
      const sessionListEl = document.getElementById('session-list');
      const eventFeedEl = document.getElementById('event-feed');
      const wsStatusEl = document.getElementById('ws-status');
      const sessionFormEl = document.getElementById('session-form');
      const sessionTitleEl = document.getElementById('session-title');
      const turnFormEl = document.getElementById('turn-form');
      const turnIntentEl = document.getElementById('turn-intent');
      const turnStatusEl = document.getElementById('turn-status');
      const executeDispatchEl = document.getElementById('execute-dispatch');
      const executeStatusEl = document.getElementById('execute-status');
      const executeDispatchLiveEl = document.getElementById('execute-dispatch-live');
      const executeLiveStatusEl = document.getElementById('execute-live-status');
      const turnListEl = document.getElementById('turn-list');
      const dispatchListEl = document.getElementById('dispatch-list');
      const mandalaListEl = document.getElementById('mandala-list');
      const capsuleListEl = document.getElementById('capsule-list');
      const capsulePreviewViewEl = document.getElementById('capsule-preview-view');
      const slotListEl = document.getElementById('slot-list');
      const artifactListEl = document.getElementById('artifact-list');
      const providerListEl = document.getElementById('provider-list');
      const bootstrapButtonEl = document.getElementById('bootstrap-core');
      const bootstrapStatusEl = document.getElementById('bootstrap-status');
      const sealButtonEl = document.getElementById('seal-core');
      const sealStatusEl = document.getElementById('seal-status');
      const providerButtonEl = document.getElementById('bootstrap-provider');
      const providerStatusEl = document.getElementById('provider-status');
      const providerSecondaryButtonEl = document.getElementById('bootstrap-provider-anthropic');
      const providerSecondaryStatusEl = document.getElementById('provider-secondary-status');
      const contextStatusEl = document.getElementById('context-status');
      const memorySearchFormEl = document.getElementById('memory-search-form');
      const memorySearchScopeEl = document.getElementById('memory-search-scope');
      const memorySearchQueryEl = document.getElementById('memory-search-query');
      const memorySearchStatusEl = document.getElementById('memory-search-status');
      const memoryCapsuleListEl = document.getElementById('memory-capsule-list');
      const memorySearchResultsEl = document.getElementById('memory-search-results');
      const memoryRelevanceListEl = document.getElementById('memory-relevance-list');
      const roomMemoryRelevanceListEl = document.getElementById('room-memory-relevance-list');
      const globalMemoryRelevanceListEl = document.getElementById('global-memory-relevance-list');
      const summaryListEl = document.getElementById('summary-list');
      const bridgeListEl = document.getElementById('bridge-list');
      const triggerListEl = document.getElementById('trigger-list');
      const promotionListEl = document.getElementById('promotion-list');
      const contextPacketViewEl = document.getElementById('context-packet-view');
      const macroStatusViewEl = document.getElementById('macro-status-view');
      const macroMenuViewEl = document.getElementById('macro-menu-view');
      const controlPlaneViewEl = document.getElementById('control-plane-view');
      const providerReadinessViewEl = document.getElementById('provider-readiness-view');
      const distillerStatusViewEl = document.getElementById('distiller-status-view');
      const securityStatusViewEl = document.getElementById('security-status-view');
      const capsuleTrustViewEl = document.getElementById('capsule-trust-view');
      const marketplaceViewEl = document.getElementById('marketplace-view');
      const approvalsViewEl = document.getElementById('approvals-view');
      const worldStateViewEl = document.getElementById('world-state-view');
      const autonomyStatusViewEl = document.getElementById('autonomy-status-view');
      const autonomyStatusLineEl = document.getElementById('autonomy-status-line');
      const toggleAutonomyEl = document.getElementById('toggle-autonomy');
      const memoryPolicyFormEl = document.getElementById('memory-policy-form');
      const hotLimitEl = document.getElementById('hot-limit');
      const relevantLimitEl = document.getElementById('relevant-limit');
      const promotionThresholdEl = document.getElementById('promotion-threshold');
      const worldStateScopeEl = document.getElementById('world-state-scope');
      const worldEntryLimitEl = document.getElementById('world-entry-limit');
      const worldProcessLimitEl = document.getElementById('world-process-limit');
      const stablePromotionToggleEl = document.getElementById('stable-promotion-toggle');
      const fieldvaultSealingToggleEl = document.getElementById('fieldvault-sealing-toggle');
      const worldStateToggleEl = document.getElementById('world-state-toggle');
      const memoryPolicyStatusEl = document.getElementById('memory-policy-status');

      const state = {
        sessions: [],
        turns: [],
        dispatches: [],
        memoryCapsules: [],
        summaries: [],
        bridges: [],
        triggers: [],
        promotions: [],
        contextPacket: null,
        events: [],
        mandalas: [],
        capsules: [],
        capsulePackages: [],
        capsulePreview: null,
        slots: [],
        artifacts: [],
        providers: [],
        macroStatus: null,
        controlPlane: null,
        providerReadiness: null,
        distillerStatus: null,
        securityStatus: null,
        capsuleTrustStatus: null,
        marketplaceStatus: null,
        approvals: null,
        worldState: null,
        autonomyStatus: null,
        memorySearch: null
      };

      function renderInventory(data) {
        const inventory = data.inventory || {};
        const items = [
          ['sessions', inventory.sessions ?? 0],
          ['events', inventory.events ?? 0],
          ['mandalas', inventory.mandalas ?? 0],
          ['capsules', inventory.capsules ?? 0],
          ['capsule packages', inventory.capsule_packages ?? 0],
          ['slots', inventory.slots ?? 0],
          ['fieldvault', inventory.fieldvault_artifacts ?? 0],
          ['providers', inventory.provider_lanes ?? 0],
          ['dispatches', inventory.turn_dispatches ?? 0],
          ['summaries', inventory.summary_checkpoints ?? 0],
          ['bridges', inventory.memory_bridges ?? 0],
          ['triggers', inventory.resynthesis_triggers ?? 0],
          ['promotions', inventory.memory_promotions ?? 0],
          ['approvals', inventory.approval_requests ?? 0],
          ['world nodes', inventory.world_state_nodes ?? 0],
          ['world deltas', inventory.world_state_deltas ?? 0],
        ];
        statsEl.innerHTML = items.map(([label, value]) => `
          <div class="stat">
            <div class="value">${value}</div>
            <div class="label">${label}</div>
          </div>
        `).join('');
      }

      function renderBar(percent) {
        const filled = Math.max(0, Math.min(10, Math.round((percent || 0) / 10)));
        return '[' + '█'.repeat(filled) + '░'.repeat(10 - filled) + ']';
      }

      function renderMacroStatus() {
        if (!state.macroStatus) {
          macroStatusViewEl.innerHTML = '<div class="empty">Macro panorama not loaded yet.</div>';
          macroMenuViewEl.innerHTML = '<div class="empty">Macro menu not loaded yet.</div>';
          return;
        }
        const macro = state.macroStatus;
        const axes = [macro.retrobuilder, macro.l1ght, macro.project, ...(macro.subsystems || [])];
        macroStatusViewEl.innerHTML = axes.map(axis => `
          <div class="card">
            <strong>${escapeHtml(axis.label)}</strong>
            <div class="meta">${escapeHtml(renderBar(axis.percent))} ${escapeHtml(axis.percent)}%</div>
            <div class="meta">${escapeHtml(axis.phase)} / ${escapeHtml(axis.summary)}</div>
            <div class="meta">steps remaining: ${escapeHtml(axis.steps_remaining ?? 0)}</div>
          </div>
        `).join('') + `
          <div class="card">
            <strong>Step Estimates</strong>
            <div class="meta">phase steps remaining: ${escapeHtml(macro.current_phase_steps_remaining ?? 0)}</div>
            <div class="meta">project steps remaining: ${escapeHtml(macro.project_steps_remaining ?? 0)}</div>
          </div>
          <div class="card">
            <strong>Done</strong>
            ${(macro.done || []).map(item => `<div class="meta">- ${escapeHtml(item)}</div>`).join('')}
          </div>
          <div class="card">
            <strong>Doing</strong>
            ${(macro.doing || []).map(item => `<div class="meta">- ${escapeHtml(item)}</div>`).join('')}
          </div>
          <div class="card">
            <strong>Next</strong>
            ${(macro.next || []).map(item => `<div class="meta">- ${escapeHtml(item)}</div>`).join('')}
          </div>
          <div class="card">
            <strong>Later</strong>
            ${(macro.later || []).map(item => `<div class="meta">- ${escapeHtml(item)}</div>`).join('')}
          </div>
        `;
        macroMenuViewEl.innerHTML = (macro.menu || []).map(item => `
          <div class="card">
            <strong>[${escapeHtml(item.key)}] ${escapeHtml(item.prompt)}</strong>
          </div>
        `).join('');
      }

      function renderControlPlane() {
        if (!state.controlPlane) {
          controlPlaneViewEl.innerHTML = '<div class="empty">Control plane map not loaded yet.</div>';
          return;
        }
        controlPlaneViewEl.innerHTML = (state.controlPlane.sections || []).map(section => `
          <div class="card">
            <strong>${escapeHtml(section.label)}</strong>
            <div class="meta">status: ${escapeHtml(section.status)}</div>
            <div class="meta">priority: ${escapeHtml(section.priority)}</div>
            <div class="meta">${escapeHtml(section.summary)}</div>
          </div>
        `).join('');
      }

      function renderProviderReadiness() {
        if (!state.providerReadiness) {
          providerReadinessViewEl.innerHTML = '<div class="empty">Provider readiness not loaded yet.</div>';
          return;
        }
        providerReadinessViewEl.innerHTML = (state.providerReadiness.providers || []).map(provider => `
          <div class="card">
            <strong>${escapeHtml(provider.provider)}</strong>
            <div class="meta">ready: ${escapeHtml(provider.ready ? 'yes' : 'no')}</div>
            <div class="meta">source: ${escapeHtml(provider.source)}</div>
            <div class="meta">${escapeHtml(provider.details)}</div>
          </div>
        `).join('');
      }

      function renderDistillerStatus() {
        if (!state.distillerStatus) {
          distillerStatusViewEl.innerHTML = '<div class="empty">Distiller status not loaded yet.</div>';
          return;
        }
        const status = state.distillerStatus;
        distillerStatusViewEl.innerHTML = `
          <div class="card">
            <strong>Transcript Distiller</strong>
            <div class="meta">running: ${escapeHtml(status.running ? 'yes' : 'no')}</div>
            <div class="meta">pending: ${escapeHtml(status.pending)}</div>
            <div class="meta">processed: ${escapeHtml(status.processed)}</div>
            <div class="meta">failed: ${escapeHtml(status.failed)}</div>
            <div class="meta">last error: ${escapeHtml(status.last_error || 'none')}</div>
          </div>
        `;
      }

      function renderSecurityStatus() {
        if (!state.securityStatus) {
          securityStatusViewEl.innerHTML = '<div class="empty">Security surface not loaded yet.</div>';
          return;
        }
        const security = state.securityStatus;
        securityStatusViewEl.innerHTML = `
          <div class="card">
            <strong>Policy Baseline</strong>
            <div class="meta">active mandala: ${escapeHtml(security.active_mandala_id || 'none')}</div>
            <div class="meta">preferred provider: ${escapeHtml(security.preferred_provider || 'none')}</div>
            <div class="meta">preferred model: ${escapeHtml(security.preferred_model || 'none')}</div>
            <div class="meta">fieldvault sealing: ${escapeHtml(security.sealing_enabled ? 'on' : 'off')}</div>
            <div class="meta">seal classes: ${escapeHtml((security.seal_privacy_classes || []).join(' | ') || 'none')}</div>
            <div class="meta">declared capabilities: ${escapeHtml(security.capability_declared)}</div>
            <div class="meta">required capabilities: ${escapeHtml(security.capability_required)}</div>
            <div class="meta">optional capabilities: ${escapeHtml(security.capability_optional)}</div>
            <div class="meta">skill packs: ${escapeHtml(security.skill_packs)}</div>
            <div class="meta">sacred shards: ${escapeHtml(security.sacred_shards)}</div>
            <div class="meta">sealed artifacts: ${escapeHtml(security.artifacts_total)}</div>
            <div class="meta">artifact levels: ${escapeHtml((security.artifact_seal_levels || []).join(' | ') || 'none')}</div>
          </div>
        `;
      }

      function renderCapsuleTrustStatus() {
        if (!state.capsuleTrustStatus) {
          capsuleTrustViewEl.innerHTML = '<div class="empty">Capsule trust surface not loaded yet.</div>';
          return;
        }
        const trust = state.capsuleTrustStatus;
        const trustLevels = trust.trust_levels || [];
        const packages = trust.packages || [];
        capsuleTrustViewEl.innerHTML = `
          <div class="card">
            <strong>Capsule Trust</strong>
            <div class="meta">total packages: ${escapeHtml(trust.total_packages ?? 0)}</div>
            <div class="meta">internal packages: ${escapeHtml(trust.internal_packages ?? 0)}</div>
            <div class="meta">external packages: ${escapeHtml(trust.external_packages ?? 0)}</div>
            <div class="meta">trust levels: ${escapeHtml(trustLevels.map(level => `${level.trust_level}:${level.packages}`).join(' | ') || 'none')}</div>
            <div class="meta">packages: ${escapeHtml(packages.map(pkg => `${pkg.display_name || pkg.package_id}(${pkg.trust_level})`).join(' | ') || 'none')}</div>
          </div>
        `;
      }

      function renderMarketplaceStatus() {
        if (!state.marketplaceStatus) {
          marketplaceViewEl.innerHTML = '<div class="empty">Marketplace shell not loaded yet.</div>';
          return;
        }
        const marketplace = state.marketplaceStatus;
        const entries = marketplace.entries || [];
        marketplaceViewEl.innerHTML = `
          <div class="card">
            <strong>Marketplace Shell</strong>
            <div class="meta">entries: ${escapeHtml(marketplace.total_entries ?? 0)}</div>
            <div class="meta">installable: ${escapeHtml(marketplace.installable_entries ?? 0)}</div>
            <div class="meta">trusted: ${escapeHtml(marketplace.trusted_entries ?? 0)}</div>
          </div>
          ${entries.length ? entries.map(entry => `
            <div class="card">
              <strong>${escapeHtml(entry.display_name || entry.package_id)}</strong>
              <div class="meta">package: ${escapeHtml(entry.package_id)}</div>
              <div class="meta">creator: ${escapeHtml(entry.creator)}</div>
              <div class="meta">source: ${escapeHtml(entry.source_origin)}</div>
              <div class="meta">trust: ${escapeHtml(entry.trust_level)}</div>
              <div class="meta">install status: ${escapeHtml(entry.install_status)}</div>
              <div class="actions">
                <button onclick="previewCapsulePackage('${escapeHtml(entry.package_id)}')">Preview</button>
                <button onclick="requestCapsuleInstall('${escapeHtml(entry.package_id)}')">Install</button>
                <button onclick="activateCapsulePackage('${escapeHtml(entry.package_id)}')">Activate</button>
                <button onclick="exportCapsulePackage('${escapeHtml(entry.package_id)}')">Export</button>
              </div>
            </div>
          `).join('') : '<div class="empty">No marketplace entries yet.</div>'}
        `;
      }

      function renderApprovals() {
        if (!state.approvals) {
          approvalsViewEl.innerHTML = '<div class="empty">Approval surface not loaded yet.</div>';
          return;
        }
        const status = state.approvals;
        const approvals = status.approvals || [];
        approvalsViewEl.innerHTML = `
          <div class="card">
            <strong>Approval Status</strong>
            <div class="meta">pending: ${escapeHtml(status.pending ?? 0)}</div>
            <div class="meta">granted: ${escapeHtml(status.granted ?? 0)}</div>
            <div class="meta">denied: ${escapeHtml(status.denied ?? 0)}</div>
          </div>
          ${approvals.length ? approvals.map(approval => `
            <div class="card">
              <strong>${escapeHtml(approval.action)}</strong>
              <div class="meta">status: ${escapeHtml(approval.status)}</div>
              <div class="meta">dispatch: ${escapeHtml(approval.dispatch_id)}</div>
              <div class="meta">provider lane: ${escapeHtml(approval.provider_lane_id)}</div>
              <div class="meta">reason: ${escapeHtml(approval.reason)}</div>
              ${approval.status === 'pending' ? `<div class="action-row">
                <button onclick="grantApproval('${escapeHtml(approval.approval_request_id)}')">Grant</button>
                <button onclick="denyApproval('${escapeHtml(approval.approval_request_id)}')">Deny</button>
              </div>` : ''}
            </div>
          `).join('') : '<div class="empty">No approvals requested yet.</div>'}
        `;
      }

      function renderWorldState() {
        if (!state.worldState) {
          worldStateViewEl.innerHTML = '<div class="empty">World state not loaded yet.</div>';
          return;
        }
        const world = state.worldState;
        const slice = world.slice || {};
        worldStateViewEl.innerHTML = `
          <div class="card">
            <strong>World State Slice</strong>
            <div class="meta">active mandala: ${escapeHtml(world.active_mandala_id || 'none')}</div>
            <div class="meta">preferred scope: ${escapeHtml(world.preferred_scope || 'none')}</div>
            <div class="meta">lookup sources: ${escapeHtml((world.lookup_sources || []).join(' | ') || 'none')}</div>
            <div class="meta">world state enabled: ${escapeHtml(slice.scope === 'disabled' ? 'no' : 'yes')}</div>
            <div class="meta">workspace root: ${escapeHtml(slice.workspace_root || 'none')}</div>
            <div class="meta">workspace health: ${escapeHtml(slice.workspace_health || 'unknown')}</div>
            <div class="meta">git dirty: ${escapeHtml(slice.git_dirty ? 'yes' : 'no')}</div>
            <div class="meta">cache state: ${escapeHtml(slice.cache_state || 'cold')}</div>
            <div class="meta">indexed nodes: ${escapeHtml(slice.indexed_nodes ?? 0)}</div>
            <div class="meta">relations: ${escapeHtml(slice.relation_count ?? 0)}</div>
            <div class="meta">workspace entries: ${escapeHtml(slice.workspace_entry_count ?? 0)}</div>
            <div class="meta">processes shown: ${escapeHtml((slice.running_processes || []).length)}</div>
          </div>
          <div class="card">
            <strong>Workspace Snapshot</strong>
            ${((slice.workspace_entries || []).map(entry => `<div class="meta">- [${escapeHtml(entry.kind)}:${escapeHtml(entry.size_bytes)}] ${escapeHtml(entry.path)}</div>`).join('')) || '<div class="meta">- none</div>'}
          </div>
          <div class="card">
            <strong>Changed Files</strong>
            ${((slice.changed_files || []).map(path => `<div class="meta">- ${escapeHtml(path)}</div>`).join('')) || '<div class="meta">- none</div>'}
          </div>
          <div class="card">
            <strong>Recent Deltas</strong>
            ${((slice.recent_deltas || []).map(delta => `<div class="meta">- ${escapeHtml(delta)}</div>`).join('')) || '<div class="meta">- none</div>'}
          </div>
          <div class="card">
            <strong>Recent Relations</strong>
            ${((slice.recent_relations || []).map(relation => `<div class="meta">- ${escapeHtml(relation)}</div>`).join('')) || '<div class="meta">- none</div>'}
          </div>
          <div class="card">
            <strong>Process Snapshot</strong>
            ${((slice.running_processes || []).map(process => `<div class="meta">- pid=${escapeHtml(process.pid)} ${escapeHtml(process.command)}</div>`).join('')) || '<div class="meta">- none</div>'}
          </div>
        `;
      }

      function renderAutonomyStatus() {
        if (!state.autonomyStatus) {
          autonomyStatusViewEl.innerHTML = '<div class="empty">Autonomy surface not loaded yet.</div>';
          return;
        }
        const autonomy = state.autonomyStatus;
        autonomyStatusLineEl.textContent = autonomy.enabled
          ? `autonomy enabled / ${autonomy.mission_phase || 'steady'}`
          : 'autonomy paused';
        autonomyStatusViewEl.innerHTML = `
          <div class="card">
            <strong>Autonomy Loop</strong>
            <div class="meta">enabled: ${escapeHtml(autonomy.enabled ? 'yes' : 'no')}</div>
            <div class="meta">running: ${escapeHtml(autonomy.running ? 'yes' : 'no')}</div>
            <div class="meta">cycles: ${escapeHtml(autonomy.cycles)}</div>
            <div class="meta">completed dispatches: ${escapeHtml(autonomy.completed_dispatches)}</div>
            <div class="meta">transition count: ${escapeHtml(autonomy.transition_count ?? 0)}</div>
            <div class="meta">last transition: ${escapeHtml(autonomy.last_transition || 'none')}</div>
            <div class="meta">mission phase: ${escapeHtml(autonomy.mission_phase || 'steady')}</div>
            <div class="meta">current goal: ${escapeHtml(autonomy.current_goal || 'none')}</div>
            <div class="meta">next actions: ${escapeHtml((autonomy.next_actions || []).join(' | ') || 'none')}</div>
            <div class="meta">queued dispatches: ${escapeHtml(autonomy.queued_dispatch_count ?? 0)}</div>
            <div class="meta">awaiting approvals: ${escapeHtml(autonomy.awaiting_approval_count ?? 0)}</div>
            <div class="meta">recommended next step: ${escapeHtml(autonomy.recommended_next_step || 'none')}</div>
            <div class="meta">transition guidance: ${escapeHtml(autonomy.transition_guidance || 'none')}</div>
            <div class="meta">last dispatch: ${escapeHtml(autonomy.last_dispatch_id || 'none')}</div>
            <div class="meta">last intent: ${escapeHtml(autonomy.last_intent_summary || 'none')}</div>
            <div class="meta">selection reason: ${escapeHtml(autonomy.last_selection_reason || 'none')}</div>
            <div class="meta">last error: ${escapeHtml(autonomy.last_error || 'none')}</div>
          </div>
        `;
      }

      function renderSessions() {
        if (!state.sessions.length) {
          sessionListEl.innerHTML = '<div class="empty">No sessions yet. Create the next JIMI room.</div>';
          return;
        }
        sessionListEl.innerHTML = state.sessions.map(session => `
          <div class="card">
            <strong>${escapeHtml(session.title)}</strong>
            <div class="meta">session: ${escapeHtml(session.session_id?.[0] || session.session_id || '')}</div>
            <div class="meta">room: ${escapeHtml(session.room_id || 'default')}</div>
            <div class="meta">active lane: ${escapeHtml(session.active_lane_id?.[0] || session.active_lane_id || '')}</div>
            <div class="meta">state: ${escapeHtml(session.state)}</div>
          </div>
        `).join('');
      }

      function renderEvents() {
        if (!state.events.length) {
          eventFeedEl.innerHTML = '<div class="empty">Waiting for the first house pulse.</div>';
          return;
        }
        const events = [...state.events].slice(-40).reverse();
        eventFeedEl.innerHTML = events.map(event => `
          <div class="event">
            <div class="event-type">${escapeHtml(event.event_type)}</div>
            <div class="meta">event: ${escapeHtml(event.event_id)}</div>
            <div class="meta">session: ${escapeHtml(event.session_id || 'none')}</div>
            <div class="meta">actor: ${escapeHtml(event.actor?.actor_id || 'unknown')}</div>
          </div>
        `).join('');
      }

      function renderTurns() {
        if (!state.turns.length) {
          turnListEl.innerHTML = '<div class="empty">No turns dispatched yet.</div>';
          return;
        }
        turnListEl.innerHTML = state.turns.map(turn => `
          <div class="card">
            <strong>${escapeHtml(turn.intent_mode)}</strong>
            <div class="meta">turn: ${escapeHtml(turn.turn_id?.[0] || turn.turn_id || '')}</div>
            <div class="meta">session: ${escapeHtml(turn.session_id?.[0] || turn.session_id || '')}</div>
            <div class="meta">lane: ${escapeHtml(turn.lane_id?.[0] || turn.lane_id || '')}</div>
            <div class="meta">state: ${escapeHtml(turn.state)}</div>
          </div>
        `).join('');
      }

      function renderDispatches() {
        if (!state.dispatches.length) {
          dispatchListEl.innerHTML = '<div class="empty">No provider dispatches yet.</div>';
          return;
        }
        dispatchListEl.innerHTML = state.dispatches.map(dispatch => `
          <div class="card">
            <strong>${escapeHtml(dispatch.intent_summary)}</strong>
            <div class="meta">dispatch: ${escapeHtml(dispatch.dispatch_id)}</div>
            <div class="meta">provider lane: ${escapeHtml(dispatch.provider_lane_id)}</div>
            <div class="meta">turn: ${escapeHtml(dispatch.turn_id?.[0] || dispatch.turn_id || '')}</div>
            <div class="meta">status: ${escapeHtml(dispatch.status)}</div>
          </div>
        `).join('');
      }

      function renderMandalas() {
        if (!state.mandalas.length) {
          mandalaListEl.innerHTML = '<div class="empty">No mandalas installed yet.</div>';
          syncMemoryPolicyForm(null);
          return;
        }
        mandalaListEl.innerHTML = state.mandalas.map(mandala => `
          <div class="card">
            <strong>${escapeHtml(mandala.self_section?.id || 'unknown')}</strong>
            <div class="meta">role: ${escapeHtml(mandala.self_section?.role || 'unknown')}</div>
            <div class="meta">soul: ${escapeHtml(mandala.self_section?.template_soul || 'unknown')}</div>
            <div class="meta">provider: ${escapeHtml(mandala.execution_policy?.preferred_provider || 'unknown')}</div>
            <div class="meta">goal: ${escapeHtml(mandala.active_snapshot?.current_goal || 'none')}</div>
          </div>
        `).join('');
        syncMemoryPolicyForm(state.mandalas[0]);
      }

      function syncMemoryPolicyForm(mandala) {
        const policy = mandala?.memory_policy;
        if (!policy) {
          memoryPolicyStatusEl.textContent = 'install a mandala to edit policy';
          return;
        }
        hotLimitEl.value = policy.hot_context_limit ?? 5;
        relevantLimitEl.value = policy.relevant_context_limit ?? 5;
        promotionThresholdEl.value = policy.promotion_confidence_threshold ?? 0.9;
        worldStateScopeEl.value = policy.world_state_scope || 'workspace+process';
        worldEntryLimitEl.value = policy.world_state_entry_limit ?? 8;
        worldProcessLimitEl.value = policy.world_state_process_limit ?? 8;
        stablePromotionToggleEl.checked = !!policy.promote_to_stable_memory;
        fieldvaultSealingToggleEl.checked = !!policy.allow_fieldvault_sealing;
        worldStateToggleEl.checked = !!policy.allow_world_state;
        memoryPolicyStatusEl.textContent = `${mandala.self_section?.id || 'mandala'} policy loaded`;
      }

      function renderCapsules() {
        if (!state.capsules.length) {
          capsuleListEl.innerHTML = '<div class="empty">No capsules installed yet.</div>';
          capsulePreviewViewEl.innerHTML = '<div class="empty">No capsule install preview loaded yet.</div>';
        } else {
          const packageMap = new Map((state.capsulePackages || []).map(pkg => [pkg.capsule_id, pkg]));
          capsuleListEl.innerHTML = state.capsules.map(capsule => {
            const pkg = packageMap.get(capsule.capsule_id);
            return `
            <div class="card">
              <strong>${escapeHtml(capsule.capsule_id)}</strong>
              <div class="meta">mandala: ${escapeHtml(capsule.mandala_id)}</div>
              <div class="meta">version: ${escapeHtml(capsule.version)}</div>
              <div class="meta">source: ${escapeHtml(capsule.install_source)}</div>
              <div class="meta">package: ${escapeHtml(pkg?.package_id || 'none')}</div>
              <div class="meta">trust: ${escapeHtml(pkg?.trust_level || 'unknown')}</div>
              <div class="meta">creator: ${escapeHtml(pkg?.creator || 'unknown')}</div>
              <div class="actions">
                <button onclick="previewCapsulePackage('${escapeHtml(pkg?.package_id || '')}')">Preview</button>
                <button onclick="classifyCapsuleTrust('${escapeHtml(pkg?.package_id || '')}', '${escapeHtml(pkg?.trust_level || 'internal')}')">Trust</button>
              </div>
            </div>
          `}).join('');
        }
        renderCapsulePreview();
      }

      function renderCapsulePreview() {
        if (!state.capsulePreview) {
          capsulePreviewViewEl.innerHTML = '<div class="empty">No capsule install preview loaded yet.</div>';
          return;
        }
        const preview = state.capsulePreview;
        capsulePreviewViewEl.innerHTML = `
          <div class="card">
            <strong>Install Preview / ${escapeHtml(preview.package?.display_name || preview.package?.package_id || 'unknown')}</strong>
            <div class="meta">package: ${escapeHtml(preview.package?.package_id || 'unknown')}</div>
            <div class="meta">trust: ${escapeHtml(preview.package?.trust_level || 'unknown')}</div>
            <div class="meta">install status: ${escapeHtml(preview.package?.install_status || 'unknown')}</div>
            <div class="meta">slot recommendation: ${escapeHtml(preview.slot_recommendation || 'none')}</div>
            <div class="meta">fieldvault requirements: ${escapeHtml(preview.fieldvault_requirements ?? 0)}</div>
            <div class="meta">required caps: ${escapeHtml((preview.required_capabilities || []).join(' | ') || 'none')}</div>
            <div class="meta">optional caps: ${escapeHtml((preview.optional_capabilities || []).join(' | ') || 'none')}</div>
            <div class="meta">${escapeHtml(preview.install_preview_summary || 'no summary')}</div>
            <div class="actions">
              <button onclick="requestCapsuleInstall('${escapeHtml(preview.package?.package_id || '')}')">Request Install</button>
            </div>
          </div>
        `;
      }

      function renderSlots() {
        if (!state.slots.length) {
          slotListEl.innerHTML = '<div class="empty">No personality slots yet.</div>';
          return;
        }
        slotListEl.innerHTML = state.slots.map(slot => `
          <div class="card">
            <strong>${escapeHtml(slot.label)}</strong>
            <div class="meta">slot: ${escapeHtml(slot.slot_id)}</div>
            <div class="meta">state: ${escapeHtml(slot.state)}</div>
            <div class="meta">capsule: ${escapeHtml(slot.capsule_id || 'none')}</div>
            <div class="meta">mandala: ${escapeHtml(slot.active_mandala_id || 'none')}</div>
          </div>
        `).join('');
      }

      function renderArtifacts() {
        if (!state.artifacts.length) {
          artifactListEl.innerHTML = '<div class="empty">No fieldvault artifacts sealed yet.</div>';
          return;
        }
        artifactListEl.innerHTML = state.artifacts.map(artifact => `
          <div class="card">
            <strong>${escapeHtml(artifact.artifact_id)}</strong>
            <div class="meta">seal: ${escapeHtml(artifact.seal_level)}</div>
            <div class="meta">capsule: ${escapeHtml(artifact.capsule_id || 'none')}</div>
            <div class="meta">slot: ${escapeHtml(artifact.slot_id || 'none')}</div>
            <div class="meta">path: ${escapeHtml(artifact.fld_path)}</div>
          </div>
        `).join('');
      }

      function renderProviders() {
        if (!state.providers.length) {
          providerListEl.innerHTML = '<div class="empty">No provider lanes connected yet.</div>';
          return;
        }
        const readinessMap = new Map((state.providerReadiness?.providers || []).map(provider => [provider.provider, provider]));
        providerListEl.innerHTML = state.providers.map(provider => `
          <div class="card">
            <strong>${escapeHtml(provider.provider_lane_id)}</strong>
            <div class="meta">provider: ${escapeHtml(provider.provider)}</div>
            <div class="meta">model: ${escapeHtml(provider.model)}</div>
            <div class="meta">routing: ${escapeHtml(provider.routing_mode)}</div>
            <div class="meta">fallback group: ${escapeHtml(provider.fallback_group || 'none')}</div>
            <div class="meta">priority: ${escapeHtml(provider.priority)}</div>
            <div class="meta">status: ${escapeHtml(provider.status)}</div>
            <div class="meta">ready: ${escapeHtml(readinessMap.get(provider.provider)?.ready ? 'yes' : 'no')}</div>
            <div class="meta">auth source: ${escapeHtml(readinessMap.get(provider.provider)?.source || 'unknown')}</div>
            <div class="meta">readiness details: ${escapeHtml(readinessMap.get(provider.provider)?.details || 'none')}</div>
            <div class="meta">fallback rail: ${escapeHtml(`${provider.fallback_group || 'ungrouped'} / p${provider.priority}`)}</div>
            <div class="actions"><button onclick="editProviderLane('${escapeHtml(provider.provider_lane_id)}', '${escapeHtml(provider.routing_mode)}', '${escapeHtml(provider.fallback_group || '')}', '${escapeHtml(provider.priority)}')">Edit lane</button></div>
          </div>
        `).join('');
      }

      function renderMemoryCapsules() {
        if (!state.memoryCapsules.length) {
          memoryCapsuleListEl.innerHTML = '<div class="empty">No memory capsules captured yet.</div>';
          return;
        }
        memoryCapsuleListEl.innerHTML = state.memoryCapsules.slice(-6).reverse().map(capsule => `
          <div class="card">
            <strong>${escapeHtml(capsule.role)} / ${escapeHtml(capsule.band)}</strong>
            <div class="meta">capsule: ${escapeHtml(capsule.memory_capsule_id)}</div>
            <div class="meta">relevance: ${escapeHtml(capsule.relevance_score)}</div>
            <div class="meta">confidence: ${escapeHtml(capsule.confidence_level)}</div>
            <div class="meta">${escapeHtml(capsule.content)}</div>
          </div>
        `).join('');
      }

      function renderMemorySearch() {
        if (!state.memorySearch) {
          memorySearchResultsEl.innerHTML = '<div class="empty">No local search results yet.</div>';
          return;
        }
        const search = state.memorySearch;
        memorySearchStatusEl.textContent = `${search.scope} / ${search.query || 'empty query'} / ${search.results.length} results`;
        memorySearchResultsEl.innerHTML = search.results.length
          ? search.results.map(capsule => `
            <div class="card">
              <strong>search / ${escapeHtml(capsule.band)}</strong>
              <div class="meta">scope: ${escapeHtml(search.scope)}</div>
              <div class="meta">room: ${escapeHtml(capsule.room_id)}</div>
              <div class="meta">session: ${escapeHtml(capsule.session_id)}</div>
              <div class="meta">score: ${escapeHtml(capsule.relevance_score)}</div>
              <div class="meta">${escapeHtml(capsule.content)}</div>
            </div>
          `).join('')
          : '<div class="empty">No local search results yet.</div>';
      }

      function renderContextPacket() {
        if (!state.contextPacket) {
          contextPacketViewEl.innerHTML = '<div class="empty">No context packet loaded yet.</div>';
          memorySearchResultsEl.innerHTML = '<div class="empty">No local search results yet.</div>';
          memoryRelevanceListEl.innerHTML = '<div class="empty">No ranked memory candidates yet.</div>';
          roomMemoryRelevanceListEl.innerHTML = '<div class="empty">No room-scoped memory candidates yet.</div>';
          globalMemoryRelevanceListEl.innerHTML = '<div class="empty">No house-scoped memory candidates yet.</div>';
          bridgeListEl.innerHTML = '<div class="empty">No memory bridges yet.</div>';
          triggerListEl.innerHTML = '<div class="empty">No re-synthesis triggers yet.</div>';
          promotionListEl.innerHTML = '<div class="empty">No memory promotions yet.</div>';
          return;
        }
        const packet = state.contextPacket;
        const relevant = packet.relevant_capsules || [];
        const roomRelevant = packet.room_relevant_capsules || [];
        const globalRelevant = packet.global_relevant_capsules || [];
        const bridges = packet.memory_bridges || [];
        const triggers = packet.resynthesis_triggers || [];
        const promotions = packet.memory_promotions || [];
        memoryRelevanceListEl.innerHTML = relevant.length
          ? relevant.map(capsule => `
            <div class="card">
              <strong>relevant / ${escapeHtml(capsule.band)}</strong>
              <div class="meta">score: ${escapeHtml(capsule.relevance_score)}</div>
              <div class="meta">confidence: ${escapeHtml(capsule.confidence_level)}</div>
              <div class="meta">${escapeHtml(capsule.content)}</div>
            </div>
          `).join('')
          : '<div class="empty">No ranked memory candidates yet.</div>';
        roomMemoryRelevanceListEl.innerHTML = roomRelevant.length
          ? roomRelevant.map(capsule => `
            <div class="card">
              <strong>room relevant / ${escapeHtml(capsule.band)}</strong>
              <div class="meta">room: ${escapeHtml(capsule.room_id)}</div>
              <div class="meta">score: ${escapeHtml(capsule.relevance_score)}</div>
              <div class="meta">${escapeHtml(capsule.content)}</div>
            </div>
          `).join('')
          : '<div class="empty">No room-scoped memory candidates yet.</div>';
        globalMemoryRelevanceListEl.innerHTML = globalRelevant.length
          ? globalRelevant.map(capsule => `
            <div class="card">
              <strong>house relevant / ${escapeHtml(capsule.band)}</strong>
              <div class="meta">room: ${escapeHtml(capsule.room_id)}</div>
              <div class="meta">session: ${escapeHtml(capsule.session_id)}</div>
              <div class="meta">score: ${escapeHtml(capsule.relevance_score)}</div>
              <div class="meta">${escapeHtml(capsule.content)}</div>
            </div>
          `).join('')
          : '<div class="empty">No house-scoped memory candidates yet.</div>';
        bridgeListEl.innerHTML = bridges.length
          ? bridges.map(bridge => `
            <div class="card">
              <strong>bridge / ${escapeHtml(bridge.bridge_kind)}</strong>
              <div class="meta">strength: ${escapeHtml(bridge.strength)}</div>
              <div class="meta">from: ${escapeHtml(bridge.from_capsule_id)}</div>
              <div class="meta">to: ${escapeHtml(bridge.to_capsule_id)}</div>
            </div>
          `).join('')
          : '<div class="empty">No memory bridges yet.</div>';
        triggerListEl.innerHTML = triggers.length
          ? triggers.map(trigger => `
            <div class="card">
              <strong>trigger / ${escapeHtml(trigger.trigger_kind)}</strong>
              <div class="meta">confidence: ${escapeHtml(trigger.confidence_level)}</div>
              <div class="meta">${escapeHtml(trigger.summary)}</div>
            </div>
          `).join('')
          : '<div class="empty">No re-synthesis triggers yet.</div>';
        promotionListEl.innerHTML = promotions.length
          ? promotions.map(promotion => `
            <div class="card">
              <strong>promotion / ${escapeHtml(promotion.target_plane)}</strong>
              <div class="meta">confidence: ${escapeHtml(promotion.confidence_level)}</div>
              <div class="meta">capsule: ${escapeHtml(promotion.memory_capsule_id)}</div>
              <div class="meta">${escapeHtml(promotion.promoted_value)}</div>
            </div>
          `).join('')
          : '<div class="empty">No memory promotions yet.</div>';
        contextPacketViewEl.innerHTML = `
          <div class="card">
            <strong>Context Packet / ${escapeHtml(packet.session_id)}</strong>
            <div class="meta">room: ${escapeHtml(packet.room_id || 'default')}</div>
            <div class="meta">active mandala: ${escapeHtml(packet.active_mandala_id || 'none')}</div>
            <div class="meta">providers: ${escapeHtml((packet.providers || []).join(', ') || 'none')}</div>
            <div class="meta">query seed: ${escapeHtml(packet.query_seed || 'none')}</div>
            <div class="meta">hot capsules: ${escapeHtml((packet.hot_capsules || []).length)}</div>
            <div class="meta">relevant capsules: ${escapeHtml((packet.relevant_capsules || []).length)}</div>
            <div class="meta">room capsules: ${escapeHtml((packet.room_relevant_capsules || []).length)}</div>
            <div class="meta">house capsules: ${escapeHtml((packet.global_relevant_capsules || []).length)}</div>
            <div class="meta">summaries: ${escapeHtml((packet.summary_checkpoints || []).length)}</div>
            <div class="meta">bridges: ${escapeHtml((packet.memory_bridges || []).length)}</div>
            <div class="meta">triggers: ${escapeHtml((packet.resynthesis_triggers || []).length)}</div>
            <div class="meta">promotions: ${escapeHtml((packet.memory_promotions || []).length)}</div>
            <div class="meta">world scope: ${escapeHtml(packet.world_state_slice?.scope || 'none')}</div>
            <div class="meta">workspace health: ${escapeHtml(packet.world_state_slice?.workspace_health || 'unknown')}</div>
            <div class="meta">git dirty: ${escapeHtml(packet.world_state_slice?.git_dirty ? 'yes' : 'no')}</div>
            <div class="meta">world cache state: ${escapeHtml(packet.world_state_slice?.cache_state || 'cold')}</div>
            <div class="meta">indexed world nodes: ${escapeHtml(packet.world_state_slice?.indexed_nodes ?? 0)}</div>
            <div class="meta">world entries: ${escapeHtml(packet.world_state_slice?.workspace_entry_count ?? 0)}</div>
            <div class="meta">changed files: ${escapeHtml((packet.world_state_slice?.changed_files || []).length)}</div>
            <div class="meta">recent deltas: ${escapeHtml((packet.world_state_slice?.recent_deltas || []).length)}</div>
            <div class="meta">world processes: ${escapeHtml((packet.world_state_slice?.running_processes || []).length)}</div>
            <div class="meta">boot include: ${escapeHtml((packet.memory_policy?.boot_include || []).join(' | ') || 'none')}</div>
            <div class="meta">lookup sources: ${escapeHtml((packet.memory_policy?.lookup_sources || []).join(' | ') || 'none')}</div>
            <div class="meta">hot limit: ${escapeHtml(packet.memory_policy?.hot_context_limit ?? 0)}</div>
            <div class="meta">relevant limit: ${escapeHtml(packet.memory_policy?.relevant_context_limit ?? 0)}</div>
            <div class="meta">promotion threshold: ${escapeHtml(packet.memory_policy?.promotion_confidence_threshold ?? 0)}</div>
            <div class="meta">stable promotion: ${escapeHtml(packet.memory_policy?.promote_to_stable_memory ? 'on' : 'off')}</div>
            <div class="meta">fieldvault sealing: ${escapeHtml(packet.memory_policy?.allow_fieldvault_sealing ? 'on' : 'off')}</div>
            <div class="meta">world state: ${escapeHtml(packet.memory_policy?.allow_world_state ? 'on' : 'off')}</div>
            <div class="meta">world scope policy: ${escapeHtml(packet.memory_policy?.world_state_scope || 'none')}</div>
            <div class="meta">world entry limit: ${escapeHtml(packet.memory_policy?.world_state_entry_limit ?? 0)}</div>
            <div class="meta">world process limit: ${escapeHtml(packet.memory_policy?.world_state_process_limit ?? 0)}</div>
            <div class="meta">seal classes: ${escapeHtml((packet.memory_policy?.seal_privacy_classes || []).join(' | ') || 'none')}</div>
            <div class="meta">stable rules: ${escapeHtml((packet.stable_memory?.learned_rules || []).length)}</div>
            <div class="meta">goal: ${escapeHtml(packet.active_snapshot?.current_goal || 'none')}</div>
            <div class="meta">blockers: ${escapeHtml((packet.active_snapshot?.blockers || []).join(' | ') || 'none')}</div>
            <div class="meta">next actions: ${escapeHtml((packet.active_snapshot?.next_actions || []).join(' | ') || 'none')}</div>
          </div>
        `;
      }

      function renderSummaries() {
        if (!state.summaries.length) {
          summaryListEl.innerHTML = '<div class="empty">No summary checkpoints yet.</div>';
          return;
        }
        summaryListEl.innerHTML = state.summaries.slice(-4).reverse().map(summary => `
          <div class="card">
            <strong>${escapeHtml(summary.source_band)} summary</strong>
            <div class="meta">checkpoint: ${escapeHtml(summary.summary_checkpoint_id)}</div>
            <div class="meta">confidence: ${escapeHtml(summary.confidence_level)}</div>
            <div class="meta">capsules: ${escapeHtml(summary.source_capsule_count)}</div>
            <div class="meta">compression: ${escapeHtml(summary.digest_text_length)}/${escapeHtml(summary.source_text_length)} (${escapeHtml(summary.compression_ratio)})</div>
            <div class="meta">digest: ${escapeHtml(summary.semantic_digest)}</div>
          </div>
        `).join('');
      }

      async function refreshInventory() {
        const res = await fetch('/inventory');
        const data = await res.json();
        renderInventory(data);
      }

      async function refreshMacroStatus() {
        const res = await fetch('/status/macro');
        state.macroStatus = await res.json();
        renderMacroStatus();
      }

      async function refreshControlPlane() {
        const res = await fetch('/status/control-plane');
        state.controlPlane = await res.json();
        renderControlPlane();
      }

      async function refreshProviderReadiness() {
        const res = await fetch('/status/provider-readiness');
        state.providerReadiness = await res.json();
        renderProviderReadiness();
      }

      async function refreshDistillerStatus() {
        const res = await fetch('/status/distiller');
        state.distillerStatus = await res.json();
        renderDistillerStatus();
      }

      async function refreshSecurityStatus() {
        const res = await fetch('/status/security');
        state.securityStatus = await res.json();
        renderSecurityStatus();
      }

      async function refreshCapsuleTrustStatus() {
        const res = await fetch('/status/capsule-trust');
        state.capsuleTrustStatus = await res.json();
        renderCapsuleTrustStatus();
      }

      async function refreshMarketplaceStatus() {
        const res = await fetch('/status/marketplace');
        state.marketplaceStatus = await res.json();
        renderMarketplaceStatus();
      }

      async function refreshApprovals() {
        const res = await fetch('/status/approvals');
        state.approvals = await res.json();
        renderApprovals();
      }

      async function refreshWorldState() {
        const res = await fetch('/status/world-state');
        state.worldState = await res.json();
        renderWorldState();
      }

      async function refreshAutonomyStatus() {
        const res = await fetch('/status/autonomy');
        state.autonomyStatus = await res.json();
        renderAutonomyStatus();
      }

      async function toggleAutonomy() {
        const enabled = !(state.autonomyStatus?.enabled ?? true);
        const res = await fetch('/autonomy', {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({ enabled })
        });
        if (!res.ok) {
          throw new Error('failed to update autonomy state');
        }
        state.autonomyStatus = await res.json();
        renderAutonomyStatus();
      }

      async function grantApproval(id) {
        const res = await fetch(`/approvals/${encodeURIComponent(id)}/grant`, { method: 'POST' });
        if (!res.ok) {
          throw new Error('failed to grant approval');
        }
        await Promise.all([
          refreshCapsulePackages(),
          refreshCapsuleTrustStatus(),
          refreshMarketplaceStatus(),
          refreshInventory(),
          refreshMacroStatus(),
          refreshControlPlane(),
          refreshSecurityStatus(),
          refreshApprovals(),
          refreshAutonomyStatus(),
          refreshTurns(),
          refreshDispatches(),
          refreshEvents()
        ]);
      }

      async function denyApproval(id) {
        const res = await fetch(`/approvals/${encodeURIComponent(id)}/deny`, { method: 'POST' });
        if (!res.ok) {
          throw new Error('failed to deny approval');
        }
        await Promise.all([
          refreshCapsulePackages(),
          refreshCapsuleTrustStatus(),
          refreshMarketplaceStatus(),
          refreshInventory(),
          refreshMacroStatus(),
          refreshControlPlane(),
          refreshSecurityStatus(),
          refreshApprovals(),
          refreshAutonomyStatus(),
          refreshTurns(),
          refreshDispatches(),
          refreshEvents()
        ]);
      }

      async function searchMemory() {
        const query = memorySearchQueryEl.value.trim();
        const scope = memorySearchScopeEl.value;
        const sessionId = state.sessions[0]?.session_id;
        const params = new URLSearchParams({ q: query, scope, limit: '8' });
        if (sessionId) {
          params.set('session_id', sessionId);
        }
        const res = await fetch(`/memory/search?${params.toString()}`);
        state.memorySearch = await res.json();
        renderMemorySearch();
      }

      async function updateMemoryPolicy() {
        const mandala = state.mandalas[0];
        if (!mandala?.self_section?.id) {
          throw new Error('no mandala available to update');
        }
        memoryPolicyStatusEl.textContent = 'updating mandala memory policy…';
        const res = await fetch(`/mandalas/${encodeURIComponent(mandala.self_section.id)}/memory-policy`, {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({
            hot_context_limit: Number(hotLimitEl.value || 5),
            relevant_context_limit: Number(relevantLimitEl.value || 5),
            promotion_confidence_threshold: Number(promotionThresholdEl.value || 0.9),
            promote_to_stable_memory: stablePromotionToggleEl.checked,
            allow_fieldvault_sealing: fieldvaultSealingToggleEl.checked,
            world_state_scope: worldStateScopeEl.value,
            world_state_entry_limit: Number(worldEntryLimitEl.value || 8),
            world_state_process_limit: Number(worldProcessLimitEl.value || 8),
            allow_world_state: worldStateToggleEl.checked
          })
        });
        if (!res.ok) {
          throw new Error('failed to update mandala memory policy');
        }
        const result = await res.json();
        memoryPolicyStatusEl.textContent = `${result.mandala_id} policy updated`;
        await Promise.all([
          refreshMandalas(),
          refreshInventory(),
          refreshMacroStatus(),
          refreshControlPlane(),
          refreshProviderReadiness(),
          refreshSecurityStatus(),
          refreshWorldState(),
          refreshAutonomyStatus(),
          refreshContextPacket(),
          refreshEvents()
        ]);
      }

      async function editProviderLane(providerLaneId, currentRoutingMode, currentFallbackGroup, currentPriority) {
        const routingMode = window.prompt('Routing mode for this lane', currentRoutingMode || 'secondary');
        if (routingMode === null) return;
        const fallbackGroup = window.prompt('Fallback group for this lane (empty = none)', currentFallbackGroup || '');
        if (fallbackGroup === null) return;
        const priorityRaw = window.prompt('Priority for this lane (higher = preferred)', String(currentPriority || 0));
        if (priorityRaw === null) return;
        const priority = Number(priorityRaw);
        if (!Number.isFinite(priority) || priority < 0) {
          throw new Error('invalid lane priority');
        }
        const res = await fetch(`/providers/${encodeURIComponent(providerLaneId)}`, {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({
            routing_mode: routingMode.trim() || 'secondary',
            fallback_group: fallbackGroup.trim(),
            priority: Math.floor(priority)
          })
        });
        if (!res.ok) {
          throw new Error('failed to update provider lane');
        }
        await Promise.all([
          refreshProviders(),
          refreshMacroStatus(),
          refreshControlPlane(),
          refreshProviderReadiness(),
          refreshAutonomyStatus(),
          refreshEvents()
        ]);
      }

      async function refreshSessions() {
        const res = await fetch('/sessions');
        state.sessions = await res.json();
        renderSessions();
      }

      async function refreshTurns() {
        const res = await fetch('/turns');
        state.turns = await res.json();
        renderTurns();
      }

      async function refreshDispatches() {
        const res = await fetch('/dispatches');
        state.dispatches = await res.json();
        renderDispatches();
      }

      async function refreshMandalas() {
        const res = await fetch('/mandalas');
        state.mandalas = await res.json();
        renderMandalas();
      }

      async function refreshCapsules() {
        const res = await fetch('/capsules');
        state.capsules = await res.json();
        renderCapsules();
      }

      async function refreshCapsulePackages() {
        const res = await fetch('/capsule-packages');
        state.capsulePackages = await res.json();
        renderCapsules();
      }

      async function previewCapsulePackage(packageId) {
        if (!packageId) {
          throw new Error('no package id available for preview');
        }
        const res = await fetch(`/capsule-packages/${encodeURIComponent(packageId)}/preview`);
        if (!res.ok) {
          throw new Error('failed to load capsule install preview');
        }
        state.capsulePreview = await res.json();
        renderCapsulePreview();
      }

      async function classifyCapsuleTrust(packageId, currentTrustLevel) {
        if (!packageId) {
          throw new Error('no package id available for trust update');
        }
        const trustLevel = window.prompt('Trust level for this package', currentTrustLevel || 'internal');
        if (trustLevel === null) return;
        const res = await fetch(`/capsule-packages/${encodeURIComponent(packageId)}/trust`, {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({ trust_level: trustLevel.trim() || 'internal' })
        });
        if (!res.ok) {
          throw new Error('failed to classify capsule trust');
        }
        await Promise.all([
          refreshCapsulePackages(),
          refreshCapsuleTrustStatus(),
          refreshInventory(),
          refreshMacroStatus(),
          refreshControlPlane(),
          refreshEvents()
        ]);
        await previewCapsulePackage(packageId);
      }

      async function requestCapsuleInstall(packageId) {
        if (!packageId) {
          throw new Error('no package id available for install request');
        }
        const res = await fetch(`/capsule-packages/${encodeURIComponent(packageId)}/install-request`, {
          method: 'POST'
        });
        if (!res.ok) {
          throw new Error('failed to request capsule install');
        }
        await Promise.all([
          refreshCapsulePackages(),
          refreshCapsuleTrustStatus(),
          refreshMarketplaceStatus(),
          refreshApprovals(),
          refreshInventory(),
          refreshMacroStatus(),
          refreshEvents()
        ]);
        await previewCapsulePackage(packageId);
      }

      async function importDemoCapsule() {
        const res = await fetch('/bootstrap/import-demo-capsule', { method: 'POST' });
        if (!res.ok) {
          throw new Error('failed to import demo capsule');
        }
        const result = await res.json();
        await Promise.all([
          refreshCapsules(),
          refreshCapsulePackages(),
          refreshCapsuleTrustStatus(),
          refreshMarketplaceStatus(),
          refreshInventory(),
          refreshMacroStatus(),
          refreshEvents()
        ]);
        await previewCapsulePackage(result.package.package_id);
      }

      async function exportCapsulePackage(packageId) {
        if (!packageId) {
          throw new Error('no package id available for export');
        }
        const res = await fetch(`/capsule-packages/${encodeURIComponent(packageId)}/export`, {
          method: 'POST'
        });
        if (!res.ok) {
          throw new Error('failed to export capsule package');
        }
        await Promise.all([
          refreshMarketplaceStatus(),
          refreshInventory(),
          refreshMacroStatus(),
          refreshEvents()
        ]);
      }

      async function activateCapsulePackage(packageId) {
        if (!packageId) {
          throw new Error('no package id available for activation');
        }
        const res = await fetch(`/capsule-packages/${encodeURIComponent(packageId)}/activate`, {
          method: 'POST'
        });
        if (!res.ok) {
          throw new Error('failed to activate capsule package');
        }
        await Promise.all([
          refreshCapsulePackages(),
          refreshCapsuleTrustStatus(),
          refreshMarketplaceStatus(),
          refreshSlots(),
          refreshInventory(),
          refreshMacroStatus(),
          refreshSecurityStatus(),
          refreshEvents()
        ]);
        await previewCapsulePackage(packageId);
      }

      async function refreshSlots() {
        const res = await fetch('/slots');
        state.slots = await res.json();
        renderSlots();
      }

      async function refreshArtifacts() {
        const res = await fetch('/artifacts');
        state.artifacts = await res.json();
        renderArtifacts();
      }

      async function refreshProviders() {
        const res = await fetch('/providers');
        state.providers = await res.json();
        renderProviders();
      }

      async function refreshMemoryCapsules() {
        const res = await fetch('/memory/capsules');
        state.memoryCapsules = await res.json();
        renderMemoryCapsules();
      }

      async function refreshSummaries() {
        const res = await fetch('/memory/summaries');
        state.summaries = await res.json();
        renderSummaries();
      }

      async function refreshPromotions() {
        const res = await fetch('/memory/promotions');
        state.promotions = await res.json();
      }

      async function refreshContextPacket() {
        const session = state.sessions[0];
        if (!session) {
          state.contextPacket = null;
          renderContextPacket();
          contextStatusEl.textContent = 'create a session to assemble context';
          return;
        }
        const sessionId = session.session_id?.[0] || session.session_id;
        const res = await fetch(`/context-packet/${sessionId}`);
        state.contextPacket = await res.json();
        renderContextPacket();
        contextStatusEl.textContent = `context ready for ${sessionId}`;
      }

      async function refreshEvents() {
        const res = await fetch('/events');
        state.events = await res.json();
        renderEvents();
      }

      async function createSession(title) {
        const res = await fetch('/sessions', {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({ title })
        });
        if (!res.ok) {
          throw new Error('failed to create session');
        }
        const session = await res.json();
        state.sessions.push(session);
        renderSessions();
        await Promise.all([refreshInventory(), refreshMacroStatus(), refreshControlPlane(), refreshProviderReadiness(), refreshSecurityStatus(), refreshApprovals(), refreshWorldState(), refreshAutonomyStatus(), refreshSummaries(), refreshPromotions(), refreshContextPacket()]);
      }

      async function createTurn(intentSummary) {
        const session = state.sessions[0];
        if (!session) {
          throw new Error('create a session before dispatching a turn');
        }
        const sessionId = session.session_id?.[0] || session.session_id;
        const res = await fetch('/turns', {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({
            session_id: sessionId,
            intent_mode: 'architect',
            intent_summary: intentSummary
          })
        });
        if (!res.ok) {
          throw new Error('failed to create turn');
        }
        const result = await res.json();
        turnStatusEl.textContent = `${result.dispatch.provider_lane_id} -> ${result.turn.intent_mode}`;
        await Promise.all([
          refreshInventory(),
          refreshMacroStatus(),
          refreshControlPlane(),
          refreshProviderReadiness(),
          refreshDistillerStatus(),
          refreshSecurityStatus(),
          refreshCapsuleTrustStatus(),
          refreshMarketplaceStatus(),
          refreshApprovals(),
          refreshWorldState(),
          refreshAutonomyStatus(),
          refreshTurns(),
          refreshDispatches(),
          refreshMemoryCapsules(),
          refreshSummaries(),
          refreshPromotions(),
          refreshContextPacket(),
          refreshEvents()
        ]);
      }

      async function executeLatestDispatch() {
        executeStatusEl.textContent = 'executing latest dispatch…';
        const res = await fetch('/dispatches/execute-latest', { method: 'POST' });
        if (!res.ok) {
          throw new Error('failed to execute latest dispatch');
        }
        const result = await res.json();
        executeStatusEl.textContent = `${result.dispatch.provider_lane_id} -> ${result.turn.state}`;
        await Promise.all([
          refreshInventory(),
          refreshMacroStatus(),
          refreshControlPlane(),
          refreshProviderReadiness(),
          refreshDistillerStatus(),
          refreshSecurityStatus(),
          refreshApprovals(),
          refreshWorldState(),
          refreshAutonomyStatus(),
          refreshTurns(),
          refreshDispatches(),
          refreshMemoryCapsules(),
          refreshSummaries(),
          refreshPromotions(),
          refreshContextPacket(),
          refreshEvents()
        ]);
      }

      async function executeLatestDispatchLive() {
        executeLiveStatusEl.textContent = 'running codex live lane…';
        const res = await fetch('/dispatches/execute-latest-live', { method: 'POST' });
        if (!res.ok) {
          const message = await res.text();
          throw new Error(message || 'failed to execute latest dispatch live');
        }
        const result = await res.json();
        executeLiveStatusEl.textContent = `${result.dispatch.provider_lane_id} -> live completed`;
        await Promise.all([
          refreshInventory(),
          refreshMacroStatus(),
          refreshControlPlane(),
          refreshProviderReadiness(),
          refreshDistillerStatus(),
          refreshSecurityStatus(),
          refreshApprovals(),
          refreshWorldState(),
          refreshAutonomyStatus(),
          refreshTurns(),
          refreshDispatches(),
          refreshMemoryCapsules(),
          refreshSummaries(),
          refreshPromotions(),
          refreshContextPacket(),
          refreshEvents()
        ]);
      }

      async function bootstrapCoreCapsule() {
        bootstrapStatusEl.textContent = 'installing core capsule…';
        const res = await fetch('/bootstrap/core-capsule', { method: 'POST' });
        if (!res.ok) {
          bootstrapStatusEl.textContent = 'bootstrap failed';
          throw new Error('failed to bootstrap core capsule');
        }
        const result = await res.json();
        bootstrapStatusEl.textContent = `${result.slot_id} -> ${result.slot_state}`;
        await Promise.all([
          refreshInventory(),
          refreshMacroStatus(),
          refreshControlPlane(),
          refreshProviderReadiness(),
          refreshDistillerStatus(),
          refreshSecurityStatus(),
          refreshWorldState(),
          refreshAutonomyStatus(),
          refreshMandalas(),
          refreshCapsules(),
          refreshCapsulePackages(),
          previewCapsulePackage(result.package_id),
          refreshSlots(),
          refreshEvents()
        ]);
      }

      async function sealCoreArtifact() {
        sealStatusEl.textContent = 'sealing fieldvault artifact…';
        const res = await fetch('/bootstrap/core-artifact', { method: 'POST' });
        if (!res.ok) {
          sealStatusEl.textContent = 'seal failed';
          throw new Error('failed to seal core artifact');
        }
        const result = await res.json();
        sealStatusEl.textContent = `${result.artifact_id} -> ${result.seal_level}`;
        await Promise.all([refreshInventory(), refreshMacroStatus(), refreshControlPlane(), refreshProviderReadiness(), refreshDistillerStatus(), refreshSecurityStatus(), refreshApprovals(), refreshWorldState(), refreshAutonomyStatus(), refreshArtifacts(), refreshEvents()]);
      }

      async function bootstrapProviderLane() {
        providerStatusEl.textContent = 'connecting provider lane…';
        const res = await fetch('/bootstrap/provider-lane', { method: 'POST' });
        if (!res.ok) {
          providerStatusEl.textContent = 'provider lane failed';
          throw new Error('failed to bootstrap provider lane');
        }
        const result = await res.json();
        providerStatusEl.textContent = `${result.provider} -> ${result.model}`;
        await Promise.all([refreshInventory(), refreshMacroStatus(), refreshControlPlane(), refreshProviderReadiness(), refreshDistillerStatus(), refreshSecurityStatus(), refreshApprovals(), refreshWorldState(), refreshAutonomyStatus(), refreshProviders(), refreshEvents()]);
      }

      async function bootstrapAnthropicLane() {
        providerSecondaryStatusEl.textContent = 'connecting anthropic lane…';
        const res = await fetch('/bootstrap/provider-lane-anthropic', { method: 'POST' });
        if (!res.ok) {
          providerSecondaryStatusEl.textContent = 'anthropic lane failed';
          throw new Error('failed to bootstrap anthropic lane');
        }
        const result = await res.json();
        providerSecondaryStatusEl.textContent = `${result.provider} -> ${result.model}`;
        await Promise.all([refreshInventory(), refreshMacroStatus(), refreshControlPlane(), refreshProviderReadiness(), refreshDistillerStatus(), refreshSecurityStatus(), refreshApprovals(), refreshWorldState(), refreshAutonomyStatus(), refreshProviders(), refreshEvents()]);
      }

      function connectEvents() {
        const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
        const ws = new WebSocket(`${protocol}//${location.host}/ws/events`);
        wsStatusEl.textContent = 'ws connected';
        wsStatusEl.className = 'small status-ok';

        ws.onmessage = (message) => {
          try {
            const event = JSON.parse(message.data);
            state.events.push(event);
            renderEvents();
            if (event.event_type === 'session_created') {
              refreshSessions().catch(console.error);
              refreshInventory().catch(console.error);
              refreshMacroStatus().catch(console.error);
              refreshControlPlane().catch(console.error);
              refreshProviderReadiness().catch(console.error);
              refreshDistillerStatus().catch(console.error);
              refreshSecurityStatus().catch(console.error);
              refreshApprovals().catch(console.error);
              refreshWorldState().catch(console.error);
              refreshAutonomyStatus().catch(console.error);
              refreshContextPacket().catch(console.error);
            } else if (event.event_type === 'turn_started') {
              refreshInventory().catch(console.error);
              refreshMacroStatus().catch(console.error);
              refreshControlPlane().catch(console.error);
              refreshProviderReadiness().catch(console.error);
              refreshDistillerStatus().catch(console.error);
              refreshSecurityStatus().catch(console.error);
              refreshApprovals().catch(console.error);
              refreshWorldState().catch(console.error);
              refreshAutonomyStatus().catch(console.error);
              refreshTurns().catch(console.error);
              refreshMemoryCapsules().catch(console.error);
              refreshSummaries().catch(console.error);
              refreshPromotions().catch(console.error);
              refreshContextPacket().catch(console.error);
            } else if (event.event_type === 'tool_started' || event.event_type === 'message_completed' || event.event_type === 'approval_required' || event.event_type === 'approval_granted' || event.event_type === 'approval_denied') {
              refreshInventory().catch(console.error);
              refreshMacroStatus().catch(console.error);
              refreshControlPlane().catch(console.error);
              refreshProviderReadiness().catch(console.error);
              refreshDistillerStatus().catch(console.error);
              refreshSecurityStatus().catch(console.error);
              refreshApprovals().catch(console.error);
              refreshWorldState().catch(console.error);
              refreshAutonomyStatus().catch(console.error);
              refreshTurns().catch(console.error);
              refreshDispatches().catch(console.error);
              refreshMemoryCapsules().catch(console.error);
              refreshSummaries().catch(console.error);
              refreshPromotions().catch(console.error);
              refreshContextPacket().catch(console.error);
            } else if (['mandala_bound', 'capsule_installed', 'slot_activated'].includes(event.event_type)) {
              refreshInventory().catch(console.error);
              refreshMacroStatus().catch(console.error);
              refreshControlPlane().catch(console.error);
              refreshProviderReadiness().catch(console.error);
              refreshDistillerStatus().catch(console.error);
              refreshSecurityStatus().catch(console.error);
              refreshApprovals().catch(console.error);
              refreshWorldState().catch(console.error);
              refreshAutonomyStatus().catch(console.error);
              refreshMandalas().catch(console.error);
              refreshCapsules().catch(console.error);
              refreshCapsulePackages().catch(console.error);
              refreshSlots().catch(console.error);
            } else if (event.event_type === 'mandala_policy_updated') {
              refreshInventory().catch(console.error);
              refreshMacroStatus().catch(console.error);
              refreshControlPlane().catch(console.error);
              refreshProviderReadiness().catch(console.error);
              refreshDistillerStatus().catch(console.error);
              refreshSecurityStatus().catch(console.error);
              refreshApprovals().catch(console.error);
              refreshWorldState().catch(console.error);
              refreshAutonomyStatus().catch(console.error);
              refreshMandalas().catch(console.error);
              refreshContextPacket().catch(console.error);
            } else if (event.event_type === 'artifact_created') {
              refreshInventory().catch(console.error);
              refreshMacroStatus().catch(console.error);
              refreshControlPlane().catch(console.error);
              refreshProviderReadiness().catch(console.error);
              refreshDistillerStatus().catch(console.error);
              refreshSecurityStatus().catch(console.error);
              refreshApprovals().catch(console.error);
              refreshWorldState().catch(console.error);
              refreshAutonomyStatus().catch(console.error);
              refreshArtifacts().catch(console.error);
            } else if (event.event_type === 'engine_selected' || event.event_type === 'engine_degraded') {
              refreshInventory().catch(console.error);
              refreshMacroStatus().catch(console.error);
              refreshControlPlane().catch(console.error);
              refreshProviderReadiness().catch(console.error);
              refreshDistillerStatus().catch(console.error);
              refreshSecurityStatus().catch(console.error);
              refreshApprovals().catch(console.error);
              refreshWorldState().catch(console.error);
              refreshAutonomyStatus().catch(console.error);
              refreshProviders().catch(console.error);
              refreshDispatches().catch(console.error);
            } else if (event.event_type === 'truth_fusion_updated') {
              refreshInventory().catch(console.error);
              refreshMacroStatus().catch(console.error);
              refreshControlPlane().catch(console.error);
              refreshProviderReadiness().catch(console.error);
              refreshDistillerStatus().catch(console.error);
              refreshSecurityStatus().catch(console.error);
              refreshApprovals().catch(console.error);
              refreshWorldState().catch(console.error);
              refreshAutonomyStatus().catch(console.error);
              refreshMemoryCapsules().catch(console.error);
              refreshContextPacket().catch(console.error);
            }
          } catch (error) {
            console.error(error);
          }
        };

        ws.onclose = () => {
          wsStatusEl.textContent = 'ws disconnected';
          wsStatusEl.className = 'small status-error';
          setTimeout(connectEvents, 1200);
        };

        ws.onerror = () => {
          wsStatusEl.textContent = 'ws error';
          wsStatusEl.className = 'small status-error';
        };
      }

      sessionFormEl.addEventListener('submit', async (event) => {
        event.preventDefault();
        const title = sessionTitleEl.value.trim();
        if (!title) return;
        try {
          await createSession(title);
          sessionTitleEl.value = '';
        } catch (error) {
          console.error(error);
        }
      });

      turnFormEl.addEventListener('submit', async (event) => {
        event.preventDefault();
        const intentSummary = turnIntentEl.value.trim();
        if (!intentSummary) return;
        try {
          await createTurn(intentSummary);
          turnIntentEl.value = '';
        } catch (error) {
          console.error(error);
          turnStatusEl.textContent = error.message;
        }
      });

      executeDispatchEl.addEventListener('click', async () => {
        try {
          await executeLatestDispatch();
        } catch (error) {
          console.error(error);
          executeStatusEl.textContent = error.message;
        }
      });

      executeDispatchLiveEl.addEventListener('click', async () => {
        try {
          await executeLatestDispatchLive();
        } catch (error) {
          console.error(error);
          executeLiveStatusEl.textContent = error.message;
        }
      });

      bootstrapButtonEl.addEventListener('click', async () => {
        try {
          await bootstrapCoreCapsule();
        } catch (error) {
          console.error(error);
        }
      });

      sealButtonEl.addEventListener('click', async () => {
        try {
          await sealCoreArtifact();
        } catch (error) {
          console.error(error);
        }
      });

      providerButtonEl.addEventListener('click', async () => {
        try {
          await bootstrapProviderLane();
        } catch (error) {
          console.error(error);
          providerStatusEl.textContent = error.message;
        }
      });

      providerSecondaryButtonEl.addEventListener('click', async () => {
        try {
          await bootstrapAnthropicLane();
        } catch (error) {
          console.error(error);
          providerSecondaryStatusEl.textContent = error.message;
        }
      });

      memoryPolicyFormEl.addEventListener('submit', async (event) => {
        event.preventDefault();
        try {
          await updateMemoryPolicy();
        } catch (error) {
          console.error(error);
          memoryPolicyStatusEl.textContent = error.message;
        }
      });

      memorySearchFormEl.addEventListener('submit', async (event) => {
        event.preventDefault();
        try {
          await searchMemory();
        } catch (error) {
          console.error(error);
          memorySearchStatusEl.textContent = error.message;
        }
      });

      toggleAutonomyEl.addEventListener('click', async () => {
        try {
          await toggleAutonomy();
        } catch (error) {
          console.error(error);
          autonomyStatusLineEl.textContent = error.message;
        }
      });

      function escapeHtml(value) {
        return String(value ?? '')
          .replaceAll('&', '&amp;')
          .replaceAll('<', '&lt;')
          .replaceAll('>', '&gt;')
          .replaceAll('"', '&quot;')
          .replaceAll("'", '&#39;');
      }

      renderMemorySearch();

      Promise.all([
        refreshInventory(),
        refreshMacroStatus(),
        refreshControlPlane(),
        refreshProviderReadiness(),
        refreshDistillerStatus(),
        refreshSecurityStatus(),
        refreshApprovals(),
        refreshWorldState(),
        refreshAutonomyStatus(),
        refreshSessions(),
        refreshTurns(),
        refreshDispatches(),
        refreshMandalas(),
        refreshCapsules(),
        refreshCapsulePackages(),
        refreshSlots(),
        refreshArtifacts(),
        refreshProviders(),
        refreshMemoryCapsules(),
        refreshSummaries(),
        refreshPromotions(),
        refreshContextPacket(),
        refreshEvents()
      ])
        .then(connectEvents)
        .catch(console.error);
    </script>
  </body>
</html>
"#;
