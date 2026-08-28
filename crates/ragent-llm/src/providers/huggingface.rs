//! HuggingFace Inference API provider implementation.
//!
//! Implements the [`Provider`] trait for the HuggingFace Inference API, which
//! exposes an OpenAI-compatible `/v1/chat/completions` endpoint. Supports both
//! the free/Pro shared Inference API and dedicated Inference Endpoints.
//!
//! **Provider ID:** `huggingface`
//! **Default base URL:** `https://router.huggingface.co`
//! **Auth:** `Authorization: Bearer <HF_TOKEN>`

use anyhow::{Context, Result, bail};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::pin::Pin;

use super::thinking::should_warn_unsupported_thinking;
use super::tool_cache::{ToolFormat, cached_tools};
use crate::llm::{ChatContent, ChatRequest, ContentPart, LlmClient, StreamEvent};
use crate::{ModelInfo, Provider};
use ragent_config::{Capabilities, Cost};
use ragent_types::event::FinishReason;

/// Default API base URL for the HuggingFace Inference API.
/// As of 2025, HuggingFace migrated from `api-inference.huggingface.co` to
/// `router.huggingface.co` for all inference endpoints.
pub const HF_API_BASE: &str = "https://router.huggingface.co";

/// Maximum number of models to return from dynamic discovery.
const MAX_DISCOVERED_MODELS: usize = 50;

/// Provider implementation for the HuggingFace Inference API.
pub struct HuggingFaceProvider;

#[async_trait::async_trait]
impl Provider for HuggingFaceProvider {
    /// Returns `"huggingface"`.
    fn id(&self) -> &'static str {
        "huggingface"
    }

    /// Returns `"Hugging Face"`.
    fn name(&self) -> &'static str {
        "Hugging Face"
    }

    fn as_any_static(&self) -> &dyn std::any::Any {
        self
    }

    /// Returns an empty catalog.
    ///
    /// HuggingFace models are discovered at runtime from the public router;
    /// no models are hard-coded.
    fn default_models(&self) -> Vec<ModelInfo> {
        Vec::new()
    }

    /// Discover available chat-completions models from HuggingFace router.
    ///
    /// The `/v1/models` endpoint on HuggingFace's router is public and does not
    /// require authentication for listing models, so this method attempts
    /// discovery even when no API token is configured.  A token is still required
    /// to actually perform chat completions via [`create_client`].
    async fn discover_models(&self) -> Result<Vec<ModelInfo>> {
        let api_key = std::env::var("HF_TOKEN")
            .or_else(|_| std::env::var("HUGGINGFACE_API_KEY"))
            .or_else(|_| std::env::var("HUGGING_FACE_HUB_TOKEN"))
            .ok()
            .filter(|k| !k.is_empty())
            .unwrap_or_default();
        let models = discover_models(&api_key)
            .await
            .with_context(|| "HuggingFace model discovery failed")?;
        Ok(models)
    }

    /// Creates an authenticated [`HuggingFaceClient`] for chat completions.
    ///
    /// # Arguments
    ///
    /// * `api_key` - HuggingFace API token (HF_TOKEN).
    /// * `base_url` - Optional override for Inference Endpoints.
    /// * `options` - Provider-specific options (`wait_for_model`, `use_cache`).
    ///
    /// # Errors
    ///
    /// Returns an error if the API key is empty.
    async fn create_client(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        if api_key.is_empty() {
            bail!("HuggingFace requires an API token. Set HF_TOKEN or configure it in ragent.");
        }

        let wait_for_model = options
            .get("wait_for_model")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let use_cache = options
            .get("use_cache")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let url = base_url
            .unwrap_or(HF_API_BASE)
            .trim_end_matches('/')
            .to_string();
        let client = HuggingFaceClient {
            api_key: api_key.to_string(),
            base_url: url.clone(),
            http: crate::provider::http_client::create_streaming_http_client(),
            wait_for_model,
            use_cache,
        };
        tracing::info!(chat_endpoint = %format!("{}/v1/chat/completions", url), models_endpoint = %format!("{}/v1/models", url), "HuggingFace provider connected");
        Ok(Box::new(client))
    }
}

/// Returns the curated default model catalog for HuggingFace.
#[must_use]
pub fn huggingface_default_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "meta-llama/Llama-3.1-8B-Instruct".to_string(),
            provider_id: "huggingface".to_string(),
            name: "Llama 3.1 8B".to_string(),
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
            max_output: Some(4_096),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "meta-llama/Llama-3.1-70B-Instruct".to_string(),
            provider_id: "huggingface".to_string(),
            name: "Llama 3.1 70B".to_string(),
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
            max_output: Some(4_096),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "Qwen/Qwen2.5-Coder-32B-Instruct".to_string(),
            provider_id: "huggingface".to_string(),
            name: "Qwen 2.5 Coder 32B".to_string(),
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
            context_window: 32_000,
            max_output: Some(4_096),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "Qwen/Qwen2.5-72B-Instruct".to_string(),
            provider_id: "huggingface".to_string(),
            name: "Qwen 2.5 72B".to_string(),
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
            max_output: Some(4_096),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "deepseek-ai/DeepSeek-R1".to_string(),
            provider_id: "huggingface".to_string(),
            name: "DeepSeek R1".to_string(),
            cost: Cost {
                input: 0.0,
                output: 0.0,
            },
            capabilities: Capabilities {
                reasoning: true,
                streaming: true,
                vision: false,
                tool_use: false,
                thinking_levels: Vec::new(),
            },
            context_window: 128_000,
            max_output: Some(8_192),
            request_multiplier: None,
            thinking_config: None,
        },
    ]
}

/// HTTP client for the HuggingFace Inference API with streaming SSE support.
///
/// Uses the OpenAI-compatible `/v1/chat/completions` endpoint, which is
/// supported by both the shared Inference API and dedicated Inference Endpoints.
pub(crate) struct HuggingFaceClient {
    api_key: String,
    base_url: String,
    http: reqwest::Client,
    wait_for_model: bool,
    use_cache: bool,
}

/// Prefix applied to all tool names sent to the HuggingFace router.
///
/// The HuggingFace inference router rejects tool names containing common
/// substrings (`read`, `write`, `search`, `list`, `open`, `memo`, `pdf`,
/// `todo`, etc.) in streaming mode. We prefix every tool name with `t_` so
/// the model sees e.g. `t_search` / `t_write_file`, and strip the prefix
/// when mapping tool-call responses back to ragent's internal names.
const HF_TOOL_PREFIX: &str = "t_";

impl HuggingFaceClient {
    /// Returns the prefixed (safe) tool name for the HuggingFace router.
    fn safe_tool_name(name: &str) -> String {
        format!("{HF_TOOL_PREFIX}{name}")
    }

    /// Strips the `t_` prefix from a tool name returned by the model,
    /// recovering the original ragent tool name.
    fn strip_tool_prefix(name: &str) -> String {
        name.strip_prefix(HF_TOOL_PREFIX)
            .unwrap_or(name)
            .to_string()
    }

    /// Rewrites tool names inside the system prompt so the model sees the
    /// same prefixed names that appear in the `tools` array.
    ///
    /// Performs a simple find-and-replace for each tool name, replacing
    /// occurrences of `name` with `t_name` where they haven't already been
    /// prefixed.
    fn rewrite_system_prompt(system: &str, tools: &[crate::llm::ToolDefinition]) -> String {
        let mut result = system.to_string();
        // Sort by descending length so longer names are replaced first,
        // preventing partial matches (e.g. `write_file` before `write`).
        let mut names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        names.sort_by_key(|b| std::cmp::Reverse(b.len()));
        for name in names {
            let safe = Self::safe_tool_name(name);
            // Only replace bare occurrences — skip if already prefixed
            result = result.replace(name, &safe);
        }
        result
    }

    /// Builds the JSON request body in OpenAI-compatible format.
    ///
    /// All tool names are prefixed with [`HF_TOOL_PREFIX`] to avoid the
    /// HuggingFace router's reserved-substring restrictions. Tool names in
    /// the system prompt and conversation history are rewritten to match.
    fn build_request_body(&self, request: &ChatRequest) -> Value {
        let mut messages = Vec::new();

        if let Some(system) = &request.system {
            let rewritten = if !request.tools.is_empty() {
                Self::rewrite_system_prompt(system, &request.tools)
            } else {
                (**system).to_string()
            };
            messages.push(json!({
                "role": "system",
                "content": rewritten
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
                                        "name": Self::safe_tool_name(name),
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
            // H2: reuse the cached serialised tool list (with the `t_` name
            // prefix) instead of building a fresh `Vec<Value>` on every call.
            let cached = cached_tools(ToolFormat::HuggingFace, &request.tools);
            body["tools"] = cached.openai_tools_array();
        }

        if should_warn_unsupported_thinking(request) {
            tracing::warn!(
                model = %request.model,
                "HuggingFace provider ignores thinking config because the API has no standard thinking parameter"
            );
        }

        body
    }
}

#[async_trait::async_trait]
impl LlmClient for HuggingFaceClient {
    /// Sends a streaming chat completion request to the HuggingFace Inference API.
    ///
    /// Uses the OpenAI-compatible `/v1/chat/completions` endpoint with SSE streaming.
    /// Handles HuggingFace-specific errors such as model loading (503) and gated
    /// model access (403).
    ///
    /// # Errors
    ///
    /// Returns an error on network failures, authentication errors, model loading
    /// timeouts, or gated model access denials.
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let body = self.build_request_body(&request);
        let body_bytes = serde_json::to_vec(&body).context("serialise HuggingFace request body")?;

        let mut req = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json");

        // HuggingFace-specific headers
        if self.wait_for_model {
            req = req.header("x-wait-for-model", "true");
        }
        if !self.use_cache {
            req = req.header("x-use-cache", "false");
        }

        let response = req
            .body(body_bytes)
            .send()
            .await
            .inspect_err(|e| {
                tracing::warn!(url = %url, error = %e, "HuggingFace chat request failed");
            })
            .with_context(|| format!("Failed to send request to HuggingFace API at {url}"))?;
        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to read HuggingFace error response body");
                String::new()
            });

            // Parse HF-specific error responses
            if status.as_u16() == 503
                && let Ok(err_json) = serde_json::from_str::<HfErrorResponse>(&body_text)
            {
                let wait_msg = err_json
                    .estimated_time
                    .map(|t| format!(" (estimated wait: {t:.0}s)"))
                    .unwrap_or_default();
                bail!(
                    "HuggingFace: model is currently loading{wait_msg}. \
                         The model needs to be loaded into memory before it can serve requests. \
                         Please try again in a moment."
                );
            }

            if status.as_u16() == 403 {
                bail!(
                    "HuggingFace: access denied for model '{}'. \
                     This model may be gated — visit the model page on huggingface.co \
                     to accept the license agreement. Error: {body_text}",
                    request.model
                );
            }

            if status.as_u16() == 401 {
                bail!(
                    "HuggingFace: invalid or expired API token. \
                     Please check your HF_TOKEN. Error: {body_text}"
                );
            }

            tracing::warn!(
                url = %url,
                model = %request.model,
                status = %status,
                error = %body_text,
                "HuggingFace API error"
            );
            bail!("HuggingFace API error ({status}): {body_text}");
        }

        let rate_limit_event = parse_hf_rate_limit_headers(response.headers());
        let stream = response.bytes_stream();

        let event_stream = async_stream::stream! {
            let mut buffer = String::new();
            let mut tool_call_ids: HashMap<u64, String> = HashMap::new();

            if let Some(ev) = rate_limit_event {
                yield ev;
            }

            futures::pin_mut!(stream);

            loop {
                let chunk = match tokio::time::timeout(
                    std::time::Duration::from_secs(super::http_client::STREAM_CHUNK_IDLE_TIMEOUT_SECS),
                    stream.next(),
                )
                .await
                {
                    Ok(Some(r)) => match r {
                        Ok(c) => c,
                        Err(e) => {
                            yield StreamEvent::Error { message: e.to_string() };
                            break;
                        }
                    },
                    Ok(None) => break,
                    Err(_) => {
                        yield StreamEvent::Error {
                            message: format!(
                                "HuggingFace: stream stalled — no data received for {}s",
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
                        yield StreamEvent::Finish { reason: FinishReason::Stop };
                        return;
                    }

                    let parsed: Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    // Handle usage info
                    if let Some(usage) = parsed.get("usage")
                        && !usage.is_null()
                    {
                        let input_tokens = usage["prompt_tokens"].as_u64().unwrap_or(0);
                        let output_tokens = usage["completion_tokens"].as_u64().unwrap_or(0);
                        if input_tokens > 0 || output_tokens > 0 {
                            yield StreamEvent::Usage { input_tokens, output_tokens };
                        }
                    }

                    let choices = match parsed["choices"].as_array() {
                        Some(c) => c,
                        None => continue,
                    };

                    for choice in choices {
                        let delta = &choice["delta"];

                        // Text content
                        if let Some(content) = delta["content"].as_str()
                            && !content.is_empty()
                        {
                            yield StreamEvent::TextDelta { text: content.to_string() };
                        }

                        // Tool calls
                        if let Some(tool_calls) = delta["tool_calls"].as_array() {
                            for tc in tool_calls {
                                let index = tc["index"].as_u64().unwrap_or(0);

                                if let Some(id) = tc["id"].as_str() {
                                    tool_call_ids.insert(index, id.to_string());
                                }

                                if let Some(function) = tc.get("function") {
                                    if let Some(name) = function["name"].as_str() {
                                        let tc_id = tool_call_ids.get(&index)
                                            .cloned()
                                            .unwrap_or_else(|| format!("tc_{index}"));
                                        let original_name =
                                            Self::strip_tool_prefix(name);
                                        yield StreamEvent::ToolCallStart {
                                            id: tc_id,
                                            name: original_name,
                                        };
                                    }

                                    if let Some(args) = function["arguments"].as_str()
                                        && !args.is_empty()
                                    {
                                        let tc_id = tool_call_ids.get(&index)
                                            .cloned()
                                            .unwrap_or_else(|| format!("tc_{index}"));
                                        yield StreamEvent::ToolCallDelta {
                                            id: tc_id,
                                            args_json: args.to_string(),
                                        };
                                    }
                                }
                            }
                        }

                        // Finish reason
                        if let Some(finish_reason) = choice["finish_reason"].as_str() {
                            // End pending tool calls in index order (see the
                            // openai provider for the rationale).
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
                        }
                    }
                }
            }
        };

        Ok(Box::pin(event_stream))
    }
}

/// Parses HuggingFace rate-limit response headers into a `StreamEvent::RateLimit`.
///
/// HuggingFace uses the standard `X-RateLimit-Limit`, `X-RateLimit-Remaining`,
/// and `X-RateLimit-Reset` headers.
fn parse_hf_rate_limit_headers(headers: &reqwest::header::HeaderMap) -> Option<StreamEvent> {
    let header_u64 = |name: &str| -> Option<u64> {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
    };

    let req_limit = header_u64("x-ratelimit-limit");
    let req_remaining = header_u64("x-ratelimit-remaining");

    let requests_used_pct = req_limit.zip(req_remaining).map(|(limit, remaining)| {
        if limit == 0 {
            0.0f32
        } else {
            ((limit.saturating_sub(remaining)) as f32 / limit as f32 * 100.0).clamp(0.0, 100.0)
        }
    });

    if requests_used_pct.is_some() {
        Some(StreamEvent::RateLimit {
            requests_used_pct,
            tokens_used_pct: None,
        })
    } else {
        None
    }
}

/// HuggingFace error response structure for model loading and other errors.
#[derive(Debug, Deserialize)]
struct HfErrorResponse {
    /// Human-readable HuggingFace error message.
    ///
    /// Currently unused; the provider relies on HTTP status codes and
    /// `estimated_time` for polling logic. Kept as a documented field so future
    /// diagnostics can surface the provider message without schema changes.
    #[serde(default)]
    #[allow(dead_code)]
    error: String,
    #[serde(default)]
    estimated_time: Option<f64>,
}

/// Discovers available chat-completions models from the authenticated
/// HuggingFace router API.
///
/// Queries the OpenAI-compatible `GET /v1/models` endpoint exposed by
/// `router.huggingface.co` and keeps only models with at least one live
/// provider. This endpoint is queried with the caller's HuggingFace token so
/// the results stay aligned with the models the authenticated account can route
/// requests to.
///
/// # Arguments
///
/// * `api_key` - HuggingFace API token for authenticated requests.
///
/// # Errors
///
/// Returns an error on network failures or invalid API responses.
pub async fn discover_models(api_key: &str) -> Result<Vec<ModelInfo>> {
    let client = crate::provider::http_client::create_http_client();
    let url = format!("{HF_API_BASE}/v1/models");

    let mut request = client.get(&url).timeout(std::time::Duration::from_secs(15));
    if !api_key.is_empty() {
        request = request.header("Authorization", format!("Bearer {api_key}"));
    }

    let response = request
        .send()
        .await
        .inspect_err(|e| {
            tracing::warn!(url = %url, error = %e, "HuggingFace model discovery failed");
        })
        .with_context(|| format!("Failed to connect to HuggingFace router models API at {url}"))?;
    if !response.status().is_success() {
        bail!(
            "HuggingFace router models API returned status {} when discovering models",
            response.status()
        );
    }

    let response_body: HfRouterModelsResponse = response
        .json()
        .await
        .context("Failed to parse HuggingFace router model list")?;

    let result: Vec<ModelInfo> = response_body
        .data
        .into_iter()
        .filter_map(router_model_to_info)
        .take(MAX_DISCOVERED_MODELS)
        .collect();

    Ok(result)
}

fn router_model_to_info(model: HfRouterModelEntry) -> Option<ModelInfo> {
    let live_providers: Vec<HfRouterProviderEntry> = model
        .providers
        .into_iter()
        .filter(|provider| provider.status == "live")
        .collect();

    if live_providers.is_empty() {
        return None;
    }

    let has_text_io = model
        .architecture
        .as_ref()
        .map(|arch| {
            arch.output_modalities
                .iter()
                .any(|modality| modality == "text")
                && arch
                    .input_modalities
                    .iter()
                    .any(|modality| modality == "text")
        })
        .unwrap_or(true);
    if !has_text_io {
        return None;
    }

    let context_window = live_providers
        .iter()
        .filter_map(|provider| provider.context_length)
        .max()
        .unwrap_or_else(|| estimate_context_from_id(&model.model_id));
    let max_output = live_providers
        .iter()
        .filter_map(|provider| provider.max_output)
        .max()
        .or(Some(4_096));
    let input_cost = live_providers
        .iter()
        .filter_map(|provider| provider.pricing.as_ref().map(|pricing| pricing.input))
        .reduce(f64::min)
        .unwrap_or(0.0);
    let output_cost = live_providers
        .iter()
        .filter_map(|provider| provider.pricing.as_ref().map(|pricing| pricing.output))
        .reduce(f64::min)
        .unwrap_or(0.0);
    let vision = model
        .architecture
        .as_ref()
        .map(|arch| {
            arch.input_modalities
                .iter()
                .any(|modality| modality == "image")
        })
        .unwrap_or(false);
    let tool_use = live_providers
        .iter()
        .any(|provider| provider.supports_tools);

    Some(ModelInfo {
        id: model.model_id.clone(),
        provider_id: "huggingface".to_string(),
        name: format_model_display_name(&model.model_id),
        cost: Cost {
            input: input_cost,
            output: output_cost,
        },
        capabilities: Capabilities {
            reasoning: false,
            streaming: true,
            vision,
            tool_use,
            thinking_levels: Vec::new(),
        },
        context_window,
        max_output,
        request_multiplier: None,
        thinking_config: None,
    })
}

/// Response body for the HuggingFace router `GET /v1/models` endpoint.
#[derive(Debug, Deserialize)]
struct HfRouterModelsResponse {
    data: Vec<HfRouterModelEntry>,
}

/// Router model entry from the OpenAI-compatible `GET /v1/models` endpoint.
#[derive(Debug, Deserialize)]
struct HfRouterModelEntry {
    #[serde(rename = "id")]
    model_id: String,
    #[serde(default)]
    architecture: Option<HfRouterArchitecture>,
    #[serde(default)]
    providers: Vec<HfRouterProviderEntry>,
}

#[derive(Debug, Deserialize)]
struct HfRouterArchitecture {
    #[serde(default)]
    input_modalities: Vec<String>,
    #[serde(default)]
    output_modalities: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct HfRouterProviderEntry {
    status: String,
    #[serde(default)]
    context_length: Option<usize>,
    #[serde(default)]
    max_output: Option<usize>,
    #[serde(default)]
    pricing: Option<HfRouterPricing>,
    #[serde(default)]
    supports_tools: bool,
}

#[derive(Debug, Deserialize)]
struct HfRouterPricing {
    input: f64,
    output: f64,
}

/// Formats a HuggingFace model ID into a human-readable display name.
///
/// Strips the org prefix and converts hyphens/underscores to spaces.
/// Examples: `"meta-llama/Llama-3.1-70B-Instruct"` → `"Llama 3.1 70B Instruct"`
fn format_model_display_name(model_id: &str) -> String {
    let (repo_id, provider_suffix) =
        model_id
            .rsplit_once(':')
            .map_or((model_id, None), |(repo_id, provider)| {
                if repo_id.contains('/') {
                    (repo_id, Some(provider))
                } else {
                    (model_id, None)
                }
            });
    let name = repo_id
        .rsplit_once('/')
        .map(|(_, name)| name)
        .unwrap_or(repo_id)
        .replace(['-', '_'], " ");

    match provider_suffix {
        Some(provider) => format!("{name} ({provider})"),
        None => name,
    }
}

/// Estimates context window size from the model ID.
///
/// Looks for common size indicators in the model name (e.g., `4k`, `128k`).
/// Falls back to parameter-size heuristics.
fn estimate_context_from_id(model_id: &str) -> usize {
    let lower = model_id
        .rsplit_once(':')
        .map_or(model_id, |(repo_id, provider)| {
            if repo_id.contains('/') {
                let _ = provider;
                repo_id
            } else {
                model_id
            }
        })
        .to_lowercase();

    // Explicit context markers
    if lower.contains("128k") || lower.contains("128000") {
        return 128_000;
    }
    if lower.contains("32k") || lower.contains("32000") {
        return 32_000;
    }
    if lower.contains("8k") || lower.contains("8000") {
        return 8_192;
    }
    if lower.contains("4k") || lower.contains("4000") {
        return 4_096;
    }

    // Parameter-size heuristics
    if lower.contains("70b") || lower.contains("72b") || lower.contains("65b") {
        return 128_000;
    }
    if lower.contains("34b") || lower.contains("30b") || lower.contains("8x7b") {
        return 32_000;
    }

    // Default for most modern models
    32_000
}

#[cfg(test)]
#[path = "../../tests/inline/huggingface.rs"]
mod tests_tests;
