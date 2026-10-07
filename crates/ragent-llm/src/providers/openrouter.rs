//! OpenRouter provider implementation.
//!
//! Connects to OpenRouter at `https://openrouter.ai` via its OpenAI-compatible
//! API, using bearer-token authentication against `OPENROUTER_API_KEY`. The
//! provider resolves keys in FR-004 precedence: per-call argument, then the
//! encrypted credential stored under provider id `openrouter`, then the
//! environment variable. Model discovery and chat streaming are filled in by
//! later spec tasks.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::RwLock;

use crate::llm::{ChatRequest, LlmClient, StreamEvent};
use crate::provider::http_client;
use crate::provider::http_client::{MAX_ERROR_BODY_BYTES, read_body_capped};
use crate::provider::thinking::{full_reasoning_levels, openrouter_reasoning_payload_from_request};
use crate::{ModelInfo, Provider};
use ragent_config::{Capabilities, Cost};

const DEFAULT_OPENROUTER_HOST: &str = "https://openrouter.ai";

/// Returns a masked fingerprint of an API key for diagnostics.
///
/// Mirrors FR-005: only the final four characters are shown behind an
/// ellipsis (e.g. `...abcd`); empty keys render as `(none)` so callers can
/// distinguish "no key" from "key present".
///
/// # Examples
///
/// ```
/// use ragent_llm::provider::openrouter::mask_key;
///
/// assert_eq!(mask_key("sk-or-v1-0123456789abcd"), "...abcd");
/// assert_eq!(mask_key(""), "(none)");
/// ```
#[must_use]
pub fn mask_key(key: &str) -> String {
    if key.is_empty() {
        return String::from("(none)");
    }
    // Find the byte offset of the 4th character from the end (or the start of
    // the string when it has fewer than four characters), avoiding the
    // intermediate `Vec<char>` allocation.
    let start = key.char_indices().rev().nth(3).map_or(0, |(i, _)| i);
    format!("...{}", &key[start..])
}

/// Provider implementation for OpenRouter.
pub struct OpenRouterProvider {
    base_url: String,
    /// Storage handle used to resolve database-backed API keys (FR-004b).
    /// Attached by the binary/TUI after storage is created.
    storage: RwLock<Option<std::sync::Arc<ragent_storage::Storage>>>,
}

impl OpenRouterProvider {
    /// Creates a provider for the OpenRouter API.
    #[must_use]
    pub fn new() -> Self {
        Self::with_url(DEFAULT_OPENROUTER_HOST)
    }

    /// Creates a provider pointing at an OpenRouter-compatible base URL.
    ///
    /// Trailing slashes are trimmed so request paths append cleanly (FR-002).
    #[must_use]
    pub fn with_url(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            storage: RwLock::new(None),
        }
    }

    /// Attach the storage handle used to resolve database-backed API keys.
    ///
    /// Called once after storage is initialized, mirroring
    /// [`crate::provider::router::RouterProvider::set_storage`].
    pub fn set_storage(&self, storage: std::sync::Arc<ragent_storage::Storage>) {
        if let Ok(mut guard) = self.storage.write() {
            *guard = Some(storage);
        }
    }

    /// Returns the configured (trailing-slash-trimmed) base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Resolves the stored credential for provider id `openrouter`.
    ///
    /// Returns `None` when no storage handle is attached or no non-empty
    /// credential is stored.
    async fn resolve_stored_key(&self) -> Option<String> {
        let storage = self.storage.read().ok()?.clone()?;
        let provider_id = self.id().to_string();
        // `get_provider_auth` performs synchronous SQLite I/O; keep it off
        // the async worker threads (same treatment as the router client).
        tokio::task::spawn_blocking(move || {
            storage
                .get_provider_auth(&provider_id)
                .ok()
                .flatten()
                .filter(|key| !key.is_empty())
        })
        .await
        .ok()
        .flatten()
    }

    /// Resolves the API key used for model discovery.
    ///
    /// Discovery follows the same precedence as chat authentication minus the
    /// per-call argument, which the [`Provider`] trait does not pass to
    /// [`discover_models`]: stored credential first, then the
    /// `OPENROUTER_API_KEY` environment variable (FR-004, FR-008).
    async fn resolve_discovery_key(&self) -> String {
        if let Some(stored) = self.resolve_stored_key().await {
            return stored;
        }
        ragent_config::credential_env::read_credential_env("OPENROUTER_API_KEY").unwrap_or_default()
    }

    /// Queries the OpenRouter `/api/v1/models` endpoint for live model discovery.
    ///
    /// The endpoint is public, so the `Authorization` header is attached only when
    /// a non-empty key is available (FR-007, FR-008). A 10-second per-request
    /// timeout bounds the call; failures are logged with `warn!` and surfaced as
    /// a human-readable error without retrying the GET (FR-007, FR-022).
    async fn discover_models_impl(&self) -> Result<Vec<ModelInfo>> {
        let api_key = self.resolve_discovery_key().await;
        let url = format!("{}/api/v1/models", self.base_url);
        let client = http_client::create_http_client();
        let mut request = client.get(&url).timeout(std::time::Duration::from_secs(10));
        if !api_key.is_empty() {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }

        let response = request
            .send()
            .await
            .inspect_err(|e| {
                tracing::warn!(url = %url, error = %e, "OpenRouter model discovery failed");
            })
            .with_context(|| format!("Failed to connect to OpenRouter model list at {url}"))?;

        if !response.status().is_success() {
            bail!(
                "OpenRouter API returned status {} from {}",
                response.status(),
                url
            );
        }

        let body: OpenRouterModelsResponse = response
            .json()
            .await
            .context("Failed to parse OpenRouter model list")?;

        let models: Vec<ModelInfo> = body
            .data
            .into_iter()
            .filter_map(openrouter_model_to_info)
            .collect();

        Ok(models)
    }
}

impl Default for OpenRouterProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Provider for OpenRouterProvider {
    fn id(&self) -> &'static str {
        "openrouter"
    }

    fn name(&self) -> &'static str {
        "OpenRouter"
    }

    fn as_any_static(&self) -> &dyn std::any::Any {
        self
    }
    fn default_models(&self) -> Vec<ModelInfo> {
        vec![]
    }

    /// Discovers models from the OpenRouter `/api/v1/models` endpoint.
    ///
    /// Implements FR-007 and FR-008: a single public GET with a 10-second
    /// timeout, optional Bearer authentication, and graceful handling of
    /// malformed entries.
    async fn discover_models(&self) -> Result<Vec<ModelInfo>> {
        self.discover_models_impl().await
    }

    async fn create_client(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        _options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        // FR-004 key sourcing precedence: (a) per-call argument passed by the
        // session layer, (b) encrypted credential under provider id
        // `openrouter`, (c) the OPENROUTER_API_KEY environment variable.
        let mut resolved = api_key.to_string();
        if resolved.is_empty()
            && let Some(stored) = self.resolve_stored_key().await
        {
            resolved = stored;
        }
        if resolved.is_empty() {
            resolved = ragent_config::credential_env::read_credential_env("OPENROUTER_API_KEY")
                .unwrap_or_default();
        }

        // FR-009: reject unauthenticated chat attempts with an explicit,
        // remediation-bearing error instead of firing an unauthorized call.
        if resolved.is_empty() {
            bail!(
                "OpenRouter requires an API key. Run 'ragent auth openrouter <key>' \
                 or set the OPENROUTER_API_KEY environment variable."
            );
        }

        // FR-005: route the resolved key through the global redaction
        // registry so any accidental error/log interpolation is masked.
        ragent_types::sanitize::register_secret(&resolved);

        // FR-002/FR-003: the call-scoped base URL overrides the provider
        // default; both are trailing-slash-trimmed (the default is HTTPS).
        let base = base_url.unwrap_or(&self.base_url);
        let base = base.trim_end_matches('/');

        tracing::info!(
            chat_endpoint = %format!("{base}/api/v1/chat/completions"),
            models_endpoint = %format!("{base}/api/v1/models"),
            key_fingerprint = %mask_key(&resolved),
            "OpenRouter provider client created"
        );

        Ok(Box::new(OpenRouterClient {
            api_key: resolved,
            base_url: base.to_string(),
            http: http_client::create_streaming_http_client(),
        }))
    }
}
/// Response envelope from OpenRouter `GET /api/v1/models`.
///
/// Tolerant to missing fields: an absent `data` array defaults to empty so
/// an otherwise well-formed response never fails parsing (FR-007).
#[derive(Debug, Deserialize)]
struct OpenRouterModelsResponse {
    #[serde(default)]
    data: Vec<OpenRouterModelEntry>,
}

/// Raw model entry returned by the OpenRouter models endpoint.
///
/// All fields are optional with defaults so that the provider can list models
/// even when OpenRouter adds or omits metadata keys (FR-022).
#[derive(Debug, Deserialize)]
struct OpenRouterModelEntry {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    // reason: description will be used for model display fallback in openrouterprov T-004.
    description: Option<String>,
    #[serde(default)]
    context_length: Option<usize>,
    #[serde(default)]
    top_provider: Option<OpenRouterTopProvider>,
    #[serde(default)]
    pricing: Option<OpenRouterPricing>,
    #[serde(default)]
    architecture: Option<OpenRouterArchitecture>,
    #[serde(default)]
    supported_parameters: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenRouterTopProvider {
    #[serde(default)]
    context_length: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenRouterPricing {
    #[serde(default)]
    prompt: Option<Value>,
    #[serde(default)]
    completion: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenRouterArchitecture {
    #[serde(default)]
    input_modalities: Option<Vec<String>>,
}

/// Parses a raw OpenRouter price value into USD per million tokens.
///
/// OpenRouter prices are quoted in US dollars per token (e.g. `0.000003`
/// or `"1e-7"`). We multiply by one million to match ragent's `Cost` scale.
/// Absent or unparseable values silently default to `0.0` so that a malformed
/// price field does not poison the whole model list (FR-010, FR-011).
fn parse_price_per_million(value: Option<&Value>) -> f64 {
    let raw = match value {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse::<f64>().ok(),
        _ => None,
    };
    raw.map(|dollars_per_token| dollars_per_token * 1_000_000.0)
        .unwrap_or(0.0)
}

/// Resolves the context window for a model entry.
///
/// Uses the top-level `context_length` when present; otherwise falls back to
/// `top_provider.context_length` (FR-010).
fn context_length_from_entry(entry: &OpenRouterModelEntry) -> usize {
    entry
        .context_length
        .or_else(|| entry.top_provider.as_ref().and_then(|tp| tp.context_length))
        .unwrap_or(0)
}

/// Returns `true` when the model advertises image input support.
fn has_vision_from_entry(entry: &OpenRouterModelEntry) -> bool {
    entry
        .architecture
        .as_ref()
        .and_then(|arch| arch.input_modalities.as_ref())
        .is_some_and(|modalities| modalities.iter().any(|m| m.eq_ignore_ascii_case("image")))
}

/// Returns `true` when the model advertises reasoning support.
///
/// Detects the literal parameter `"reasoning"` as well as supported/allowed
/// variants such as `"reasoning:required"` or `"reasoning:low"` (FR-010).
fn has_reasoning_from_entry(entry: &OpenRouterModelEntry) -> bool {
    entry.supported_parameters.as_ref().is_some_and(|params| {
        params.iter().any(|p| {
            let p = p.to_ascii_lowercase();
            p == "reasoning" || p.starts_with("reasoning:")
        })
    })
}

/// Converts a raw OpenRouter model entry into a [`ModelInfo`].
///
/// Skips entries with empty ids and emits a `warn!` log so a single bad row
/// does not fail the whole discovery response (FR-022).
fn openrouter_model_to_info(entry: OpenRouterModelEntry) -> Option<ModelInfo> {
    if entry.id.is_empty() {
        tracing::warn!("OpenRouter model entry has an empty id; skipping");
        return None;
    }

    let name = entry
        .name
        .as_ref()
        .filter(|name| !name.is_empty())
        .cloned()
        .unwrap_or_else(|| entry.id.clone());

    let reasoning = has_reasoning_from_entry(&entry);
    let vision = has_vision_from_entry(&entry);
    let context_window = context_length_from_entry(&entry);
    let cost = Cost {
        input: parse_price_per_million(entry.pricing.as_ref().and_then(|p| p.prompt.as_ref())),
        output: parse_price_per_million(entry.pricing.as_ref().and_then(|p| p.completion.as_ref())),
    };
    let capabilities = Capabilities {
        reasoning,
        streaming: true,
        vision,
        tool_use: true,
        thinking_levels: if reasoning {
            full_reasoning_levels()
        } else {
            Vec::new()
        },
    };

    Some(ModelInfo {
        id: entry.id,
        provider_id: "openrouter".to_string(),
        name,
        cost,
        capabilities,
        context_window,
        max_output: None,
        request_multiplier: None,
        thinking_config: None,
    })
}

/// OpenRouter chat client constructed by [`OpenRouterProvider::create_client`].
///
/// FR-025: the chat POST path must use a single `.send()` per request - no
/// `execute_with_retry` wrapper - because automatically retrying a chat POST
/// can double-bill. Only the discovery GET (spec task T-003) is ever
/// retry-eligible.
pub struct OpenRouterClient {
    api_key: String,
    base_url: String,
    http: reqwest::Client,
}

impl OpenRouterClient {
    /// Create a new OpenRouter chat client from an API key and base URL.
    ///
    /// Exposed to integration tests so they can build request bodies directly
    /// without creating a provider or hitting the network.
    #[must_use]
    pub fn new(api_key: &str, base_url: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            http: http_client::create_streaming_http_client(),
        }
    }

    /// Returns the configured (trailing-slash-trimmed) base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

impl OpenRouterClient {
    /// Build the JSON request body for the OpenRouter chat-completions API.
    ///
    /// The message packing, base body, sampling fields, and cached tool list
    /// come from the shared [`OpenAiCompat`] builder (audit T-401); OpenRouter
    /// only adds its `reasoning` payload tail.
    ///
    /// # Errors
    ///
    /// This function is infallible.
    #[must_use]
    pub fn build_request_body(&self, request: &ChatRequest) -> Value {
        let mut compat = super::openai_compat::OpenAiCompat::base_body(
            request,
            super::openai_compat::OpenAiCompatSpec::openai(),
            &request.tools,
        );
        if let Some(reasoning) = openrouter_reasoning_payload_from_request(request) {
            compat.body_mut()["reasoning"] = reasoning;
        }
        compat.finish()
    }

    /// Parses an OpenAI-compatible SSE stream into [`StreamEvent`]s.
    ///
    /// Delegates to the shared [`super::sse::parse_openai_sse_stream`] parser
    /// (audit T-402). OpenRouter keeps its `reasoning` / `reasoning_content`
    /// delta routing (FR-020) and its provider-specific empty-stream message
    /// through [`super::sse::OpenAiSseSpec::openrouter`].
    fn parse_sse_stream(
        &self,
        response: reqwest::Response,
    ) -> Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>> {
        super::sse::parse_openai_sse_stream(
            response,
            super::sse::OpenAiSseSpec::openrouter(self.base_url.clone()),
        )
    }
}

#[async_trait::async_trait]
impl LlmClient for OpenRouterClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = crate::llm::StreamEvent> + Send>>> {
        let url = format!("{}/api/v1/chat/completions", self.base_url);
        let body = self.build_request_body(&request);
        let body_bytes = serde_json::to_vec(&body).context("serialise OpenRouter request body")?;

        tracing::debug!(
            url = %url,
            model = %request.model,
            has_tools = !request.tools.is_empty(),
            "OpenRouter chat request"
        );

        // ANTIPAT 3.5: first-byte timeout defaults to the shared
        // `DEFAULT_STREAM_TIMEOUT_SECS` (mirrors Ollama Cloud); the per-chunk
        // idle guard is separate (`STREAM_CHUNK_IDLE_TIMEOUT_SECS`).
        let first_byte_timeout = request
            .stream_timeout_secs
            .unwrap_or(http_client::DEFAULT_STREAM_TIMEOUT_SECS);
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(first_byte_timeout),
            self.http
                .post(&url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("content-type", "application/json")
                .body(body_bytes)
                .send(),
        )
        .await
        .inspect_err(|e| {
            tracing::warn!(provider = "openrouter", url = %url, error = %e, "chat request timed out");
        })
        .map_err(|_| {
            anyhow::anyhow!("OpenRouter: initial response timed out after {first_byte_timeout}s")
        })?
        .inspect_err(|e| {
            tracing::warn!(provider = "openrouter", url = %url, error = %e, "chat request failed");
        })
        .with_context(|| format!("Failed to connect to OpenRouter at {url}"))?;

        if !response.status().is_success() {
            let status = response.status();
            // ANTIPAT 3.1/3.2: capped error-body read (replaces the local
            // MAX_ERR_LEN truncation).
            let error_body = read_body_capped(response, MAX_ERROR_BODY_BYTES).await;
            const MAX_ERR_LEN: usize = 4096;
            // Clamp to a char boundary: slicing at a raw byte offset panics when
            // it lands inside a multibyte UTF-8 character (FUNC-001).
            let error_body = if error_body.len() > MAX_ERR_LEN {
                format!(
                    "{}...[truncated {} bytes]",
                    ragent_types::strutil::truncate_bytes_no_ellipsis(&error_body, MAX_ERR_LEN),
                    error_body.len() - MAX_ERR_LEN
                )
            } else {
                error_body
            };
            // SEC: redact the provider error body before logging or surfacing it.
            let error_body = ragent_types::sanitize::redact_secrets(&error_body);
            tracing::warn!(
                provider = "openrouter",
                url = %url,
                model = %request.model,
                status = %status,
                error = %error_body,
                "API error"
            );
            bail!("OpenRouter API error ({status}): {error_body}");
        }

        Ok(self.parse_sse_stream(response))
    }
}
