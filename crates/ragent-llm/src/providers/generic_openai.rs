//! Generic OpenAI-compatible provider implementation.
//!
//! This provider mirrors the `OpenAI` Chat Completions flow but uses a
//! configurable API base URL, including custom ports.

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;

use crate::llm::LlmClient;
use crate::provider::openai::{OPENAI_API_BASE, OpenAiClient, discover_openai_models};
use crate::{ModelInfo, Provider};

/// Read an environment-supplied base URL and validate its scheme and host.
///
/// Returns the normalised URL (trailing `/` stripped) or `None` when the
/// variable is unset, empty, or not an `http(s)` URL with a host, so a poisoned
/// environment cannot redirect requests to another scheme or hostless target.
fn resolve_validated_env_base(env_key: &str) -> Option<String> {
    let raw = std::env::var(env_key).ok()?;
    if raw.trim().is_empty() {
        return None;
    }
    match super::base_url::validate_base_url("generic_openai", &raw) {
        Ok(valid) => Some(valid),
        Err(e) => {
            tracing::warn!(env = %env_key, error = %e, "ignoring invalid base URL from environment");
            None
        }
    }
}

/// Provider implementation for arbitrary OpenAI-compatible endpoints.
pub struct GenericOpenAiProvider;

impl GenericOpenAiProvider {
    const ENDPOINT_OPTION_KEY: &'static str = "endpoint_url";
    const DEFAULT_ENV_ENDPOINT_KEY: &'static str = "GENERIC_OPENAI_API_BASE";
}

#[async_trait::async_trait]
impl Provider for GenericOpenAiProvider {
    fn id(&self) -> &'static str {
        "generic_openai"
    }

    fn name(&self) -> &'static str {
        "Generic OpenAI API"
    }

    fn as_any_static(&self) -> &dyn std::any::Any {
        self
    }

    /// Returns an empty catalog.
    ///
    /// Generic OpenAI-compatible models are discovered at runtime from the
    /// configured endpoint's `/v1/models` endpoint; no models are hard-coded.
    fn default_models(&self) -> Vec<ModelInfo> {
        Vec::new()
    }

    /// Discover available models from the configured `/v1/models` endpoint.
    async fn discover_models(&self) -> Result<Vec<ModelInfo>> {
        let api_key = ragent_config::credential_env::provider_credential_env("generic_openai")
            .context(
                "Generic OpenAI model discovery requires GENERIC_OPENAI_API_KEY or OPENAI_API_KEY",
            )?;
        let base_url = resolve_validated_env_base(Self::DEFAULT_ENV_ENDPOINT_KEY)
            .unwrap_or_else(|| OPENAI_API_BASE.to_string());
        let models = discover_openai_models(&api_key, &base_url, "generic_openai")
            .await
            .with_context(|| "Generic OpenAI model discovery failed")?;
        Ok(models)
    }

    async fn create_client(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        // ANTIPAT 3.6: register the credential with the shared redaction
        // registry so any text passed through `redact_secrets` masks it.
        ragent_types::sanitize::register_secret(api_key);
        let env_endpoint = resolve_validated_env_base(Self::DEFAULT_ENV_ENDPOINT_KEY);
        let configured_endpoint = options
            .get(Self::ENDPOINT_OPTION_KEY)
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty());
        let resolved_base = configured_endpoint
            .or(base_url)
            .or(env_endpoint.as_deref())
            .unwrap_or(OPENAI_API_BASE)
            .trim_end_matches('/')
            .to_string();
        let client = OpenAiClient::new(api_key, &resolved_base);
        tracing::info!(chat_endpoint = %format!("{}/v1/chat/completions", resolved_base), models_endpoint = %format!("{}/v1/models", resolved_base), "Generic OpenAI provider connected");
        Ok(Box::new(client))
    }
}
