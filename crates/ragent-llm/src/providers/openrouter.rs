//! OpenRouter provider implementation.
//!
//! Connects to OpenRouter at `https://openrouter.ai` via its OpenAI-compatible
//! API, using bearer-token authentication against `OPENROUTER_API_KEY`. The
//! provider resolves keys in FR-004 precedence: per-call argument, then the
//! encrypted credential stored under provider id `openrouter`, then the
//! environment variable. Model discovery and chat streaming are filled in by
//! later spec tasks.

use anyhow::{Context, Result, bail};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::RwLock;

use crate::llm::{ChatContent, ChatRequest, ContentPart, LlmClient, StreamEvent};
use crate::provider::http_client;
use crate::provider::thinking::{full_reasoning_levels, openrouter_reasoning_payload_from_request};
use crate::{ModelInfo, Provider};
use ragent_config::{Capabilities, Cost};
use ragent_types::event::FinishReason;

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
    let chars: Vec<char> = key.chars().collect();
    let start = chars.len().saturating_sub(4);
    let tail: String = chars[start..].iter().collect();
    format!("...{tail}")
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
        std::env::var("OPENROUTER_API_KEY").unwrap_or_default()
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
            resolved = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
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
/// FR-025: the chat POST path must use a single `.send()` per request — no
/// `execute_with_retry` wrapper — because automatically retrying a chat POST
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
    /// Build the JSON request body for the OpenRouter `/api/v1/chat/completions`
    /// endpoint.
    ///
    /// OpenRouter accepts an OpenAI-compatible payload, so the body follows the
    /// same shape as [`super::openai::OpenAiClient::build_request_body`]:
    /// system-first messages, `stream: true`, optional `temperature`/`top_p`/
    /// `max_tokens`, cached OpenAI-format tools, and `stream_options` with
    /// `include_usage` so the final chunk carries token counts.
    ///
    /// # Errors
    ///
    /// This function is infallible.
    #[must_use]
    pub fn build_request_body(&self, request: &ChatRequest) -> Value {
        let mut messages = Vec::new();

        if let Some(system) = &request.system {
            messages.push(json!({
                "role": "system",
                "content": &**system
            }));
        }

        for msg in request.messages.iter() {
            let content = match &msg.content {
                ChatContent::Text(text) => json!(text),
                ChatContent::Parts(parts) => {
                    let content_parts: Vec<Value> = parts
                        .iter()
                        .filter_map(|part| match part {
                            ContentPart::Text { text } => Some(json!({
                                "type": "text",
                                "text": text
                            })),
                            ContentPart::ImageUrl { url } => Some(json!({
                                "type": "image_url",
                                "image_url": { "url": url }
                            })),
                            ContentPart::ToolResult { .. } | ContentPart::ToolUse { .. } => None,
                        })
                        .collect();
                    if content_parts.len() == 1 {
                        content_parts[0]["text"].clone()
                    } else {
                        json!(content_parts)
                    }
                }
            };

            match &msg.content {
                ChatContent::Parts(parts) => {
                    let tool_results: Vec<&ContentPart> = parts
                        .iter()
                        .filter(|p| matches!(p, ContentPart::ToolResult { .. }))
                        .collect();
                    let tool_uses: Vec<&ContentPart> = parts
                        .iter()
                        .filter(|p| matches!(p, ContentPart::ToolUse { .. }))
                        .collect();

                    if !tool_uses.is_empty() {
                        let tool_calls: Vec<Value> = tool_uses
                            .iter()
                            .filter_map(|p| match p {
                                ContentPart::ToolUse { id, name, input } => Some(json!({
                                    "id": id,
                                    "type": "function",
                                    "function": {
                                        "name": name,
                                        "arguments": input.to_string()
                                    }
                                })),
                                _ => None,
                            })
                            .collect();
                        messages.push(json!({
                            "role": "assistant",
                            "tool_calls": tool_calls
                        }));
                    } else if !tool_results.is_empty() {
                        for result in tool_results {
                            if let ContentPart::ToolResult {
                                tool_use_id,
                                content,
                            } = result
                            {
                                messages.push(json!({
                                    "role": "tool",
                                    "tool_call_id": tool_use_id,
                                    "content": content
                                }));
                            }
                        }
                    } else {
                        messages.push(json!({
                            "role": msg.role,
                            "content": content
                        }));
                    }
                }
                _ => {
                    messages.push(json!({
                        "role": msg.role,
                        "content": content
                    }));
                }
            }
        }

        let mut body = json!({
            "model": request.model,
            "messages": messages,
            "stream": true,
            "stream_options": { "include_usage": true }
        });

        if let Some(temp) = request.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = json!(top_p);
        }
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        if !request.tools.is_empty() {
            // H2: reuse the cached serialised OpenAI-format tool list.
            let cached = super::tool_cache::cached_tools(
                super::tool_cache::ToolFormat::OpenAi,
                &request.tools,
            );
            body["tools"] = cached.openai_tools_array();
        }
        if let Some(reasoning) = openrouter_reasoning_payload_from_request(request) {
            body["reasoning"] = reasoning;
        }

        body
    }

    /// Parses an OpenAI-compatible SSE stream into [`StreamEvent`]s.
    ///
    /// Handles `data: {...}` lines, `[DONE]`, `choices[0].delta` text,
    /// `reasoning`/`reasoning_content` deltas (FR-020), incremental
    /// `tool_calls`, final-chunk `usage`, and `finish_reason` mapping.
    fn parse_sse_stream(
        &self,
        response: reqwest::Response,
    ) -> Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>> {
        let status = response.status();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();
        let rate_limit_event = super::openai::parse_openai_rate_limit_headers(response.headers());
        let stream = response.bytes_stream();
        let base_url = self.base_url.clone();

        let event_stream = async_stream::stream! {
            let mut buffer = String::new();
            let mut tool_call_ids: HashMap<u64, String> = HashMap::new();
            let mut in_reasoning_block = false;
            let mut yielded_event = false;

            if let Some(ev) = rate_limit_event {
                yield ev;
                yielded_event = true;
            }

            futures::pin_mut!(stream);

            loop {
                let chunk = match tokio::time::timeout(
                    std::time::Duration::from_secs(
                        super::http_client::STREAM_CHUNK_IDLE_TIMEOUT_SECS,
                    ),
                    stream.next(),
                )
                .await
                {
                    Ok(Some(r)) => match r {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::warn!(
                                provider = "OpenRouter",
                                status = %status,
                                content_type = %content_type,
                                yielded_events = yielded_event,
                                error = %e,
                                "SSE stream decode error"
                            );
                            let err_text = e.to_string();
                            let is_decode_failure =
                                err_text.to_lowercase().contains("error decoding response body");
                            let message = if is_decode_failure && !yielded_event && status.is_success() {
                                format!(
                                    "OpenRouter returned an empty/malformed event stream \
                                     (status {}, content-type {}).",
                                    status, content_type
                                )
                            } else {
                                err_text
                            };
                            yield StreamEvent::Error { message };
                            break;
                        }
                    },
                    Ok(None) => break,
                    Err(_) => {
                        yield StreamEvent::Error {
                            message: format!(
                                "OpenRouter: stream stalled — no data received for {}s",
                                super::http_client::STREAM_CHUNK_IDLE_TIMEOUT_SECS
                            ),
                        };
                        break;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(line) = super::http_client::take_sse_line(&mut buffer) {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    let data = match line.strip_prefix("data: ") {
                        Some(d) => d.trim(),
                        None => continue,
                    };

                    if data == "[DONE]" {
                        if in_reasoning_block {
                            yield StreamEvent::ReasoningEnd;
                        }
                        yield StreamEvent::Finish { reason: FinishReason::Stop };
                        return;
                    }

                    let parsed: Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::warn!(
                                provider = "OpenRouter",
                                line = %data,
                                error = %e,
                                "OpenRouter: failed to parse SSE line; skipping"
                            );
                            continue;
                        }
                    };

                    // Final-chunk usage (only present when stream_options.include_usage).
                    if let Some(usage) = parsed.get("usage")
                        && !usage.is_null()
                    {
                        let input_tokens = usage["prompt_tokens"].as_u64().unwrap_or(0);
                        let output_tokens = usage["completion_tokens"].as_u64().unwrap_or(0);
                        if input_tokens > 0 || output_tokens > 0 {
                            yield StreamEvent::Usage { input_tokens, output_tokens };
                            yielded_event = true;
                        }
                    }

                    let choices = match parsed["choices"].as_array() {
                        Some(c) => c,
                        None => continue,
                    };

                    for choice in choices {
                        let delta = &choice["delta"];

                        // OpenRouter exposes reasoning text under either
                        // `delta.reasoning` or `delta.reasoning_content`
                        // depending on the upstream model.
                        let reasoning_text = delta
                            .get("reasoning")
                            .or_else(|| delta.get("reasoning_content"))
                            .and_then(|v| v.as_str());

                        if let Some(text) = reasoning_text {
                            if !text.is_empty() {
                                if !in_reasoning_block {
                                    yield StreamEvent::ReasoningStart;
                                    in_reasoning_block = true;
                                }
                                yield StreamEvent::ReasoningDelta {
                                    text: text.to_string(),
                                };
                                yielded_event = true;
                            }
                        }

                        // Normal text content. Close any open reasoning block
                        // first so consumers see contiguous content phases.
                        if let Some(content) = delta["content"].as_str() {
                            if in_reasoning_block && !content.is_empty() {
                                yield StreamEvent::ReasoningEnd;
                                in_reasoning_block = false;
                            }
                            if !content.is_empty() {
                                yield StreamEvent::TextDelta {
                                    text: content.to_string(),
                                };
                                yielded_event = true;
                            }
                        }

                        // Tool calls (incremental fragments indexed by `index`).
                        if let Some(tool_calls) = delta["tool_calls"].as_array() {
                            for tc in tool_calls {
                                let index = tc["index"].as_u64().unwrap_or(0);                                  if let Some(id) = tc["id"].as_str() {
                                      tool_call_ids.insert(index, id.to_string());
                                  }

                                  if let Some(function) = tc.get("function") {
                                      if let Some(name) = function["name"].as_str() {
                                          let tc_id = tool_call_ids
                                              .get(&index)
                                              .cloned()
                                              .unwrap_or_else(|| format!("tc_{index}"));
                                          yield StreamEvent::ToolCallStart {
                                              id: tc_id,
                                              name: name.to_string(),
                                          };
                                          yielded_event = true;
                                      }

                                      if let Some(args) = function["arguments"].as_str()
                                          && !args.is_empty()
                                      {
                                          let tc_id = tool_call_ids
                                              .get(&index)
                                              .cloned()
                                              .unwrap_or_else(|| format!("tc_{index}"));
                                          yield StreamEvent::ToolCallDelta {
                                              id: tc_id,
                                              args_json: args.to_string(),
                                          };
                                          yielded_event = true;
                                      }
                                  }
                              }
                          }

                          // Finish reason: flush open reasoning/tool-call state,
                          // then emit the terminal event.
                          if let Some(finish_reason) = choice["finish_reason"].as_str() {
                              if in_reasoning_block {
                                  yield StreamEvent::ReasoningEnd;
                              }

                              let mut ends: Vec<(u64, String)> = tool_call_ids.drain().collect();
                              ends.sort_unstable_by_key(|(idx, _)| *idx);
                              for (_, id) in ends {
                                  yield StreamEvent::ToolCallEnd { id };
                              }

                              let reason = match finish_reason {
                                  "tool_calls" => FinishReason::ToolUse,
                                  "length" => FinishReason::Length,
                                  "content_filter" => FinishReason::ContentFilter,
                                  _ => FinishReason::Stop,
                              };
                              yield StreamEvent::Finish { reason };
                              yielded_event = true;
                          }
                    }
                }
            }

            if in_reasoning_block {
                yield StreamEvent::ReasoningEnd;
            }

            if !yielded_event {
                tracing::warn!(
                    provider = "OpenRouter",
                    status = %status,
                    content_type = %content_type,
                    base_url = %base_url,
                    "OpenRouter response stream ended without producing any events"
                );
                let message = format!(
                    "OpenRouter response stream ended without producing any events \
                     (status {}, content-type {}).",
                    status, content_type
                );
                yield StreamEvent::Error { message };
            }
        };
        Box::pin(event_stream)
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

        // FR-016: first-byte timeout defaults to 600 s (mirrors Ollama Cloud).
        let first_byte_timeout = request.stream_timeout_secs.unwrap_or(600);
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
            tracing::warn!(url = %url, error = %e, "OpenRouter chat request timed out");
        })
        .map_err(|_| {
            anyhow::anyhow!("OpenRouter: initial response timed out after {first_byte_timeout}s")
        })?
        .inspect_err(|e| {
            tracing::warn!(url = %url, error = %e, "OpenRouter chat request failed");
        })
        .with_context(|| format!("Failed to connect to OpenRouter at {url}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            const MAX_ERR_LEN: usize = 4096;
            let error_body = if error_body.len() > MAX_ERR_LEN {
                format!(
                    "{}...[truncated {} bytes]",
                    &error_body[..MAX_ERR_LEN],
                    error_body.len() - MAX_ERR_LEN
                )
            } else {
                error_body
            };
            tracing::warn!(
                url = %url,
                model = %request.model,
                status = %status,
                error = %error_body,
                "OpenRouter API error"
            );
            bail!("OpenRouter API error ({status}): {error_body}");
        }

        Ok(self.parse_sse_stream(response))
    }
}
