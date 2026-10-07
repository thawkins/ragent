//! `OpenAI` provider implementation.
//!
//! Implements the [`Provider`] trait for the `OpenAI` Chat Completions API, supporting
//! streaming responses, tool calls, and usage tracking.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::pin::Pin;

use super::openai_compat::OpenAiCompat;
use super::thinking::openai_thinking_levels_for_model;
use crate::llm::{ChatRequest, LlmClient, StreamEvent};
use crate::provider::http_client::{MAX_ERROR_BODY_BYTES, read_body_capped};
use crate::{ModelInfo, Provider};
use ragent_config::{Capabilities, Cost};

/// Default API base URL for OpenAI-compatible endpoints.
pub const OPENAI_API_BASE: &str = "https://api.openai.com";

/// Fallback context window (tokens) reported for a discovered model whose
/// `/v1/models` entry does not declare one (ANTIPAT M6.10).
const DEFAULT_DISCOVERED_CONTEXT_WINDOW: usize = 128_000;

/// Fallback max-output (tokens) reported for a discovered model whose
/// `/v1/models` entry does not declare one (ANTIPAT M6.10).
const DEFAULT_DISCOVERED_MAX_OUTPUT: usize = 16_384;

/// Returns the default `OpenAI` model catalog with `provider_id` attached.
///
/// This catalog is only used by tests; the `OpenAiProvider` itself no longer
/// ships hard-coded default models and discovers them at runtime instead.
#[must_use]
#[cfg(test)]
pub fn openai_default_models(provider_id: &str) -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "gpt-4o".to_string(),
            provider_id: provider_id.to_string(),
            name: "GPT-4o".to_string(),
            cost: Cost {
                input: 2.50,
                output: 10.0,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: true,
                tool_use: true,
                thinking_levels: openai_thinking_levels_for_model("gpt-4o"),
            },
            context_window: 128_000,
            max_output: Some(16_384),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "gpt-4o-mini".to_string(),
            provider_id: provider_id.to_string(),
            name: "GPT-4o Mini".to_string(),
            cost: Cost {
                input: 0.15,
                output: 0.60,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: true,
                tool_use: true,
                thinking_levels: openai_thinking_levels_for_model("gpt-4o-mini"),
            },
            context_window: 128_000,
            max_output: Some(16_384),
            request_multiplier: None,
            thinking_config: None,
        },
    ]
}

/// Provider implementation for the `OpenAI` Chat Completions API.
pub struct OpenAiProvider;

#[async_trait::async_trait]
impl Provider for OpenAiProvider {
    /// Returns `"openai"`.
    ///
    /// # Errors
    ///
    /// This function is infallible.
    fn id(&self) -> &'static str {
        "openai"
    }

    /// Returns `"OpenAI"`.
    ///
    /// # Errors
    ///
    /// This function is infallible.
    fn name(&self) -> &'static str {
        "OpenAI"
    }

    fn as_any_static(&self) -> &dyn std::any::Any {
        self
    }

    /// Returns an empty catalog.
    ///
    /// OpenAI models are discovered at runtime from the `/v1/models` endpoint;
    /// no models are hard-coded.
    fn default_models(&self) -> Vec<ModelInfo> {
        Vec::new()
    }

    /// Discover available models from the OpenAI `/v1/models` endpoint.
    async fn discover_models(&self) -> Result<Vec<ModelInfo>> {
        let api_key = ragent_config::credential_env::provider_credential_env("openai")
            .context("OpenAI model discovery requires OPENAI_API_KEY")?;
        let models = discover_openai_models(&api_key, OPENAI_API_BASE, "openai")
            .await
            .with_context(|| "OpenAI model discovery failed")?;
        Ok(models)
    }

    /// Creates an [`OpenAiClient`] configured with the given API key and optional base URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be constructed.
    async fn create_client(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        _options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        // ANTIPAT 3.6: register the credential with the shared redaction
        // registry so any text passed through `redact_secrets` masks it.
        ragent_types::sanitize::register_secret(api_key);
        let resolved = base_url.unwrap_or(OPENAI_API_BASE);
        let client = OpenAiClient::new(api_key, resolved);
        tracing::info!(chat_endpoint = %format!("{}/v1/chat/completions", resolved.trim_end_matches('/')), "OpenAI provider connected");
        Ok(Box::new(client))
    }
}

/// HTTP client for the `OpenAI` Chat Completions API with streaming SSE support.
pub struct OpenAiClient {
    api_key: String,
    base_url: String,
    http: reqwest::Client,
    provider_name: String,
}

impl OpenAiClient {
    /// Create a new OpenAI-compatible client.
    #[must_use]
    pub fn new(api_key: &str, base_url: &str) -> Self {
        Self::new_with_provider(api_key, base_url, "openai")
    }

    /// Create a new OpenAI-compatible client with a custom provider label.
    ///
    /// The label is used only for diagnostic logging/messages.
    #[must_use]
    pub fn new_with_provider(api_key: &str, base_url: &str, provider_name: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            http: crate::provider::http_client::create_streaming_http_client(),
            provider_name: provider_name.to_string(),
        }
    }

    /// Returns the API key used by this client.
    #[must_use]
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Returns the base URL used by this client.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Returns the reusable HTTP client held by this client.
    ///
    /// Used by wrapper providers (e.g. Azure AI Foundry) that send their own
    /// authenticated request but want to share the same connection pool.
    #[must_use]
    pub(crate) fn http_client(&self) -> &reqwest::Client {
        &self.http
    }

    /// Build the JSON request body for the `OpenAI` Chat Completions API.
    ///
    /// The message packing, base body, sampling fields, cached tool list, and
    /// trailing `reasoning_effort` are produced by the shared
    /// [`OpenAiCompat`] builder (audit T-401); this method only applies the
    /// OpenAI dialect and the OpenAI-specific reasoning tail.
    ///
    /// # Errors
    ///
    /// This function is infallible.
    pub fn build_request_body(&self, request: &ChatRequest) -> Value {
        let mut compat = OpenAiCompat::base_body(
            request,
            super::openai_compat::OpenAiCompatSpec::openai(),
            &request.tools,
        );
        if let Some(reasoning_effort) = super::thinking::reasoning_effort_from_request(request) {
            compat.body_mut()["reasoning_effort"] = json!(reasoning_effort);
        }
        compat.finish()
    }
}
#[async_trait::async_trait]
impl LlmClient for OpenAiClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let body = self.build_request_body(&request);
        // H2: serialise directly into a byte buffer (single allocation, no
        // intermediate string) and send via `RequestBuilder::body` rather than
        // `.json(&body)` which re-serialises through a serde adapter.
        let body_bytes = serde_json::to_vec(&body).context("serialise OpenAI request body")?;

        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .body(body_bytes)
            .send()
            .await
            .inspect_err(|e| {
                tracing::warn!(provider = %self.provider_name, url = %url, error = %e, "chat request failed");
            })
            .with_context(|| format!("Failed to send request to OpenAI API at {url}"))?;

        if !response.status().is_success() {
            let status = response.status();
            // ANTIPAT 3.1/3.2: capped error-body read.
            let body = read_body_capped(response, MAX_ERROR_BODY_BYTES).await;
            // SEC: redact the provider error body before logging or surfacing it.
            let body = ragent_types::sanitize::redact_secrets(&body);
            tracing::warn!(
                provider = %self.provider_name,
                url = %url,
                model = %request.model,
                status = %status,
                error = %body,
                "API error"
            );
            bail!("OpenAI API error ({status}): {body}");
        }

        Ok(self.parse_sse_stream(response))
    }
}

impl OpenAiClient {
    /// Parse an SSE stream from an already-successful HTTP response.
    ///
    /// Delegates to the shared [`super::sse::parse_openai_sse_stream`] parser
    /// (audit T-402) so the OpenAI, Azure Foundry, and OpenRouter clients all
    /// run the same event-mapping logic. Kept as a method so existing callers
    /// (Azure Foundry) are unchanged.
    pub(crate) fn parse_sse_stream(
        &self,
        response: reqwest::Response,
    ) -> Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>> {
        super::sse::parse_openai_sse_stream(
            response,
            super::sse::OpenAiSseSpec::openai(self.provider_name.clone()),
        )
    }
}

/// Parses OpenAI-style rate-limit response headers into a `StreamEvent::RateLimit`.
///
/// Used by `OpenAI` and Copilot providers (both follow the same header convention).
/// Headers: `x-ratelimit-limit-requests`, `x-ratelimit-remaining-requests`,
///          `x-ratelimit-limit-tokens`, `x-ratelimit-remaining-tokens`.
pub(crate) fn parse_openai_rate_limit_headers(
    headers: &reqwest::header::HeaderMap,
) -> Option<crate::llm::StreamEvent> {
    let header_u64 = |name: &str| -> Option<u64> {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
    };

    let req_limit = header_u64("x-ratelimit-limit-requests");
    let req_remaining = header_u64("x-ratelimit-remaining-requests");
    let tok_limit = header_u64("x-ratelimit-limit-tokens");
    let tok_remaining = header_u64("x-ratelimit-remaining-tokens");

    let used_pct = |limit: u64, remaining: u64| {
        if limit == 0 {
            0.0f32
        } else {
            ((limit.saturating_sub(remaining)) as f32 / limit as f32 * 100.0).clamp(0.0, 100.0)
        }
    };

    let requests_used_pct = req_limit.zip(req_remaining).map(|(l, r)| used_pct(l, r));
    let tokens_used_pct = tok_limit.zip(tok_remaining).map(|(l, r)| used_pct(l, r));

    if requests_used_pct.is_some() || tokens_used_pct.is_some() {
        Some(crate::llm::StreamEvent::RateLimit {
            requests_used_pct,
            tokens_used_pct,
        })
    } else {
        None
    }
}

/// Query an OpenAI-compatible `/v1/models` endpoint and return discovered
/// [`ModelInfo`] entries.
///
/// This helper is shared by the `openai`, `generic_openai`, and `xai`
/// providers. It performs a lightweight heuristic pass over the returned IDs
/// to skip obvious non-chat models (embeddings, audio, image generation,
/// moderations, and legacy completion engines).
pub async fn discover_openai_models(
    api_key: &str,
    base_url: &str,
    provider_id: &str,
) -> Result<Vec<ModelInfo>> {
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    let response = crate::provider::http_client::create_http_client()
        .get(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .with_context(|| format!("Failed to connect to {provider_id} models endpoint at {url}"))?;
    if !response.status().is_success() {
        bail!(
            "{provider_id} models endpoint returned status {}",
            response.status()
        );
    }
    let payload: Value = response
        .json()
        .await
        .context("Failed to parse models response")?;
    let data = payload
        .get("data")
        .and_then(Value::as_array)
        .context("Unexpected models response format: missing 'data' array")?;

    Ok(data
        .iter()
        .filter_map(|entry| entry.get("id").and_then(Value::as_str))
        .filter(|id| is_openai_chat_model_id(id))
        .map(|id| {
            let model_id = id.to_string();
            let vision = model_id.contains("vision") || model_id.contains("gpt-4o");
            let thinking_levels = openai_thinking_levels_for_model(&model_id);
            let reasoning = !thinking_levels.is_empty();
            ModelInfo {
                id: model_id.clone(),
                provider_id: provider_id.to_string(),
                name: model_id,
                cost: Cost {
                    input: 0.0,
                    output: 0.0,
                },
                capabilities: Capabilities {
                    reasoning,
                    streaming: true,
                    vision,
                    tool_use: true,
                    thinking_levels,
                },
                context_window: DEFAULT_DISCOVERED_CONTEXT_WINDOW,
                max_output: Some(DEFAULT_DISCOVERED_MAX_OUTPUT),
                request_multiplier: None,
                thinking_config: None,
            }
        })
        .collect())
}

/// Heuristic filter for OpenAI-compatible `/v1/models` responses.
///
/// Skips models that are clearly not chat-completion endpoints, such as
/// embeddings, audio/image generation, moderation, and legacy engines.
fn is_openai_chat_model_id(model_id: &str) -> bool {
    let lower = model_id.to_ascii_lowercase();
    let non_chat_prefixes = [
        "text-embedding",
        "embedding",
        "whisper",
        "tts",
        "dall-e",
        "dall",
        "audio",
        "babbage",
        "davinci",
        "curie",
        "ada",
        "moderation",
        "omni-moderation",
        "gpt-3.5-turbo-instruct",
    ];
    if non_chat_prefixes.iter().any(|p| lower.starts_with(p)) {
        return false;
    }
    let non_chat_keywords = [
        "-embedding-",
        "embed",
        "transcribe",
        "translate",
        "speech",
        "image",
        "instruct",
    ];
    !non_chat_keywords.iter().any(|k| lower.contains(k))
}
