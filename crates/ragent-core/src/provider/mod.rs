//! LLM provider abstraction layer.
//!
//! Defines the [`Provider`] trait for integrating LLM backends (e.g. Anthropic, OpenAI)
//! and [`ProviderRegistry`] for managing and querying available providers and models.

pub mod anthropic;
pub mod copilot;
pub mod ollama;
pub mod openai;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::config::{Capabilities, Cost};
use crate::llm::LlmClient;

/// Metadata describing an LLM model, including its cost, capabilities, and context limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Unique model identifier (e.g. `"gpt-4o"`).
    pub id: String,
    /// Identifier of the provider that hosts this model.
    pub provider_id: String,
    /// Human-readable model name.
    pub name: String,
    /// Per-token input/output cost in USD per million tokens.
    pub cost: Cost,
    /// Feature flags indicating what the model supports.
    pub capabilities: Capabilities,
    /// Maximum number of tokens in the model's context window.
    pub context_window: usize,
    /// Maximum number of output tokens, if limited.
    pub max_output: Option<usize>,
}

/// Summary information about a provider and the models it offers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    /// Unique provider identifier (e.g. `"anthropic"`).
    pub id: String,
    /// Human-readable provider name.
    pub name: String,
    /// Default models available from this provider.
    pub models: Vec<ModelInfo>,
}

/// Trait for LLM provider backends.
///
/// Implementors supply model metadata and can construct an [`LlmClient`] for
/// making chat completion requests against the provider's API.
#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    /// Returns the unique identifier for this provider (e.g. `"openai"`).
    fn id(&self) -> &str;
    /// Returns the human-readable display name (e.g. `"OpenAI"`).
    fn name(&self) -> &str;
    /// Returns the list of models available by default from this provider.
    fn default_models(&self) -> Vec<ModelInfo>;
    /// Creates an authenticated [`LlmClient`] for this provider.
    ///
    /// # Errors
    ///
    /// Returns an error if the client cannot be constructed (e.g. invalid credentials).
    async fn create_client(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        options: &HashMap<String, serde_json::Value>,
    ) -> Result<Box<dyn LlmClient>>;
}

/// Registry that holds all available [`Provider`] implementations and supports
/// lookup by provider ID and model resolution.
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn Provider>>,
}

impl ProviderRegistry {
    /// Creates an empty provider registry.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Registers a provider, keyed by its [`Provider::id`].
    pub fn register(&mut self, provider: Box<dyn Provider>) {
        self.providers.insert(provider.id().to_string(), provider);
    }

    /// Returns a reference to the provider with the given `id`, if registered.
    pub fn get(&self, id: &str) -> Option<&dyn Provider> {
        self.providers.get(id).map(|p| p.as_ref())
    }

    /// Returns [`ProviderInfo`] for every registered provider.
    pub fn list(&self) -> Vec<ProviderInfo> {
        self.providers
            .values()
            .map(|p| ProviderInfo {
                id: p.id().to_string(),
                name: p.name().to_string(),
                models: p.default_models(),
            })
            .collect()
    }

    /// Looks up a specific model by provider and model ID.
    ///
    /// Returns `None` if the provider or model is not found.
    pub fn resolve_model(&self, provider_id: &str, model_id: &str) -> Option<ModelInfo> {
        self.providers
            .get(provider_id)
            .and_then(|p| p.default_models().into_iter().find(|m| m.id == model_id))
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates a [`ProviderRegistry`] pre-populated with the built-in Anthropic and OpenAI providers.
pub fn create_default_registry() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(anthropic::AnthropicProvider));
    registry.register(Box::new(copilot::CopilotProvider::new()));
    registry.register(Box::new(openai::OpenAiProvider));
    registry.register(Box::new(ollama::OllamaProvider::new()));
    registry
}
