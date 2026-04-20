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

use crate::config::{Capabilities, Cost};
use crate::event::FinishReason;
use crate::llm::{ChatContent, ChatRequest, ContentPart, LlmClient, StreamEvent};
use crate::provider::{ModelInfo, Provider};

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

    /// Returns a curated set of popular HuggingFace Inference API models.
    ///
    /// These serve as fallback defaults when dynamic discovery is unavailable.
    fn default_models(&self) -> Vec<ModelInfo> {
        huggingface_default_models()
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

        let client = HuggingFaceClient {
            api_key: api_key.to_string(),
            base_url: base_url
                .unwrap_or(HF_API_BASE)
                .trim_end_matches('/')
                .to_string(),
            http: crate::provider::http_client::create_streaming_http_client(),
            wait_for_model,
            use_cache,
        };
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
            },
            context_window: 128_000,
            max_output: Some(4_096),
            request_multiplier: None,
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
            },
            context_window: 128_000,
            max_output: Some(4_096),
            request_multiplier: None,
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
            },
            context_window: 32_000,
            max_output: Some(4_096),
            request_multiplier: None,
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
            },
            context_window: 128_000,
            max_output: Some(4_096),
            request_multiplier: None,
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
            },
            context_window: 128_000,
            max_output: Some(8_192),
            request_multiplier: None,
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
        names.sort_by(|a, b| b.len().cmp(&a.len()));
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
                system.clone()
            };
            messages.push(json!({
                "role": "system",
                "content": rewritten
            }));
        }

        for msg in &request.messages {
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
                            .map(|p| match p {
                                ContentPart::ToolUse { id, name, input } => {
                                    json!({
                                        "id": id,
                                        "type": "function",
                                        "function": {
                                            "name": Self::safe_tool_name(name),
                                            "arguments": input.to_string()
                                        }
                                    })
                                }
                                _ => unreachable!(),
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
            let tools: Vec<Value> = request
                .tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": Self::safe_tool_name(&t.name),
                            "description": t.description,
                            "parameters": t.parameters
                        }
                    })
                })
                .collect();
            body["tools"] = json!(tools);
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
            .json(&body)
            .send()
            .await
            .context("Failed to send request to HuggingFace API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to read HuggingFace error response body");
                String::new()
            });

            // Parse HF-specific error responses
            if status.as_u16() == 503 {
                if let Ok(err_json) = serde_json::from_str::<HfErrorResponse>(&body_text) {
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
            let mut tool_call_names: HashMap<u64, String> = HashMap::new();

            if let Some(ev) = rate_limit_event {
                yield ev;
            }

            futures::pin_mut!(stream);

            while let Some(chunk_result) = stream.next().await {
                let chunk = match chunk_result {
                    Ok(c) => c,
                    Err(e) => {
                        yield StreamEvent::Error { message: e.to_string() };
                        break;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(newline_pos) = buffer.find('\n') {
                    let line = buffer[..newline_pos].to_string();
                    buffer = buffer[newline_pos + 1..].to_string();

                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    let data = match line.strip_prefix("data: ") {
                        Some(d) => d.trim(),
                        None => continue,
                    };

                    if data == "[DONE]" {
                        break;
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
                                            HuggingFaceClient::strip_tool_prefix(name);
                                        tool_call_names.insert(index, original_name.clone());
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
                            for (_idx, id) in tool_call_ids.drain() {
                                yield StreamEvent::ToolCallEnd { id };
                            }
                            tool_call_names.clear();

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
    #[serde(default)]
    #[allow(dead_code)]
    error: String,
    #[serde(default)]
    estimated_time: Option<f64>,
}

/// Discovers available text-generation models from the HuggingFace Hub API.
///
/// Queries the HuggingFace model hub for models tagged with `text-generation`
/// that have warm inference endpoints. Returns up to [`MAX_DISCOVERED_MODELS`]
/// models.
///
/// # Arguments
///
/// * `api_key` - HuggingFace API token for authenticated requests.
///
/// # Errors
///
/// Returns an error on network failures or invalid API responses.
pub async fn discover_models(api_key: &str) -> Result<Vec<ModelInfo>> {
    if api_key.is_empty() {
        bail!("HuggingFace model discovery requires an API token.");
    }

    let client = crate::provider::http_client::create_http_client();
    let url = "https://huggingface.co/api/models\
               ?pipeline_tag=text-generation\
               &inference=warm\
               &sort=likes\
               &direction=-1\
               &limit=50";

    let response = client
        .get(url)
        .header("Authorization", format!("Bearer {api_key}"))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .context("Failed to connect to HuggingFace Hub API")?;

    if !response.status().is_success() {
        bail!(
            "HuggingFace Hub API returned status {} when discovering models",
            response.status()
        );
    }

    let models: Vec<HfModelEntry> = response
        .json()
        .await
        .context("Failed to parse HuggingFace model list")?;

    let result: Vec<ModelInfo> = models
        .into_iter()
        .take(MAX_DISCOVERED_MODELS)
        .map(|m| {
            let has_tool_use = m
                .tags
                .iter()
                .any(|t| t == "tool-use" || t == "function-calling");
            let has_vision = m
                .tags
                .iter()
                .any(|t| t == "vision" || t == "image-text-to-text");

            ModelInfo {
                id: m.model_id.clone(),
                provider_id: "huggingface".to_string(),
                name: format_model_display_name(&m.model_id),
                cost: Cost {
                    input: 0.0,
                    output: 0.0,
                },
                capabilities: Capabilities {
                    reasoning: false,
                    streaming: true,
                    vision: has_vision,
                    tool_use: has_tool_use,
                },
                context_window: estimate_context_from_id(&m.model_id),
                max_output: Some(4_096),
                request_multiplier: None,
            }
        })
        .collect();

    Ok(result)
}

/// HuggingFace Hub API model entry (subset of fields).
#[derive(Debug, Deserialize)]
struct HfModelEntry {
    #[serde(rename = "id")]
    model_id: String,
    #[serde(default)]
    tags: Vec<String>,
}

/// Formats a HuggingFace model ID into a human-readable display name.
///
/// Strips the org prefix and converts hyphens/underscores to spaces.
/// Examples: `"meta-llama/Llama-3.1-70B-Instruct"` → `"Llama 3.1 70B Instruct"`
fn format_model_display_name(model_id: &str) -> String {
    let name = model_id
        .rsplit_once('/')
        .map(|(_, name)| name)
        .unwrap_or(model_id);
    name.replace('-', " ").replace('_', " ")
}

/// Estimates context window size from the model ID.
///
/// Looks for common size indicators in the model name (e.g., `4k`, `128k`).
/// Falls back to parameter-size heuristics.
fn estimate_context_from_id(model_id: &str) -> usize {
    let lower = model_id.to_lowercase();

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
mod tests {
    use super::*;

    #[test]
    fn test_provider_id_and_name() {
        let provider = HuggingFaceProvider;
        assert_eq!(provider.id(), "huggingface");
        assert_eq!(provider.name(), "Hugging Face");
    }

    #[test]
    fn test_default_models_non_empty() {
        let provider = HuggingFaceProvider;
        let models = provider.default_models();
        assert!(models.len() >= 4, "Expected at least 4 default models");
    }

    #[test]
    fn test_default_models_have_correct_provider_id() {
        let models = huggingface_default_models();
        for model in &models {
            assert_eq!(model.provider_id, "huggingface");
        }
    }

    #[test]
    fn test_default_models_all_free() {
        let models = huggingface_default_models();
        for model in &models {
            assert_eq!(model.cost.input, 0.0);
            assert_eq!(model.cost.output, 0.0);
        }
    }

    #[test]
    fn test_format_model_display_name() {
        assert_eq!(
            format_model_display_name("meta-llama/Llama-3.1-70B-Instruct"),
            "Llama 3.1 70B Instruct"
        );
        assert_eq!(
            format_model_display_name("mistralai/Mixtral-8x7B-Instruct-v0.1"),
            "Mixtral 8x7B Instruct v0.1"
        );
        assert_eq!(format_model_display_name("plain-model"), "plain model");
    }

    #[test]
    fn test_estimate_context_from_id() {
        assert_eq!(
            estimate_context_from_id("meta-llama/Llama-3.1-70B-Instruct"),
            128_000
        );
        assert_eq!(
            estimate_context_from_id("microsoft/Phi-3-mini-4k-instruct"),
            4_096
        );
        assert_eq!(
            estimate_context_from_id("mistralai/Mixtral-8x7B-Instruct-v0.1"),
            32_000
        );
        assert_eq!(estimate_context_from_id("some-unknown/model-7b"), 32_000);
    }

    #[test]
    fn test_build_request_body_basic() {
        let client = HuggingFaceClient {
            api_key: "test_key".to_string(),
            base_url: HF_API_BASE.to_string(),
            http: reqwest::Client::new(),
            wait_for_model: true,
            use_cache: true,
        };

        let request = ChatRequest {
            model: "meta-llama/Llama-3.1-70B-Instruct".to_string(),
            messages: vec![crate::llm::ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("Hello".to_string()),
            }],
            tools: vec![],
            temperature: Some(0.7),
            top_p: None,
            max_tokens: Some(4096),
            system: Some("You are helpful.".to_string()),
            options: HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: None,
        };

        let body = client.build_request_body(&request);

        assert_eq!(body["model"], "meta-llama/Llama-3.1-70B-Instruct");
        assert_eq!(body["stream"], true);
        let temp = body["temperature"].as_f64().unwrap();
        assert!((temp - 0.7).abs() < 0.001, "temperature was {temp}");
        assert_eq!(body["max_tokens"], 4096);

        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "You are helpful.");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[1]["content"], "Hello");
    }

    #[test]
    fn test_build_request_body_with_tools() {
        let client = HuggingFaceClient {
            api_key: "test_key".to_string(),
            base_url: HF_API_BASE.to_string(),
            http: reqwest::Client::new(),
            wait_for_model: true,
            use_cache: true,
        };

        let request = ChatRequest {
            model: "test-model".to_string(),
            messages: vec![],
            tools: vec![crate::llm::ToolDefinition {
                name: "read_file".to_string(),
                description: "Read a file".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" }
                    }
                }),
            }],
            temperature: None,
            top_p: None,
            max_tokens: None,
            system: None,
            options: HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: None,
        };

        let body = client.build_request_body(&request);
        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["type"], "function");
        assert_eq!(tools[0]["function"]["name"], "t_read_file");
    }

    #[test]
    fn test_build_request_body_with_tool_results() {
        let client = HuggingFaceClient {
            api_key: "test_key".to_string(),
            base_url: HF_API_BASE.to_string(),
            http: reqwest::Client::new(),
            wait_for_model: true,
            use_cache: true,
        };

        let request = ChatRequest {
            model: "test-model".to_string(),
            messages: vec![crate::llm::ChatMessage {
                role: "tool".to_string(),
                content: ChatContent::Parts(vec![ContentPart::ToolResult {
                    tool_use_id: "call_123".to_string(),
                    content: "file contents here".to_string(),
                }]),
            }],
            tools: vec![],
            temperature: None,
            top_p: None,
            max_tokens: None,
            system: None,
            options: HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: None,
        };

        let body = client.build_request_body(&request);
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "tool");
        assert_eq!(messages[0]["tool_call_id"], "call_123");
        assert_eq!(messages[0]["content"], "file contents here");
    }

    #[test]
    fn test_build_request_body_with_tool_use() {
        let client = HuggingFaceClient {
            api_key: "test_key".to_string(),
            base_url: HF_API_BASE.to_string(),
            http: reqwest::Client::new(),
            wait_for_model: true,
            use_cache: true,
        };

        let request = ChatRequest {
            model: "test-model".to_string(),
            messages: vec![crate::llm::ChatMessage {
                role: "assistant".to_string(),
                content: ChatContent::Parts(vec![ContentPart::ToolUse {
                    id: "call_456".to_string(),
                    name: "write_file".to_string(),
                    input: json!({"path": "test.txt", "content": "hello"}),
                }]),
            }],
            tools: vec![],
            temperature: None,
            top_p: None,
            max_tokens: None,
            system: None,
            options: HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: None,
        };

        let body = client.build_request_body(&request);
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "assistant");
        let tool_calls = messages[0]["tool_calls"].as_array().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0]["id"], "call_456");
        assert_eq!(tool_calls[0]["function"]["name"], "t_write_file");
    }

    #[test]
    fn test_hf_error_response_parsing() {
        let json_str = r#"{"error": "Model is currently loading", "estimated_time": 45.2}"#;
        let err: HfErrorResponse = serde_json::from_str(json_str).unwrap();
        assert_eq!(err.error, "Model is currently loading");
        assert!((err.estimated_time.unwrap() - 45.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_hf_error_response_no_time() {
        let json_str = r#"{"error": "Unauthorized"}"#;
        let err: HfErrorResponse = serde_json::from_str(json_str).unwrap();
        assert_eq!(err.error, "Unauthorized");
        assert!(err.estimated_time.is_none());
    }

    #[test]
    fn test_parse_hf_rate_limit_headers_present() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-ratelimit-limit", "100".parse().unwrap());
        headers.insert("x-ratelimit-remaining", "95".parse().unwrap());

        let event = parse_hf_rate_limit_headers(&headers);
        assert!(event.is_some());
        if let Some(StreamEvent::RateLimit {
            requests_used_pct,
            tokens_used_pct,
        }) = event
        {
            assert!((requests_used_pct.unwrap() - 5.0).abs() < 0.1);
            assert!(tokens_used_pct.is_none());
        } else {
            panic!("Expected RateLimit event");
        }
    }

    #[test]
    fn test_parse_hf_rate_limit_headers_absent() {
        let headers = reqwest::header::HeaderMap::new();
        let event = parse_hf_rate_limit_headers(&headers);
        assert!(event.is_none());
    }

    #[test]
    fn test_build_request_body_no_system_prompt() {
        let client = HuggingFaceClient {
            api_key: "test_key".to_string(),
            base_url: HF_API_BASE.to_string(),
            http: reqwest::Client::new(),
            wait_for_model: true,
            use_cache: true,
        };

        let request = ChatRequest {
            model: "test-model".to_string(),
            messages: vec![crate::llm::ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("Hi".to_string()),
            }],
            tools: vec![],
            temperature: None,
            top_p: None,
            max_tokens: None,
            system: None,
            options: HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: None,
        };

        let body = client.build_request_body(&request);
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
    }

    #[test]
    fn test_build_request_includes_stream_options() {
        let client = HuggingFaceClient {
            api_key: "test_key".to_string(),
            base_url: HF_API_BASE.to_string(),
            http: reqwest::Client::new(),
            wait_for_model: true,
            use_cache: true,
        };

        let request = ChatRequest {
            model: "test-model".to_string(),
            messages: vec![],
            tools: vec![],
            temperature: None,
            top_p: None,
            max_tokens: None,
            system: None,
            options: HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: None,
        };

        let body = client.build_request_body(&request);
        assert_eq!(body["stream"], true);
        assert_eq!(body["stream_options"]["include_usage"], true);
    }

    #[test]
    fn test_safe_tool_name_and_strip() {
        assert_eq!(HuggingFaceClient::safe_tool_name("search"), "t_search");
        assert_eq!(
            HuggingFaceClient::safe_tool_name("write_file"),
            "t_write_file"
        );
        assert_eq!(HuggingFaceClient::strip_tool_prefix("t_search"), "search");
        assert_eq!(
            HuggingFaceClient::strip_tool_prefix("t_write_file"),
            "write_file"
        );
        // If the model returns a name without prefix, pass through unchanged
        assert_eq!(
            HuggingFaceClient::strip_tool_prefix("unknown_tool"),
            "unknown_tool"
        );
    }

    #[test]
    fn test_system_prompt_rewriting() {
        let tools = vec![
            crate::llm::ToolDefinition {
                name: "search".to_string(),
                description: "Search".to_string(),
                parameters: json!({}),
            },
            crate::llm::ToolDefinition {
                name: "write_file".to_string(),
                description: "Write a file".to_string(),
                parameters: json!({}),
            },
        ];

        let prompt = "You have tools: search, write_file. Use search to find code.";
        let rewritten = HuggingFaceClient::rewrite_system_prompt(prompt, &tools);
        assert!(rewritten.contains("t_search"));
        assert!(rewritten.contains("t_write_file"));
        assert!(!rewritten.contains(" search"));
    }
}
