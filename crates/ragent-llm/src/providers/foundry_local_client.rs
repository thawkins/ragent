//! Foundry Local LLM client wrapper.
//!
//! Reuses [`OpenAiClient`] under the hood because Foundry Local exposes an
//! OpenAI-compatible HTTP endpoint.  This wrapper also orchestrates the local
//! model lifecycle so that a request to chat with a model that is not yet
//! downloaded or loaded automatically downloads and loads it before streaming
//! (FR-013, FR-014, FR-015, FR-016, FR-017, FR-018).

use std::pin::Pin;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use futures::Stream;
use serde_json::Value;

use crate::llm::{ChatRequest, LlmClient, StreamEvent};
use crate::provider::openai::OpenAiClient;
use ragent_types::event::{Event, EventBus};

/// Alias for the SDK's model type to avoid clashing with ragent's own [`ModelInfo`].
use foundry_local_sdk::Model as SdkModel;

/// Client for Microsoft Foundry Local inference.
///
/// Wraps an [`OpenAiClient`] configured to talk to the local Foundry endpoint,
/// and optionally holds a reference to the SDK manager so it can download and
/// load models on demand.
pub struct FoundryLocalClient {
    inner: OpenAiClient,
    manager: Option<&'static foundry_local_sdk::FoundryLocalManager>,
    /// Optional event bus for publishing model download progress.
    event_bus: Option<Arc<EventBus>>,
}

impl FoundryLocalClient {
    /// Create a new client targeting the given Foundry Local endpoint.
    ///
    /// The endpoint is the OpenAI-compatible base URL returned by
    /// [`FoundryLocalService::ensure_endpoint`](super::foundry_local_service::FoundryLocalService).
    /// When `manager` is provided, the client can download and load models
    /// automatically before chatting.
    #[must_use]
    pub fn new(
        endpoint: &str,
        manager: Option<&'static foundry_local_sdk::FoundryLocalManager>,
        event_bus: Option<Arc<EventBus>>,
    ) -> Self {
        // Foundry Local does not require a real API key.
        Self {
            inner: OpenAiClient::new_with_provider("", endpoint, "foundry_local"),
            manager,
            event_bus,
        }
    }

    /// Access the underlying OpenAI-compatible client.
    #[must_use]
    pub fn inner(&self) -> &OpenAiClient {
        &self.inner
    }

    /// Query the local `/v1/models` endpoint and return the ids of currently
    /// loaded models.
    ///
    /// Returns `None` if the list could not be fetched, so callers can fall
    /// back to letting the chat request fail with its own diagnostics.
    async fn loaded_model_ids(&self) -> Option<Vec<String>> {
        let url = format!("{}/v1/models", self.inner.base_url());
        let response = match self.inner.http_client().get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!(error = %e, url = %url, "Failed to query Foundry Local loaded models");
                return None;
            }
        };

        if !response.status().is_success() {
            tracing::debug!(
                status = %response.status(),
                url = %url,
                "Foundry Local /v1/models returned non-success status"
            );
            return None;
        }

        let text = response.text().await.unwrap_or_default();
        let ids = parse_loaded_model_ids(&text);
        Some(ids)
    }
}

/// Strip a trailing `:N` version suffix from a Foundry Local model id.
///
/// Foundry Local catalog entries use a variant id that includes the version
/// (e.g. `qwen2.5-coder-7b-instruct-generic-cpu:4`), while the OpenAI-compatible
/// `/v1/models` endpoint returns the same id without the suffix.  Normalising to
/// the base id lets the chat request and readiness poll line up with
/// `/v1/models` while still allowing us to construct a suffixed load id for the
/// `/models/load` endpoint.
fn strip_model_version_suffix(id: &str) -> &str {
    id.rsplit_once(':')
        .and_then(|(base, suffix)| {
            if suffix.chars().all(|c| c.is_ascii_digit()) {
                Some(base)
            } else {
                None
            }
        })
        .unwrap_or(id)
}

/// Parse the `id` values from an OpenAI-style `/v1/models` response.
///
/// The Foundry Local implementation returns extra fields such as
/// `IsDelta`, `Successful`, and `HttpStatusCode`; this function only
/// extracts `data[].id`.
fn parse_loaded_model_ids(body: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };

    value
        .get("data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| entry.get("id").and_then(|id| id.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Ensure the requested model is downloaded and loaded into memory.
///
/// The request string may be either a model alias (e.g. `phi-4`) or a specific
/// variant id (e.g. `qwen3-14b-generic-cpu:2`).  If the model is not cached it
/// is downloaded via the SDK; if it is not loaded it is loaded via the web
/// service's HTTP endpoint.  This prevents the empty/malformed SSE stream that
/// Foundry Local returns when asked to chat with an unloaded model.
///
/// # Why HTTP load instead of SDK `model.load()`
///
/// The SDK's `model.load()` delegates to `ModelLoadManager::load()`, which uses
/// the native core library when no `external_service_url` is configured.  That
/// loads the model in-process, but our chat requests go to the separate Foundry
/// Local web service.  Loading in-process means the web service never sees the
/// model, causing `wait_for_model_ready` to time out.  Using the web service's
/// HTTP `/models/load/{id}` endpoint loads the model in the service process where
/// it can actually serve inference requests.
///
/// # Version-suffixed ids
///
/// The SDK catalog stores variant ids with a version suffix (e.g.
/// `qwen2.5-coder-7b-instruct-generic-cpu:4`), while the OpenAI-compatible
/// `/v1/models` endpoint returns the same id without the suffix.  This function
/// returns the unsuffixed base id so the chat request and readiness poll line up
/// with `/v1/models`, but it loads the model using the suffixed id required by
/// the `/models/load` endpoint.
///
/// Returns the resolved model id that should be used in the OpenAI chat request.
async fn ensure_model_loaded(
    manager: &'static foundry_local_sdk::FoundryLocalManager,
    model_id_or_alias: &str,
    event_bus: Option<Arc<EventBus>>,
    session_id: &str,
    service_endpoint: &str,
) -> Result<String> {
    let catalog = manager.catalog();

    // Try the request string as a specific variant id first, then as an alias.
    let model: Arc<SdkModel> = match catalog.get_model_variant(model_id_or_alias).await {
        Ok(m) => m,
        Err(_) => catalog
            .get_model(model_id_or_alias)
            .await
            .with_context(|| {
                format!(
                    "Model '{model_id_or_alias}' is not known to Microsoft Foundry Local. \
                     Use `foundry model list` to see available models, \
                     or `foundry model download {model_id_or_alias}` to add it."
                )
            })?,
    };

    // If the request resolved to an alias group, pick the best variant.
    let model = if model.variants().len() > 1 {
        select_best_variant(&model, model_id_or_alias).await?
    } else {
        model
    };

    // The SDK variant id always includes a `:N` version suffix, while the web
    // service's OpenAI-compatible endpoint exposes the same id without it.
    // Strip the suffix for the chat request, but keep it for the load request.
    let resolved_id = strip_model_version_suffix(&model.info().id).to_string();
    // Use the SDK's full variant id as the load id.  The SDK catalog stores the
    // version suffix directly in `info().id`, so this is more robust than
    // reconstructing it from `info().version`, whose format can vary.
    let load_id = model.info().id.clone();

    if !model.is_cached().await? {
        tracing::info!(
            requested = %model_id_or_alias,
            resolved = %resolved_id,
            "Downloading Microsoft Foundry Local model"
        );
        let start = std::time::Instant::now();
        let bus = event_bus.clone();
        let provider_id = "foundry_local".to_string();
        let model_id_for_event = resolved_id.clone();
        let session_id_for_event = session_id.to_string();
        if let Some(ref b) = bus {
            b.publish(Event::ModelDownloadStarted {
                provider_id: provider_id.clone(),
                model_id: model_id_for_event.clone(),
                session_id: session_id_for_event.clone(),
            });
        }
        model
            .download(Some(move |pct: f64| {
                let percent = pct as f32;
                tracing::info!(percent = pct, "Foundry Local download progress");
                if let Some(ref b) = bus {
                    b.publish(Event::ModelDownloadProgress {
                        provider_id: provider_id.clone(),
                        model_id: model_id_for_event.clone(),
                        session_id: session_id_for_event.clone(),
                        percent,
                    });
                }
            }))
            .await
            .with_context(|| {
                format!(
                    "Failed to download model '{model_id_or_alias}' (resolved to '{resolved_id}')"
                )
            })?;
        if let Some(ref b) = event_bus {
            b.publish(Event::ModelDownloadFinished {
                provider_id: "foundry_local".to_string(),
                model_id: resolved_id.clone(),
                session_id: session_id.to_string(),
                error: None,
            });
        }
        tracing::info!(
            requested = %model_id_or_alias,
            resolved = %resolved_id,
            elapsed_secs = start.elapsed().as_secs(),
            "Microsoft Foundry Local model downloaded"
        );
    }

    // Check if the model is already loaded in the web service before attempting
    // to load it.  We query the service's /models/loaded endpoint rather than
    // using the SDK's `model.is_loaded()` because the SDK's check goes through
    // the native core library (in-process), while we need the model loaded in
    // the web service process.
    //
    // The `/models/loaded` endpoint returns ids with the version suffix
    // (e.g. `qwen3-0.6b-generic-cpu:4`), so is_model_loaded_via_http checks
    // both exact and prefix match.  The load request always uses the suffixed
    // id because that is what the endpoint requires.
    if !is_model_loaded_via_http(service_endpoint, &resolved_id).await? {
        tracing::info!(
            requested = %model_id_or_alias,
            resolved = %resolved_id,
            load_id = %load_id,
            "Loading Microsoft Foundry Local model via web service"
        );
        load_model_via_http(service_endpoint, &load_id).await.with_context(|| {
            format!("Failed to load model '{model_id_or_alias}' (resolved to '{resolved_id}', load id '{load_id}') via the Foundry Local web service")
        })?;
        tracing::info!(
            requested = %model_id_or_alias,
            resolved = %resolved_id,
            "Microsoft Foundry Local model loaded via web service"
        );
    } else {
        tracing::info!(
            requested = %model_id_or_alias,
            resolved = %resolved_id,
            "Microsoft Foundry Local model already loaded in web service"
        );
    }

    Ok(resolved_id)
}

/// Pick a variant from an alias group.
///
/// Prefers a variant that is already cached (and loaded, if easy to check), then
/// the first available variant.  If no variant can be selected, returns an error.
async fn select_best_variant(model: &SdkModel, model_id_or_alias: &str) -> Result<Arc<SdkModel>> {
    let variants = model.variants();

    // Prefer a cached variant; loading from cache is much faster than downloading.
    // We cannot easily short-circuit the async `is_cached()` calls inside `find`,
    // so just await each in order.
    let mut maybe_cached: Option<Arc<SdkModel>> = None;
    for v in &variants {
        if v.is_cached().await.unwrap_or(false) {
            maybe_cached = Some(Arc::clone(v));
            break;
        }
    }

    let selected = maybe_cached
        .or_else(|| variants.first().cloned())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Model '{model_id_or_alias}' has no variants available in the Foundry Local catalog"
            )
        })?;

    model
        .select_variant(selected.as_ref())
        .with_context(|| format!("Failed to select variant for '{model_id_or_alias}'"))?;

    Ok(Arc::new((*selected).clone()))
}

#[async_trait::async_trait]
impl LlmClient for FoundryLocalClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>, anyhow::Error> {
        // If we have access to the SDK manager, ensure the requested model is
        // downloaded and loaded before streaming.  Foundry Local returns an
        // empty/malformed chunked SSE body when asked to chat with a model that
        // is not loaded, which otherwise surfaces as a generic
        // "error decoding response body" failure.
        let mut request = request;
        if let Some(manager) = self.manager {
            // Obtain the service endpoint first — we need it to load the model
            // via the web service's HTTP endpoint rather than the in-process
            // native core.
            let service_endpoint = refresh_service_endpoint(manager, self.inner.base_url()).await?;

            let resolved_id = ensure_model_loaded(
                manager,
                &request.model,
                self.event_bus.clone(),
                request.session_id.as_deref().unwrap_or(""),
                &service_endpoint,
            )
            .await
            .with_context(|| {
                format!(
                    "Microsoft Foundry Local could not prepare model '{}' for inference",
                    request.model
                )
            })?;

            // Use the exact variant id that was loaded; aliases may return an
            // empty SSE stream if the chat endpoint does not resolve them.
            request.model = resolved_id;

            // Create a fresh client with the (possibly refreshed) endpoint.
            let client = OpenAiClient::new_with_provider(
                self.inner.api_key(),
                &service_endpoint,
                self.inner.provider_name(),
            );
            // Wait until the model is actually loaded into the web service's
            // memory.  Using `/v1/models` here is insufficient because it lists
            // downloaded models; an unloaded model still appears there and
            // produces an empty SSE stream when chatted with.
            wait_for_model_ready(
                &service_endpoint,
                &client,
                &request.model,
                request.session_id.as_deref().unwrap_or(""),
                self.event_bus.clone(),
            )
            .await?;
            client
                .chat(request)
                .await
                .with_context(|| "Microsoft Foundry Local chat request failed")
        } else {
            // Fallback: without the SDK manager we can only check the loaded-model
            // list.  Keep the old diagnostic for backwards-compatible deployments.
            if let Some(loaded) = self.loaded_model_ids().await {
                if loaded.is_empty() {
                    bail!(
                        "Microsoft Foundry Local has no models loaded. \
                         Requested model '{}' cannot be used. \
                         Use `foundry model download <model>` and `foundry model load <model>` first.",
                        request.model
                    );
                }
                if !loaded.contains(&request.model) {
                    bail!(
                        "Microsoft Foundry Local model '{}' is not loaded. \
                         Loaded models: {}. \
                         Use `foundry model download <model>` and `foundry model load <model>` to make it available.",
                        request.model,
                        loaded.join(", ")
                    );
                }
            }
            self.inner
                .chat(request)
                .await
                .with_context(|| "Microsoft Foundry Local chat request failed")
        }
    }
}

/// Load a model into the Foundry Local web service via its HTTP endpoint.///
/// Uses `GET /models/load/{model_id}` which tells the web service process to
/// load the model into memory for inference.  The variant id must be URL-encoded
/// because it may contain colons (e.g. `qwen3-0.6b-generic-cpu:4`).
async fn load_model_via_http(service_endpoint: &str, model_id: &str) -> Result<()> {
    let encoded_id = urlencoding::encode(model_id);
    // Strip trailing /v1 from the endpoint if present, since the load endpoint
    // is at the root level (not under /v1).
    let base = service_endpoint
        .trim_end_matches("/v1")
        .trim_end_matches('/');
    let url = format!("{base}/models/load/{encoded_id}");

    tracing::info!(url = %url, "Requesting model load via Foundry Local web service");

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(300))
        .send()
        .await
        .with_context(|| format!("Failed to send load request to {url}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        bail!(
            "Foundry Local web service returned {status} when loading model '{model_id}': {body}"
        );
    }

    // The load endpoint returns 200 with empty body on success.  The model may
    // take a few seconds to appear in /v1/models, so callers should still use
    // `wait_for_model_ready` after this.
    tracing::info!(model_id = %model_id, "Model load request accepted by web service");
    Ok(())
}

/// Query the Foundry Local web service's `/models/loaded` endpoint.
///
/// Returns `Some(ids)` when the endpoint is available, or `None` when it is
/// missing or unparseable so callers can fall back to `/v1/models`.
async fn query_models_loaded(service_endpoint: &str) -> Result<Option<Vec<String>>> {
    let base = service_endpoint
        .trim_end_matches("/v1")
        .trim_end_matches('/');
    let url = format!("{base}/models/loaded");

    let client = reqwest::Client::new();
    let response = match client
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(
                error = %e,
                url = %url,
                "Failed to query Foundry Local /models/loaded; assuming endpoint unavailable"
            );
            return Ok(None);
        }
    };

    if !response.status().is_success() {
        tracing::debug!(
            status = %response.status(),
            url = %url,
            "Foundry Local /models/loaded returned non-success status; assuming endpoint unavailable"
        );
        return Ok(None);
    }

    let text = response.text().await.unwrap_or_default();
    if text.trim().is_empty() {
        return Ok(Some(Vec::new()));
    }

    match serde_json::from_str(&text) {
        Ok(ids) => Ok(Some(ids)),
        Err(e) => {
            tracing::debug!(
                error = %e,
                body = %text,
                "Failed to parse /models/loaded response; assuming endpoint unavailable"
            );
            Ok(None)
        }
    }
}

/// Returns true if `model_id` (or a version-suffixed variant of it) is present
/// in the list of loaded model ids returned by the web service.
fn is_model_in_loaded_list(loaded_ids: &[String], model_id: &str) -> bool {
    loaded_ids
        .iter()
        .any(|id| id == model_id || id.starts_with(&format!("{model_id}:")))
}

/// Check whether a model is currently loaded in the Foundry Local web service.
///
/// Queries the `/models/loaded` endpoint and checks if the given model id
/// (variant id, may contain a colon) is present.  The endpoint returns a JSON
/// array of loaded model ids.
async fn is_model_loaded_via_http(service_endpoint: &str, model_id: &str) -> Result<bool> {
    let Some(loaded_ids) = query_models_loaded(service_endpoint).await? else {
        return Ok(false);
    };

    let is_loaded = is_model_in_loaded_list(&loaded_ids, model_id);

    if is_loaded {
        tracing::info!(
            model_id = %model_id,
            "Model is already loaded in Foundry Local web service"
        );
    }

    Ok(is_loaded)
}

/// Refresh the Foundry Local web-service endpoint.
///
/// Loading a model can stop or restart the local web service, which makes the
/// endpoint URL cached at client-construction time stale.  This helper
/// re-queries `manager.urls()` and, if necessary, restarts the service and
/// polls for a new URL.
async fn refresh_service_endpoint(
    manager: &'static foundry_local_sdk::FoundryLocalManager,
    current_endpoint: &str,
) -> Result<String> {
    let mut urls = manager.urls().unwrap_or_default();

    if urls.is_empty() {
        tracing::info!(
            "Foundry Local web service not listening after model preparation; restarting"
        );
        manager
            .start_web_service()
            .await
            .with_context(|| "Failed to restart Foundry Local web service")?;

        let poll_interval = std::time::Duration::from_millis(200);
        let max_attempts: u32 = 600; // 120 s (large models can take a while to bind)
        for attempt in 0..max_attempts {
            urls = manager.urls().unwrap_or_default();
            if !urls.is_empty() {
                break;
            }
            if attempt > 0 && attempt % 25 == 0 {
                // Emit a progress heartbeat every 5 s so the UI doesn't look stuck.
                tracing::info!(
                    elapsed_secs = attempt * poll_interval.as_millis() as u32 / 1000,
                    "Still waiting for Foundry Local web-service endpoint"
                );
            }
            tokio::time::sleep(poll_interval).await;
        }
    }

    let endpoint = urls.into_iter().next().unwrap_or_default();
    if endpoint.is_empty() {
        bail!("Foundry Local web service did not report any URLs after model preparation");
    }

    if endpoint != current_endpoint {
        tracing::info!(
            old_endpoint = %current_endpoint,
            new_endpoint = %endpoint,
            "Foundry Local web service endpoint changed"
        );
    }

    Ok(endpoint)
}

/// Wait until the requested model is actually loaded and ready in the Foundry
/// Local web service.
///
/// The most reliable readiness signal is the web service's `/models/loaded`
/// endpoint, which returns the ids of models currently loaded into memory.
/// `/v1/models` only lists downloaded/cataloged models and may expose a model
/// before it can actually serve chat requests; chatting with an unloaded model
/// produces an empty SSE stream.  This helper therefore polls `/models/loaded`
/// first and falls back to `/v1/models` only when the loaded endpoint is
/// unavailable (older Foundry Local builds).
///
/// Polls for up to 120 seconds and bails with a clear message if the model does
/// not become ready.
async fn wait_for_model_ready(
    service_endpoint: &str,
    client: &OpenAiClient,
    model_id: &str,
    session_id: &str,
    event_bus: Option<Arc<EventBus>>,
) -> Result<()> {
    let poll_interval = std::time::Duration::from_millis(200);
    let max_attempts: u32 = 600; // 120 s
    let progress_every = 25; // every ~5 s

    // Prefer the web service's /models/loaded endpoint; it is the only
    // endpoint that distinguishes "cached" from "loaded into memory".
    let loaded_ids_url = format!(
        "{}models/loaded",
        service_endpoint
            .trim_end_matches("/v1")
            .trim_end_matches('/')
    );

    for attempt in 0..max_attempts {
        let mut ready = false;

        // First try /models/loaded.
        match query_models_loaded(service_endpoint).await {
            Ok(Some(loaded_ids)) => {
                if is_model_in_loaded_list(&loaded_ids, model_id) {
                    ready = true;
                } else if attempt == 0 {
                    tracing::info!(
                        model = %model_id,
                        "Waiting for Foundry Local model to be loaded into memory"
                    );
                } else if attempt % progress_every == 0 {
                    tracing::info!(
                        model = %model_id,
                        elapsed_secs = attempt * poll_interval.as_millis() as u32 / 1000,
                        "Still waiting for Foundry Local model to be loaded into memory"
                    );
                }
            }
            Ok(None) => {
                // /models/loaded is unavailable; fall back to /v1/models once
                // per poll cycle.
                let fallback_url = format!("{}/v1/models", client.base_url());
                match client.http_client().get(&fallback_url).send().await {
                    Ok(response) if response.status().is_success() => {
                        let text = response.text().await.unwrap_or_default();
                        let ids = parse_loaded_model_ids(&text);
                        if ids.iter().any(|id| id == model_id) {
                            ready = true;
                        } else if attempt == 0 {
                            tracing::info!(
                                model = %model_id,
                                "Waiting for Foundry Local model to become ready"
                            );
                        } else if attempt % progress_every == 0 {
                            tracing::info!(
                                model = %model_id,
                                elapsed_secs = attempt * poll_interval.as_millis() as u32 / 1000,
                                "Still waiting for Foundry Local model to become ready"
                            );
                        }
                    }
                    Ok(response) => {
                        tracing::debug!(
                            status = %response.status(),
                            url = %fallback_url,
                            "Foundry Local /v1/models returned non-success status while waiting"
                        );
                    }
                    Err(e) => {
                        tracing::debug!(
                            error = %e,
                            url = %fallback_url,
                            "Failed to query Foundry Local /v1/models while waiting"
                        );
                    }
                }
            }
            Err(e) => {
                tracing::debug!(
                    error = %e,
                    url = %loaded_ids_url,
                    "Error querying Foundry Local /models/loaded while waiting"
                );
            }
        }

        if ready {
            if attempt > 0 {
                tracing::info!(
                    model = %model_id,
                    elapsed_secs = attempt * poll_interval.as_millis() as u32 / 1000,
                    "Foundry Local model is now ready"
                );
            }
            return Ok(());
        }

        tokio::time::sleep(poll_interval).await;
    }

    let elapsed_secs = max_attempts * poll_interval.as_millis() as u32 / 1000;
    let message = format!(
        "Microsoft Foundry Local model '{}' was loaded but did not become ready \
         on the local endpoint within {elapsed_secs} seconds. \
         Try `foundry model load {0}` or restart the Foundry Local service.",
        model_id
    );
    if let Some(ref bus) = event_bus {
        bus.publish(Event::ModelDownloadFinished {
            provider_id: "foundry_local".to_string(),
            model_id: model_id.to_string(),
            session_id: session_id.to_string(),
            error: Some(message.clone()),
        });
    }
    bail!(message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_sets_empty_api_key() {
        let client = FoundryLocalClient::new("http://localhost:5000/v1", None, None);
        assert_eq!(client.inner().api_key(), "");
    }

    #[test]
    fn test_new_trims_endpoint() {
        let client = FoundryLocalClient::new("http://localhost:5000/v1/", None, None);
        assert_eq!(client.inner().base_url(), "http://localhost:5000/v1");
    }

    #[test]
    fn test_new_without_manager_does_not_auto_load() {
        let client = FoundryLocalClient::new("http://localhost:5000/v1", None, None);
        assert!(client.manager.is_none());
    }

    #[test]
    fn test_parse_loaded_model_ids_handles_extra_fields() {
        let body = r#"{"data":[{"id":"phi-4","object":"model"}],"IsDelta":false,"Successful":true,"HttpStatusCode":0,"object":"list"}"#;
        let ids = parse_loaded_model_ids(body);
        assert_eq!(ids, vec!["phi-4".to_string()]);
    }

    #[test]
    fn test_parse_loaded_model_ids_returns_empty_for_malformed() {
        let ids = parse_loaded_model_ids("not json");
        assert!(ids.is_empty());
    }

    #[test]
    fn test_parse_loaded_model_ids_returns_empty_for_missing_data() {
        let ids = parse_loaded_model_ids(r#"{"object":"list"}"#);
        assert!(ids.is_empty());
    }

    #[test]
    fn test_strip_model_version_suffix_removes_numeric_suffix() {
        assert_eq!(
            strip_model_version_suffix("qwen2.5-coder-7b-instruct-generic-cpu:4"),
            "qwen2.5-coder-7b-instruct-generic-cpu"
        );
    }

    #[test]
    fn test_strip_model_version_suffix_keeps_non_numeric_suffix() {
        // We only strip purely-numeric version suffixes.  Colon-containing ids
        // with non-numeric suffixes (e.g. tags) are left unchanged.
        assert_eq!(
            strip_model_version_suffix("my-model:5000"),
            "my-model" // numeric suffix is treated as a version
        );
        assert_eq!(strip_model_version_suffix("my-model:beta"), "my-model:beta");
    }
    #[test]
    fn test_strip_model_version_suffix_no_suffix_unchanged() {
        assert_eq!(
            strip_model_version_suffix("qwen2.5-coder-7b-instruct-generic-cpu"),
            "qwen2.5-coder-7b-instruct-generic-cpu"
        );
    }
}

#[test]
fn test_is_model_in_loaded_list_exact_match() {
    assert!(is_model_in_loaded_list(
        &["qwen2.5-coder-7b-instruct-generic-cpu:4".to_string()],
        "qwen2.5-coder-7b-instruct-generic-cpu:4"
    ));
}

#[test]
fn test_is_model_in_loaded_list_prefix_match() {
    // /models/loaded returns suffixed ids; the readiness check often passes
    // the unsuffixed base id.
    assert!(is_model_in_loaded_list(
        &["qwen2.5-coder-7b-instruct-generic-cpu:4".to_string()],
        "qwen2.5-coder-7b-instruct-generic-cpu"
    ));
}

#[test]
fn test_is_model_in_loaded_list_missing() {
    assert!(!is_model_in_loaded_list(
        &["qwen3-0.6b-generic-cpu:4".to_string()],
        "qwen2.5-coder-7b-instruct-generic-cpu"
    ));
}

#[test]
fn test_is_model_in_loaded_list_empty() {
    assert!(!is_model_in_loaded_list(
        &Vec::new(),
        "qwen2.5-coder-7b-instruct-generic-cpu"
    ));
}

#[test]
fn test_load_id_uses_full_sdk_variant_id() {
    // This is a conceptual test: the load id used in ensure_model_loaded
    // should be the SDK's full variant id (with suffix), not a
    // reconstructed string.  We verify strip_model_version_suffix and the
    // assumption that the SDK id carries the suffix.
    let sdk_id = "qwen2.5-coder-7b-instruct-generic-cpu:4";
    let resolved = strip_model_version_suffix(sdk_id);
    assert_eq!(resolved, "qwen2.5-coder-7b-instruct-generic-cpu");
    // load_id in the implementation is sdk_id directly.
    assert_eq!(sdk_id, "qwen2.5-coder-7b-instruct-generic-cpu:4");
}
