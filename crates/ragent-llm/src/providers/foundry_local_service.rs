//! Foundry Local service lifecycle wrapper.
//!
//! Wraps the `foundry-local-sdk` [`FoundryLocalManager`] to provide:
//! - Lazy initialisation with the default SDK singleton (FR-004).
//! - Automatic service start before the first chat request (FR-005).
//! - Endpoint URL caching across requests (FR-006).
//! - Clear, actionable errors when the Foundry CLI or runtime is missing (FR-007).
//!
//! All synchronous SDK calls that may block are wrapped in
//! `tokio::task::spawn_blocking` so the async runtime stays responsive
//! (NFR-001).

use anyhow::{Context, Result};
use tracing::info;

use foundry_local_sdk::{FoundryLocalConfig, FoundryLocalManager};

use super::foundry_local_error::FoundryServiceError;

/// Wrapper around the Foundry Local SDK manager.
///
/// Holds a lazily-initialised [`FoundryLocalManager`] and caches the resolved
/// OpenAI-compatible endpoint so that subsequent requests reuse it without
/// re-querying the service status.
pub struct FoundryLocalService {
    /// The underlying SDK singleton.  The SDK uses internal locking and its
    /// async methods take `&self`, so no additional mutex is required.
    manager: &'static FoundryLocalManager,
    /// Cached endpoint URL (`http://…/v1`).  `None` until the service has been
    /// successfully started at least once.
    cached_endpoint: std::sync::Mutex<Option<String>>,
    /// Whether the wrapper is allowed to auto-start the local web service.
    auto_start: bool,
    /// Optional override for the local model cache directory.
    /// Stored for diagnostics and potential future re-initialisation.
    #[allow(dead_code)]
    models_path: Option<String>,
}

impl std::fmt::Debug for FoundryLocalService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cached = self.cached_endpoint.lock().ok().and_then(|g| g.clone());
        f.debug_struct("FoundryLocalService")
            .field("cached_endpoint", &cached)
            .field("auto_start", &self.auto_start)
            .finish_non_exhaustive()
    }
}

impl FoundryLocalService {
    /// Create a new service wrapper.
    ///
    /// This initialises the SDK singleton with `bootstrap = false` so no
    /// model download or load occurs — only the service singleton is prepared.
    ///
    /// # Arguments
    ///
    /// * `auto_start` -- If `true`, the wrapper will automatically invoke
    ///   `start_web_service()` when the endpoint is requested and the service is
    ///   not yet running.
    /// * `models_path` -- Optional override for the local model cache directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the Foundry Local SDK cannot be initialised (for
    /// example because the native core library is missing or incompatible).
    pub async fn new(auto_start: bool, models_path: Option<String>) -> Result<Self> {
        let models_path_for_config = models_path.clone();
        let manager = tokio::task::spawn_blocking(move || {
            let mut config = FoundryLocalConfig::new("ragent");
            if let Some(path) = models_path_for_config {
                config = config.model_cache_dir(path);
            }
            FoundryLocalManager::create(config)
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking panicked: {e}"))?
        .with_context(|| {
            "Microsoft Foundry Local does not appear to be installed. \
             Please install it via the official Microsoft installer, \
             winget, or brew, and ensure the native core library is available."
        })?;

        Ok(Self {
            manager,
            cached_endpoint: std::sync::Mutex::new(None),
            auto_start,
            models_path,
        })
    }

    /// Ensure the local web service is running and return its endpoint.
    ///
    /// 1. Returns the cached endpoint if already known (FR-006).
    /// 2. Checks whether the service is already running via the SDK by asking
    ///    for the list of listening URLs.
    /// 3. If no URLs are available and `auto_start` is enabled, starts it
    ///    (FR-005).
    /// 4. If `auto_start` is disabled and the service is not running, returns
    ///    an error.
    ///
    /// The synchronous SDK start call is wrapped in `spawn_blocking` so the
    /// async runtime is not blocked for more than a few milliseconds
    /// (NFR-001).
    ///
    /// When auto-starting, if the service fails to become ready within 120
    /// seconds, a [`FoundryServiceError`] is returned with a diagnostic message.
    pub async fn ensure_endpoint(&self) -> Result<String> {
        // Fast path: cached endpoint.
        if let Ok(guard) = self.cached_endpoint.lock()
            && let Some(endpoint) = guard.as_ref()
        {
            return Ok(endpoint.clone());
        }

        let mut urls = self.manager.urls().unwrap_or_default();

        if !urls.is_empty() {
            let endpoint = urls.into_iter().next().unwrap_or_default();
            if let Ok(mut guard) = self.cached_endpoint.lock() {
                *guard = Some(endpoint.clone());
            }
            return Ok(endpoint);
        }

        if self.auto_start {
            info!("Foundry Local web service not running; starting automatically");

            let manager = self.manager;
            tokio::task::spawn_blocking(move || {
                let rt = tokio::runtime::Handle::try_current()
                    .map_err(|e| anyhow::anyhow!("no tokio runtime: {e}"))?;
                rt.block_on(manager.start_web_service())
                    .map_err(|e| anyhow::anyhow!("{e}"))
            })
            .await
            .map_err(|e| anyhow::anyhow!("spawn_blocking panicked: {e}"))?
            .map_err(|e| {
                anyhow::anyhow!(
                    "{}",
                    FoundryServiceError {
                        command_path: String::new(),
                        stdout: String::new(),
                        stderr: e.to_string(),
                        error: "Foundry Local web service failed to start. \
                             Ensure the Foundry Local runtime is installed."
                            .to_string(),
                    }
                )
            })?;

            // Poll until the service reports URLs (up to 120 seconds).
            let poll_interval = tokio::time::Duration::from_millis(200);
            let max_attempts: u32 = 600; // 600 × 200ms = 120s
            let progress_every = 25; // ~5 s
            for attempt in 0..max_attempts {
                tokio::time::sleep(poll_interval).await;
                urls = self.manager.urls().unwrap_or_default();
                if !urls.is_empty() {
                    let endpoint = urls.into_iter().next().unwrap_or_default();
                    if let Ok(mut guard) = self.cached_endpoint.lock() {
                        *guard = Some(endpoint.clone());
                    }
                    return Ok(endpoint);
                }
                if attempt > 0 && attempt % progress_every == 0 {
                    tracing::info!(
                        elapsed_secs = attempt * poll_interval.as_millis() as u32 / 1000,
                        "Still waiting for Foundry Local web-service URLs"
                    );
                }
            }

            let elapsed_secs = max_attempts * poll_interval.as_millis() as u32 / 1000;
            Err(anyhow::anyhow!(
                "{}",
                FoundryServiceError {
                    command_path: String::new(),
                    stdout: String::new(),
                    stderr: String::new(),
                    error: format!(
                        "Foundry Local web service did not report any URLs \
                         within {elapsed_secs} seconds after starting."
                    ),
                }
            ))
        } else {
            Err(anyhow::anyhow!(
                "Foundry Local web service is not running and auto_start is disabled. \
                 Start it manually with `foundry service start`."
            ))
        }
    }

    /// Return the cached endpoint without trying to start the service.
    ///
    /// `None` if the service has never been successfully started in this
    /// process.
    pub fn cached_endpoint(&self) -> Option<String> {
        self.cached_endpoint.lock().ok()?.clone()
    }

    /// Reset the cached endpoint so the next call to `ensure_endpoint`
    /// re-queries the service status.
    ///
    /// Useful when the user manually stops and restarts the Foundry service
    /// outside of ragent.
    pub fn refresh(&self) {
        if let Ok(mut guard) = self.cached_endpoint.lock() {
            guard.take();
        }
        info!("Foundry Local endpoint cache cleared");
    }

    /// Access the underlying SDK manager (for catalog / model queries).
    pub fn manager(&self) -> &'static FoundryLocalManager {
        self.manager
    }

    /// Returns whether the wrapper is configured to auto-start the service.
    pub fn auto_start(&self) -> bool {
        self.auto_start
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that `cached_endpoint` returns `None` before `ensure_endpoint`
    /// has ever been called.
    #[tokio::test]
    async fn test_cached_endpoint_none_before_ensure() {
        let Ok(service) = FoundryLocalService::new(true, None).await else {
            return; // Foundry not installed -- test not applicable.
        };
        assert!(service.cached_endpoint().is_none());
    }

    /// Verify that `refresh` clears the cache.
    #[tokio::test]
    async fn test_refresh_clears_cache() {
        let Ok(service) = FoundryLocalService::new(true, None).await else {
            return; // Foundry not installed -- test not applicable.
        };
        // Pretend we cached something.
        if let Ok(mut guard) = service.cached_endpoint.lock() {
            *guard = Some("http://localhost:1234/v1".to_string());
        }
        service.refresh();
        assert!(service.cached_endpoint().is_none());
    }

    /// Verify the `auto_start` flag is preserved.
    #[tokio::test]
    async fn test_auto_start_preserved() {
        let Ok(service) = FoundryLocalService::new(false, None).await else {
            return; // Foundry not installed -- test not applicable.
        };
        assert!(!service.auto_start());
    }

    /// Verify `models_path` is preserved.
    #[tokio::test]
    async fn test_models_path_preserved() {
        let Ok(service) = FoundryLocalService::new(true, Some("/tmp/foundry-models".into())).await
        else {
            return; // Foundry not installed -- test not applicable.
        };
        assert_eq!(service.models_path.as_deref(), Some("/tmp/foundry-models"));
    }
}
