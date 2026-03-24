use std::{path::PathBuf, process::Command};

use jimi_kernel::ProviderLaneRecord;

#[derive(Debug, Clone)]
pub enum ProviderAdapterKind {
    CodexCli,
    Unsupported(String),
}

pub trait ProviderAdapter {
    fn label(&self) -> &'static str;
    fn execute(&self, house_root: &PathBuf, provider_prompt: &str) -> Result<String, String>;
}

#[derive(Debug, Default)]
struct CodexCliAdapter;

#[derive(Debug, Clone)]
struct UnsupportedAdapter {
    provider: String,
}

impl ProviderAdapter for CodexCliAdapter {
    fn label(&self) -> &'static str {
        "live_codex"
    }

    fn execute(&self, house_root: &PathBuf, provider_prompt: &str) -> Result<String, String> {
        run_codex_exec(house_root, provider_prompt)
    }
}

impl ProviderAdapter for UnsupportedAdapter {
    fn label(&self) -> &'static str {
        "unsupported"
    }

    fn execute(&self, _house_root: &PathBuf, _provider_prompt: &str) -> Result<String, String> {
        Err(format!(
            "provider adapter not implemented yet: {}",
            self.provider
        ))
    }
}

pub fn resolve_provider_adapter(provider_lane: &ProviderLaneRecord) -> ProviderAdapterKind {
    match provider_lane.provider.as_str() {
        "codex" => ProviderAdapterKind::CodexCli,
        other => ProviderAdapterKind::Unsupported(other.to_string()),
    }
}

pub fn run_provider_adapter(
    adapter: &ProviderAdapterKind,
    house_root: &PathBuf,
    provider_prompt: &str,
) -> Result<String, String> {
    match adapter {
        ProviderAdapterKind::CodexCli => CodexCliAdapter.execute(house_root, provider_prompt),
        ProviderAdapterKind::Unsupported(provider) => UnsupportedAdapter {
            provider: provider.clone(),
        }
        .execute(house_root, provider_prompt),
    }
}

pub fn provider_adapter_label(adapter: &ProviderAdapterKind) -> &'static str {
    match adapter {
        ProviderAdapterKind::CodexCli => CodexCliAdapter.label(),
        ProviderAdapterKind::Unsupported(provider) => UnsupportedAdapter {
            provider: provider.clone(),
        }
        .label(),
    }
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
