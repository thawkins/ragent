//! Virtual provider implementation for the Model Router.
//!
//! Implements the [`Provider`] trait for the router, which acts as a virtual
//! provider that classifies prompts across 15 dimensions and routes each
//! request to the cheapest model that can satisfy it (FR-001, FR-003).
//!
//! The router is registered in the default provider registry alongside
//! concrete providers like Anthropic and OpenAI. When selected as the active
//! provider, it intercepts chat requests, classifies the prompt, and delegates
//! to the resolved provider/model pair.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use serde_json::Value;

use super::router_config::RouterConfig;
use crate::llm::LlmClient;
use crate::provider::{ModelInfo, Provider, ProviderRegistry};

/// Virtual provider that implements intelligent model routing.
///
/// The `RouterProvider` presents itself as a standard `Provider` with
/// `id() == "router"` and `name() == "Model Router"`. It does not directly
/// handle LLM requests; instead, it creates a [`RouterClient`] that
/// classifies prompts and delegates to the appropriate provider/model pair.
pub struct RouterProvider {
    /// Current router configuration.
    config: std::sync::RwLock<RouterConfig>,
    /// Reference to the provider registry, set after the registry is created
    /// so the router can delegate requests to concrete providers.
    registry: std::sync::RwLock<Option<Arc<ProviderRegistry>>>,
    /// Optional storage handle for resolving database-backed provider API keys.
    /// Set by the binary/TUI after the registry and storage are created.
    storage: std::sync::RwLock<Option<Arc<ragent_storage::Storage>>>,
    /// Optional event bus for publishing router lifecycle events.
    event_bus: std::sync::RwLock<Option<Arc<ragent_types::event::EventBus>>>,
}

impl RouterProvider {
    /// Create a new `RouterProvider` with the given configuration.
    pub fn new(config: RouterConfig) -> Self {
        Self {
            config: std::sync::RwLock::new(config),
            registry: std::sync::RwLock::new(None),
            storage: std::sync::RwLock::new(None),
            event_bus: std::sync::RwLock::new(None),
        }
    }

    /// Create a `RouterProvider` with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(RouterConfig::default())
    }

    /// Returns a snapshot of the current router configuration.
    ///
    /// Acquires a read lock on the internal config.
    pub fn config(&self) -> RouterConfig {
        self.config
            .read()
            .expect("router config lock poisoned")
            .clone()
    }

    /// Reload the router configuration from the provided `RouterConfig`.
    ///
    /// Acquires a write lock and replaces the configuration atomically.
    pub fn reload_config(&self, new_config: RouterConfig) {
        let mut guard = self.config.write().expect("router config lock poisoned");
        *guard = new_config;
        tracing::info!("Router configuration reloaded");
    }

    /// Update whether the router is enabled.
    pub fn set_enabled(&self, enabled: bool) {
        let mut guard = self.config.write().expect("router config lock poisoned");
        guard.enabled = enabled;
        tracing::info!(enabled = enabled, "Router enabled state updated");
    }

    /// Set the provider registry that the router will use to delegate chats.
    ///
    /// Called once after the default registry is created, before the router is
    /// used for chat.
    pub fn set_registry(&self, registry: Arc<ProviderRegistry>) {
        let mut guard = self
            .registry
            .write()
            .expect("router registry lock poisoned");
        *guard = Some(registry);
        tracing::info!("Router provider registry attached");
    }

    /// Returns the attached provider registry, if any.
    pub fn registry(&self) -> Option<Arc<ProviderRegistry>> {
        self.registry
            .read()
            .expect("router registry lock poisoned")
            .clone()
    }

    /// Set the storage handle used to resolve database-backed provider API keys.
    ///
    /// Called once after storage is initialized. When set, the router will
    /// look up stored API keys for downstream providers (e.g. `ollama_cloud`
    /// keys saved via `ragent auth`) before falling back to environment
    /// variables.
    pub fn set_storage(&self, storage: Arc<ragent_storage::Storage>) {
        let mut guard = self.storage.write().expect("router storage lock poisoned");
        *guard = Some(storage);
        tracing::info!("Router storage attached");
    }

    /// Returns the attached storage handle, if any.
    pub fn storage(&self) -> Option<Arc<ragent_storage::Storage>> {
        self.storage
            .read()
            .expect("router storage lock poisoned")
            .clone()
    }

    /// Attach an event bus so the router can publish lifecycle events.
    pub fn set_event_bus(&self, event_bus: Option<Arc<ragent_types::event::EventBus>>) {
        let mut guard = self
            .event_bus
            .write()
            .expect("router event bus lock poisoned");
        *guard = event_bus;
    }

    /// Returns the attached event bus, if any.
    pub fn event_bus(&self) -> Option<Arc<ragent_types::event::EventBus>> {
        self.event_bus
            .read()
            .expect("router event bus lock poisoned")
            .clone()
    }

    /// Returns whether the router is currently enabled.
    pub fn is_enabled(&self) -> bool {
        self.config
            .read()
            .expect("router config lock poisoned")
            .enabled
    }
}

#[async_trait::async_trait]
impl Provider for RouterProvider {
    /// Returns `"router"` (FR-001).
    fn id(&self) -> &'static str {
        "router"
    }

    /// Returns `"Model Router"` (FR-001).
    fn name(&self) -> &'static str {
        "Model Router"
    }

    /// Attach an event bus so the router can publish lifecycle events.
    ///
    /// This overrides the trait default no-op so that provider-registry-wide
    /// event bus attachment (used by the TUI) actually reaches the router and
    /// makes `Event::RouterClassification` visible in the log panel.
    fn set_event_bus(&self, event_bus: Option<Arc<ragent_types::event::EventBus>>) {
        self.set_event_bus(event_bus);
    }

    /// Expose the concrete router instance for state inspection from the TUI.
    fn as_any_static(&self) -> &dyn std::any::Any {
        self
    }

    /// The router exposes a single virtual model representing itself.
    ///
    /// This lets the TUI model picker show "Model Router" instead of an empty
    /// "no models available" message, and makes the router selectable like a
    /// concrete provider.
    fn default_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "router".to_string(),
            provider_id: "router".to_string(),
            name: "Model Router".to_string(),
            cost: ragent_config::Cost {
                input: 0.0,
                output: 0.0,
            },
            capabilities: ragent_config::Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: Vec::new(),
            },
            context_window: 0,
            max_output: None,
            request_multiplier: None,
            thinking_config: None,
        }]
    }

    /// Creates a [`RouterClient`] for routing chat requests.
    ///
    /// The `api_key` and `base_url` parameters are not used by the router
    /// directly — authentication is delegated to the resolved downstream
    /// provider. The `options` map may contain a `"router_config"` key with
    /// serialised [`RouterConfig`] to override the built-in defaults.
    ///
    /// # Errors
    ///
    /// Returns an error if the router client cannot be constructed (currently
    /// never fails).
    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        _options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        let config = self.config();
        let registry = self.registry();
        let storage = self.storage();
        let event_bus = self.event_bus();
        Ok(Box::new(
            super::router_client::RouterClient::new(config, registry, storage)
                .with_event_bus(event_bus),
        ))
    }
}
