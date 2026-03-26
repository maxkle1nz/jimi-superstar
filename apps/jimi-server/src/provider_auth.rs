use std::{env, path::PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ProviderCredentialStatus {
    pub provider: String,
    pub ready: bool,
    pub source: String,
    pub details: String,
}

pub fn detect_provider_credentials() -> Vec<ProviderCredentialStatus> {
    vec![
        detect_codex_openai(),
        detect_anthropic(),
        detect_copilot(),
        detect_google(),
        detect_moonshot(),
        detect_openrouter(),
        detect_ollama(),
    ]
}

fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn detect_codex_openai() -> ProviderCredentialStatus {
    let auth_path = home_dir().join(".codex").join("auth.json");
    if auth_path.exists() {
        return ProviderCredentialStatus {
            provider: "codex".into(),
            ready: true,
            source: "cli-oauth".into(),
            details: auth_path.display().to_string(),
        };
    }
    if env::var("OPENAI_API_KEY").is_ok() {
        return ProviderCredentialStatus {
            provider: "codex".into(),
            ready: true,
            source: "env".into(),
            details: "OPENAI_API_KEY".into(),
        };
    }
    ProviderCredentialStatus {
        provider: "codex".into(),
        ready: false,
        source: "missing".into(),
        details: "expected ~/.codex/auth.json or OPENAI_API_KEY".into(),
    }
}

fn detect_anthropic() -> ProviderCredentialStatus {
    let candidate_paths = [
        home_dir().join(".claude").join("credentials.json"),
        home_dir()
            .join(".config")
            .join("claude")
            .join("credentials.json"),
    ];
    if let Some(path) = candidate_paths.iter().find(|path| path.exists()) {
        return ProviderCredentialStatus {
            provider: "anthropic".into(),
            ready: true,
            source: "cli-oauth".into(),
            details: path.display().to_string(),
        };
    }
    if env::var("ANTHROPIC_API_KEY").is_ok() || env::var("ANTHROPIC_OAUTH_TOKEN").is_ok() {
        return ProviderCredentialStatus {
            provider: "anthropic".into(),
            ready: true,
            source: "env".into(),
            details: "ANTHROPIC_API_KEY / ANTHROPIC_OAUTH_TOKEN".into(),
        };
    }
    ProviderCredentialStatus {
        provider: "anthropic".into(),
        ready: false,
        source: "missing".into(),
        details: "expected Claude credentials or ANTHROPIC_API_KEY".into(),
    }
}

fn detect_google() -> ProviderCredentialStatus {
    let candidate_paths = [
        home_dir().join(".gemini").join("oauth_creds.json"),
        home_dir()
            .join(".config")
            .join("gemini")
            .join("api_key"),
        home_dir()
            .join(".config")
            .join("gcloud")
            .join("application_default_credentials.json"),
    ];
    if let Some(path) = candidate_paths.iter().find(|path| path.exists()) {
        return ProviderCredentialStatus {
            provider: "google".into(),
            ready: true,
            source: "cli-or-file".into(),
            details: path.display().to_string(),
        };
    }
    if env::var("GOOGLE_API_KEY").is_ok() || env::var("GEMINI_API_KEY").is_ok() {
        return ProviderCredentialStatus {
            provider: "google".into(),
            ready: true,
            source: "env".into(),
            details: "GOOGLE_API_KEY / GEMINI_API_KEY".into(),
        };
    }
    ProviderCredentialStatus {
        provider: "google".into(),
        ready: false,
        source: "missing".into(),
        details: "expected Gemini credentials, ADC, or GOOGLE_API_KEY".into(),
    }
}

fn detect_moonshot() -> ProviderCredentialStatus {
    let auth_path = home_dir()
        .join(".kimi")
        .join("credentials")
        .join("kimi-code.json");
    if auth_path.exists() {
        return ProviderCredentialStatus {
            provider: "moonshot".into(),
            ready: true,
            source: "cli-oauth".into(),
            details: auth_path.display().to_string(),
        };
    }
    if env::var("MOONSHOT_API_KEY").is_ok() {
        return ProviderCredentialStatus {
            provider: "moonshot".into(),
            ready: true,
            source: "env".into(),
            details: "MOONSHOT_API_KEY".into(),
        };
    }
    ProviderCredentialStatus {
        provider: "moonshot".into(),
        ready: false,
        source: "missing".into(),
        details: "expected Kimi credentials or MOONSHOT_API_KEY".into(),
    }
}

fn detect_openrouter() -> ProviderCredentialStatus {
    if env::var("OPENROUTER_API_KEY").is_ok() {
        return ProviderCredentialStatus {
            provider: "openrouter".into(),
            ready: true,
            source: "env".into(),
            details: "OPENROUTER_API_KEY".into(),
        };
    }
    ProviderCredentialStatus {
        provider: "openrouter".into(),
        ready: false,
        source: "missing".into(),
        details: "expected OPENROUTER_API_KEY".into(),
    }
}

fn detect_ollama() -> ProviderCredentialStatus {
    ProviderCredentialStatus {
        provider: "ollama".into(),
        ready: true,
        source: "local".into(),
        details: "no auth required; local daemon expected".into(),
    }
}

fn detect_copilot() -> ProviderCredentialStatus {
    // Check GITHUB_TOKEN env var
    if env::var("GITHUB_TOKEN").is_ok() {
        return ProviderCredentialStatus {
            provider: "copilot".into(),
            ready: true,
            source: "env".into(),
            details: "GITHUB_TOKEN".into(),
        };
    }
    // Check gh CLI config (login stores tokens here)
    let gh_hosts = home_dir().join(".config").join("gh").join("hosts.yml");
    if gh_hosts.exists() {
        return ProviderCredentialStatus {
            provider: "copilot".into(),
            ready: true,
            source: "cli-oauth".into(),
            details: gh_hosts.display().to_string(),
        };
    }
    ProviderCredentialStatus {
        provider: "copilot".into(),
        ready: false,
        source: "missing".into(),
        details: "expected GITHUB_TOKEN or gh CLI login (supports OAuth device flow)".into(),
    }
}

