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

use anyhow::Result;
use serde_json::Value;

use super::router_config::RouterConfig;
use crate::llm::LlmClient;
use crate::provider::{ModelInfo, Provider};

/// Virtual provider that implements intelligent model routing.
///
/// The `RouterProvider` presents itself as a standard `Provider` with
/// `id() == "router"` and `name() == "Model Router"`. It does not directly
/// handle LLM requests; instead, it creates a [`RouterClient`] that
/// classifies prompts and delegates to the appropriate provider/model pair.
pub struct RouterProvider {
    /// Current router configuration.
    config: std::sync::RwLock<RouterConfig>,
}

impl RouterProvider {
    /// Create a new `RouterProvider` with the given configuration.
    pub fn new(config: RouterConfig) -> Self {
        Self {
            config: std::sync::RwLock::new(config),
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
    fn id(&self) -> &str {
        "router"
    }

    /// Returns `"Model Router"` (FR-001).
    fn name(&self) -> &str {
        "Model Router"
    }

    /// The router does not expose its own models — it routes to other
    /// providers. Returns an empty list because the router itself is not
    /// a model host; it delegates to the concrete providers configured
    /// in the tier mappings.
    fn default_models(&self) -> Vec<ModelInfo> {
        vec![]
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
        // RouterClient will be implemented in T-007; for now return a stub
        // that records the config. The actual chat routing is part of T-007.
        let config = self.config();
        Ok(Box::new(super::router_client::RouterClient::new(config)))
    }
}
