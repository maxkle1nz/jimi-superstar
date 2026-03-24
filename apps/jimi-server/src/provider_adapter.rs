use std::{path::PathBuf, process::Command};

use jimi_kernel::ProviderLaneRecord;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub enum ProviderAdapterKind {
    CodexCli,
    AnthropicApi,
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

pub fn resolve_provider_adapter(provider_lane: &ProviderLaneRecord) -> ProviderAdapterKind {
    match provider_lane.provider.as_str() {
        "codex" => ProviderAdapterKind::CodexCli,
        "anthropic" => ProviderAdapterKind::AnthropicApi,
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

    candidates.sort_by(|left, right| {
        let left_same_provider = (left.provider == primary_lane.provider) as u8;
        let right_same_provider = (right.provider == primary_lane.provider) as u8;
        let left_primary = (left.routing_mode == "primary") as u8;
        let right_primary = (right.routing_mode == "primary") as u8;

        right_same_provider
            .cmp(&left_same_provider)
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
