//! LLM provider abstraction layer.
//!
//! Defines the [`Provider`] trait for integrating LLM backends (e.g. Anthropic, `OpenAI`)
//! and [`ProviderRegistry`] for managing and querying available providers and models.

pub mod anthropic;
pub mod copilot;
pub mod gemini;
pub mod generic_openai;
pub mod http_client;
pub mod huggingface;
pub mod ollama;
pub mod ollama_cloud;
pub mod openai;
mod thinking;

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
    /// Premium request multiplier for Copilot models (e.g., 0.33, 1.0, 3.0).
    /// `None` for non-Copilot providers or models without multiplier info.
    /// This is the billing multiplier per GitHub Copilot documentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_multiplier: Option<f64>,
    /// Default or user-configured thinking configuration for this model.
    /// When `None`, the model uses its built-in default (Auto for reasoning-capable, Off for others).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<ragent_types::ThinkingConfig>,
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

/// Plan-based usage information returned by [`Provider::fetch_usage`].
#[derive(Debug, Clone)]
pub struct UsageInfo {
    /// Human-readable plan or tier label (e.g. `"Pro"`, `"Free"`, `"Business"`).
    /// `None` if the provider does not expose plan information.
    pub plan: Option<String>,
    /// Usage as a percentage of the plan's quota (0.0–100.0).
    /// `None` if the provider cannot determine quota usage.
    pub percent: Option<f32>,
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

    /// Returns plan and quota usage information for the current API key.
    ///
    /// Providers that do not expose plan information return `None`. The default
    /// implementation always returns `None`; providers override this to surface
    /// plan labels and quota percentages.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use ragent_llm::provider::copilot::CopilotProvider;
    /// # use ragent_llm::provider::Provider;
    /// # async fn example() {
    /// let provider = CopilotProvider::new();
    /// if let Some(info) = provider.fetch_usage("ghu_xxx").await {
    ///     println!("Plan: {:?}, Usage: {:?}%", info.plan, info.percent);
    /// }
    /// # }
    /// ```
    async fn fetch_usage(&self, api_key: &str) -> Option<UsageInfo> {
        let _ = api_key;
        None
    }
}

/// Registry that holds all available [`Provider`] implementations and supports
/// lookup by provider ID and model resolution.
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn Provider>>,
}

impl ProviderRegistry {
    /// Creates an empty provider registry.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_llm::provider::ProviderRegistry;
    ///
    /// let registry = ProviderRegistry::new();
    /// assert!(registry.list().is_empty());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Registers a provider, keyed by its [`Provider::id`].
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_llm::provider::{ProviderRegistry, openai::OpenAiProvider};
    ///
    /// let mut registry = ProviderRegistry::new();
    /// registry.register(Box::new(OpenAiProvider));
    /// assert_eq!(registry.list().len(), 1);
    /// ```
    pub fn register(&mut self, provider: Box<dyn Provider>) {
        self.providers.insert(provider.id().to_string(), provider);
    }

    /// Returns a reference to the provider with the given `id`, if registered.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_llm::provider::create_default_registry;
    ///
    /// let registry = create_default_registry();
    /// assert!(registry.get("anthropic").is_some());
    /// assert!(registry.get("nonexistent").is_none());
    /// ```
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&dyn Provider> {
        self.providers.get(id).map(std::convert::AsRef::as_ref)
    }

    /// Returns [`ProviderInfo`] for every registered provider.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_llm::provider::create_default_registry;
    ///
    /// let registry = create_default_registry();
    /// let providers = registry.list();
    /// assert!(!providers.is_empty());
    /// ```
    #[must_use]
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
    /// The `model_id` parameter supports multiple formats:
    /// - Exact model ID (e.g. `"gpt-4o"`)
    /// - Display name match (e.g. `"GPT-4o"`)
    /// - Model with vendor suffix (e.g. `"gpt-4o@azure"` or `"gpt-4o@openai"`)
    ///
    /// Returns `None` if the provider is not found or no matching model exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_llm::provider::ProviderRegistry;
    ///
    /// let registry = ProviderRegistry::new();
    /// // No providers registered, so resolution returns `None`.
    /// assert!(registry.resolve_model("openai", "gpt-4o").is_none());
    /// ```
    pub fn resolve_model(&self, provider_id: &str, model_id: &str) -> Option<ModelInfo> {
        let provider = self.providers.get(provider_id)?;
        let models = provider.default_models();

        // First try exact ID match
        if let Some(model) = models.iter().find(|m| m.id == model_id) {
            return Some(model.clone());
        }

        // D2 fix: Strip vendor suffix (e.g., "gpt-4o@azure" -> "gpt-4o")
        let model_id_without_suffix = model_id
            .split_once('@')
            .map(|(base, _)| base)
            .unwrap_or(model_id);
        if let Some(model) = models.iter().find(|m| m.id == model_id_without_suffix) {
            return Some(model.clone());
        }

        // Try display name match (case-insensitive)
        let model_lower = model_id.to_lowercase();
        if let Some(model) = models.iter().find(|m| m.name.to_lowercase() == model_lower) {
            return Some(model.clone());
        }

        None
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates a [`ProviderRegistry`] pre-populated with the built-in providers.
///
/// # Examples
///
/// ```
/// use ragent_llm::provider::create_default_registry;
///
/// let registry = create_default_registry();
/// assert!(registry.get("anthropic").is_some());
/// assert!(registry.get("openai").is_some());
/// assert!(registry.get("generic_openai").is_some());
/// ```
#[must_use]
pub fn create_default_registry() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(anthropic::AnthropicProvider));
    registry.register(Box::new(copilot::CopilotProvider::new()));
    registry.register(Box::new(gemini::GeminiProvider));
    registry.register(Box::new(openai::OpenAiProvider));
    registry.register(Box::new(huggingface::HuggingFaceProvider));
    registry.register(Box::new(generic_openai::GenericOpenAiProvider));
    registry.register(Box::new(ollama_cloud::OllamaCloudProvider::new()));
    registry.register(Box::new(ollama::OllamaProvider::new()));
    registry
}
