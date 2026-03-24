use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::Html,
    routing::get,
    Json, Router,
};
use jimi_kernel::{DurableStore, EventEnvelope, HouseInventory, HouseRuntime, SessionRecord};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

#[derive(Clone)]
struct AppState {
    runtime: Arc<Mutex<HouseRuntime>>,
    store: Arc<Mutex<DurableStore>>,
    events_tx: broadcast::Sender<EventEnvelope>,
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

#[derive(Debug, Serialize)]
struct InventoryResponse {
    inventory: HouseInventory,
    mandalas: Vec<String>,
    slots: Vec<String>,
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
    };

    let app = Router::new()
        .route("/", get(cockpit))
        .route("/health", get(health))
        .route("/sessions", get(list_sessions).post(create_session))
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

async fn list_sessions(
    State(state): State<AppState>,
) -> Json<Vec<SessionRecord>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(
        runtime
            .sessions
            .sessions()
            .into_iter()
            .cloned()
            .collect(),
    )
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

async fn list_events(State(state): State<AppState>) -> Json<Vec<EventEnvelope>> {
    let runtime = state.runtime.lock().expect("runtime lock poisoned");
    Json(runtime.events.all().to_vec())
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

async fn ws_events(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_events(socket, state))
}

fn persist_runtime(
    state: &AppState,
    runtime: &HouseRuntime,
) -> Result<(), (StatusCode, String)> {
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
    socket.send(Message::Text(payload.into())).await.map_err(|_| ())
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
        <div class="panel">
          <h2>Sessions</h2>
          <form class="session-form" id="session-form">
            <input id="session-title" placeholder="Name a new room or mission" value="Boot the next JIMI lane" />
            <button type="submit">Create Session</button>
          </form>
          <div class="list" id="session-list"></div>
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

      const state = {
        sessions: [],
        events: []
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
        await refreshInventory();
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

      function escapeHtml(value) {
        return String(value ?? '')
          .replaceAll('&', '&amp;')
          .replaceAll('<', '&lt;')
          .replaceAll('>', '&gt;')
          .replaceAll('"', '&quot;')
          .replaceAll("'", '&#39;');
      }

      Promise.all([refreshInventory(), refreshSessions(), refreshEvents()])
        .then(connectEvents)
        .catch(console.error);
    </script>
  </body>
</html>
"#;
