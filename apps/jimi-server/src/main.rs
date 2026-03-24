use std::{
    net::SocketAddr,
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::Html,
    routing::{get, post},
};
use jimi_kernel::{
    ActorRef, DurableStore, EventEnvelope, EventType, FieldVaultArtifact, HouseInventory,
    HouseRuntime, MandalaActiveSnapshot, MandalaCapabilityPolicy, MandalaCapsuleContract,
    MandalaExecutionPolicy, MandalaManifest, MandalaMemoryPolicy, MandalaProjection, MandalaRefs,
    MandalaSelf, MandalaStableMemory, MemoryBridgeRecord, MemoryCapsuleRecord,
    MemoryPromotionRecord, ResynthesisTriggerRecord, SealLevel, SessionRecord, SlotBindingState,
    SubjectRef, SummaryCheckpointRecord, TurnDispatchRecord, TurnRecord,
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

#[derive(Clone)]
struct AppState {
    runtime: Arc<Mutex<HouseRuntime>>,
    store: Arc<Mutex<DurableStore>>,
    events_tx: broadcast::Sender<EventEnvelope>,
    house_root: PathBuf,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    inventory: HouseInventory,
}

#[derive(Debug, Deserialize)]
struct CreateSessionRequest {
    title: String,
}

#[derive(Debug, Deserialize)]
struct CreateTurnRequest {
    session_id: String,
    intent_mode: String,
    intent_summary: String,
}

#[derive(Debug, Serialize)]
struct InventoryResponse {
    inventory: HouseInventory,
    mandalas: Vec<String>,
    slots: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CapsuleBootstrapResponse {
    mandala_id: String,
    capsule_id: String,
    slot_id: String,
    slot_state: SlotBindingState,
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
    status: String,
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
    active_mandala_id: Option<String>,
    memory_policy: MandalaMemoryPolicy,
    hot_capsules: Vec<MemoryCapsuleRecord>,
    relevant_capsules: Vec<MemoryCapsuleRecord>,
    summary_checkpoints: Vec<SummaryCheckpointRecord>,
    memory_bridges: Vec<MemoryBridgeRecord>,
    resynthesis_triggers: Vec<ResynthesisTriggerRecord>,
    memory_promotions: Vec<MemoryPromotionRecord>,
    stable_memory: Option<MandalaStableMemory>,
    active_snapshot: Option<MandalaActiveSnapshot>,
    query_seed: String,
    providers: Vec<String>,
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

    let state = AppState {
        runtime: Arc::new(Mutex::new(runtime)),
        store: Arc::new(Mutex::new(store)),
        events_tx: broadcast::channel(256).0,
        house_root: std::env::current_dir().expect("failed to resolve current dir"),
    };

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
        .route("/memory/query/:session_id", get(query_memory))
        .route("/context-packet/:session_id", get(context_packet))
        .route("/mandalas", get(list_mandalas))
        .route("/capsules", get(list_capsules))
        .route("/slots", get(list_slots))
        .route("/artifacts", get(list_artifacts))
        .route("/providers", get(list_providers))
        .route("/bootstrap/core-capsule", post(bootstrap_core_capsule))
        .route("/bootstrap/core-artifact", post(bootstrap_core_artifact))
        .route("/bootstrap/provider-lane", post(bootstrap_provider_lane))
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
    let session = runtime.bootstrap_session(request.title);
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

    let provider_lane = runtime
        .providers
        .all()
        .into_iter()
        .next()
        .cloned()
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "connect a provider lane before dispatching a turn".to_string(),
            )
        })?;

    let turn = runtime
        .sessions
        .create_turn(
            &session.session_id,
            &session.active_lane_id,
            request.intent_mode.clone(),
        )
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;

    runtime.events.append(
        ActorRef {
            actor_type: "operator".into(),
            actor_id: "cockpit.turns".into(),
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
            "intent_mode": request.intent_mode,
            "intent_summary": request.intent_summary.clone(),
        }),
    );

    let dispatch = runtime.dispatches.dispatch(
        turn.turn_id.clone(),
        turn.session_id.clone(),
        turn.lane_id.clone(),
        provider_lane.provider_lane_id.clone(),
        request.intent_summary.clone(),
        "queued",
    );

    runtime.memory_capsules.append(
        turn.session_id.clone(),
        turn.lane_id.clone(),
        Some(turn.turn_id.clone()),
        "operator",
        request.intent_summary.clone(),
        Some(request.intent_summary.clone()),
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
            "intent_summary": request.intent_summary,
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
        Json(TurnBootstrapResponse { turn, dispatch }),
    ))
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
    let runtime = state.runtime.lock().map_err(internal_lock_error)?;
    let session_id = jimi_kernel::SessionId(session_id);
    Ok(Json(assemble_context_packet(&runtime, &session_id)))
}

fn assemble_context_packet(
    runtime: &HouseRuntime,
    session_id: &jimi_kernel::SessionId,
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

    let relevant_capsules = runtime.query_memory(
        session_id,
        &query_seed,
        memory_policy.relevant_context_limit.max(1),
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

    ContextPacketResponse {
        session_id: session_id.0.clone(),
        active_mandala_id,
        memory_policy,
        hot_capsules,
        relevant_capsules,
        summary_checkpoints,
        memory_bridges,
        resynthesis_triggers,
        memory_promotions,
        stable_memory,
        active_snapshot,
        query_seed,
        providers,
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

    runtime.memory_capsules.append(
        completed_turn.session_id.clone(),
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
    let (dispatch, provider_lane_id, provider_prompt, house_root) = {
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
        let _ = runtime
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
                "mode": "live_codex",
                "context_packet_ready": true,
            }),
        );

        let packet = assemble_context_packet(&runtime, &running_dispatch.session_id);
        let provider_prompt = build_provider_prompt(
            &running_dispatch.provider_lane_id,
            &packet,
            &running_dispatch.intent_summary,
        );

        let new_event = runtime.events.all().last().cloned();
        persist_runtime(&state, &runtime)?;
        drop(runtime);

        if let Some(event) = new_event {
            let _ = state.events_tx.send(event);
        }

        (
            running_dispatch.clone(),
            running_dispatch.provider_lane_id.clone(),
            provider_prompt,
            state.house_root.clone(),
        )
    };

    let output_text =
        tokio::task::spawn_blocking(move || run_codex_exec(&house_root, &provider_prompt))
            .await
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
            .map_err(|error| (StatusCode::BAD_GATEWAY, error))?;
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

    runtime.memory_capsules.append(
        completed_turn.session_id.clone(),
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
            actor_id: provider_lane_id,
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
            "mode": "live_codex",
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
        Json(DispatchExecutionResponse {
            dispatch: completed_dispatch,
            turn: completed_turn,
            output_text,
        }),
    ))
}

async fn list_mandalas(State(state): State<AppState>) -> Json<Vec<MandalaManifest>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(runtime.mandalas.all().into_iter().cloned().collect())
}

async fn list_capsules(State(state): State<AppState>) -> Json<Vec<jimi_kernel::CapsuleRecord>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(runtime.capsules.all().into_iter().cloned().collect())
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

async fn bootstrap_core_capsule(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<CapsuleBootstrapResponse>), (StatusCode, String)> {
    let mut runtime = state.runtime.lock().map_err(internal_lock_error)?;

    let mandala_id = "jimi.superstar.core".to_string();
    let capsule_id = "capsule.jimi.superstar.core.v1".to_string();
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
            slot_id,
            slot_state: slot.state,
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

    let provider = runtime.providers.connect(
        "provider.codex.primary",
        "codex",
        "gpt-5",
        "primary",
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
            "status": provider.status.clone(),
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
        Json(ProviderBootstrapResponse {
            provider_lane_id: provider.provider_lane_id,
            provider: provider.provider,
            model: provider.model,
            routing_mode: provider.routing_mode,
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

fn run_codex_exec(house_root: &PathBuf, provider_prompt: &str) -> Result<String, String> {
    let output_path =
        std::env::temp_dir().join(format!("jimi-codex-output-{}.txt", uuid::Uuid::now_v7()));

    let status = Command::new("codex")
        .arg("exec")
        .arg("--skip-git-repo-check")
        .arg("--sandbox")
        .arg("workspace-write")
        .arg("-a")
        .arg("never")
        .arg("--output-last-message")
        .arg(&output_path)
        .arg("--cd")
        .arg(house_root)
        .arg(provider_prompt)
        .status()
        .map_err(|error| error.to_string())?;

    if !status.success() {
        let _ = std::fs::remove_file(&output_path);
        return Err(format!("codex exec failed with status {}", status));
    }

    let output = std::fs::read_to_string(&output_path).map_err(|error| error.to_string())?;
    let _ = std::fs::remove_file(output_path);
    Ok(output.trim().to_string())
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
            <div class="list" id="mandala-list"></div>
            <div class="list" id="capsule-list"></div>
            <div class="list" id="slot-list"></div>
            <div class="list" id="artifact-list"></div>
            <div class="list" id="provider-list"></div>
          </div>

          <div class="panel">
            <h2>Capsule Memory</h2>
            <div class="small" id="context-status">awaiting context packet</div>
            <div class="list" id="memory-capsule-list"></div>
            <div class="list" id="memory-relevance-list"></div>
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
      const slotListEl = document.getElementById('slot-list');
      const artifactListEl = document.getElementById('artifact-list');
      const providerListEl = document.getElementById('provider-list');
      const bootstrapButtonEl = document.getElementById('bootstrap-core');
      const bootstrapStatusEl = document.getElementById('bootstrap-status');
      const sealButtonEl = document.getElementById('seal-core');
      const sealStatusEl = document.getElementById('seal-status');
      const providerButtonEl = document.getElementById('bootstrap-provider');
      const providerStatusEl = document.getElementById('provider-status');
      const contextStatusEl = document.getElementById('context-status');
      const memoryCapsuleListEl = document.getElementById('memory-capsule-list');
      const memoryRelevanceListEl = document.getElementById('memory-relevance-list');
      const summaryListEl = document.getElementById('summary-list');
      const bridgeListEl = document.getElementById('bridge-list');
      const triggerListEl = document.getElementById('trigger-list');
      const promotionListEl = document.getElementById('promotion-list');
      const contextPacketViewEl = document.getElementById('context-packet-view');

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
        slots: [],
        artifacts: [],
        providers: []
      };

      function renderInventory(data) {
        const inventory = data.inventory || {};
        const items = [
          ['sessions', inventory.sessions ?? 0],
          ['events', inventory.events ?? 0],
          ['mandalas', inventory.mandalas ?? 0],
          ['capsules', inventory.capsules ?? 0],
          ['slots', inventory.slots ?? 0],
          ['fieldvault', inventory.fieldvault_artifacts ?? 0],
          ['providers', inventory.provider_lanes ?? 0],
          ['dispatches', inventory.turn_dispatches ?? 0],
          ['summaries', inventory.summary_checkpoints ?? 0],
          ['bridges', inventory.memory_bridges ?? 0],
          ['triggers', inventory.resynthesis_triggers ?? 0],
          ['promotions', inventory.memory_promotions ?? 0],
        ];
        statsEl.innerHTML = items.map(([label, value]) => `
          <div class="stat">
            <div class="value">${value}</div>
            <div class="label">${label}</div>
          </div>
        `).join('');
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
      }

      function renderCapsules() {
        if (!state.capsules.length) {
          capsuleListEl.innerHTML = '<div class="empty">No capsules installed yet.</div>';
        } else {
          capsuleListEl.innerHTML = state.capsules.map(capsule => `
            <div class="card">
              <strong>${escapeHtml(capsule.capsule_id)}</strong>
              <div class="meta">mandala: ${escapeHtml(capsule.mandala_id)}</div>
              <div class="meta">version: ${escapeHtml(capsule.version)}</div>
              <div class="meta">source: ${escapeHtml(capsule.install_source)}</div>
            </div>
          `).join('');
        }
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
        providerListEl.innerHTML = state.providers.map(provider => `
          <div class="card">
            <strong>${escapeHtml(provider.provider_lane_id)}</strong>
            <div class="meta">provider: ${escapeHtml(provider.provider)}</div>
            <div class="meta">model: ${escapeHtml(provider.model)}</div>
            <div class="meta">routing: ${escapeHtml(provider.routing_mode)}</div>
            <div class="meta">status: ${escapeHtml(provider.status)}</div>
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

      function renderContextPacket() {
        if (!state.contextPacket) {
          contextPacketViewEl.innerHTML = '<div class="empty">No context packet loaded yet.</div>';
          memoryRelevanceListEl.innerHTML = '<div class="empty">No ranked memory candidates yet.</div>';
          bridgeListEl.innerHTML = '<div class="empty">No memory bridges yet.</div>';
          triggerListEl.innerHTML = '<div class="empty">No re-synthesis triggers yet.</div>';
          promotionListEl.innerHTML = '<div class="empty">No memory promotions yet.</div>';
          return;
        }
        const packet = state.contextPacket;
        const relevant = packet.relevant_capsules || [];
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
            <div class="meta">active mandala: ${escapeHtml(packet.active_mandala_id || 'none')}</div>
            <div class="meta">providers: ${escapeHtml((packet.providers || []).join(', ') || 'none')}</div>
            <div class="meta">query seed: ${escapeHtml(packet.query_seed || 'none')}</div>
            <div class="meta">hot capsules: ${escapeHtml((packet.hot_capsules || []).length)}</div>
            <div class="meta">relevant capsules: ${escapeHtml((packet.relevant_capsules || []).length)}</div>
            <div class="meta">summaries: ${escapeHtml((packet.summary_checkpoints || []).length)}</div>
            <div class="meta">bridges: ${escapeHtml((packet.memory_bridges || []).length)}</div>
            <div class="meta">triggers: ${escapeHtml((packet.resynthesis_triggers || []).length)}</div>
            <div class="meta">promotions: ${escapeHtml((packet.memory_promotions || []).length)}</div>
            <div class="meta">boot include: ${escapeHtml((packet.memory_policy?.boot_include || []).join(' | ') || 'none')}</div>
            <div class="meta">lookup sources: ${escapeHtml((packet.memory_policy?.lookup_sources || []).join(' | ') || 'none')}</div>
            <div class="meta">hot limit: ${escapeHtml(packet.memory_policy?.hot_context_limit ?? 0)}</div>
            <div class="meta">relevant limit: ${escapeHtml(packet.memory_policy?.relevant_context_limit ?? 0)}</div>
            <div class="meta">promotion threshold: ${escapeHtml(packet.memory_policy?.promotion_confidence_threshold ?? 0)}</div>
            <div class="meta">stable promotion: ${escapeHtml(packet.memory_policy?.promote_to_stable_memory ? 'on' : 'off')}</div>
            <div class="meta">fieldvault sealing: ${escapeHtml(packet.memory_policy?.allow_fieldvault_sealing ? 'on' : 'off')}</div>
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
            <div class="meta">digest: ${escapeHtml(summary.semantic_digest)}</div>
          </div>
        `).join('');
      }

      async function refreshInventory() {
        const res = await fetch('/inventory');
        const data = await res.json();
        renderInventory(data);
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
        await Promise.all([refreshInventory(), refreshSummaries(), refreshPromotions(), refreshContextPacket()]);
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
          throw new Error('failed to execute latest dispatch live');
        }
        const result = await res.json();
        executeLiveStatusEl.textContent = `${result.dispatch.provider_lane_id} -> live completed`;
        await Promise.all([
          refreshInventory(),
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
          refreshMandalas(),
          refreshCapsules(),
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
        await Promise.all([refreshInventory(), refreshArtifacts(), refreshEvents()]);
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
        await Promise.all([refreshInventory(), refreshProviders(), refreshEvents()]);
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
              refreshContextPacket().catch(console.error);
            } else if (event.event_type === 'turn_started') {
              refreshInventory().catch(console.error);
              refreshTurns().catch(console.error);
              refreshMemoryCapsules().catch(console.error);
              refreshSummaries().catch(console.error);
              refreshPromotions().catch(console.error);
              refreshContextPacket().catch(console.error);
            } else if (event.event_type === 'tool_started' || event.event_type === 'message_completed') {
              refreshInventory().catch(console.error);
              refreshTurns().catch(console.error);
              refreshDispatches().catch(console.error);
              refreshMemoryCapsules().catch(console.error);
              refreshSummaries().catch(console.error);
              refreshPromotions().catch(console.error);
              refreshContextPacket().catch(console.error);
            } else if (['mandala_bound', 'capsule_installed', 'slot_activated'].includes(event.event_type)) {
              refreshInventory().catch(console.error);
              refreshMandalas().catch(console.error);
              refreshCapsules().catch(console.error);
              refreshSlots().catch(console.error);
            } else if (event.event_type === 'artifact_created') {
              refreshInventory().catch(console.error);
              refreshArtifacts().catch(console.error);
            } else if (event.event_type === 'engine_selected') {
              refreshInventory().catch(console.error);
              refreshProviders().catch(console.error);
              refreshDispatches().catch(console.error);
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

      Promise.all([
        refreshInventory(),
        refreshSessions(),
        refreshTurns(),
        refreshDispatches(),
        refreshMandalas(),
        refreshCapsules(),
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
