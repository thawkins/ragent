//! Registry wiring for Rig-backed providers (T-006).
//!
//! This module exposes [`register_rig_providers`], which turns each
//! [`RigProviderConfig`] alias into a first-class ragent [`Provider`] in the
//! [`ProviderRegistry`].  Keeping the wiring here avoids a dependency cycle:
//! `ragent-rig` depends on `ragent-llm`, but the reverse is not true.

use std::collections::HashMap;

use ragent_config::{Capabilities, Config, Cost, RigProviderConfig};
use ragent_llm::llm::LlmClient;
use ragent_llm::provider::{ModelInfo, Provider, ProviderRegistry};

use crate::provider::{RigLlmClient, build_backend_by_provider};

/// A ragent [`Provider`] backed by a Rig `CompletionModel`.
///
/// Each configured Rig provider alias becomes one instance of this type,
/// registered under the alias as the provider id.  The model advertised by
/// [`Provider::default_models`] is the model from the Rig provider config.
#[derive(Clone, Debug)]
pub struct RigProvider {
    alias: String,
    provider_id: String,
    model_id: String,
    api_key: Option<String>,
    base_url: Option<String>,
    streaming: bool,
}

impl RigProvider {
    /// Construct a Rig provider from a config entry.
    #[must_use]
    pub fn new(config: &RigProviderConfig) -> Self {
        Self {
            alias: config.alias.clone(),
            provider_id: config.provider.clone(),
            model_id: config.model.clone(),
            api_key: config.api_key.clone(),
            base_url: config.base_url.clone(),
            streaming: config.streaming,
        }
    }

    /// Resolve the effective API key for this provider alias.
    ///
    /// Order of precedence:
    /// 1. The `api_key` field in `RigProviderConfig`.
    /// 2. The `RIG_<provider>_API_KEY` environment variable, normalised from
    ///    the Rig provider id (e.g. `RIG_OPENAI_API_KEY` for `openai`).
    /// 3. The underlying Rig provider's conventional env var as a fallback.
    #[must_use]
    pub fn resolve_api_key(&self) -> String {
        if let Some(key) = self.api_key.as_ref() {
            return key.clone();
        }
        let rig_var = format!(
            "RIG_{}_API_KEY",
            self.provider_id.to_uppercase().replace('-', "_")
        );
        std::env::var(&rig_var)
            .or_else(|_| match self.provider_id.as_str() {
                "openai" => std::env::var("OPENAI_API_KEY"),
                "anthropic" => std::env::var("ANTHROPIC_API_KEY"),
                "gemini" => std::env::var("GEMINI_API_KEY"),
                "ollama" => std::env::var("OLLAMA_API_KEY"),
                _ => Err(std::env::VarError::NotPresent),
            })
            .unwrap_or_default()
    }
}

#[async_trait::async_trait]
impl Provider for RigProvider {
    fn id(&self) -> &str {
        &self.alias
    }

    fn name(&self) -> &str {
        &self.alias
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: self.model_id.clone(),
            provider_id: self.alias.clone(),
            name: format!("{} {}", self.provider_id, self.model_id),
            cost: Cost::default(),
            capabilities: Capabilities {
                streaming: self.streaming,
                tool_use: true,
                ..Capabilities::default()
            },
            context_window: 128_000,
            max_output: None,
            request_multiplier: None,
            thinking_config: None,
        }]
    }

    async fn create_client(
        &self,
        api_key: &str,
        _base_url: Option<&str>,
        _options: &HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<Box<dyn LlmClient>> {
        let api_key = if api_key.is_empty() {
            self.resolve_api_key()
        } else {
            api_key.to_owned()
        };
        let backend = build_backend_by_provider(
            self.alias.clone(),
            &self.provider_id,
            &api_key,
            self.base_url.as_deref(),
            self.model_id.clone(),
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(Box::new(RigLlmClient::new(Box::new(backend))))
    }
}

/// Register a [`RigProvider`] for every provider entry in `config.rig`.
///
/// Aliases must be unique within the registry; a duplicate alias logs a
/// warning and is skipped so a malformed config cannot shadow another provider.
/// This function is a no-op when `config.rig` is `None` or contains no
/// providers.
pub fn register_rig_providers(config: &Config, registry: &mut ProviderRegistry) {
    let Some(rig_config) = config.rig.as_ref() else {
        return;
    };

    for provider_cfg in &rig_config.providers {
        if provider_cfg.alias.is_empty() {
            tracing::warn!(
                provider = %provider_cfg.provider,
                model = %provider_cfg.model,
                "Skipping Rig provider with empty alias"
            );
            continue;
        }

        let existing = registry.list();
        if existing.iter().any(|p| p.id == provider_cfg.alias) {
            tracing::warn!(
                alias = %provider_cfg.alias,
                "Skipping duplicate Rig provider alias"
            );
            continue;
        }

        registry.register(Box::new(RigProvider::new(provider_cfg)));
        tracing::info!(
            alias = %provider_cfg.alias,
            provider = %provider_cfg.provider,
            model = %provider_cfg.model,
            "Registered Rig-backed provider"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rig_provider_exposes_configured_model() {
        let cfg = RigProviderConfig {
            alias: "rig-openai".to_owned(),
            provider: "openai".to_owned(),
            model: "gpt-4o".to_owned(),
            api_key: None,
            base_url: None,
            streaming: true,
        };
        let provider = RigProvider::new(&cfg);
        assert_eq!(provider.id(), "rig-openai");
        let models = provider.default_models();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "gpt-4o");
        assert!(models[0].capabilities.streaming);
    }

    #[test]
    fn register_rig_providers_adds_alias_to_registry() {
        let mut config = Config::default();
        config.rig = Some(ragent_config::RigConfig {
            providers: vec![RigProviderConfig {
                alias: "rig-openai".to_owned(),
                provider: "openai".to_owned(),
                model: "gpt-4o".to_owned(),
                api_key: Some("test-key".to_owned()),
                base_url: None,
                streaming: true,
            }],
            memory: None,
            embeddings: None,
            vector_store: None,
        });
        let mut registry = ProviderRegistry::new();
        register_rig_providers(&config, &mut registry);

        let info = registry.list();
        assert!(info.iter().any(|p| p.id == "rig-openai"));
        assert!(registry.resolve_model("rig-openai", "gpt-4o").is_some());
    }

    #[test]
    fn duplicate_alias_is_skipped() {
        let mut config = Config::default();
        config.rig = Some(ragent_config::RigConfig {
            providers: vec![
                RigProviderConfig {
                    alias: "rig-openai".to_owned(),
                    provider: "openai".to_owned(),
                    model: "gpt-4o".to_owned(),
                    api_key: None,
                    base_url: None,
                    streaming: true,
                },
                RigProviderConfig {
                    alias: "rig-openai".to_owned(),
                    provider: "anthropic".to_owned(),
                    model: "claude-sonnet-4-20250514".to_owned(),
                    api_key: None,
                    base_url: None,
                    streaming: true,
                },
            ],
            memory: None,
            embeddings: None,
            vector_store: None,
        });
        let mut registry = ProviderRegistry::new();
        register_rig_providers(&config, &mut registry);

        let info = registry.list();
        assert_eq!(info.iter().filter(|p| p.id == "rig-openai").count(), 1);
        // The first provider wins.
        assert!(registry.resolve_model("rig-openai", "gpt-4o").is_some());
        assert!(
            registry
                .resolve_model("rig-openai", "claude-sonnet-4-20250514")
                .is_none()
        );
    }
}
