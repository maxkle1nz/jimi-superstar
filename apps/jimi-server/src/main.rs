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
    total_imports: usize,
    total_exports: usize,
    entries: Vec<CapsulePackageRecord>,
    recent_imports: Vec<CapsuleImportRecord>,
    recent_exports: Vec<CapsuleExportRecord>,
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
struct CapsuleDeactivationResponse {
    package_id: String,
    capsule_id: String,
    slot_id: String,
    slot_state: SlotBindingState,
}

#[derive(Debug, Serialize)]
struct CapsuleUninstallResponse {
    package_id: String,
    capsule_id: String,
    install_status: String,
    had_active_slot: bool,
}

#[derive(Debug, Serialize)]
struct CapsuleUpgradeResponse {
    package_id: String,
    capsule_id: String,
    version: u32,
    package_digest: String,
    install_status: String,
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
    let db_path = std::env::var("ANOMALY_DB_PATH")
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
            "/capsule-packages/{package_id}/deactivate",
            post(deactivate_capsule_package),
        )
        .route(
            "/capsule-packages/{package_id}/uninstall",
            post(uninstall_capsule_package),
        )
        .route(
            "/capsule-packages/{package_id}/upgrade",
            post(upgrade_capsule_package),
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
        .route("/ws/chat", get(ws_chat))
        .route(
            "/bootstrap/provider-lane-copilot",
            post(bootstrap_copilot_lane),
        )
        .route("/inventory", get(inventory))
        .route("/graph/ingest", post(graph_ingest))
        .route("/graph/activate", post(graph_activate))
        .route("/graph/snapshot", post(graph_snapshot))
        .route("/graph/topology", get(graph_topology))
        .route("/graph/layers", get(graph_layers))
        .route("/graph/stats", get(graph_stats))
        .route("/graph/predict", post(graph_predict))
        .route("/graph/impact", post(graph_impact))
        .route("/graph/flow", get(graph_flow))
        .route("/graph/twins", get(graph_twins))
        .route("/graph/epidemic", post(graph_epidemic))
        .route("/graph/trust", get(graph_trust))
        .route("/graph/refactor", post(graph_refactor))
        .route("/graph/resonance", post(graph_resonance))
        .route("/graph/tremor", get(graph_tremor))
        .route("/graph/taint", post(graph_taint))
        .route("/graph/antibody-scan", post(graph_antibody_scan))
        .route("/graph/plasticity", post(graph_plasticity))
        .route("/graph/xlr", post(graph_xlr))
        .with_state(state);

    let addr: SocketAddr = std::env::var("ANOMALY_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".into())
        .parse()
        .expect("invalid ANOMALY_ADDR");

    println!("ANOMALY server listening on http://{}", addr);
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

async fn cockpit(State(state): State<AppState>) -> Html<String> {
    let cockpit_path = state.house_root.join("apps/jimi-server/static/cockpit.html");
    match fs::read_to_string(&cockpit_path) {
        Ok(html) => Html(html),
        Err(_) => Html("<h1>cockpit.html not found</h1>".into()),
    }
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
            "You are the live provider lane for ANOMALY.\n",
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
        "ANOMALY routed '{}' through {} and completed the first simulated execution loop.",
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

async fn deactivate_capsule_package(
    Path(package_id): Path<String>,
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<CapsuleDeactivationResponse>), (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let package = runtime
        .capsule_packages
        .get(&package_id)
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?
        .clone();

    let slot = runtime
        .slots
        .all()
        .into_iter()
        .find(|slot| slot.capsule_id.as_deref() == Some(package.capsule_id.as_str()))
        .map(|slot| slot.slot_id.clone())
        .ok_or_else(|| {
            (
                StatusCode::CONFLICT,
                format!("package {} is not active in any slot", package.package_id),
            )
        })?;

    runtime
        .slots
        .unbind(&slot)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    let updated_package = runtime
        .capsule_packages
        .update_install_status(&package_id, "available")
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?;
    let slot_state = runtime
        .slots
        .get(&slot)
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?
        .state;

    runtime.events.append(
        ActorRef {
            actor_type: "operator".into(),
            actor_id: "cockpit.marketplace".into(),
        },
        SubjectRef {
            subject_type: "slot".into(),
            subject_id: slot.clone(),
        },
        EventType::SlotActivated,
        None,
        None,
        None,
        serde_json::json!({
            "package_id": updated_package.package_id,
            "capsule_id": updated_package.capsule_id,
            "slot_id": slot,
            "slot_state": slot_state,
            "change_kind": "deactivated",
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
        Json(CapsuleDeactivationResponse {
            package_id: updated_package.package_id,
            capsule_id: updated_package.capsule_id,
            slot_id: slot,
            slot_state,
        }),
    ))
}

async fn uninstall_capsule_package(
    Path(package_id): Path<String>,
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<CapsuleUninstallResponse>), (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let package = runtime
        .capsule_packages
        .get(&package_id)
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?
        .clone();

    let active_slot = runtime
        .slots
        .all()
        .into_iter()
        .find(|slot| slot.capsule_id.as_deref() == Some(package.capsule_id.as_str()))
        .map(|slot| slot.slot_id.clone());

    if let Some(slot_id) = &active_slot {
        runtime
            .slots
            .unbind(slot_id)
            .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    }

    let updated_package = runtime
        .capsule_packages
        .update_install_status(&package_id, "uninstalled")
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?;

    runtime.events.append(
        ActorRef {
            actor_type: "operator".into(),
            actor_id: "cockpit.marketplace".into(),
        },
        SubjectRef {
            subject_type: "capsule_package".into(),
            subject_id: updated_package.package_id.clone(),
        },
        EventType::CapsuleInstalled,
        None,
        None,
        None,
        serde_json::json!({
            "package_id": updated_package.package_id,
            "capsule_id": updated_package.capsule_id,
            "install_status": updated_package.install_status,
            "change_kind": "uninstalled",
            "had_active_slot": active_slot.is_some(),
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
        Json(CapsuleUninstallResponse {
            package_id: updated_package.package_id,
            capsule_id: updated_package.capsule_id,
            install_status: updated_package.install_status,
            had_active_slot: active_slot.is_some(),
        }),
    ))
}

async fn upgrade_capsule_package(
    Path(package_id): Path<String>,
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<CapsuleUpgradeResponse>), (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let package = runtime
        .capsule_packages
        .get(&package_id)
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?
        .clone();

    let next_version = package.version + 1;
    let next_digest = format!("{}.v{}", package.package_digest, next_version);
    let next_status = if package.install_status == "installed" {
        "update_available"
    } else {
        "upgrade_ready"
    };
    let updated_package = runtime
        .capsule_packages
        .upgrade(&package_id, next_version, next_digest.clone(), next_status)
        .map_err(|error| (StatusCode::NOT_FOUND, error.to_string()))?;

    runtime.events.append(
        ActorRef {
            actor_type: "operator".into(),
            actor_id: "cockpit.marketplace".into(),
        },
        SubjectRef {
            subject_type: "capsule_package".into(),
            subject_id: updated_package.package_id.clone(),
        },
        EventType::CapsuleMarketplaceListed,
        None,
        None,
        None,
        serde_json::json!({
            "package_id": updated_package.package_id,
            "capsule_id": updated_package.capsule_id,
            "version": updated_package.version,
            "package_digest": updated_package.package_digest,
            "install_status": updated_package.install_status,
            "change_kind": "upgraded",
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
        Json(CapsuleUpgradeResponse {
            package_id: updated_package.package_id,
            capsule_id: updated_package.capsule_id,
            version: updated_package.version,
            package_digest: updated_package.package_digest,
            install_status: updated_package.install_status,
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
    let recent_imports = runtime
        .capsule_imports
        .all()
        .into_iter()
        .rev()
        .take(4)
        .cloned()
        .collect::<Vec<_>>();
    let recent_exports = runtime
        .capsule_exports
        .all()
        .into_iter()
        .rev()
        .take(4)
        .cloned()
        .collect::<Vec<_>>();

    Json(MarketplaceStatusResponse {
        total_entries: entries.len(),
        installable_entries,
        trusted_entries,
        total_imports: runtime.capsule_imports.all().len(),
        total_exports: runtime.capsule_exports.all().len(),
        entries,
        recent_imports,
        recent_exports,
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
            "ANOMALY Core Capsule",
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

async fn ws_chat(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_chat(socket, state))
}

async fn handle_ws_chat(mut socket: WebSocket, state: AppState) {
    // Send a ready signal
    let ready_msg = serde_json::json!({ "type": "ready", "message": "ANOMALY chat online" }).to_string();
    let _ = socket.send(Message::Text(ready_msg.into())).await;

    while let Some(Ok(msg)) = socket.recv().await {
        let user_text = match msg {
            Message::Text(text) => text.to_string(),
            Message::Close(_) => break,
            _ => continue,
        };

        let parsed: serde_json::Value = match serde_json::from_str(&user_text) {
            Ok(v) => v,
            Err(_) => serde_json::json!({ "message": user_text }),
        };
        let user_message = parsed["message"]
            .as_str()
            .unwrap_or(&user_text)
            .to_string();

        if user_message.trim().is_empty() {
            continue;
        }

        // Typing indicator
        let typing_msg = serde_json::json!({ "type": "typing" }).to_string();
        let _ = socket.send(Message::Text(typing_msg.into())).await;

        // --- All mutex work happens in sync blocks, never across await ---

        // 1. Ensure session exists (sync block)
        let session_id = {
            let mut runtime = match state.runtime.lock() {
                Ok(r) => r,
                Err(_) => break,
            };
            let sessions = runtime.sessions.sessions();
            if let Some(session) = sessions.first() {
                session.session_id.clone()
            } else {
                let session = runtime.bootstrap_session(
                    "ANOMALY Chat".to_string(),
                    "anomaly-chat".to_string(),
                );
                let new_event = runtime.events.all().last().cloned();
                let _ = persist_runtime(&state, &runtime);
                if let Some(event) = new_event {
                    let _ = state.events_tx.send(event);
                }
                session.session_id.clone()
            }
        }; // runtime dropped here

        // 2. Store user message (sync block)
        {
            let mut runtime = match state.runtime.lock() {
                Ok(r) => r,
                Err(_) => break,
            };
            let room_id = runtime
                .sessions
                .session(&session_id)
                .map(|s| s.room_id.clone())
                .unwrap_or_else(|_| "anomaly-chat".to_string());
            let lane_id = runtime
                .sessions
                .session(&session_id)
                .ok()
                .map(|s| s.active_lane_id.clone())
                .unwrap_or_else(|| jimi_kernel::LaneId("chat".to_string()));
            runtime.memory_capsules.append(
                session_id.clone(),
                room_id,
                lane_id,
                None,
                "user",
                user_message.clone(),
                None,
                0.95,
                0.95,
                "session_open",
                "hot",
            );
        } // runtime dropped here

        // 3. Build provider prompt (sync block, returns Option)
        let provider_data = {
            let mut runtime = match state.runtime.lock() {
                Ok(r) => r,
                Err(_) => break,
            };
            let provider_candidates = runtime.providers.all();
            let provider_lane = provider_candidates
                .iter()
                .find(|p| p.routing_mode == "primary" && p.status == "connected")
                .or_else(|| provider_candidates.iter().find(|p| p.status == "connected"))
                .cloned()
                .cloned();

            match provider_lane {
                Some(lane) => {
                    let world_state_policy = runtime.active_memory_policy().unwrap_or_default();
                    refresh_world_state_cache(&mut runtime, &state.house_root, &world_state_policy);
                    let packet = assemble_context_packet(&runtime, &session_id, &state.house_root);
                    let prompt = build_provider_prompt(
                        &lane.provider_lane_id,
                        &packet,
                        &user_message,
                    );
                    Some((lane, prompt, state.house_root.clone()))
                }
                None => None,
            }
        }; // runtime dropped here

        // Now we can safely await — no mutex held
        let (provider_lane, provider_prompt, house_root) = match provider_data {
            Some(data) => data,
            None => {
                let err_msg = serde_json::json!({
                    "type": "error",
                    "message": "No provider lane connected. Bootstrap a provider first."
                })
                .to_string();
                let _ = socket.send(Message::Text(err_msg.into())).await;
                continue;
            }
        };

        // 4. Execute provider (blocking, no mutex)
        let adapter = resolve_provider_adapter(&provider_lane);
        eprintln!("  ⛛ chat: calling {} / {} via {}", provider_lane.provider, provider_lane.model, provider_adapter_label(&adapter));
        let lane_for_exec = provider_lane.clone();
        let adapter_for_exec = adapter.clone();
        let prompt_for_exec = provider_prompt.clone();
        let root_for_exec = house_root.clone();

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            tokio::task::spawn_blocking(move || {
                run_provider_adapter(&lane_for_exec, &adapter_for_exec, &root_for_exec, &prompt_for_exec)
            }),
        )
        .await;

        let response_text = match result {
            Ok(Ok(Ok(text))) => {
                eprintln!("  ✔ chat: got {} bytes from provider", text.len());
                text
            }
            Ok(Ok(Err(err))) => {
                eprintln!("  ✘ chat: provider error ({}): {}", err.class(), err.message());
                let err_msg = serde_json::json!({
                    "type": "error",
                    "message": format!("Provider error ({}): {}", err.class(), err.message())
                })
                .to_string();
                let _ = socket.send(Message::Text(err_msg.into())).await;
                continue;
            }
            Ok(Err(err)) => {
                eprintln!("  ✘ chat: task join error: {}", err);
                let err_msg = serde_json::json!({
                    "type": "error",
                    "message": format!("Execution error: {}", err)
                })
                .to_string();
                let _ = socket.send(Message::Text(err_msg.into())).await;
                continue;
            }
            Err(_) => {
                eprintln!("  ✘ chat: provider timed out after 30s");
                let err_msg = serde_json::json!({
                    "type": "error",
                    "message": "Provider timed out after 30 seconds"
                })
                .to_string();
                let _ = socket.send(Message::Text(err_msg.into())).await;
                continue;
            }
        };

        // 5. Store response + emit event (sync block)
        {
            let mut runtime = match state.runtime.lock() {
                Ok(r) => r,
                Err(_) => break,
            };
            let room_id = runtime
                .sessions
                .session(&session_id)
                .map(|s| s.room_id.clone())
                .unwrap_or_else(|_| "anomaly-chat".to_string());
            let lane_id = runtime
                .sessions
                .session(&session_id)
                .ok()
                .map(|s| s.active_lane_id.clone())
                .unwrap_or_else(|| jimi_kernel::LaneId("chat".to_string()));
            let compacted = compact_provider_response(&response_text);
            runtime.memory_capsules.append(
                session_id.clone(),
                room_id,
                lane_id,
                None,
                "assistant",
                compacted.memory_text.clone(),
                Some(user_message.clone()),
                0.92,
                compacted.confidence_level,
                "session_open",
                "hot",
            );
            runtime.events.append(
                ActorRef {
                    actor_type: "provider".into(),
                    actor_id: provider_lane.provider_lane_id.clone(),
                },
                SubjectRef {
                    subject_type: "chat".into(),
                    subject_id: session_id.0.clone(),
                },
                EventType::MessageCompleted,
                Some(&session_id),
                None,
                None,
                serde_json::json!({
                    "provider": provider_lane.provider,
                    "model": provider_lane.model,
                    "adapter": provider_adapter_label(&adapter),
                    "user_message_length": user_message.len(),
                    "response_length": response_text.len(),
                }),
            );
            let new_event = runtime.events.all().last().cloned();
            let _ = persist_runtime(&state, &runtime);
            drop(runtime);
            if let Some(event) = new_event {
                let _ = state.events_tx.send(event);
            }
        } // runtime dropped here

        // 6. Send response (await is safe, no mutex held)
        let resp_msg = serde_json::json!({
            "type": "message",
            "role": "assistant",
            "content": response_text,
            "provider": provider_lane.provider,
            "model": provider_lane.model,
        })
        .to_string();
        let _ = socket.send(Message::Text(resp_msg.into())).await;
    }
}

async fn bootstrap_copilot_lane(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<ProviderLaneRecord>), (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let lane_id = format!("copilot-lane-{}", uuid::Uuid::now_v7());
    let lane = runtime.providers.connect(
        lane_id,
        "copilot".to_string(),
        "gpt-4o".to_string(),
        "primary".to_string(),
        Some("default".to_string()),
        10,
        "connected".to_string(),
    );
    runtime.events.append(
        ActorRef {
            actor_type: "operator".into(),
            actor_id: "bootstrap".into(),
        },
        SubjectRef {
            subject_type: "provider_lane".into(),
            subject_id: lane.provider_lane_id.clone(),
        },
        EventType::EngineSelected,
        None,
        None,
        None,
        serde_json::json!({
            "provider": "copilot",
            "model": "gpt-4o",
            "routing_mode": "primary",
        }),
    );
    let new_event = runtime.events.all().last().cloned();
    persist_runtime(&state, &runtime)?;
    drop(runtime);
    if let Some(event) = new_event {
        let _ = state.events_tx.send(event);
    }
    Ok((StatusCode::CREATED, Json(lane)))
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

// ────── m1nd native graph ──────

async fn graph_ingest(State(state): State<AppState>) -> Json<serde_json::Value> {
    let root = state.house_root.clone();
    let result = tokio::task::spawn_blocking(move || {
        let config = m1nd_ingest::IngestConfig {
            root: root.clone(),
            ..Default::default()
        };
        let ingestor = m1nd_ingest::Ingestor::new(config);
        ingestor.ingest()
    })
    .await;

    match result {
        Ok(Ok((graph, stats))) => {
            let nodes = stats.nodes_created;
            let edges = stats.edges_created;
            let elapsed = stats.elapsed_ms;
            {
                let mut runtime = state.runtime.lock().expect("runtime lock");
                runtime.graph = graph;
            }
            Json(serde_json::json!({
                "status": "ok",
                "nodes": nodes,
                "edges": edges,
                "files_parsed": stats.files_parsed,
                "elapsed_ms": elapsed,
            }))
        }
        Ok(Err(e)) => Json(serde_json::json!({
            "status": "error",
            "error": format!("{}", e),
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "error": format!("task join: {}", e),
        })),
    }
}

#[derive(Deserialize)]
struct GraphActivateRequest {
    query: String,
    #[serde(default = "default_top_k")]
    top_k: usize,
}

fn default_top_k() -> usize { 20 }

async fn graph_activate(
    State(state): State<AppState>,
    Json(req): Json<GraphActivateRequest>,
) -> Json<serde_json::Value> {
    use jimi_kernel::m1nd_core::activation::{ActivationEngine, WavefrontEngine};
    use jimi_kernel::m1nd_core::seed::SeedFinder;
    use jimi_kernel::m1nd_core::types::PropagationConfig;

    let mut runtime = state.runtime.lock().expect("runtime lock");
    let graph = &mut runtime.graph;

    // Find seeds via tokenized label matching
    let seeds = match SeedFinder::find_seeds(graph, &req.query, req.top_k * 3) {
        Ok(s) => s,
        Err(e) => {
            return Json(serde_json::json!({
                "status": "error",
                "error": format!("{}", e),
            }));
        }
    };

    if seeds.is_empty() {
        return Json(serde_json::json!({
            "status": "ok",
            "query": req.query,
            "results": [],
            "total": 0,
        }));
    }

    // Spread activation from seeds
    let engine = WavefrontEngine::new();
    let config = PropagationConfig::default();
    let spread_result = engine.propagate(graph, &seeds, &config);

    let results: Vec<serde_json::Value> = match spread_result {
        Ok(dim_result) => {
            dim_result
                .scores
                .iter()
                .take(req.top_k)
                .map(|(node_id, score)| {
                    let idx = node_id.0 as usize;
                    let label = graph.strings.resolve(graph.nodes.label[idx]);
                    let node_type = format!("{:?}", graph.nodes.node_type[idx]);
                    serde_json::json!({
                        "id": idx,
                        "label": label,
                        "node_type": node_type,
                        "score": score.get(),
                    })
                })
                .collect()
        }
        Err(_) => {
            // Fallback to seed results if propagation fails
            seeds
                .iter()
                .take(req.top_k)
                .map(|(node_id, score)| {
                    let idx = node_id.0 as usize;
                    let label = graph.strings.resolve(graph.nodes.label[idx]);
                    let node_type = format!("{:?}", graph.nodes.node_type[idx]);
                    serde_json::json!({
                        "id": idx,
                        "label": label,
                        "node_type": node_type,
                        "score": score.get(),
                    })
                })
                .collect()
        }
    };

    let total = results.len();
    Json(serde_json::json!({
        "status": "ok",
        "query": req.query,
        "results": results,
        "total": total,
    }))
}

#[derive(Deserialize)]
struct GraphSnapshotRequest {
    action: String, // "save" or "load"
}

async fn graph_snapshot(
    State(state): State<AppState>,
    Json(req): Json<GraphSnapshotRequest>,
) -> Json<serde_json::Value> {
    use jimi_kernel::m1nd_core::snapshot;

    let snapshot_path = state.house_root.join("data/m1nd-graph.json");
    if let Some(parent) = snapshot_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match req.action.as_str() {
        "save" => {
            let runtime = state.runtime.lock().expect("runtime lock");
            match snapshot::save_graph(&runtime.graph, &snapshot_path) {
                Ok(()) => Json(serde_json::json!({
                    "status": "ok",
                    "action": "save",
                    "path": snapshot_path.display().to_string(),
                })),
                Err(e) => Json(serde_json::json!({
                    "status": "error",
                    "error": format!("{}", e),
                })),
            }
        }
        "load" => {
            match snapshot::load_graph(&snapshot_path) {
                Ok(graph) => {
                    let node_count = graph.nodes.label.len();
                    {
                        let mut runtime = state.runtime.lock().expect("runtime lock");
                        runtime.graph = graph;
                    }
                    Json(serde_json::json!({
                        "status": "ok",
                        "action": "load",
                        "nodes": node_count,
                        "path": snapshot_path.display().to_string(),
                    }))
                }
                Err(e) => Json(serde_json::json!({
                    "status": "error",
                    "error": format!("{}", e),
                })),
            }
        }
        _ => Json(serde_json::json!({
            "status": "error",
            "error": "action must be 'save' or 'load'",
        })),
    }
}

async fn graph_topology(State(state): State<AppState>) -> Json<serde_json::Value> {
    use jimi_kernel::m1nd_core::topology::{BridgeDetector, CommunityDetector};

    let runtime = state.runtime.lock().expect("runtime lock");
    let graph = &runtime.graph;

    let detector = CommunityDetector::with_defaults();
    let communities = match detector.detect(graph) {
        Ok(c) => c,
        Err(e) => {
            return Json(serde_json::json!({
                "status": "error",
                "error": format!("{}", e),
            }));
        }
    };

    let stats = CommunityDetector::community_stats(graph, &communities);
    let bridges = BridgeDetector::detect(graph, &communities).unwrap_or_default();

    let community_list: Vec<serde_json::Value> = stats
        .iter()
        .map(|s| {
            serde_json::json!({
                "community_id": s.id.0,
                "node_count": s.node_count,
                "internal_edges": s.internal_edges,
                "external_edges": s.external_edges,
            })
        })
        .collect();

    let bridge_list: Vec<serde_json::Value> = bridges
        .iter()
        .take(20)
        .map(|b| {
            let src_label = graph.strings.resolve(graph.nodes.label[b.source.0 as usize]);
            let tgt_label = graph.strings.resolve(graph.nodes.label[b.target.0 as usize]);
            serde_json::json!({
                "source": src_label,
                "target": tgt_label,
                "importance": b.importance.get(),
                "communities": [b.source_community.0, b.target_community.0],
            })
        })
        .collect();

    Json(serde_json::json!({
        "status": "ok",
        "communities": community_list,
        "bridges": bridge_list,
        "modularity": communities.modularity.get(),
    }))
}

async fn graph_layers(State(state): State<AppState>) -> Json<serde_json::Value> {
    use jimi_kernel::m1nd_core::layer::LayerDetector;

    let runtime = state.runtime.lock().expect("runtime lock");
    let graph = &runtime.graph;

    let detector = LayerDetector::with_defaults();
    let result = match detector.detect(graph, None, &[], false, "auto") {
        Ok(r) => r,
        Err(e) => {
            return Json(serde_json::json!({
                "status": "error",
                "error": format!("{}", e),
            }));
        }
    };

    let layers: Vec<serde_json::Value> = result
        .layers
        .iter()
        .map(|l| {
            let sample_nodes: Vec<String> = l
                .nodes
                .iter()
                .take(5)
                .map(|nid| {
                    graph
                        .strings
                        .resolve(graph.nodes.label[nid.0 as usize])
                        .to_string()
                })
                .collect();
            serde_json::json!({
                "level": l.level,
                "name": l.name,
                "node_count": l.nodes.len(),
                "sample_nodes": sample_nodes,
            })
        })
        .collect();

    let violations: Vec<serde_json::Value> = result
        .violations
        .iter()
        .take(20)
        .map(|v| {
            let src_label = graph.strings.resolve(graph.nodes.label[v.source.0 as usize]);
            let tgt_label = graph.strings.resolve(graph.nodes.label[v.target.0 as usize]);
            serde_json::json!({
                "source": src_label,
                "source_layer": v.source_layer,
                "target": tgt_label,
                "target_layer": v.target_layer,
                "severity": format!("{:?}", v.severity),
            })
        })
        .collect();

    Json(serde_json::json!({
        "status": "ok",
        "layers": layers,
        "violations": violations,
        "total_violations": result.violations.len(),
    }))
}

async fn graph_stats(State(state): State<AppState>) -> Json<serde_json::Value> {
    let runtime = state.runtime.lock().expect("runtime lock");
    let graph = &runtime.graph;

    let node_count = graph.nodes.label.len();
    let edge_count = graph.csr.targets.len();

    // Top 10 nodes by PageRank
    let mut pr_indices: Vec<usize> = (0..node_count).collect();
    pr_indices.sort_by(|a, b| {
        graph.nodes.pagerank[*b]
            .get()
            .partial_cmp(&graph.nodes.pagerank[*a].get())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let top_nodes: Vec<serde_json::Value> = pr_indices
        .iter()
        .take(10)
        .map(|&idx| {
            let label = graph.strings.resolve(graph.nodes.label[idx]);
            let node_type = format!("{:?}", graph.nodes.node_type[idx]);
            serde_json::json!({
                "label": label,
                "node_type": node_type,
                "pagerank": graph.nodes.pagerank[idx].get(),
            })
        })
        .collect();

    Json(serde_json::json!({
        "status": "ok",
        "nodes": node_count,
        "edges": edge_count,
        "top_by_pagerank": top_nodes,
    }))
}

// ────── Ring 1: temporal intelligence ──────

#[derive(Deserialize)]
struct GraphPredictRequest {
    node_label: String,
    #[serde(default = "default_top_k")]
    top_k: usize,
}

async fn graph_predict(
    State(state): State<AppState>,
    Json(req): Json<GraphPredictRequest>,
) -> Json<serde_json::Value> {
    use jimi_kernel::m1nd_core::temporal::CoChangeMatrix;

    let runtime = state.runtime.lock().expect("runtime lock");
    let graph = &runtime.graph;

    // Find the node by label
    let node_id = graph.nodes.label.iter().enumerate().find_map(|(idx, lbl)| {
        let l = graph.strings.resolve(*lbl);
        if l == req.node_label {
            Some(jimi_kernel::m1nd_core::types::NodeId(idx as u32))
        } else {
            None
        }
    });

    let node_id = match node_id {
        Some(id) => id,
        None => {
            return Json(serde_json::json!({
                "status": "error",
                "error": format!("node '{}' not found", req.node_label),
            }));
        }
    };

    let matrix = match CoChangeMatrix::bootstrap(graph, 10_000) {
        Ok(m) => m,
        Err(e) => {
            return Json(serde_json::json!({
                "status": "error",
                "error": format!("{}", e),
            }));
        }
    };

    let predictions = matrix.predict(node_id, req.top_k);
    let results: Vec<serde_json::Value> = predictions
        .iter()
        .map(|entry| {
            let label = graph.strings.resolve(graph.nodes.label[entry.target.0 as usize]);
            serde_json::json!({
                "label": label,
                "score": entry.strength.get(),
            })
        })
        .collect();

    Json(serde_json::json!({
        "status": "ok",
        "changed_node": req.node_label,
        "predictions": results,
    }))
}

// ────── Ring 2: structural analysis ──────

#[derive(Deserialize)]
struct GraphImpactRequest {
    node_label: String,
}

async fn graph_impact(
    State(state): State<AppState>,
    Json(req): Json<GraphImpactRequest>,
) -> Json<serde_json::Value> {
    use jimi_kernel::m1nd_core::counterfactual::CounterfactualEngine;
    use jimi_kernel::m1nd_core::activation::HybridEngine;
    use jimi_kernel::m1nd_core::types::PropagationConfig;

    let runtime = state.runtime.lock().expect("runtime lock");
    let graph = &runtime.graph;

    let node_id = graph.nodes.label.iter().enumerate().find_map(|(idx, lbl)| {
        let l = graph.strings.resolve(*lbl);
        if l == req.node_label {
            Some(jimi_kernel::m1nd_core::types::NodeId(idx as u32))
        } else {
            None
        }
    });

    let node_id = match node_id {
        Some(id) => id,
        None => {
            return Json(serde_json::json!({
                "status": "error",
                "error": format!("node '{}' not found", req.node_label),
            }));
        }
    };

    let simulator = CounterfactualEngine::with_defaults();
    let engine = HybridEngine::new();
    let config = PropagationConfig::default();

    match simulator.simulate_removal(graph, &engine, &config, &[node_id]) {
        Ok(result) => {
            let weakened: Vec<serde_json::Value> = result
                .weakened_nodes
                .iter()
                .take(20)
                .map(|(nid, pct_lost)| {
                    let label = graph.strings.resolve(graph.nodes.label[nid.0 as usize]);
                    serde_json::json!({
                        "label": label,
                        "pct_activation_lost": pct_lost.get(),
                    })
                })
                .collect();

            let orphaned: Vec<String> = result
                .orphaned_nodes
                .iter()
                .take(10)
                .map(|nid| graph.strings.resolve(graph.nodes.label[nid.0 as usize]).to_string())
                .collect();

            Json(serde_json::json!({
                "status": "ok",
                "removed_node": req.node_label,
                "total_impact": result.total_impact.get(),
                "pct_activation_lost": result.pct_activation_lost.get(),
                "weakened_nodes": weakened,
                "orphaned_nodes": orphaned,
                "communities_split": result.communities_split,
            }))
        }
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "error": format!("{}", e),
        })),
    }
}

async fn graph_flow(State(state): State<AppState>) -> Json<serde_json::Value> {
    use jimi_kernel::m1nd_core::flow::{FlowConfig, FlowEngine};

    let runtime = state.runtime.lock().expect("runtime lock");
    let graph = &runtime.graph;

    let config = FlowConfig::with_defaults();
    let engine = FlowEngine::new();

    // Auto-discover entry points (no explicit entry nodes)
    match engine.simulate(graph, &[], 2, &config) {
        Ok(result) => {
            let turbulence: Vec<serde_json::Value> = result
                .turbulence_points
                .iter()
                .take(15)
                .map(|t| {
                    serde_json::json!({
                        "label": t.node_label,
                        "particle_count": t.particle_count,
                        "has_lock": t.has_lock,
                    })
                })
                .collect();

            Json(serde_json::json!({
                "status": "ok",
                "turbulence_points": turbulence,
                "turbulence_count": result.summary.turbulence_count,
                "valve_count": result.summary.valve_count,
                "total_particles": result.summary.total_particles,
                "elapsed_ms": result.summary.elapsed_ms,
            }))
        }
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "error": format!("{}", e),
        })),
    }
}

// ────── Ring 3: pattern analysis ──────

async fn graph_twins(State(state): State<AppState>) -> Json<serde_json::Value> {
    use jimi_kernel::m1nd_core::twins::{find_twins, TwinConfig};

    let runtime = state.runtime.lock().expect("runtime lock");
    let graph = &runtime.graph;

    let config = TwinConfig::default();
    match find_twins(graph, &config) {
        Ok(result) => {
            let pairs: Vec<serde_json::Value> = result
                .pairs
                .iter()
                .take(20)
                .map(|p| {
                    serde_json::json!({
                        "node_a": p.node_a_label,
                        "node_b": p.node_b_label,
                        "similarity": p.similarity,
                    })
                })
                .collect();

            Json(serde_json::json!({
                "status": "ok",
                "twin_pairs": pairs,
                "total": result.pairs.len(),
            }))
        }
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "error": format!("{}", e),
        })),
    }
}

#[derive(Deserialize)]
struct GraphEpidemicRequest {
    infected_labels: Vec<String>,
}

async fn graph_epidemic(
    State(state): State<AppState>,
    Json(req): Json<GraphEpidemicRequest>,
) -> Json<serde_json::Value> {
    use jimi_kernel::m1nd_core::epidemic::{EpidemicConfig, EpidemicEngine};

    let runtime = state.runtime.lock().expect("runtime lock");
    let graph = &runtime.graph;

    let infected: Vec<_> = req
        .infected_labels
        .iter()
        .filter_map(|label| {
            graph.nodes.label.iter().enumerate().find_map(|(idx, lbl)| {
                let l = graph.strings.resolve(*lbl);
                if l == *label {
                    Some(jimi_kernel::m1nd_core::types::NodeId(idx as u32))
                } else {
                    None
                }
            })
        })
        .collect();

    if infected.is_empty() {
        return Json(serde_json::json!({
            "status": "error",
            "error": "no matching infected nodes found",
        }));
    }

    let engine = EpidemicEngine::new();
    let config = EpidemicConfig {
        infection_rate: None,
        recovery_rate: 0.0,
        iterations: 50,
        direction: jimi_kernel::m1nd_core::epidemic::EpidemicDirection::Both,
        top_k: 20,
        burnout_threshold: 0.8,
        promotion_threshold: 0.0,
    };

    match engine.simulate(graph, &infected, &[], &config) {
        Ok(result) => {
            let predictions: Vec<serde_json::Value> = result
                .predictions
                .iter()
                .take(20)
                .map(|p| {
                    serde_json::json!({
                        "label": p.label,
                        "probability": p.infection_probability,
                        "generation": p.generation,
                    })
                })
                .collect();

            Json(serde_json::json!({
                "status": "ok",
                "predictions": predictions,
                "total_at_risk": result.predictions.len(),
            }))
        }
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "error": format!("{}", e),
        })),
    }
}

async fn graph_trust(State(state): State<AppState>) -> Json<serde_json::Value> {
    use jimi_kernel::m1nd_core::trust::TrustLedger;

    let runtime = state.runtime.lock().expect("runtime lock");
    let graph = &runtime.graph;

    // Build trust scores from graph structure (no defect history yet — virgin ledger)
    let ledger = TrustLedger::new();
    let now = chrono::Utc::now().timestamp() as f64;

    // Report nodes with highest PageRank as most critical
    let node_count = graph.nodes.label.len();
    let mut trust_results: Vec<serde_json::Value> = Vec::new();

    let mut pr_indices: Vec<usize> = (0..node_count).collect();
    pr_indices.sort_by(|a, b| {
        graph.nodes.pagerank[*b]
            .get()
            .partial_cmp(&graph.nodes.pagerank[*a].get())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for &idx in pr_indices.iter().take(20) {
        let label = graph.strings.resolve(graph.nodes.label[idx]);
        let node_type = format!("{:?}", graph.nodes.node_type[idx]);
        let score = ledger.compute_trust(label, now);
        trust_results.push(serde_json::json!({
            "label": label,
            "node_type": node_type,
            "trust": score.trust_score,
            "risk_multiplier": score.risk_multiplier,
            "pagerank": graph.nodes.pagerank[idx].get(),
        }));
    }

    Json(serde_json::json!({
        "status": "ok",
        "trust_scores": trust_results,
    }))
}

#[derive(Deserialize)]
struct GraphRefactorRequest {
    target_label: String,
}

async fn graph_refactor(
    State(state): State<AppState>,
    Json(req): Json<GraphRefactorRequest>,
) -> Json<serde_json::Value> {
    use jimi_kernel::m1nd_core::refactor::{plan_refactoring, RefactorConfig};

    let runtime = state.runtime.lock().expect("runtime lock");
    let graph = &runtime.graph;

    let node_id = graph.nodes.label.iter().enumerate().find_map(|(idx, lbl)| {
        let l = graph.strings.resolve(*lbl);
        if l == req.target_label {
            Some(jimi_kernel::m1nd_core::types::NodeId(idx as u32))
        } else {
            None
        }
    });

    let _node_id = match node_id {
        Some(id) => id,
        None => {
            return Json(serde_json::json!({
                "status": "error",
                "error": format!("node '{}' not found", req.target_label),
            }));
        }
    };

    let config = RefactorConfig::default();

    match plan_refactoring(graph, &config) {
        Ok(plan) => {
            let candidates: Vec<serde_json::Value> = plan
                .candidates
                .iter()
                .take(10)
                .map(|c| {
                    serde_json::json!({
                        "community_id": c.community_id,
                        "extracted_labels": c.extracted_labels,
                        "interface_edges": c.interface_edges.len(),
                        "risk": format!("{:?}", c.risk),
                        "cohesion": c.cohesion,
                        "coupling": c.coupling,
                    })
                })
                .collect();

            Json(serde_json::json!({
                "status": "ok",
                "target": req.target_label,
                "modularity": plan.graph_modularity,
                "candidates": candidates,
                "nodes_analyzed": plan.nodes_analyzed,
            }))
        }
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "error": format!("{}", e),
        })),
    }
}

// ────── Ring 4: resonance, tremor, taint ──────

#[derive(Deserialize)]
struct GraphResonanceRequest {
    seed_label: String,
    #[serde(default = "default_top_k")]
    top_k: usize,
}

async fn graph_resonance(
    State(state): State<AppState>,
    Json(req): Json<GraphResonanceRequest>,
) -> Json<serde_json::Value> {
    use jimi_kernel::m1nd_core::resonance::StandingWavePropagator;
    use jimi_kernel::m1nd_core::types::{FiniteF32, PosF32};

    let runtime = state.runtime.lock().expect("runtime lock");
    let graph = &runtime.graph;

    // Find seed node
    let seed_id = graph.nodes.label.iter().enumerate().find_map(|(idx, lbl)| {
        let l = graph.strings.resolve(*lbl);
        if l == req.seed_label {
            Some(jimi_kernel::m1nd_core::types::NodeId(idx as u32))
        } else {
            None
        }
    });

    let seed_id = match seed_id {
        Some(id) => id,
        None => {
            return Json(serde_json::json!({
                "status": "error",
                "error": format!("node '{}' not found", req.seed_label),
            }));
        }
    };

    let freq = PosF32::new(1.0).unwrap();
    let wavelength = PosF32::new(3.0).unwrap();
    let min_amp = FiniteF32::new(0.01);
    let propagator = StandingWavePropagator::new(6, min_amp, 10_000);

    let seed_weight = FiniteF32::new(1.0);
    let seeds = vec![(seed_id, seed_weight)];

    match propagator.propagate(graph, &seeds, freq, wavelength) {
        Ok(result) => {
            let antinodes: Vec<serde_json::Value> = result
                .antinodes
                .iter()
                .take(req.top_k)
                .map(|(nid, amp)| {
                    let label = graph.strings.resolve(graph.nodes.label[nid.0 as usize]);
                    serde_json::json!({
                        "label": label,
                        "amplitude": amp.get(),
                    })
                })
                .collect();

            Json(serde_json::json!({
                "status": "ok",
                "seed": req.seed_label,
                "antinodes": antinodes,
                "total_energy": result.total_energy.get(),
                "pulses_processed": result.pulses_processed,
            }))
        }
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "error": format!("{}", e),
        })),
    }
}

async fn graph_tremor(State(state): State<AppState>) -> Json<serde_json::Value> {
    use jimi_kernel::m1nd_core::tremor::{TremorRegistry, TremorWindow};

    let _runtime = state.runtime.lock().expect("runtime lock");

    // Tremor accumulates over time — fresh registry has no observations yet
    let registry = TremorRegistry::with_defaults();
    let now = chrono::Utc::now().timestamp() as f64;

    let result = registry.analyze(
        TremorWindow::Days30,
        0.1,  // threshold
        20,   // top_k
        None, // no filter
        now,
        3,    // min_observations
    );

    let tremors: Vec<serde_json::Value> = result
        .tremors
        .iter()
        .take(20)
        .map(|t| {
            serde_json::json!({
                "node_id": t.node_id,
                "label": t.label,
                "magnitude": t.magnitude,
                "direction": format!("{:?}", t.direction),
                "observation_count": t.observation_count,
            })
        })
        .collect();

    Json(serde_json::json!({
        "status": "ok",
        "tremors": tremors,
        "total_analyzed": result.total_nodes_analyzed,
        "note": if result.tremors.is_empty() { "no tremor history yet — accumulates over sessions" } else { "" },
    }))
}

#[derive(Deserialize)]
struct GraphTaintRequest {
    entry_labels: Vec<String>,
}

async fn graph_taint(
    State(state): State<AppState>,
    Json(req): Json<GraphTaintRequest>,
) -> Json<serde_json::Value> {
    use jimi_kernel::m1nd_core::taint::{TaintConfig, TaintEngine};

    let runtime = state.runtime.lock().expect("runtime lock");
    let graph = &runtime.graph;

    let entries: Vec<_> = req
        .entry_labels
        .iter()
        .filter_map(|label| {
            graph.nodes.label.iter().enumerate().find_map(|(idx, lbl)| {
                let l = graph.strings.resolve(*lbl);
                if l == *label {
                    Some(jimi_kernel::m1nd_core::types::NodeId(idx as u32))
                } else {
                    None
                }
            })
        })
        .collect();

    if entries.is_empty() {
        return Json(serde_json::json!({
            "status": "error",
            "error": "no matching entry nodes found. provide node labels from /graph/stats",
        }));
    }

    let config = TaintConfig::default();
    match TaintEngine::analyze(graph, &entries, &config) {
        Ok(result) => {
            let boundary_hits: Vec<serde_json::Value> = result
                .boundary_hits
                .iter()
                .take(10)
                .map(|b| {
                    serde_json::json!({
                        "label": b.label,
                        "boundary_type": format!("{:?}", b.boundary_type),
                    })
                })
                .collect();

            let leaks: Vec<serde_json::Value> = result
                .leaks
                .iter()
                .take(10)
                .map(|l| {
                    serde_json::json!({
                        "entry_node": l.entry_node,
                        "exit_node": l.exit_node,
                        "probability": l.probability,
                    })
                })
                .collect();

            Json(serde_json::json!({
                "status": "ok",
                "boundary_hits": boundary_hits,
                "boundary_misses": result.boundary_misses.len(),
                "leaks": leaks,
                "total_leaks": result.leaks.len(),
            }))
        }
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "error": format!("{}", e),
        })),
    }
}

// ────── Ring 5: antibody, plasticity, xlr — 100% ──────

#[derive(Deserialize)]
struct GraphAntibodyScanRequest {
    /// Antibody pattern to scan for
    name: String,
    description: String,
    #[serde(default = "default_severity")]
    severity: String,
    /// Pattern nodes
    nodes: Vec<AntibodyPatternNodeInput>,
    /// Pattern edges (source_idx → target_idx)
    #[serde(default)]
    edges: Vec<AntibodyPatternEdgeInput>,
}

fn default_severity() -> String {
    "warning".to_string()
}

#[derive(Deserialize)]
struct AntibodyPatternNodeInput {
    role: String,
    #[serde(default)]
    node_type: Option<String>,
    #[serde(default)]
    label_contains: Option<String>,
}

#[derive(Deserialize)]
struct AntibodyPatternEdgeInput {
    from_idx: usize,
    to_idx: usize,
}

async fn graph_antibody_scan(
    State(state): State<AppState>,
    Json(req): Json<GraphAntibodyScanRequest>,
) -> Json<serde_json::Value> {
    use jimi_kernel::m1nd_core::antibody::{
        match_antibody, Antibody, AntibodyPattern, AntibodySeverity,
        PatternNode, PatternEdge,
    };

    let runtime = state.runtime.lock().expect("runtime lock");
    let graph = &runtime.graph;

    // Build pattern nodes
    let pattern_nodes: Vec<PatternNode> = req.nodes.iter().map(|n| {
        PatternNode {
            role: n.role.clone(),
            node_type: n.node_type.clone(),
            required_tags: Vec::new(),
            label_contains: n.label_contains.clone(),
        }
    }).collect();

    // Build pattern edges
    let pattern_edges: Vec<PatternEdge> = req.edges.iter().map(|e| {
        PatternEdge {
            source_idx: e.from_idx,
            target_idx: e.to_idx,
            relation: None,
        }
    }).collect();

    let severity = match req.severity.as_str() {
        "info" => AntibodySeverity::Info,
        "critical" => AntibodySeverity::Critical,
        _ => AntibodySeverity::Warning,
    };

    let antibody = Antibody {
        id: format!("adhoc-{}", chrono::Utc::now().timestamp()),
        name: req.name.clone(),
        description: req.description.clone(),
        pattern: AntibodyPattern {
            nodes: pattern_nodes,
            edges: pattern_edges,
            negative_edges: Vec::new(),
        },
        severity,
        enabled: true,
        created_at: chrono::Utc::now().timestamp() as f64,
        match_count: 0,
        last_match_at: None,
        created_by: "anomaly-server".to_string(),
        source_query: req.name.clone(),
        source_nodes: Vec::new(),
        specificity: 1.0,
    };

    let matches = match_antibody(graph, &antibody, 5000);

    let results: Vec<serde_json::Value> = matches.iter().take(20).map(|m| {
        let bound: Vec<serde_json::Value> = m.bound_nodes.iter().map(|b| {
            serde_json::json!({
                "role": b.role,
                "label": b.label,
                "node_id": b.node_id,
            })
        }).collect();
        serde_json::json!({
            "antibody": m.antibody_name,
            "confidence": m.confidence,
            "bound_nodes": bound,
        })
    }).collect();

    Json(serde_json::json!({
        "status": "ok",
        "antibody_name": req.name,
        "matches": results,
        "total_matches": matches.len(),
    }))
}

#[derive(Deserialize)]
struct GraphPlasticityRequest {
    /// Query that was just executed (for recording)
    query: String,
    /// Node labels that were activated by this query
    activated_labels: Vec<String>,
    /// Seed labels used
    seed_labels: Vec<String>,
}

async fn graph_plasticity(
    State(state): State<AppState>,
    Json(req): Json<GraphPlasticityRequest>,
) -> Json<serde_json::Value> {
    use jimi_kernel::m1nd_core::plasticity::{PlasticityConfig, PlasticityEngine};
    use jimi_kernel::m1nd_core::types::FiniteF32;

    let mut runtime = state.runtime.lock().expect("runtime lock");
    let graph = &mut runtime.graph;

    // Resolve node labels to IDs
    let activated: Vec<_> = req.activated_labels.iter().filter_map(|label| {
        graph.nodes.label.iter().enumerate().find_map(|(idx, lbl)| {
            let l = graph.strings.resolve(*lbl);
            if l == *label {
                Some((jimi_kernel::m1nd_core::types::NodeId(idx as u32), FiniteF32::new(1.0)))
            } else {
                None
            }
        })
    }).collect();

    let seeds: Vec<_> = req.seed_labels.iter().filter_map(|label| {
        graph.nodes.label.iter().enumerate().find_map(|(idx, lbl)| {
            let l = graph.strings.resolve(*lbl);
            if l == *label {
                Some((jimi_kernel::m1nd_core::types::NodeId(idx as u32), FiniteF32::new(1.0)))
            } else {
                None
            }
        })
    }).collect();

    if activated.is_empty() {
        return Json(serde_json::json!({
            "status": "error",
            "error": "no matching activated nodes found",
        }));
    }

    let config = PlasticityConfig::default();
    let mut engine = PlasticityEngine::new(graph, config);

    match engine.update(graph, &activated, &seeds, &req.query) {
        Ok(result) => {
            Json(serde_json::json!({
                "status": "ok",
                "query": req.query,
                "edges_strengthened": result.edges_strengthened,
                "edges_decayed": result.edges_decayed,
                "ltp_events": result.ltp_events,
                "ltd_events": result.ltd_events,
                "homeostatic_rescales": result.homeostatic_rescales,
                "priming_nodes": result.priming_nodes,
            }))
        }
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "error": format!("{}", e),
        })),
    }
}

#[derive(Deserialize)]
struct GraphXlrRequest {
    seed_labels: Vec<String>,
    #[serde(default = "default_top_k")]
    top_k: usize,
}

async fn graph_xlr(
    State(state): State<AppState>,
    Json(req): Json<GraphXlrRequest>,
) -> Json<serde_json::Value> {
    use jimi_kernel::m1nd_core::xlr::AdaptiveXlrEngine;
    use jimi_kernel::m1nd_core::types::{FiniteF32, PropagationConfig};

    let runtime = state.runtime.lock().expect("runtime lock");
    let graph = &runtime.graph;

    let seeds: Vec<_> = req.seed_labels.iter().filter_map(|label| {
        graph.nodes.label.iter().enumerate().find_map(|(idx, lbl)| {
            let l = graph.strings.resolve(*lbl);
            if l == *label {
                Some((jimi_kernel::m1nd_core::types::NodeId(idx as u32), FiniteF32::new(1.0)))
            } else {
                None
            }
        })
    }).collect();

    if seeds.is_empty() {
        return Json(serde_json::json!({
            "status": "error",
            "error": "no matching seed nodes found",
        }));
    }

    let engine = AdaptiveXlrEngine::with_defaults();
    let config = PropagationConfig::default();

    match engine.query(graph, &seeds, &config) {
        Ok(result) => {
            let activations: Vec<serde_json::Value> = result
                .activations
                .iter()
                .take(req.top_k)
                .map(|(nid, score)| {
                    let label = graph.strings.resolve(graph.nodes.label[nid.0 as usize]);
                    serde_json::json!({
                        "label": label,
                        "score": score.get(),
                    })
                })
                .collect();

            let anti_seeds: Vec<String> = result
                .anti_seeds
                .iter()
                .take(5)
                .map(|nid| graph.strings.resolve(graph.nodes.label[nid.0 as usize]).to_string())
                .collect();

            Json(serde_json::json!({
                "status": "ok",
                "activations": activations,
                "anti_seeds": anti_seeds,
                "fallback_to_hot_only": result.fallback_to_hot_only,
                "pulses_processed": result.pulses_processed,
            }))
        }
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "error": format!("{}", e),
        })),
    }
}
