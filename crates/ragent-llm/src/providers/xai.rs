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

use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

use crate::llm::LlmClient;
use crate::provider::openai::OpenAiClient;
use crate::{ModelInfo, Provider};
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
/// The catalog includes Grok 3, Grok 3 Mini, Grok 3 Mini Fast, Grok 2,
/// Grok 2 Mini, and Grok 2 Vision. Vision capability is enabled only for
/// models whose IDs contain `"vision"`.
#[must_use]
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

    /// Returns the default xAI Grok model catalog.
    fn default_models(&self) -> Vec<ModelInfo> {
        xai_default_models("xai")
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
mod tests {
    use super::*;

    #[test]
    fn test_provider_id() {
        let provider = XaiProvider;
        assert_eq!(provider.id(), "xai");
    }

    #[test]
    fn test_provider_name() {
        let provider = XaiProvider;
        assert_eq!(provider.name(), "xAI");
    }

    #[test]
    fn test_default_models_count() {
        let models = xai_default_models("xai");
        assert_eq!(models.len(), 6);
    }

    #[test]
    fn test_default_models_ids() {
        let models = xai_default_models("xai");
        let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            ids,
            &[
                "grok-3",
                "grok-3-mini",
                "grok-3-mini-fast",
                "grok-2",
                "grok-2-mini",
                "grok-2-vision-1212",
            ]
        );
    }

    #[test]
    fn test_default_models_provider_id() {
        let models = xai_default_models("xai");
        for model in &models {
            assert_eq!(model.provider_id, "xai");
        }
    }

    #[test]
    fn test_vision_only_for_vision_models() {
        let models = xai_default_models("xai");
        for model in &models {
            let expected_vision = model.id.contains("vision");
            assert_eq!(
                model.capabilities.vision, expected_vision,
                "model {} vision={}; expected={}",
                model.id, model.capabilities.vision, expected_vision
            );
        }
    }

    #[test]
    fn test_all_models_support_tool_use() {
        let models = xai_default_models("xai");
        for model in &models {
            assert!(
                model.capabilities.tool_use,
                "model {} should support tool_use",
                model.id
            );
        }
    }

    #[test]
    fn test_all_models_support_streaming() {
        let models = xai_default_models("xai");
        for model in &models {
            assert!(
                model.capabilities.streaming,
                "model {} should support streaming",
                model.id
            );
        }
    }

    #[test]
    fn test_context_window() {
        let models = xai_default_models("xai");
        for model in &models {
            assert_eq!(
                model.context_window, 131_072,
                "model {} context_window should be 131072",
                model.id
            );
        }
    }

    #[test]
    fn test_max_output() {
        let models = xai_default_models("xai");
        for model in &models {
            assert_eq!(
                model.max_output,
                Some(16_384),
                "model {} max_output should be 16384",
                model.id
            );
        }
    }

    #[test]
    fn test_vendor_suffix_stripping() {
        assert_eq!(resolve_xai_model_id("grok-3@xai"), "grok-3");
        assert_eq!(
            resolve_xai_model_id("grok-2-vision-1212@xai"),
            "grok-2-vision-1212"
        );
        assert_eq!(resolve_xai_model_id("grok-3@XAI"), "grok-3");
    }

    #[test]
    fn test_vendor_suffix_non_xai_unchanged() {
        // Non-xAI suffixes should be left unchanged
        assert_eq!(resolve_xai_model_id("grok-3@other"), "grok-3@other");
    }

    #[test]
    fn test_no_suffix_unchanged() {
        assert_eq!(resolve_xai_model_id("grok-3"), "grok-3");
        assert_eq!(
            resolve_xai_model_id("grok-2-vision-1212"),
            "grok-2-vision-1212"
        );
    }

    #[test]
    fn test_alias_resolution() {
        assert_eq!(resolve_xai_model_id("grok3"), "grok-3");
        assert_eq!(resolve_xai_model_id("grok2"), "grok-2");
        assert_eq!(resolve_xai_model_id("grok2mini"), "grok-2-mini");
        assert_eq!(resolve_xai_model_id("grok2vision"), "grok-2-vision-1212");
        assert_eq!(resolve_xai_model_id("grok3mini"), "grok-3-mini");
        assert_eq!(resolve_xai_model_id("grok3minifast"), "grok-3-mini-fast");
    }

    #[test]
    fn test_alias_case_insensitive() {
        assert_eq!(resolve_xai_model_id("Grok3"), "grok-3");
        assert_eq!(resolve_xai_model_id("GROK2"), "grok-2");
    }

    #[test]
    fn test_base_url_default() {
        // When no env var and no base_url, should default to XAI_API_BASE
        // We can't easily test env var behavior in unit tests, but we verify
        // the constant is correct.
        assert_eq!(XAI_API_BASE, "https://api.x.ai");
    }

    #[test]
    fn test_base_url_env_key() {
        assert_eq!(XAI_API_BASE_ENV, "XAI_API_BASE");
    }

    #[test]
    fn test_model_costs() {
        let models = xai_default_models("xai");
        let grok3 = models.iter().find(|m| m.id == "grok-3").unwrap();
        assert_eq!(grok3.cost.input, 3.00);
        assert_eq!(grok3.cost.output, 15.00);

        let grok3mini = models.iter().find(|m| m.id == "grok-3-mini").unwrap();
        assert_eq!(grok3mini.cost.input, 0.35);
        assert_eq!(grok3mini.cost.output, 0.50);

        let grok2 = models.iter().find(|m| m.id == "grok-2").unwrap();
        assert_eq!(grok2.cost.input, 2.00);
        assert_eq!(grok2.cost.output, 10.00);

        let grok2vision = models
            .iter()
            .find(|m| m.id == "grok-2-vision-1212")
            .unwrap();
        assert_eq!(grok2vision.cost.input, 2.00);
        assert_eq!(grok2vision.cost.output, 10.00);
    }
}
