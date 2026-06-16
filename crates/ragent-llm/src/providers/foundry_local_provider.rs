//! Microsoft Foundry Local provider implementation.
//!
//! Implements the [`Provider`] trait for Microsoft Foundry Local, which hosts
//! MAI models (Phi-4, Phi-3.5, etc.) via a local OpenAI-compatible HTTP
//! endpoint managed by the `foundry-local-sdk`.
//!
//! The provider is always compiled and appears in the default registry because
//! Foundry Local is a first-class, non-optional backend for ragent.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::llm::LlmClient;
use crate::provider::foundry_local_client::FoundryLocalClient;
use crate::provider::foundry_local_inproc_client::{
    FoundryLocalInProcClient, device_type_from_str,
};
use crate::provider::foundry_local_service::FoundryLocalService;
use crate::{ModelInfo, Provider};
use ragent_config::{Capabilities, Cost};
use ragent_types::event::EventBus;

use tracing::info;

/// Alias for the SDK's catalog metadata type to avoid clashing with ragent's
/// own [`ModelInfo`].
use foundry_local_sdk::ModelInfo as SdkModelInfo;

/// Resolved per-request options for the Foundry Local provider.
#[derive(Debug, Clone)]
struct FoundryLocalOptions {
    auto_start: bool,
    device: Option<String>,
    models_path: Option<String>,
    in_process: bool,
}

/// Validate a Foundry Local device preference string.
fn validate_device(device: &str) -> Result<()> {
    match device {
        "auto" | "cpu" | "gpu" | "npu" => Ok(()),
        _ => anyhow::bail!(
            "Invalid Foundry Local device '{device}'. Must be one of: auto, cpu, gpu, npu"
        ),
    }
}

static TEST_OVERRIDE: std::sync::Mutex<Option<bool>> = std::sync::Mutex::new(None);

/// Returns `true` if the Foundry Local native runtime can be initialised.
///
/// This performs a lightweight synchronous check by attempting to create the
/// SDK manager with `bootstrap = false`, which only verifies that the native
/// core library is present and compatible. It does not download or load any
/// models.
///
/// The result is cached for the lifetime of the process because manager
/// creation is idempotent but not free. Tests can force availability via
/// [`set_foundry_local_available_for_tests`] and reset the cache with
/// [`clear_availability_cache`].
#[must_use]
pub fn is_foundry_local_available() -> bool {
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<Option<bool>>> = OnceLock::new();

    // Test override takes precedence so unit tests can simulate availability
    // without installing the Foundry Local runtime.
    if let Ok(guard) = TEST_OVERRIDE.lock()
        && let Some(value) = *guard
    {
        return value;
    }

    let cache = CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache.lock().expect("foundry availability cache poisoned");
    if let Some(value) = *guard {
        return value;
    }
    let config = foundry_local_sdk::FoundryLocalConfig::new("ragent");
    let value = foundry_local_sdk::FoundryLocalManager::create(config).is_ok();
    *guard = Some(value);
    value
}

/// Force [`is_foundry_local_available`] to return `value` without touching the
/// real runtime.
///
/// Intended for tests only. Call [`clear_foundry_local_test_override`] when
/// done.
pub fn set_foundry_local_available_for_tests(value: bool) {
    if let Ok(mut guard) = TEST_OVERRIDE.lock() {
        *guard = Some(value);
    }
}

/// Remove the test override set by [`set_foundry_local_available_for_tests`].
pub fn clear_foundry_local_test_override() {
    if let Ok(mut guard) = TEST_OVERRIDE.lock() {
        *guard = None;
    }
}

/// Clear the cached real-time availability result.
///
/// This is exposed so tests can re-evaluate availability after changing the
/// environment. The test override is not affected.
pub fn clear_availability_cache() {
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<Option<bool>>> = OnceLock::new();
    if let Some(cache) = CACHE.get()
        && let Ok(mut guard) = cache.lock()
    {
        *guard = None;
    }
}

/// Discover available models from the Foundry Local catalog.
///
/// This is a convenience wrapper that creates a temporary provider instance
/// and queries `FoundryLocalManager::catalog()`.  It returns an empty list
/// when the Foundry service is not running or the catalog cannot be fetched,
/// so callers should fall back to [`foundry_local_default_models`] when the
/// result is empty.
pub async fn discover_foundry_local_models() -> Result<Vec<ModelInfo>> {
    let provider = FoundryLocalProvider::new();
    provider.discover_models().await
}

/// Returns the default Foundry Local model catalog.
///
/// All models are local, therefore input/output costs are zero.
#[must_use]
pub fn foundry_local_default_models() -> Vec<ModelInfo> {
    let provider_id = "foundry_local";
    vec![
        ModelInfo {
            id: "phi-4".to_string(),
            provider_id: provider_id.to_string(),
            name: "Phi-4".to_string(),
            cost: Cost {
                input: 0.0,
                output: 0.0,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: Vec::new(),
            },
            context_window: 128_000,
            max_output: Some(16_384),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "phi-3.5-mini".to_string(),
            provider_id: provider_id.to_string(),
            name: "Phi-3.5 Mini".to_string(),
            cost: Cost {
                input: 0.0,
                output: 0.0,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: Vec::new(),
            },
            context_window: 128_000,
            max_output: Some(16_384),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "phi-3.5-moe".to_string(),
            provider_id: provider_id.to_string(),
            name: "Phi-3.5 MoE".to_string(),
            cost: Cost {
                input: 0.0,
                output: 0.0,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: Vec::new(),
            },
            context_window: 128_000,
            max_output: Some(16_384),
            request_multiplier: None,
            thinking_config: None,
        },
    ]
}

/// Provider implementation for Microsoft Foundry Local.
pub struct FoundryLocalProvider {
    /// Lazily-initialised service wrapper.  `None` until the first call to
    /// `create_client` or `discover_models`.
    service: Mutex<Option<Arc<FoundryLocalService>>>,
    /// Whether the service should be started automatically when the endpoint
    /// is requested and it is not already running.
    auto_start: bool,
    /// Preferred inference device (`auto`, `cpu`, `gpu`, `npu`).
    device: Option<String>,
    /// Whether to use the in-process native core backend instead of the
    /// Foundry Local web service.
    in_process: Option<bool>,
    /// Override path for the local model cache directory.
    models_path: Option<String>,
    /// Optional event bus for publishing download/lifecycle events.
    event_bus: std::sync::Mutex<Option<Arc<EventBus>>>,
}

impl FoundryLocalProvider {
    /// Create a new provider with the default `auto_start = true`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            service: Mutex::new(None),
            auto_start: true,
            device: None,
            in_process: None,
            models_path: None,
            event_bus: std::sync::Mutex::new(None),
        }
    }

    /// Create a new provider with explicit configuration values.
    ///
    /// # Arguments
    ///
    /// * `auto_start` — Automatically start the Foundry service on first use.
    /// * `device` — Preferred inference device (`auto`, `cpu`, `gpu`, `npu`).
    /// * `models_path` — Override path for the local model cache.
    #[must_use]
    pub fn with_config(
        auto_start: bool,
        device: Option<String>,
        models_path: Option<String>,
    ) -> Self {
        Self {
            service: Mutex::new(None),
            auto_start,
            device,
            in_process: None,
            models_path,
            event_bus: std::sync::Mutex::new(None),
        }
    }

    /// Create a new provider with explicit configuration values, including
    /// the in-process backend flag.
    ///
    /// # Arguments
    ///
    /// * `auto_start` — Automatically start the Foundry service on first use.
    /// * `device` — Preferred inference device (`auto`, `cpu`, `gpu`, `npu`).
    /// * `models_path` — Override path for the local model cache.
    /// * `in_process` — Use the in-process native core backend when `true`.
    #[must_use]
    pub fn with_full_config(
        auto_start: bool,
        device: Option<String>,
        models_path: Option<String>,
        in_process: Option<bool>,
    ) -> Self {
        Self {
            service: Mutex::new(None),
            auto_start,
            device,
            in_process,
            models_path,
            event_bus: std::sync::Mutex::new(None),
        }
    }

    /// Resolve provider options from the per-request `options` hashmap and the
    /// provider-level defaults, validating the `device` value.
    fn resolve_options(&self, options: &HashMap<String, Value>) -> Result<FoundryLocalOptions> {
        let auto_start = options
            .get("auto_start")
            .and_then(Value::as_bool)
            .unwrap_or(self.auto_start);

        let device = options
            .get("device")
            .and_then(Value::as_str)
            .map(String::from)
            .or_else(|| self.device.clone());

        if let Some(ref d) = device {
            validate_device(d)?;
        }

        let models_path = options
            .get("models_path")
            .and_then(Value::as_str)
            .map(String::from)
            .or_else(|| self.models_path.clone());

        let in_process = options
            .get("in_process")
            .and_then(Value::as_bool)
            .or(self.in_process)
            .unwrap_or(false);

        Ok(FoundryLocalOptions {
            auto_start,
            device,
            models_path,
            in_process,
        })
    }

    /// Ensure the service wrapper is initialised and return it.
    ///
    /// On the first call this creates the underlying [`FoundryLocalManager`]
    /// via the SDK; subsequent calls reuse the cached instance.
    async fn ensure_service(&self) -> Result<Arc<FoundryLocalService>> {
        let maybe_svc = {
            let guard = self.service.lock().await;
            guard.clone()
        };

        if let Some(svc) = maybe_svc {
            return Ok(svc);
        }

        let new_svc =
            Arc::new(FoundryLocalService::new(self.auto_start, self.models_path.clone()).await?);
        let mut guard = self.service.lock().await;
        *guard = Some(new_svc.clone());
        Ok(new_svc)
    }

    /// Discover available models from the Foundry Local catalog (FR-011).
    ///
    /// Queries the SDK catalog for all models and maps them to ragent's
    /// [`ModelInfo`] type.  Models whose `runtime` indicates local CPU/GPU/NPU
    /// compatibility are included.
    ///
    /// If the catalog query fails, returns an empty list so the TUI falls
    /// back to the static default catalog (FR-012).
    pub async fn discover_models(&self) -> Result<Vec<ModelInfo>> {
        let service = match self.ensure_service().await {
            Ok(s) => s,
            Err(_) => return Ok(Vec::new()),
        };

        let catalog = service.manager().catalog();
        let models_result = catalog.get_models().await;

        match models_result {
            Ok(models) => {
                let mapped: Vec<ModelInfo> =
                    models.iter().map(|m| Self::map_sdk_model(m)).collect();
                Ok(mapped)
            }
            Err(_) => Ok(Vec::new()),
        }
    }

    /// Map a single SDK [`Model`](foundry_local_sdk::Model) to ragent's [`ModelInfo`].
    fn map_sdk_model(model: &foundry_local_sdk::Model) -> ModelInfo {
        let info: &SdkModelInfo = model.info();
        Self::map_sdk_model_info(info)
    }

    /// Map the SDK's [`ModelInfo`](SdkModelInfo) to ragent's [`ModelInfo`].
    fn map_sdk_model_info(info: &SdkModelInfo) -> ModelInfo {
        let context_window = info.context_length.map(|n| n as usize).unwrap_or(128_000);
        let max_output = info.max_output_tokens.map(|n| n as usize);
        let supports_tool_calling = info.supports_tool_calling.unwrap_or(false);
        ModelInfo {
            id: info.id.clone(),
            provider_id: "foundry_local".to_string(),
            name: info
                .display_name
                .clone()
                .unwrap_or_else(|| info.alias.clone()),
            cost: Cost {
                input: 0.0,
                output: 0.0,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: supports_tool_calling,
                thinking_levels: Vec::new(),
            },
            context_window,
            max_output,
            request_multiplier: None,
            thinking_config: None,
        }
    }
}

impl Default for FoundryLocalProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Provider for FoundryLocalProvider {
    /// Returns `"foundry_local"`.
    fn id(&self) -> &str {
        "foundry_local"
    }

    /// Returns `"Microsoft Foundry Local"`.
    fn name(&self) -> &str {
        "Microsoft Foundry Local"
    }

    fn set_event_bus(&self, event_bus: Option<Arc<EventBus>>) {
        if let Ok(mut guard) = self.event_bus.lock() {
            *guard = event_bus;
        }
    }

    /// Returns the default catalog (Phi-4, Phi-3.5 Mini, Phi-3.5 MoE).
    fn default_models(&self) -> Vec<ModelInfo> {
        foundry_local_default_models()
    }

    /// Discover available models from the Foundry Local SDK catalog.
    async fn discover_models(&self) -> Result<Vec<ModelInfo>> {
        let service = match self.ensure_service().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "Foundry Local service not available; using default catalog");
                return Ok(self.default_models());
            }
        };

        let catalog = service.manager().catalog();
        match catalog.get_models().await {
            Ok(models) => Ok(models.iter().map(|m| Self::map_sdk_model(m)).collect()),
            Err(e) => {
                tracing::warn!(error = %e, "Foundry Local catalog discovery failed; using default catalog");
                Ok(self.default_models())
            }
        }
    }

    /// Creates a [`FoundryLocalClient`] or [`FoundryLocalInProcClient`] depending
    /// on the `in_process` configuration flag.
    ///
    /// The service wrapper is initialised lazily on the first call and cached
    /// for the lifetime of the provider.  Configuration values are read from
    /// the `options` hashmap with fallback to the struct defaults (FR-019).
    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        // Parse configuration with fallback to stored defaults (FR-019, FR-022).
        let opts = self.resolve_options(options)?;

        // Environment escape hatch to force the web-service path (FR-030).
        let force_web = std::env::var("RAGENT_FOUNDRY_LOCAL_FORCE_WEB")
            .ok()
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);
        let in_process = opts.in_process && !force_web;

        // Log backend and device preference for diagnostics (FR-020, FR-021, FR-027).
        if in_process {
            info!("Foundry Local in-process backend selected");
        }
        if let Some(ref d) = opts.device {
            info!(device = %d, "Foundry Local device preference");
        }

        // We always need the SDK manager; ensure_service only creates the
        // singleton and does not start the web service.
        let svc = if opts.auto_start == self.auto_start && opts.models_path == self.models_path {
            self.ensure_service().await?
        } else {
            // Per-request override differs from default; create a one-off service (FR-022).
            Arc::new(FoundryLocalService::new(opts.auto_start, opts.models_path).await?)
        };
        let manager = svc.manager();
        let event_bus = self.event_bus.lock().ok().and_then(|g| g.clone());

        if in_process {
            let device = opts
                .device
                .as_deref()
                .map(device_type_from_str)
                .transpose()?;
            let client = FoundryLocalInProcClient::new(manager, event_bus, device);
            Ok(Box::new(client))
        } else {
            let endpoint = svc.ensure_endpoint().await?;
            let client = FoundryLocalClient::new(&endpoint, Some(manager), event_bus);
            Ok(Box::new(client))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_id() {
        let p = FoundryLocalProvider::new();
        assert_eq!(p.id(), "foundry_local");
    }

    #[test]
    fn test_provider_name() {
        let p = FoundryLocalProvider::new();
        assert_eq!(p.name(), "Microsoft Foundry Local");
    }

    #[test]
    fn test_default_models_count() {
        let p = FoundryLocalProvider::new();
        assert_eq!(p.default_models().len(), 3);
    }

    #[test]
    fn test_default_models_ids() {
        let p = FoundryLocalProvider::new();
        let ids: Vec<String> = p.default_models().into_iter().map(|m| m.id).collect();
        assert!(ids.contains(&"phi-4".to_string()));
        assert!(ids.contains(&"phi-3.5-mini".to_string()));
        assert!(ids.contains(&"phi-3.5-moe".to_string()));
    }

    #[test]
    fn test_default_models_provider_id() {
        let p = FoundryLocalProvider::new();
        for m in p.default_models() {
            assert_eq!(m.provider_id, "foundry_local");
        }
    }

    #[test]
    fn test_default_models_cost_is_zero() {
        let p = FoundryLocalProvider::new();
        for m in p.default_models() {
            assert_eq!(m.cost.input, 0.0);
            assert_eq!(m.cost.output, 0.0);
        }
    }

    #[test]
    fn test_default_models_capabilities() {
        let p = FoundryLocalProvider::new();
        for m in p.default_models() {
            assert!(m.capabilities.streaming);
            assert!(m.capabilities.tool_use);
            assert!(!m.capabilities.vision);
            assert!(!m.capabilities.reasoning);
        }
    }

    #[test]
    fn test_auto_start_default() {
        let p = FoundryLocalProvider::new();
        assert!(p.auto_start);
    }

    #[test]
    fn test_with_auto_start_false() {
        let p = FoundryLocalProvider::with_config(false, None, None);
        assert!(!p.auto_start);
    }

    #[test]
    fn test_device_and_models_path_stored() {
        let p = FoundryLocalProvider::with_config(
            true,
            Some("gpu".to_string()),
            Some("/tmp/models".to_string()),
        );
        assert_eq!(p.device, Some("gpu".to_string()));
        assert_eq!(p.models_path, Some("/tmp/models".to_string()));
    }

    #[test]
    fn test_resolve_options_defaults() {
        let p = FoundryLocalProvider::new();
        let opts = p.resolve_options(&HashMap::new()).unwrap();
        assert!(opts.auto_start);
        assert_eq!(opts.device, None);
        assert_eq!(opts.models_path, None);
        assert!(!opts.in_process);
    }

    #[test]
    fn test_resolve_options_in_process_from_options() {
        let p = FoundryLocalProvider::new();
        let mut options = HashMap::new();
        options.insert("in_process".to_string(), serde_json::json!(true));
        options.insert("device".to_string(), serde_json::json!("gpu"));
        options.insert("auto_start".to_string(), serde_json::json!(false));
        let opts = p.resolve_options(&options).unwrap();
        assert!(!opts.auto_start);
        assert_eq!(opts.device, Some("gpu".to_string()));
        assert!(opts.in_process);
    }

    #[test]
    fn test_resolve_options_in_process_from_provider_default() {
        let p = FoundryLocalProvider::with_full_config(true, None, None, Some(true));
        let opts = p.resolve_options(&HashMap::new()).unwrap();
        assert!(opts.in_process);
    }

    #[test]
    fn test_resolve_options_invalid_device() {
        let p = FoundryLocalProvider::new();
        let mut options = HashMap::new();
        options.insert("device".to_string(), serde_json::json!("cuda"));
        let err = p.resolve_options(&options).unwrap_err();
        assert!(err.to_string().contains("Invalid Foundry Local device"));
    }

    #[test]
    fn test_with_full_config_stores_in_process() {
        let p = FoundryLocalProvider::with_full_config(
            false,
            Some("cpu".to_string()),
            Some("/tmp".to_string()),
            Some(true),
        );
        assert!(!p.auto_start);
        assert_eq!(p.device, Some("cpu".to_string()));
        assert_eq!(p.models_path, Some("/tmp".to_string()));
        assert_eq!(p.in_process, Some(true));
    }
}
