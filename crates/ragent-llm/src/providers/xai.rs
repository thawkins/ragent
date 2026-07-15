//! xAI Grok model provider implementation.
//!
//! Implements the [`Provider`] trait for the xAI Chat Completions API. The xAI
//! endpoint is OpenAI-compatible, so this provider reuses the existing
//! [`OpenAiClient`] for request building and SSE stream parsing, following the
//! same delegation pattern as `GenericOpenAiProvider` and
//! `AzureFoundryProvider`.
//!
//! # Configuration
//!
//! Set `XAI_API_KEY` as the API key. Optionally set `XAI_API_BASE` for a
//! custom endpoint. The base URL can also be configured via `ragent.json`
//! under `provider.xai.base_url`.
//!
//! # Default endpoint
//!
//! `https://api.x.ai` with path `/v1/chat/completions`.

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;

use crate::llm::LlmClient;
use crate::provider::openai::{OpenAiClient, discover_openai_models};
use crate::{ModelInfo, Provider};
#[cfg(test)]
use ragent_config::{Capabilities, Cost};

/// Default API base URL for the xAI Grok endpoint.
const XAI_API_BASE: &str = "https://api.x.ai";

/// Environment variable for overriding the xAI API base URL.
const XAI_API_BASE_ENV: &str = "XAI_API_BASE";

/// Short aliases for xAI model names, mapping simplified names to canonical
/// model IDs.
const XAI_MODEL_ALIASES: &[(&str, &str)] = &[
    ("grok2", "grok-2"),
    ("grok2mini", "grok-2-mini"),
    ("grok2vision", "grok-2-vision-1212"),
    ("grok3", "grok-3"),
    ("grok3mini", "grok-3-mini"),
    ("grok3minifast", "grok-3-mini-fast"),
];

/// Returns the default xAI Grok model catalog with `provider_id` attached.
///
/// This catalog is only used by tests; the `XaiProvider` itself no longer ships
/// hard-coded default models and discovers them at runtime instead.
#[must_use]
#[cfg(test)]
pub fn xai_default_models(provider_id: &str) -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "grok-3".to_string(),
            provider_id: provider_id.to_string(),
            name: "Grok 3".to_string(),
            cost: Cost {
                input: 3.00,
                output: 15.00,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: vec![],
            },
            context_window: 131_072,
            max_output: Some(16_384),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "grok-3-mini".to_string(),
            provider_id: provider_id.to_string(),
            name: "Grok 3 Mini".to_string(),
            cost: Cost {
                input: 0.35,
                output: 0.50,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: vec![],
            },
            context_window: 131_072,
            max_output: Some(16_384),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "grok-3-mini-fast".to_string(),
            provider_id: provider_id.to_string(),
            name: "Grok 3 Mini Fast".to_string(),
            cost: Cost {
                input: 0.35,
                output: 0.50,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: vec![],
            },
            context_window: 131_072,
            max_output: Some(16_384),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "grok-2".to_string(),
            provider_id: provider_id.to_string(),
            name: "Grok 2".to_string(),
            cost: Cost {
                input: 2.00,
                output: 10.00,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: vec![],
            },
            context_window: 131_072,
            max_output: Some(16_384),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "grok-2-mini".to_string(),
            provider_id: provider_id.to_string(),
            name: "Grok 2 Mini".to_string(),
            cost: Cost {
                input: 0.35,
                output: 0.50,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: vec![],
            },
            context_window: 131_072,
            max_output: Some(16_384),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "grok-2-vision-1212".to_string(),
            provider_id: provider_id.to_string(),
            name: "Grok 2 Vision".to_string(),
            cost: Cost {
                input: 2.00,
                output: 10.00,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: true,
                tool_use: true,
                thinking_levels: vec![],
            },
            context_window: 131_072,
            max_output: Some(16_384),
            request_multiplier: None,
            thinking_config: None,
        },
    ]
}

/// Resolves a model ID or alias to its canonical xAI model ID.
///
/// Handles:
/// - Vendor suffix stripping (`grok-3@xai` → `grok-3`)
/// - Short alias expansion (`grok3` → `grok-3`)
///
/// Returns the resolved model ID unchanged if no alias or suffix applies.
#[must_use]
pub fn resolve_xai_model_id(model_id: &str) -> String {
    // Strip @xai vendor suffix
    let without_suffix = model_id
        .split_once('@')
        .map(|(base, suffix)| {
            if suffix.eq_ignore_ascii_case("xai") {
                base.to_string()
            } else {
                model_id.to_string()
            }
        })
        .unwrap_or_else(|| model_id.to_string());

    // Resolve short aliases (case-insensitive)
    let lower = without_suffix.to_ascii_lowercase();
    for (alias, canonical) in XAI_MODEL_ALIASES {
        if lower == *alias {
            return canonical.to_string();
        }
    }

    without_suffix
}

/// Provider implementation for the xAI Grok Chat Completions API.
///
/// Reuses [`OpenAiClient`] for request building and SSE stream parsing since
/// the xAI endpoint is fully OpenAI-compatible.
pub struct XaiProvider;

#[async_trait::async_trait]
impl Provider for XaiProvider {
    /// Returns `"xai"`.
    fn id(&self) -> &'static str {
        "xai"
    }

    /// Returns `"xAI"`.
    fn name(&self) -> &'static str {
        "xAI"
    }

    fn as_any_static(&self) -> &dyn std::any::Any {
        self
    }

    /// Returns an empty catalog.
    ///
    /// xAI Grok models are discovered at runtime from the `/v1/models`
    /// endpoint; no models are hard-coded.
    fn default_models(&self) -> Vec<ModelInfo> {
        Vec::new()
    }

    /// Discover available models from the xAI `/v1/models` endpoint.
    async fn discover_models(&self) -> Result<Vec<ModelInfo>> {
        let api_key = std::env::var("XAI_API_KEY")
            .ok()
            .filter(|k| !k.is_empty())
            .context("xAI model discovery requires XAI_API_KEY")?;
        let base_url = std::env::var(XAI_API_BASE_ENV)
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| XAI_API_BASE.to_string());
        let models = discover_openai_models(&api_key, &base_url, "xai")
            .await
            .with_context(|| "xAI model discovery failed")?;
        Ok(models)
    }

    /// Creates an [`OpenAiClient`] configured for the xAI endpoint.
    ///
    /// Base URL resolution priority:
    /// 1. `base_url` parameter (from configuration)
    /// 2. `XAI_API_BASE` environment variable
    /// 3. `https://api.x.ai` (default)
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be constructed.
    async fn create_client(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        _options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        let env_endpoint = std::env::var(XAI_API_BASE_ENV)
            .ok()
            .filter(|s| !s.trim().is_empty());
        let resolved_base = base_url
            .or(env_endpoint.as_deref())
            .unwrap_or(XAI_API_BASE)
            .trim_end_matches('/')
            .to_string();
        let client = OpenAiClient::new(api_key, &resolved_base);
        tracing::info!(
            chat_endpoint = %format!("{}/v1/chat/completions", resolved_base),
            models_endpoint = %format!("{}/v1/models", resolved_base),
            "xAI provider connected"
        );
        Ok(Box::new(client))
    }
}

#[cfg(test)]
#[path = "../../tests/inline/xai.rs"]
mod tests_tests;
