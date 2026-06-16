//! Foundry Local executor for the internal-LLM service.
//!
//! Routes internal-LLM requests (session titles, compaction, etc.) through
//! Microsoft Foundry Local's OpenAI-compatible HTTP endpoint instead of the
//! embedded Candle runtime.  The endpoint is discovered lazily via the
//! `foundry_local_sdk` and cached for the lifetime of the executor.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use tracing::{info, warn};

use super::{
    EmbeddedRuntimeStatus, InternalLlmError, InternalLlmExecutionRequest, InternalLlmExecutor,
};

// ── OpenAI-compatible chat-completion response ──────────────────────────

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Debug, Deserialize)]
struct Message {
    content: String,
}

// ── Executor ────────────────────────────────────────────────────────────

/// Internal-LLM executor backed by Microsoft Foundry Local.
///
/// On the first [`execute`] call the executor discovers the local Foundry
/// service endpoint (starting it automatically if `auto_start` is configured)
/// and then makes standard OpenAI-compatible `POST /chat/completions` calls.
///
/// The endpoint URL is cached so subsequent calls skip discovery.
pub struct FoundryLocalExecutor {
    /// Whether the Foundry Local manager has been initialised.
    initialised: AtomicBool,
    /// Cached OpenAI-compatible base URL (e.g. `http://127.0.0.1:5272/v1`).
    endpoint: std::sync::Mutex<Option<String>>,
    /// HTTP client reused across requests.
    http: reqwest::Client,
    /// Whether to auto-start the Foundry Local service.
    auto_start: bool,
    /// Whether to allow lazy initialisation from the worker thread.
    _allow_lazy: bool,
}

impl FoundryLocalExecutor {
    /// Create a new executor.
    ///
    /// The actual Foundry Local endpoint is discovered lazily on the first
    /// [`execute`] call so that construction is cheap and never blocks.
    pub fn new(auto_start: bool) -> Self {
        Self {
            initialised: AtomicBool::new(false),
            endpoint: std::sync::Mutex::new(None),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            auto_start,
            _allow_lazy: true,
        }
    }

    /// Ensure the Foundry Local endpoint is known.
    ///
    /// If the endpoint is already cached this returns immediately.
    /// Otherwise it creates a `FoundryLocalManager`, optionally auto-starts
    /// the service, and caches the endpoint URL.
    async fn ensure_endpoint(&self) -> Result<String, InternalLlmError> {
        // Fast path — already cached.
        if let Ok(guard) = self.endpoint.lock()
            && let Some(ref url) = *guard
        {
            return Ok(url.clone());
        }

        // Slow path — discover the endpoint via the SDK.
        // Everything is done inside a single spawn_blocking call so we don't
        // have lifetime issues with the manager across spawn boundaries.
        let auto_start = self.auto_start;
        let discovery = tokio::task::spawn_blocking(move || {
            let config = foundry_local_sdk::FoundryLocalConfig::new("ragent");
            let manager = match foundry_local_sdk::FoundryLocalManager::create(config) {
                Ok(m) => m,
                Err(e) => {
                    return Err(InternalLlmError::Unavailable {
                        message: format!(
                            "Microsoft Foundry Local does not appear to be installed: {e}. \
                             Please install it and ensure the native core library is available."
                        ),
                    });
                }
            };

            // Try to get URLs from a running service.
            let mut urls = manager.urls().unwrap_or_default();

            // If no URLs and auto_start is enabled, start the service.
            if urls.is_empty() && auto_start {
                info!("Foundry Local service not running; starting automatically for internal-LLM");
                // The SDK's start_web_service is async, so we need a runtime.
                // Since we're on a spawn_blocking thread, create a small one.
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| InternalLlmError::Unavailable {
                        message: format!(
                            "Failed to create tokio runtime for Foundry Local start: {e}"
                        ),
                    })?;
                rt.block_on(manager.start_web_service()).map_err(|e| {
                    InternalLlmError::Unavailable {
                        message: format!("Foundry Local web service failed to start: {e}"),
                    }
                })?;

                // Poll for URLs (up to 60 seconds — less than the 120s used by
                // the full provider because internal-LLM tasks are best-effort).
                for attempt in 0..300u32 {
                    std::thread::sleep(Duration::from_millis(200));
                    urls = manager.urls().unwrap_or_default();
                    if !urls.is_empty() {
                        break;
                    }
                    if attempt > 0 && attempt % 50 == 0 {
                        info!(
                            elapsed_secs = attempt * 200 / 1000,
                            "Still waiting for Foundry Local service URLs (internal-LLM)"
                        );
                    }
                }
            }

            let endpoint =
                urls.into_iter()
                    .next()
                    .ok_or_else(|| InternalLlmError::Unavailable {
                        message: if auto_start {
                            "Foundry Local web service did not report any URLs after auto-start"
                                .to_string()
                        } else {
                            "Foundry Local web service is not running and auto_start is disabled. \
                     Start it manually with `foundry service start`."
                                .to_string()
                        },
                    })?;

            Ok(endpoint)
        })
        .await
        .map_err(|e| InternalLlmError::Unavailable {
            message: format!("Foundry Local endpoint discovery panicked: {e}"),
        })?;

        let endpoint = discovery?;
        info!(endpoint = %endpoint, "Foundry Local endpoint discovered for internal-LLM");

        if let Ok(mut guard) = self.endpoint.lock() {
            *guard = Some(endpoint.clone());
        }
        self.initialised.store(true, Ordering::Relaxed);

        Ok(endpoint)
    }
}

#[async_trait]
impl InternalLlmExecutor for FoundryLocalExecutor {
    async fn execute(
        &self,
        request: InternalLlmExecutionRequest,
    ) -> Result<String, InternalLlmError> {
        let endpoint = self.ensure_endpoint().await?;

        let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));

        let body = json!({
            "model": request.model_id,
            "messages": [
                { "role": "system", "content": request.system_prompt },
                { "role": "user", "content": request.prompt },
            ],
            "max_tokens": request.max_output_tokens,
            "temperature": 0.0,
            "stream": false,
        });

        let timeout = Duration::from_millis(request.timeout_ms);
        let response = self
            .http
            .post(&url)
            .header("content-type", "application/json")
            .timeout(timeout)
            .json(&body)
            .send()
            .await
            .map_err(|e| InternalLlmError::Unavailable {
                message: format!("Foundry Local HTTP request failed: {e}"),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            warn!(status = %status, body = %text, "Foundry Local chat completion failed");
            return Err(InternalLlmError::Execution {
                message: format!(
                    "Foundry Local returned HTTP {}: {}",
                    status,
                    text.chars().take(200).collect::<String>()
                ),
            });
        }

        let parsed: ChatCompletionResponse =
            response
                .json()
                .await
                .map_err(|e| InternalLlmError::Execution {
                    message: format!("Failed to parse Foundry Local response: {e}"),
                })?;

        let content = parsed
            .choices
            .first()
            .map(|c| c.message.content.trim().to_string())
            .filter(|c| !c.is_empty())
            .ok_or_else(|| InternalLlmError::EmptyOutput {
                task: request.task_kind.as_config_key(),
            })?;

        Ok(content)
    }

    fn status(&self) -> Option<EmbeddedRuntimeStatus> {
        // Foundry Local doesn't expose the same runtime state as the embedded
        // candle backend. Return None so status surfaces only show config-level
        // information (backend, model_id, etc.).
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_foundry_executor_constructs_without_blocking() {
        let executor = FoundryLocalExecutor::new(true);
        assert!(!executor.initialised.load(Ordering::Relaxed));
        assert!(executor.endpoint.lock().unwrap().is_none());
    }

    #[test]
    fn test_foundry_executor_auto_start_flag() {
        let on = FoundryLocalExecutor::new(true);
        assert!(on.auto_start);

        let off = FoundryLocalExecutor::new(false);
        assert!(!off.auto_start);
    }
}
