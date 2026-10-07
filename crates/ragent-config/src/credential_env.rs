//! Canonical resolution of credential-bearing environment variables.
//!
//! Audit T-111: credential env reads were scattered across the agent, LLM,
//! VCS, and TUI crates with slightly different blank-handling and trimming
//! rules. This module centralises the read so every consumer treats a
//! present-but-blank value the same way and the set of credential variables
//! lives in one place.
//!
//! This is the foundation the provider env-key resolver (audit T-403)
//! consolidates onto.

/// Read a credential environment variable.
///
/// Returns `None` when the variable is unset, present-but-blank, or contains
/// only whitespace. The returned value is trimmed of surrounding whitespace
/// so a value pasted with stray spaces (a common cause of `401`) still works.
#[must_use]
pub fn read_credential_env(name: &str) -> Option<String> {
    let value = std::env::var(name).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Return the first variable in `names` that resolves to a non-blank value.
///
/// The order of `names` is the precedence order (most-specific first).
#[must_use]
pub fn first_credential_env(names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| read_credential_env(name))
}

/// The environment variables a provider may take its API key from, in
/// precedence order.
///
/// An empty slice means the provider has no environment-based key (either it
/// needs none - local Ollama - or it uses a different credential chain, such
/// as Copilot's device-flow token or Bedrock's AWS chain).
#[must_use]
pub fn provider_credential_env_vars(provider_id: &str) -> &'static [&'static str] {
    match provider_id {
        "anthropic" => &["ANTHROPIC_API_KEY"],
        "openai" => &["OPENAI_API_KEY"],
        "gemini" | "google" => &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
        "xai" => &["XAI_API_KEY"],
        "huggingface" => &["HF_TOKEN", "HUGGING_FACE_HUB_TOKEN", "HUGGINGFACE_API_KEY"],
        "generic_openai" => &["GENERIC_OPENAI_API_KEY", "OPENAI_API_KEY"],
        "ollama_cloud" => &["OLLAMA_CLOUD_API_KEY", "OLLAMA_API_KEY"],
        "azure_foundry" => &["AZURE_AI_FOUNDRY_API_KEY"],
        "openrouter" => &["OPENROUTER_API_KEY"],
        "copilot" => &["GITHUB_COPILOT_TOKEN", "GITHUB_TOKEN"],
        _ => &[],
    }
}

/// Resolve a provider's API key from the environment, in precedence order.
///
/// Convenience wrapper over [`provider_credential_env_vars`] and
/// [`first_credential_env`] for callers that do not need the raw variable list.
#[must_use]
pub fn provider_credential_env(provider_id: &str) -> Option<String> {
    first_credential_env(provider_credential_env_vars(provider_id))
}
