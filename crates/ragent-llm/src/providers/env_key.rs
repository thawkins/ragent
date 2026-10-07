//! Canonical provider -> API-key environment resolver (audit T-403).
//!
//! Before T-403 the provider/environment-variable mapping was re-declared in
//! four places - the session processor, the one-shot dispatch helper, the
//! research adapter, and the model-router client - each with subtly different
//! coverage, precedence, and blank/trim rules. This module is the single
//! resolver those call sites now delegate to; it builds on the canonical
//! credential table in [`ragent_config::credential_env`] (T-111).

use ragent_config::credential_env::{first_credential_env, provider_credential_env_vars};

/// The environment variables a provider may take its API key from, in
/// precedence order.
///
/// This is the canonical [`provider_credential_env_vars`] table plus the one
/// provider it deliberately omits: local `ollama` is keyless, but a remote or
/// authenticated Ollama server still accepts `OLLAMA_API_KEY`.
#[must_use]
pub fn provider_env_key_vars(provider_id: &str) -> &'static [&'static str] {
    match provider_id {
        "ollama" => &["OLLAMA_API_KEY"],
        other => provider_credential_env_vars(other),
    }
}

/// Resolve a provider's API key from the process environment.
///
/// Returns `None` when no variable in the provider's precedence list is set to
/// a non-blank value. This is the single place the agent, router, and research
/// paths consult for an environment-supplied key; each caller layers its own
/// storage fallback and provider-specific pre-steps on top.
#[must_use]
pub fn provider_env_key(provider_id: &str) -> Option<String> {
    first_credential_env(provider_env_key_vars(provider_id))
}
