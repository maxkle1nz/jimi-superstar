use std::{path::PathBuf, process::Command, sync::OnceLock, time::Duration};

use jimi_kernel::ProviderLaneRecord;
use serde::Deserialize;

// ─── OAuth Token ────────────────────────────────────────────────────────────
// Ported from crush: oauth/token.go — unified token with expiry tracking.

#[derive(Debug, Clone, serde::Serialize, Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: String,
    #[serde(default)]
    pub expires_in: i64,
    #[serde(default)]
    pub expires_at: i64,
}

impl OAuthToken {
    /// Returns true if the token is expired or about to expire (within 10% of
    /// lifetime). Same safety margin logic as crush token.go.
    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        now >= (self.expires_at - self.expires_in / 10)
    }

    pub fn set_expires_at(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.expires_at = now + self.expires_in;
    }

    pub fn set_expires_in(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.expires_in = self.expires_at - now;
    }
}

// ─── Token Store ────────────────────────────────────────────────────────────
// Thread-safe, persistent token store with auto-refresh.

use std::sync::Mutex;

static TOKEN_STORE: OnceLock<Mutex<TokenStore>> = OnceLock::new();

#[derive(Debug, Default, serde::Serialize, Deserialize)]
struct TokenStore {
    tokens: std::collections::HashMap<String, OAuthToken>,
}

impl TokenStore {
    fn store_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join(".config")
            .join("anomaly")
            .join("oauth_tokens.json")
    }

    fn load() -> Self {
        let path = Self::store_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default()
    }

    fn persist(&self) {
        let path = Self::store_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    fn get(&self, provider: &str) -> Option<&OAuthToken> {
        self.tokens.get(provider)
    }

    fn set(&mut self, provider: &str, token: OAuthToken) {
        self.tokens.insert(provider.to_string(), token);
        self.persist();
    }
}

fn token_store() -> &'static Mutex<TokenStore> {
    TOKEN_STORE.get_or_init(|| Mutex::new(TokenStore::load()))
}

// ─── Provider Adapter Kinds ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ProviderAdapterKind {
    CodexCli,
    OpenAiApi,
    AnthropicApi,
    CopilotApi,
    Unsupported(String),
}

#[derive(Debug, Clone)]
pub enum ProviderExecutionError {
    MissingCredentials(String),
    UnsupportedProvider(String),
    TransportFailure(String),
    RateLimited(String),
    UpstreamRejected(String),
    EmptyResponse(String),
    LocalProcessFailure(String),
}

impl ProviderExecutionError {
    pub fn class(&self) -> &'static str {
        match self {
            Self::MissingCredentials(_) => "missing_credentials",
            Self::UnsupportedProvider(_) => "unsupported_provider",
            Self::TransportFailure(_) => "transport_failure",
            Self::RateLimited(_) => "rate_limited",
            Self::UpstreamRejected(_) => "upstream_rejected",
            Self::EmptyResponse(_) => "empty_response",
            Self::LocalProcessFailure(_) => "local_process_failure",
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::MissingCredentials(message)
            | Self::UnsupportedProvider(message)
            | Self::TransportFailure(message)
            | Self::RateLimited(message)
            | Self::UpstreamRejected(message)
            | Self::EmptyResponse(message)
            | Self::LocalProcessFailure(message) => message,
        }
    }

    pub fn should_fallback(&self) -> bool {
        matches!(
            self,
            Self::MissingCredentials(_)
                | Self::UnsupportedProvider(_)
                | Self::TransportFailure(_)
                | Self::RateLimited(_)
                | Self::EmptyResponse(_)
        )
    }

    /// Returns true if this error is a 401 that should trigger token refresh.
    pub fn is_auth_failure(&self) -> bool {
        match self {
            Self::UpstreamRejected(msg) => {
                msg.contains("401") || msg.contains("Unauthorized") || msg.contains("unauthorized")
            }
            _ => false,
        }
    }
}

pub trait ProviderAdapter {
    fn label(&self) -> &'static str;
    fn execute(
        &self,
        provider_lane: &ProviderLaneRecord,
        house_root: &PathBuf,
        provider_prompt: &str,
    ) -> Result<String, ProviderExecutionError>;
}

#[derive(Debug, Default)]
struct CodexCliAdapter;

#[derive(Debug, Default)]
struct OpenAiApiAdapter;

#[derive(Debug, Default)]
struct AnthropicApiAdapter;

#[derive(Debug, Default)]
struct CopilotApiAdapter;

#[derive(Debug, Clone)]
struct UnsupportedAdapter {
    provider: String,
}

// ─── Response Types ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AnthropicMessagesResponse {
    content: Vec<AnthropicContentBlock>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CopilotDeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    interval: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct CopilotTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
}

/// OpenAI chat completion response (used for both Copilot and OpenAI direct).
#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChatChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatChoice {
    message: OpenAiChatMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatMessage {
    content: Option<String>,
}

// ─── Copilot Constants ──────────────────────────────────────────────────────
// Ported from crush: oauth/copilot/http.go — exact same headers.

static COPILOT_TOKEN: OnceLock<String> = OnceLock::new();
const COPILOT_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";
const COPILOT_USER_AGENT: &str = "GitHubCopilotChat/0.32.4";
const COPILOT_EDITOR_VERSION: &str = "vscode/1.105.1";
const COPILOT_EDITOR_PLUGIN_VERSION: &str = "copilot-chat/0.32.4";
const COPILOT_INTEGRATION_ID: &str = "vscode-chat";
const PROVIDER_TIMEOUT: Duration = Duration::from_secs(30);

// ─── Token File Paths ───────────────────────────────────────────────────────

fn copilot_token_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("anomaly")
        .join("copilot_token.json")
}

/// Path where GitHub Copilot (VS Code / crush / opencode) stores OAuth tokens.
/// Ported from crush: oauth/copilot/disk.go.
fn github_copilot_apps_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("github-copilot")
        .join("apps.json")
}

// ─── Token Persistence ─────────────────────────────────────────────────────

fn save_copilot_token(token: &str) {
    let path = copilot_token_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::File::create(&path) {
        let _ = std::io::Write::write_all(
            &mut file,
            serde_json::json!({ "token": token }).to_string().as_bytes(),
        );
        eprintln!("  ⛛ copilot token persisted to {}", path.display());
    }
}

fn load_copilot_token() -> Option<String> {
    let path = copilot_token_path();
    let data = std::fs::read_to_string(&path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&data).ok()?;
    parsed.get("token")?.as_str().map(|s| s.to_string())
}

/// Import OAuth token from GitHub Copilot's native disk store.
/// Ported from crush: oauth/copilot/disk.go — reads the same file that
/// VS Code, crush, opencode, and openfang all share.
fn import_copilot_from_github() -> Option<String> {
    let path = github_copilot_apps_path();
    let data = std::fs::read_to_string(&path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&data).ok()?;
    let obj = parsed.as_object()?;
    // Look for the exact key that crush uses:
    // "github.com:Iv1.b507a08c87ecfe98" -> { "oauth_token": "..." }
    let key = format!("github.com:{}", COPILOT_CLIENT_ID);
    let entry = obj.get(&key)?;
    let oauth_token = entry.get("oauth_token")?.as_str()?;
    eprintln!("  ⛛ imported copilot token from GitHub Copilot disk store");
    Some(oauth_token.to_string())
}

// ─── Provider Resolution ────────────────────────────────────────────────────

pub fn resolve_provider_adapter(provider_lane: &ProviderLaneRecord) -> ProviderAdapterKind {
    match provider_lane.provider.as_str() {
        "codex" => {
            // Model-aware routing: if a model is specified, use Copilot API (supports all models).
            // If model is empty or "codex-cli", use CLI adapter.
            if provider_lane.model.is_empty() || provider_lane.model == "codex-cli" {
                ProviderAdapterKind::CodexCli
            } else {
                // All models (gpt-5.4, o3, o4, gpt-4o, claude-*) route through Copilot API
                ProviderAdapterKind::CopilotApi
            }
        }
        "copilot" => ProviderAdapterKind::CopilotApi,
        // Route model-capable providers through Copilot API (single OAuth token)
        "openai" => ProviderAdapterKind::CopilotApi,
        "anthropic" => ProviderAdapterKind::CopilotApi,
        // Direct API fallbacks (explicit opt-in via provider name)
        "openai_direct" => ProviderAdapterKind::OpenAiApi,
        "anthropic_direct" => ProviderAdapterKind::AnthropicApi,
        other => ProviderAdapterKind::Unsupported(other.to_string()),
    }
}

/// Run a provider adapter with 401 auto-retry. If the first attempt fails
/// with an auth error, refresh the token and retry once. Ported from crush:
/// coordinator.go refreshOAuth2Token pattern.
pub fn run_provider_adapter(
    provider_lane: &ProviderLaneRecord,
    adapter: &ProviderAdapterKind,
    house_root: &PathBuf,
    provider_prompt: &str,
) -> Result<String, ProviderExecutionError> {
    let result = execute_adapter(adapter, provider_lane, house_root, provider_prompt);

    match &result {
        Err(err) if err.is_auth_failure() => {
            eprintln!(
                "  ⛛ 401 detected for {} — attempting token refresh",
                provider_lane.provider
            );
            if refresh_provider_token(&provider_lane.provider) {
                eprintln!("  ⛛ token refreshed — retrying");
                execute_adapter(adapter, provider_lane, house_root, provider_prompt)
            } else {
                result
            }
        }
        _ => result,
    }
}

fn execute_adapter(
    adapter: &ProviderAdapterKind,
    provider_lane: &ProviderLaneRecord,
    house_root: &PathBuf,
    provider_prompt: &str,
) -> Result<String, ProviderExecutionError> {
    match adapter {
        ProviderAdapterKind::CodexCli => {
            CodexCliAdapter.execute(provider_lane, house_root, provider_prompt)
        }
        ProviderAdapterKind::OpenAiApi => {
            OpenAiApiAdapter.execute(provider_lane, house_root, provider_prompt)
        }
        ProviderAdapterKind::AnthropicApi => {
            AnthropicApiAdapter.execute(provider_lane, house_root, provider_prompt)
        }
        ProviderAdapterKind::CopilotApi => {
            CopilotApiAdapter.execute(provider_lane, house_root, provider_prompt)
        }
        ProviderAdapterKind::Unsupported(provider) => UnsupportedAdapter {
            provider: provider.clone(),
        }
        .execute(provider_lane, house_root, provider_prompt),
    }
}

/// Attempt to refresh the OAuth token for a provider.
/// Returns true if refresh succeeded.
fn refresh_provider_token(provider: &str) -> bool {
    match provider {
        "copilot" => {
            // For copilot, the refresh token is the GitHub OAuth token.
            // Re-exchange it for a fresh Copilot internal token.
            let refresh = {
                if let Ok(store) = token_store().lock() {
                    store
                        .get("copilot")
                        .filter(|t| !t.refresh_token.is_empty())
                        .map(|t| t.refresh_token.clone())
                } else {
                    None
                }
            }; // store dropped
            if let Some(refresh) = refresh {
                if let Ok(new_token) = exchange_github_for_copilot_token(&refresh) {
                    if let Ok(mut store) = token_store().lock() {
                        store.set("copilot", new_token);
                        return true;
                    }
                }
            }
            false
        }
        "codex" | "openai" => {
            // Codex/OpenAI OAuth token doesn't expire the same way.
            // Try re-importing from GitHub Copilot disk store.
            if let Some(github_token) = import_copilot_from_github() {
                if let Ok(mut store) = token_store().lock() {
                    store.set(
                        "openai",
                        OAuthToken {
                            access_token: github_token.clone(),
                            refresh_token: github_token,
                            expires_in: 3600,
                            expires_at: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() as i64
                                + 3600,
                        },
                    );
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

pub fn provider_adapter_label(adapter: &ProviderAdapterKind) -> &'static str {
    match adapter {
        ProviderAdapterKind::CodexCli => CodexCliAdapter.label(),
        ProviderAdapterKind::OpenAiApi => OpenAiApiAdapter.label(),
        ProviderAdapterKind::AnthropicApi => AnthropicApiAdapter.label(),
        ProviderAdapterKind::CopilotApi => CopilotApiAdapter.label(),
        ProviderAdapterKind::Unsupported(provider) => UnsupportedAdapter {
            provider: provider.clone(),
        }
        .label(),
    }
}

pub fn fallback_candidates(
    primary_lane: &ProviderLaneRecord,
    all_lanes: &[ProviderLaneRecord],
    ready_providers: &[String],
) -> Vec<ProviderLaneRecord> {
    let mut candidates = all_lanes
        .iter()
        .filter(|lane| lane.provider_lane_id != primary_lane.provider_lane_id)
        .filter(|lane| ready_providers.iter().any(|provider| provider == &lane.provider))
        .cloned()
        .collect::<Vec<_>>();

    let same_group_candidates = candidates
        .iter()
        .filter(|lane| {
            lane.fallback_group.is_some() && lane.fallback_group == primary_lane.fallback_group
        })
        .cloned()
        .collect::<Vec<_>>();

    if !same_group_candidates.is_empty() {
        candidates = same_group_candidates;
    }

    candidates.sort_by(|left, right| {
        let left_same_group = (left.fallback_group == primary_lane.fallback_group
            && left.fallback_group.is_some()) as u8;
        let right_same_group = (right.fallback_group == primary_lane.fallback_group
            && right.fallback_group.is_some()) as u8;
        let left_same_provider = (left.provider == primary_lane.provider) as u8;
        let right_same_provider = (right.provider == primary_lane.provider) as u8;
        let left_primary = (left.routing_mode == "primary") as u8;
        let right_primary = (right.routing_mode == "primary") as u8;

        right_same_group
            .cmp(&left_same_group)
            .then_with(|| right_same_provider.cmp(&left_same_provider))
            .then_with(|| right.priority.cmp(&left.priority))
            .then_with(|| right_primary.cmp(&left_primary))
            .then_with(|| left.connected_at.cmp(&right.connected_at))
    });

    candidates
}

// ─── Adapter Implementations ────────────────────────────────────────────────

impl ProviderAdapter for CodexCliAdapter {
    fn label(&self) -> &'static str {
        "live_codex"
    }

    fn execute(
        &self,
        _provider_lane: &ProviderLaneRecord,
        house_root: &PathBuf,
        provider_prompt: &str,
    ) -> Result<String, ProviderExecutionError> {
        run_codex_exec(house_root, provider_prompt)
    }
}

impl ProviderAdapter for OpenAiApiAdapter {
    fn label(&self) -> &'static str {
        "openai_api"
    }

    fn execute(
        &self,
        provider_lane: &ProviderLaneRecord,
        _house_root: &PathBuf,
        provider_prompt: &str,
    ) -> Result<String, ProviderExecutionError> {
        run_openai_chat(provider_lane, provider_prompt)
    }
}

impl ProviderAdapter for UnsupportedAdapter {
    fn label(&self) -> &'static str {
        "unsupported"
    }

    fn execute(
        &self,
        _provider_lane: &ProviderLaneRecord,
        _house_root: &PathBuf,
        _provider_prompt: &str,
    ) -> Result<String, ProviderExecutionError> {
        Err(ProviderExecutionError::UnsupportedProvider(format!(
            "provider adapter not implemented yet: {}",
            self.provider
        )))
    }
}

impl ProviderAdapter for AnthropicApiAdapter {
    fn label(&self) -> &'static str {
        "anthropic_api"
    }

    fn execute(
        &self,
        provider_lane: &ProviderLaneRecord,
        _house_root: &PathBuf,
        provider_prompt: &str,
    ) -> Result<String, ProviderExecutionError> {
        run_anthropic_messages(provider_lane, provider_prompt)
    }
}

impl ProviderAdapter for CopilotApiAdapter {
    fn label(&self) -> &'static str {
        "copilot_api"
    }

    fn execute(
        &self,
        provider_lane: &ProviderLaneRecord,
        _house_root: &PathBuf,
        provider_prompt: &str,
    ) -> Result<String, ProviderExecutionError> {
        run_copilot_chat(provider_lane, provider_prompt)
    }
}

// ─── Codex CLI ──────────────────────────────────────────────────────────────

fn run_codex_exec(
    house_root: &PathBuf,
    provider_prompt: &str,
) -> Result<String, ProviderExecutionError> {
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
        .map_err(|error| ProviderExecutionError::LocalProcessFailure(error.to_string()))?;

    if !status.success() {
        let _ = std::fs::remove_file(&output_path);
        return Err(ProviderExecutionError::LocalProcessFailure(format!(
            "codex exec failed with status {}",
            status
        )));
    }

    let output = std::fs::read_to_string(&output_path)
        .map_err(|error| ProviderExecutionError::LocalProcessFailure(error.to_string()))?;
    let _ = std::fs::remove_file(output_path);
    Ok(output.trim().to_string())
}

// ─── OpenAI Direct API ──────────────────────────────────────────────────────
// Uses Codex OAuth token for direct OpenAI API access (gpt-5.4, o3, etc).
// Rule from ROOMANIZER.md: "OpenAI OAuth = OK (Codex OAuth→API direto permitido)"

fn obtain_openai_token() -> Result<String, ProviderExecutionError> {
    // 1. Check token store
    if let Ok(store) = token_store().lock() {
        if let Some(token) = store.get("openai") {
            if !token.is_expired() {
                return Ok(token.access_token.clone());
            }
        }
    }

    // 2. Try OPENAI_API_KEY env var
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        return Ok(key);
    }

    // 3. Import from GitHub Copilot disk store (shared with crush/VS Code)
    if let Some(github_token) = import_copilot_from_github() {
        // Store it for future use
        if let Ok(mut store) = token_store().lock() {
            let mut token = OAuthToken {
                access_token: github_token.clone(),
                refresh_token: github_token.clone(),
                expires_in: 3600,
                expires_at: 0,
            };
            token.set_expires_at();
            store.set("openai", token);
        }
        return Ok(github_token);
    }

    // 4. Try Codex CLI environment (codex shares the same OAuth)
    if let Ok(codex_token) = std::env::var("CODEX_TOKEN") {
        return Ok(codex_token);
    }

    Err(ProviderExecutionError::MissingCredentials(
        "No OpenAI/Codex token found. Set OPENAI_API_KEY, have Codex CLI logged in, or have GitHub Copilot authenticated.".to_string(),
    ))
}

fn run_openai_chat(
    provider_lane: &ProviderLaneRecord,
    provider_prompt: &str,
) -> Result<String, ProviderExecutionError> {
    let token = obtain_openai_token()?;
    let model = if provider_lane.model.is_empty() {
        "gpt-5.4"
    } else {
        &provider_lane.model
    };

    eprintln!(
        "  ⛛ openai: calling {} with {} bytes of context",
        model,
        provider_prompt.len()
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(PROVIDER_TIMEOUT)
        .build()
        .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are ANOMALY, a sovereign agent house runtime. Respond concisely and precisely."
                },
                {
                    "role": "user",
                    "content": provider_prompt,
                }
            ],
            "max_tokens": 4096,
            "stream": false
        }))
        .send()
        .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_else(|_| "<no body>".into());
        return Err(if status.as_u16() == 429 {
            ProviderExecutionError::RateLimited(format!(
                "openai api rate limited ({}): {}",
                status, body
            ))
        } else if status.as_u16() == 401 {
            ProviderExecutionError::UpstreamRejected(format!(
                "openai api 401 Unauthorized: {}",
                body
            ))
        } else {
            ProviderExecutionError::UpstreamRejected(format!(
                "openai api failed ({}): {}",
                status, body
            ))
        });
    }

    let parsed: OpenAiChatResponse = response
        .json()
        .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

    let text = parsed
        .choices
        .into_iter()
        .filter_map(|choice| choice.message.content)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    if text.is_empty() {
        Err(ProviderExecutionError::EmptyResponse(
            "openai returned no content".into(),
        ))
    } else {
        Ok(text)
    }
}

// ─── Anthropic API ──────────────────────────────────────────────────────────

fn run_anthropic_messages(
    provider_lane: &ProviderLaneRecord,
    provider_prompt: &str,
) -> Result<String, ProviderExecutionError> {
    let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
        ProviderExecutionError::MissingCredentials(
            "ANTHROPIC_API_KEY is not set for anthropic provider lane".to_string(),
        )
    })?;

    let client = reqwest::blocking::Client::builder()
        .timeout(PROVIDER_TIMEOUT)
        .build()
        .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({
            "model": provider_lane.model,
            "max_tokens": 1024,
            "messages": [
                {
                    "role": "user",
                    "content": provider_prompt,
                }
            ]
        }))
        .send()
        .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_else(|_| "<no body>".into());
        return Err(if status.as_u16() == 429 {
            ProviderExecutionError::RateLimited(format!(
                "anthropic api failed with status {}: {}",
                status, body
            ))
        } else {
            ProviderExecutionError::UpstreamRejected(format!(
                "anthropic api failed with status {}: {}",
                status, body
            ))
        });
    }

    let parsed: AnthropicMessagesResponse = response
        .json()
        .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;
    let text = parsed
        .content
        .into_iter()
        .filter(|block| block.block_type == "text")
        .filter_map(|block| block.text)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    if text.is_empty() {
        Err(ProviderExecutionError::EmptyResponse(
            "anthropic api returned no text content".into(),
        ))
    } else {
        Ok(text)
    }
}

// ─── Copilot API ────────────────────────────────────────────────────────────
// Enhanced with: disk import, X-Initiator header, proper token lifecycle.

/// Obtain a GitHub Copilot OAuth token using cascading credential sources.
/// Priority: OnceLock cache → TokenStore → disk (ANOMALY) → disk (GitHub
/// Copilot/VS Code/crush) → GITHUB_TOKEN → Device Code Flow.
pub fn obtain_copilot_token() -> Result<String, ProviderExecutionError> {
    // 1. Process-global cache (fastest path)
    if let Some(token) = COPILOT_TOKEN.get() {
        return Ok(token.clone());
    }

    // 2. Token store (persistent, with expiry tracking)
    let store_refresh: Option<(Option<String>, String)> = {
        if let Ok(store) = token_store().lock() {
            if let Some(token) = store.get("copilot") {
                if !token.is_expired() {
                    let _ = COPILOT_TOKEN.set(token.access_token.clone());
                    return Ok(token.access_token.clone());
                }
                if !token.refresh_token.is_empty() {
                    Some((None, token.refresh_token.clone()))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }; // store dropped here

    if let Some((_, refresh)) = store_refresh {
        if let Ok(new_token) = exchange_github_for_copilot_token(&refresh) {
            let access = new_token.access_token.clone();
            if let Ok(mut store) = token_store().lock() {
                store.set("copilot", new_token);
            }
            let _ = COPILOT_TOKEN.set(access.clone());
            eprintln!("  ⛛ copilot token refreshed from store");
            return Ok(access);
        }
    }

    // 3. ANOMALY's own persisted token
    if let Some(saved_token) = load_copilot_token() {
        let _ = COPILOT_TOKEN.set(saved_token.clone());
        eprintln!("  ⛛ copilot token loaded from disk");
        return Ok(saved_token);
    }

    // 4. Import from GitHub Copilot native disk store (shared with crush/VS Code)
    if let Some(github_oauth_token) = import_copilot_from_github() {
        // Exchange for Copilot internal token
        if let Ok(copilot_token) = exchange_github_for_copilot_token(&github_oauth_token) {
            let access = copilot_token.access_token.clone();
            save_copilot_token(&access);
            if let Ok(mut store) = token_store().lock() {
                store.set("copilot", copilot_token);
            }
            let _ = COPILOT_TOKEN.set(access.clone());
            return Ok(access);
        }
    }

    // 5. GITHUB_TOKEN environment variable
    if let Ok(gh_token) = std::env::var("GITHUB_TOKEN") {
        let client = reqwest::blocking::Client::builder()
            .timeout(PROVIDER_TIMEOUT)
            .build()
            .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

        let response = client
            .get("https://api.github.com/copilot_internal/v2/token")
            .header("Authorization", format!("token {}", gh_token))
            .header("User-Agent", "anomaly-kernel/1.0")
            .header("Accept", "application/json")
            .send()
            .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

        if response.status().is_success() {
            #[derive(Deserialize)]
            struct CopilotInternalToken {
                token: String,
                expires_at: Option<i64>,
            }
            if let Ok(parsed) = response.json::<CopilotInternalToken>() {
                let token = OAuthToken {
                    access_token: parsed.token.clone(),
                    refresh_token: gh_token,
                    expires_in: 1800,
                    expires_at: parsed.expires_at.unwrap_or(0),
                };
                if let Ok(mut store) = token_store().lock() {
                    store.set("copilot", token);
                }
                let _ = COPILOT_TOKEN.set(parsed.token.clone());
                save_copilot_token(&parsed.token);
                return Ok(parsed.token);
            }
        }
    }

    // 6. No credentials found — signal to chat handler to start UI device flow
    Err(ProviderExecutionError::MissingCredentials(
        "no copilot credentials available — device flow authorization required".to_string(),
    ))
}

/// Exchange a GitHub OAuth token for a Copilot internal API token.
/// Ported from crush: oauth/copilot/oauth.go getCopilotToken + RefreshToken.
fn exchange_github_for_copilot_token(
    github_token: &str,
) -> Result<OAuthToken, ProviderExecutionError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(PROVIDER_TIMEOUT)
        .build()
        .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

    let response = client
        .get("https://api.github.com/copilot_internal/v2/token")
        .header("Authorization", format!("Bearer {}", github_token))
        .header("User-Agent", COPILOT_USER_AGENT)
        .header("Editor-Version", COPILOT_EDITOR_VERSION)
        .header("Editor-Plugin-Version", COPILOT_EDITOR_PLUGIN_VERSION)
        .header("Copilot-Integration-Id", COPILOT_INTEGRATION_ID)
        .header("Accept", "application/json")
        .send()
        .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(ProviderExecutionError::MissingCredentials(format!(
            "copilot token exchange failed ({}): {}",
            status, body
        )));
    }

    #[derive(Deserialize)]
    struct InternalToken {
        token: String,
        expires_at: i64,
    }

    let parsed: InternalToken = response
        .json()
        .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

    let mut token = OAuthToken {
        access_token: parsed.token,
        refresh_token: github_token.to_string(),
        expires_in: 0,
        expires_at: parsed.expires_at,
    };
    token.set_expires_in();

    Ok(token)
}

/// Info returned from the device code request, to be shown in the UI.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceFlowInfo {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub interval: u64,
}

/// Request a device code from GitHub — fast (~1s), non-blocking.
/// Returns the code + URI that should be displayed to the user in the UI.
pub fn request_copilot_device_code() -> Result<DeviceFlowInfo, ProviderExecutionError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

    let device_response = client
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", COPILOT_CLIENT_ID),
            ("scope", "read:user"),
        ])
        .send()
        .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

    let device: CopilotDeviceCodeResponse = device_response
        .json()
        .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

    Ok(DeviceFlowInfo {
        user_code: device.user_code,
        verification_uri: device.verification_uri,
        device_code: device.device_code,
        interval: device.interval.unwrap_or(5),
    })
}

/// Poll GitHub once to check if the user has authorized. Returns Ok(token)
/// if authorized, Err(MissingCredentials("pending")) if still waiting.
pub fn poll_copilot_device_auth(device_code: &str) -> Result<String, ProviderExecutionError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

    let token_response = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", COPILOT_CLIENT_ID),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

    let token_result: CopilotTokenResponse = token_response
        .json()
        .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

    if let Some(access_token) = token_result.access_token {
        // Exchange GitHub OAuth token for Copilot internal token
        match exchange_github_for_copilot_token(&access_token) {
            Ok(copilot_token) => {
                let internal = copilot_token.access_token.clone();
                save_copilot_token(&internal);
                if let Ok(mut store) = token_store().lock() {
                    store.set("copilot", copilot_token);
                }
                let _ = COPILOT_TOKEN.set(internal.clone());
                eprintln!("  ⛛ copilot token obtained and persisted via device flow");
                return Ok(internal);
            }
            Err(_) => {
                let _ = COPILOT_TOKEN.set(access_token.clone());
                save_copilot_token(&access_token);
                eprintln!("  ⛛ GitHub token persisted (direct mode)");
                return Ok(access_token);
            }
        }
    }

    if let Some(error) = &token_result.error {
        if error == "authorization_pending" || error == "slow_down" {
            return Err(ProviderExecutionError::MissingCredentials(
                "pending".to_string(),
            ));
        }
        return Err(ProviderExecutionError::MissingCredentials(format!(
            "copilot oauth failed: {}",
            error
        )));
    }

    Err(ProviderExecutionError::MissingCredentials(
        "pending".to_string(),
    ))
}

/// Device Code OAuth flow — blocking version (used as fallback from
/// obtain_copilot_token). Delegates to request + poll.
fn run_copilot_device_flow() -> Result<String, ProviderExecutionError> {
    let info = request_copilot_device_code()?;
    eprintln!("\n  ⛛ Device flow started — code: {} at {}", info.user_code, info.verification_uri);

    let interval = std::time::Duration::from_secs(info.interval);
    let max_attempts = 60;

    for _ in 0..max_attempts {
        std::thread::sleep(interval);
        match poll_copilot_device_auth(&info.device_code) {
            Ok(token) => return Ok(token),
            Err(ProviderExecutionError::MissingCredentials(ref msg)) if msg == "pending" => {
                continue;
            }
            Err(err) => return Err(err),
        }
    }

    Err(ProviderExecutionError::MissingCredentials(
        "copilot oauth device flow timed out".to_string(),
    ))
}

/// Determine the X-Initiator header value for Copilot requests.
/// Ported from crush: oauth/copilot/client.go — user vs agent based on
/// whether there are prior assistant messages in the prompt.
fn copilot_x_initiator(provider_prompt: &str) -> &'static str {
    if provider_prompt.contains("assistant:") || provider_prompt.contains("\"role\":\"assistant\"")
    {
        "agent"
    } else {
        "user"
    }
}

fn run_copilot_chat(
    provider_lane: &ProviderLaneRecord,
    provider_prompt: &str,
) -> Result<String, ProviderExecutionError> {
    let token = obtain_copilot_token()?;
    let model = if provider_lane.model.is_empty() {
        "gpt-4o"
    } else {
        &provider_lane.model
    };

    let client = reqwest::blocking::Client::builder()
        .timeout(PROVIDER_TIMEOUT)
        .build()
        .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

    let response = client
        .post("https://api.githubcopilot.com/chat/completions")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        // Crush-compatible headers (oauth/copilot/http.go)
        .header("User-Agent", COPILOT_USER_AGENT)
        .header("Editor-Version", COPILOT_EDITOR_VERSION)
        .header("Editor-Plugin-Version", COPILOT_EDITOR_PLUGIN_VERSION)
        .header("Copilot-Integration-Id", COPILOT_INTEGRATION_ID)
        // X-Initiator (crush: copilot/client.go)
        .header("X-Initiator", copilot_x_initiator(provider_prompt))
        .json(&serde_json::json!({
            "model": model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are ANOMALY, a sovereign agent house runtime. Respond concisely and precisely."
                },
                {
                    "role": "user",
                    "content": provider_prompt,
                }
            ],
            "max_tokens": 2048,
            "stream": false
        }))
        .send()
        .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_else(|_| "<no body>".into());
        return Err(if status.as_u16() == 429 {
            ProviderExecutionError::RateLimited(format!(
                "copilot api rate limited ({}): {}",
                status, body
            ))
        } else if status.as_u16() == 401 {
            ProviderExecutionError::UpstreamRejected(format!(
                "copilot api 401 Unauthorized: {}",
                body
            ))
        } else {
            ProviderExecutionError::UpstreamRejected(format!(
                "copilot api failed ({}): {}",
                status, body
            ))
        });
    }

    let parsed: OpenAiChatResponse = response
        .json()
        .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

    let text = parsed
        .choices
        .into_iter()
        .filter_map(|choice| choice.message.content)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    if text.is_empty() {
        Err(ProviderExecutionError::EmptyResponse(
            "copilot returned no content".into(),
        ))
    } else {
        Ok(text)
    }
}
