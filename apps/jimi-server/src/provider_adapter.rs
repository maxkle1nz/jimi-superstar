use std::{path::PathBuf, process::Command, sync::OnceLock, time::Duration};

use jimi_kernel::ProviderLaneRecord;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub enum ProviderAdapterKind {
    CodexCli,
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
struct AnthropicApiAdapter;

#[derive(Debug, Default)]
struct CopilotApiAdapter;

#[derive(Debug, Clone)]
struct UnsupportedAdapter {
    provider: String,
}

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

#[derive(Debug, Deserialize)]
struct CopilotChatResponse {
    choices: Vec<CopilotChatChoice>,
}

#[derive(Debug, Deserialize)]
struct CopilotChatChoice {
    message: CopilotChatMessage,
}

#[derive(Debug, Deserialize)]
struct CopilotChatMessage {
    content: Option<String>,
}

static COPILOT_TOKEN: OnceLock<String> = OnceLock::new();
const COPILOT_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";
const PROVIDER_TIMEOUT: Duration = Duration::from_secs(30);

fn copilot_token_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("anomaly")
        .join("copilot_token.json")
}

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

pub fn resolve_provider_adapter(provider_lane: &ProviderLaneRecord) -> ProviderAdapterKind {
    match provider_lane.provider.as_str() {
        "codex" => ProviderAdapterKind::CodexCli,
        "anthropic" => ProviderAdapterKind::AnthropicApi,
        "copilot" => ProviderAdapterKind::CopilotApi,
        other => ProviderAdapterKind::Unsupported(other.to_string()),
    }
}

pub fn run_provider_adapter(
    provider_lane: &ProviderLaneRecord,
    adapter: &ProviderAdapterKind,
    house_root: &PathBuf,
    provider_prompt: &str,
) -> Result<String, ProviderExecutionError> {
    match adapter {
        ProviderAdapterKind::CodexCli => {
            CodexCliAdapter.execute(provider_lane, house_root, provider_prompt)
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

pub fn provider_adapter_label(adapter: &ProviderAdapterKind) -> &'static str {
    match adapter {
        ProviderAdapterKind::CodexCli => CodexCliAdapter.label(),
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

fn run_anthropic_messages(
    provider_lane: &ProviderLaneRecord,
    provider_prompt: &str,
) -> Result<String, ProviderExecutionError> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| {
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

/// Obtain a GitHub Copilot OAuth token using the device code flow.
/// The token is cached in a process-global OnceLock so the flow only runs once.
/// If GITHUB_TOKEN is set, it is used directly to exchange for a Copilot token.
pub fn obtain_copilot_token() -> Result<String, ProviderExecutionError> {
    if let Some(token) = COPILOT_TOKEN.get() {
        return Ok(token.clone());
    }

    // Try loading persisted token from disk
    if let Some(saved_token) = load_copilot_token() {
        let _ = COPILOT_TOKEN.set(saved_token.clone());
        eprintln!("  ⛛ copilot token loaded from disk");
        return Ok(saved_token);
    }

    // If the user already has a GitHub token (e.g. from gh CLI), try to use it directly
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
            }
            if let Ok(parsed) = response.json::<CopilotInternalToken>() {
                let _ = COPILOT_TOKEN.set(parsed.token.clone());
                save_copilot_token(&parsed.token);
                return Ok(parsed.token);
            }
        }
    }

    // Device code OAuth flow
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120)) // Device flow needs longer timeout for polling
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

    eprintln!("\n╔══════════════════════════════════════════════════╗");
    eprintln!("║  ⛛ ANOMALY — GitHub Copilot Authorization      ║");
    eprintln!("║                                                  ║");
    eprintln!("║  Go to: {}  ║", device.verification_uri);
    eprintln!("║  Enter code: {:>8}                           ║", device.user_code);
    eprintln!("╚══════════════════════════════════════════════════╝\n");

    let interval = std::time::Duration::from_secs(device.interval.unwrap_or(5));
    let max_attempts = 60;

    for _ in 0..max_attempts {
        std::thread::sleep(interval);

        let token_response = client
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .form(&[
                ("client_id", COPILOT_CLIENT_ID),
                ("device_code", device.device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

        let token_result: CopilotTokenResponse = token_response
            .json()
            .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

        if let Some(access_token) = token_result.access_token {
            // Exchange GitHub OAuth token for Copilot internal token
            let copilot_response = client
                .get("https://api.github.com/copilot_internal/v2/token")
                .header("Authorization", format!("token {}", access_token))
                .header("User-Agent", "anomaly-kernel/1.0")
                .header("Accept", "application/json")
                .send()
                .map_err(|error| ProviderExecutionError::TransportFailure(error.to_string()))?;

            if copilot_response.status().is_success() {
                #[derive(Deserialize)]
                struct CopilotInternalToken {
                    token: String,
                }
                if let Ok(parsed) = copilot_response.json::<CopilotInternalToken>() {
                    let _ = COPILOT_TOKEN.set(parsed.token.clone());
                    save_copilot_token(&parsed.token);
                    eprintln!("  ✔ Copilot token obtained and persisted");
                    return Ok(parsed.token);
                }
            }

            // Fallback: use the OAuth token directly
            let _ = COPILOT_TOKEN.set(access_token.clone());
            save_copilot_token(&access_token);
            eprintln!("  ✔ GitHub token persisted (direct mode)");
            return Ok(access_token);
        }

        if let Some(error) = &token_result.error {
            if error == "authorization_pending" || error == "slow_down" {
                continue;
            }
            return Err(ProviderExecutionError::MissingCredentials(format!(
                "copilot oauth failed: {}",
                error
            )));
        }
    }

    Err(ProviderExecutionError::MissingCredentials(
        "copilot oauth device flow timed out".to_string(),
    ))
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
        .header("Copilot-Integration-Id", "vscode-chat")
        .header("Editor-Version", "anomaly/1.0")
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
        } else {
            ProviderExecutionError::UpstreamRejected(format!(
                "copilot api failed ({}): {}",
                status, body
            ))
        });
    }

    let parsed: CopilotChatResponse = response
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

