//! Anthropic (Claude) provider implementation.
//!
//! Implements the [`Provider`] trait for the Anthropic Messages API, supporting
//! streaming chat completions, tool use, and extended thinking.

use anyhow::{Context, Result, bail};
use futures::StreamExt;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::pin::Pin;

use super::thinking::{
    anthropic_thinking_levels_for_model, anthropic_thinking_payload_from_request,
    full_reasoning_levels, reasoning_levels_from_supported_efforts,
    request_uses_unsupported_anthropic_display,
};
use crate::event::FinishReason;
use crate::llm::{ChatContent, ChatRequest, ContentPart, LlmClient, StreamEvent};
use crate::{ModelInfo, Provider};
use ragent_config::{Capabilities, Cost};

/// Extract the MIME type from a `data:<mime>;base64,<data>` URI.
///
/// # Errors
///
/// This function does not return errors; it returns `None` if the URI is malformed.
fn extract_mime_from_data_uri(uri: &str) -> Option<&str> {
    uri.strip_prefix("data:").and_then(|s| s.split(';').next())
}

/// Extract the raw base64 payload from a `data:<mime>;base64,<data>` URI.
///
/// # Errors
///
/// This function does not return errors; it returns `None` if the URI is malformed.
fn extract_base64_from_data_uri(uri: &str) -> Option<&str> {
    uri.find(",base64,")
        .map(|i| &uri[i + 8..])
        .or_else(|| uri.find(',').map(|i| &uri[i + 1..]))
}

/// Provider implementation for the Anthropic Claude API.
pub struct AnthropicProvider;

impl AnthropicProvider {
    const API_BASE: &'static str = "https://api.anthropic.com";
}

#[async_trait::async_trait]
impl Provider for AnthropicProvider {
    /// Returns `"anthropic"`.
    fn id(&self) -> &'static str {
        "anthropic"
    }

    /// Returns `"Anthropic"`.
    fn name(&self) -> &'static str {
        "Anthropic"
    }

    /// Returns default Claude models (Sonnet 4, 3.5 Haiku).
    fn default_models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "claude-sonnet-4-20250514".to_string(),
                provider_id: "anthropic".to_string(),
                name: "Claude Sonnet 4".to_string(),
                cost: Cost {
                    input: 3.0,
                    output: 15.0,
                },
                capabilities: Capabilities {
                    reasoning: true,
                    streaming: true,
                    vision: true,
                    tool_use: true,
                    thinking_levels: anthropic_thinking_levels_for_model(
                        "claude-sonnet-4-20250514",
                    ),
                },
                context_window: 200_000,
                max_output: Some(64_000),
                request_multiplier: None,
                thinking_config: None,
            },
            ModelInfo {
                id: "claude-3-5-haiku-latest".to_string(),
                provider_id: "anthropic".to_string(),
                name: "Claude 3.5 Haiku".to_string(),
                cost: Cost {
                    input: 0.80,
                    output: 4.0,
                },
                capabilities: Capabilities {
                    reasoning: false,
                    streaming: true,
                    vision: true,
                    tool_use: true,
                    thinking_levels: anthropic_thinking_levels_for_model("claude-3-5-haiku-latest"),
                },
                context_window: 200_000,
                max_output: Some(8_192),
                request_multiplier: None,
                thinking_config: None,
            },
        ]
    }

    /// Creates an [`AnthropicClient`] configured with the given API key and optional base URL.
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
        let client = AnthropicClient {
            api_key: api_key.to_string(),
            base_url: base_url
                .unwrap_or(Self::API_BASE)
                .trim_end_matches('/')
                .to_string(),
            http: crate::provider::http_client::create_streaming_http_client(),
        };
        Ok(Box::new(client))
    }
}

fn anthropic_string(entry: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| entry.get(*key).and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

fn anthropic_usize(entry: &Value, keys: &[&str]) -> Option<usize> {
    keys.iter().find_map(|key| {
        entry
            .get(*key)
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
    })
}

fn anthropic_bool(entry: &Value, pointers: &[&str]) -> Option<bool> {
    pointers
        .iter()
        .find_map(|pointer| entry.pointer(pointer).and_then(Value::as_bool))
}

fn anthropic_model_to_info(
    entry: &Value,
    defaults: &HashMap<String, ModelInfo>,
) -> Option<ModelInfo> {
    let id = anthropic_string(entry, &["id", "name"])?;
    let default = defaults.get(&id);

    let reasoning_efforts = entry
        .pointer("/capabilities/reasoning_effort")
        .or_else(|| entry.pointer("/capabilities/supports/reasoning_effort"))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        });

    let detected_levels = reasoning_efforts
        .as_deref()
        .map(|efforts| reasoning_levels_from_supported_efforts(Some(efforts)))
        .filter(|levels| !levels.is_empty())
        .or_else(|| {
            let thinking_supported = anthropic_bool(
                entry,
                &[
                    "/capabilities/thinking/supported",
                    "/capabilities/supports/thinking",
                    "/capabilities/reasoning/supported",
                ],
            )
            .unwrap_or(false);
            let has_thinking_types = entry
                .pointer("/capabilities/thinking/types")
                .and_then(Value::as_array)
                .is_some_and(|types| !types.is_empty());

            if thinking_supported || has_thinking_types {
                Some(full_reasoning_levels())
            } else {
                None
            }
        })
        .unwrap_or_else(|| anthropic_thinking_levels_for_model(&id));

    let capabilities = default.map_or_else(
        || Capabilities {
            reasoning: !detected_levels.is_empty(),
            streaming: true,
            vision: anthropic_bool(
                entry,
                &["/capabilities/vision", "/capabilities/vision/supported"],
            )
            .unwrap_or(false),
            tool_use: anthropic_bool(
                entry,
                &[
                    "/capabilities/tools",
                    "/capabilities/tools/supported",
                    "/capabilities/tool_use",
                ],
            )
            .unwrap_or(true),
            thinking_levels: detected_levels.clone(),
        },
        |model| {
            let mut capabilities = model.capabilities.clone();
            capabilities.reasoning = !detected_levels.is_empty();
            capabilities.thinking_levels = detected_levels.clone();
            capabilities
        },
    );

    Some(ModelInfo {
        id: id.clone(),
        provider_id: "anthropic".to_string(),
        name: anthropic_string(entry, &["display_name", "displayName", "name"])
            .or_else(|| default.map(|model| model.name.clone()))
            .unwrap_or(id),
        cost: default.map(|model| model.cost.clone()).unwrap_or(Cost {
            input: 0.0,
            output: 0.0,
        }),
        capabilities,
        context_window: anthropic_usize(entry, &["context_window", "input_token_limit"])
            .or_else(|| default.map(|model| model.context_window))
            .unwrap_or(200_000),
        max_output: anthropic_usize(entry, &["max_output_tokens", "output_token_limit"])
            .or_else(|| default.and_then(|model| model.max_output)),
        request_multiplier: default.and_then(|model| model.request_multiplier),
        thinking_config: default.and_then(|model| model.thinking_config.clone()),
    })
}

/// Queries Anthropic's `/v1/models` endpoint and converts the response into
/// `ModelInfo` rows enriched with live thinking metadata when available.
///
/// Falls back to the existing model-ID heuristics for thinking levels if the
/// API omits explicit reasoning capability fields.
///
/// # Errors
///
/// Returns an error if the HTTP request fails or the response cannot be parsed.
pub async fn list_anthropic_models(
    api_key: &str,
    base_url: Option<&str>,
) -> Result<Vec<ModelInfo>> {
    let base_url = base_url
        .unwrap_or(AnthropicProvider::API_BASE)
        .trim_end_matches('/');
    let url = format!("{base_url}/v1/models");
    let http = crate::provider::http_client::create_http_client();
    let resp = http
        .get(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .context("Failed to fetch Anthropic models")?;

    if !resp.status().is_success() {
        bail!("Anthropic models endpoint returned HTTP {}", resp.status());
    }

    let payload: Value = resp
        .json()
        .await
        .context("Failed to parse Anthropic models response")?;
    let data = payload
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let defaults = AnthropicProvider.default_models();
    let defaults_by_id = defaults
        .into_iter()
        .map(|model| (model.id.clone(), model))
        .collect::<HashMap<_, _>>();

    let mut models: Vec<ModelInfo> = data
        .iter()
        .filter_map(|entry| anthropic_model_to_info(entry, &defaults_by_id))
        .collect();

    models.sort_by(|a, b| a.name.cmp(&b.name));
    models.dedup_by(|a, b| a.id == b.id);
    Ok(models)
}

/// HTTP client for the Anthropic Messages API with streaming SSE support.
pub struct AnthropicClient {
    api_key: String,
    base_url: String,
    http: reqwest::Client,
}

impl AnthropicClient {
    fn build_request_body(&self, request: &ChatRequest) -> Value {
        let mut messages = Vec::new();

        for msg in &request.messages {
            let content = match &msg.content {
                ChatContent::Text(text) => json!(text),
                ChatContent::Parts(parts) => {
                    let content_parts: Vec<Value> = parts
                        .iter()
                        .map(|part| match part {
                            ContentPart::Text { text } => json!({
                                "type": "text",
                                "text": text
                            }),
                            ContentPart::ToolUse { id, name, input } => json!({
                                "type": "tool_use",
                                "id": id,
                                "name": name,
                                "input": input
                            }),
                            ContentPart::ToolResult {
                                tool_use_id,
                                content,
                            } => json!({
                                "type": "tool_result",
                                "tool_use_id": tool_use_id,
                                "content": content
                            }),
                            ContentPart::ImageUrl { url } => json!({
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": extract_mime_from_data_uri(url).unwrap_or("image/png"),
                                    "data": extract_base64_from_data_uri(url).unwrap_or(url.as_str())
                                }
                            }),
                        })
                        .collect();
                    json!(content_parts)
                }
            };
            messages.push(json!({
                "role": msg.role,
                "content": content
            }));
        }

        let mut body = json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(8192),
            "stream": true
        });

        if let Some(system) = &request.system {
            body["system"] = json!(system);
        }
        if let Some(temp) = request.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = json!(top_p);
        }
        if !request.tools.is_empty() {
            let tools: Vec<Value> = request
                .tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.parameters
                    })
                })
                .collect();
            body["tools"] = json!(tools);
        }

        if request_uses_unsupported_anthropic_display(request) {
            tracing::warn!(
                model = %request.model,
                "Anthropic summarized thinking display is not supported yet; falling back to standard thinking output"
            );
        }

        if let Some(thinking) = anthropic_thinking_payload_from_request(request) {
            body["thinking"] = thinking;
        }
        body
    }
}

#[async_trait::async_trait]
impl LlmClient for AnthropicClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        let url = format!("{}/v1/messages", self.base_url);
        let body = self.build_request_body(&request);

        let response = self
            .http
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to send request to Anthropic API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to read response body");
                String::new()
            });
            tracing::warn!(
                url = %url,
                model = %request.model,
                status = %status,
                error = %body,
                "Anthropic API error"
            );
            bail!("Anthropic API error ({status}): {body}");
        }

        let rate_limit_event = parse_anthropic_rate_limit_headers(response.headers());
        let stream = response.bytes_stream();

        let event_stream = async_stream::stream! {
            let mut buffer = String::new();
            let mut current_event_type = String::new();
            let mut tool_call_args: HashMap<String, String> = HashMap::new();

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

                    if let Some(event_type) = line.strip_prefix("event: ") {
                        current_event_type = event_type.trim().to_string();
                        continue;
                    }

                    if let Some(data) = line.strip_prefix("data: ") {
                        let data = data.trim();
                        if data == "[DONE]" {
                            break;
                        }

                        let parsed: Value = match serde_json::from_str(data) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };

                        match current_event_type.as_str() {
                            "content_block_start" => {
                                let content_block = &parsed["content_block"];
                                match content_block["type"].as_str() {
                                    Some("text") => {}
                                    Some("thinking") => {
                                        yield StreamEvent::ReasoningStart;
                                    }
                                    Some("tool_use") => {
                                        let id = content_block["id"].as_str().unwrap_or("").to_string();
                                        let name = content_block["name"].as_str().unwrap_or("").to_string();
                                        tool_call_args.insert(id.clone(), String::new());
                                        yield StreamEvent::ToolCallStart { id, name };
                                    }
                                    _ => {}
                                }
                            }
                            "content_block_delta" => {
                                let delta = &parsed["delta"];
                                match delta["type"].as_str() {
                                    Some("text_delta") => {
                                        if let Some(text) = delta["text"].as_str() {
                                            yield StreamEvent::TextDelta { text: text.to_string() };
                                        }
                                    }
                                    Some("thinking_delta") => {
                                        if let Some(text) = delta["thinking"].as_str() {
                                            yield StreamEvent::ReasoningDelta { text: text.to_string() };
                                        }
                                    }
                                    Some("input_json_delta") => {
                                        if let Some(json_str) = delta["partial_json"].as_str() {
                                            // Find which tool call this belongs to
                                            // Anthropic sends index-based, use the last active tool
                                            let _idx = parsed["index"].as_u64().unwrap_or(0);
                                            // We track by last started tool call
                                            if let Some((id, args)) = tool_call_args.iter_mut().last() {
                                                args.push_str(json_str);
                                                yield StreamEvent::ToolCallDelta {
                                                    id: id.clone(),
                                                    args_json: json_str.to_string(),
                                                };
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            "content_block_stop" => {
                                let _idx = parsed["index"].as_u64().unwrap_or(0);
                                // Check if this is a tool_use block ending
                                // We emit ToolCallEnd for the last known tool
                                if let Some((id, _)) = tool_call_args.iter().last() {
                                    // We check if we have pending tool args
                                    let id = id.clone();
                                    if !id.is_empty() {
                                        yield StreamEvent::ToolCallEnd { id: id.clone() };
                                        tool_call_args.remove(&id);
                                    }
                                }
                            }
                            "message_delta" => {
                                if let Some(usage) = parsed.get("usage") {
                                    let output_tokens = usage["output_tokens"].as_u64().unwrap_or(0);
                                    yield StreamEvent::Usage {
                                        input_tokens: 0,
                                        output_tokens,
                                    };
                                }
                                if let Some(stop_reason) = parsed["delta"]["stop_reason"].as_str() {
                                    let reason = match stop_reason {
                                        "tool_use" => FinishReason::ToolUse,
                                        "max_tokens" => FinishReason::Length,
                                        _ => FinishReason::Stop,
                                    };
                                    yield StreamEvent::Finish { reason };
                                }
                            }
                            "message_start" => {
                                if let Some(usage) = parsed["message"].get("usage") {
                                    let input_tokens = usage["input_tokens"].as_u64().unwrap_or(0);
                                    yield StreamEvent::Usage {
                                        input_tokens,
                                        output_tokens: 0,
                                    };
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        };

        Ok(Box::pin(event_stream))
    }
}

/// Parses Anthropic rate-limit response headers into a `StreamEvent::RateLimit`.
///
/// Headers: `anthropic-ratelimit-requests-limit`, `anthropic-ratelimit-requests-remaining`,
///          `anthropic-ratelimit-tokens-limit`, `anthropic-ratelimit-tokens-remaining`.
fn parse_anthropic_rate_limit_headers(
    headers: &reqwest::header::HeaderMap,
) -> Option<crate::llm::StreamEvent> {
    let header_u64 = |name: &str| -> Option<u64> {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
    };

    let req_limit = header_u64("anthropic-ratelimit-requests-limit");
    let req_remaining = header_u64("anthropic-ratelimit-requests-remaining");
    let tok_limit = header_u64("anthropic-ratelimit-tokens-limit");
    let tok_remaining = header_u64("anthropic-ratelimit-tokens-remaining");

    let requests_used_pct = req_limit.zip(req_remaining).map(|(limit, remaining)| {
        if limit == 0 {
            0.0f32
        } else {
            ((limit.saturating_sub(remaining)) as f32 / limit as f32 * 100.0).clamp(0.0, 100.0)
        }
    });

    let tokens_used_pct = tok_limit.zip(tok_remaining).map(|(limit, remaining)| {
        if limit == 0 {
            0.0f32
        } else {
            ((limit.saturating_sub(remaining)) as f32 / limit as f32 * 100.0).clamp(0.0, 100.0)
        }
    });

    if requests_used_pct.is_some() || tokens_used_pct.is_some() {
        Some(crate::llm::StreamEvent::RateLimit {
            requests_used_pct,
            tokens_used_pct,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_model_to_info_uses_live_thinking_capability() {
        let defaults = AnthropicProvider
            .default_models()
            .into_iter()
            .map(|model| (model.id.clone(), model))
            .collect::<HashMap<_, _>>();
        let entry = json!({
            "id": "claude-sonnet-4-20250514",
            "display_name": "Claude Sonnet 4",
            "context_window": 250000,
            "max_output_tokens": 32000,
            "capabilities": {
                "thinking": {
                    "supported": true,
                    "types": ["adaptive", "enabled"]
                }
            }
        });

        let model = anthropic_model_to_info(&entry, &defaults).expect("model info");
        assert!(model.capabilities.reasoning);
        assert_eq!(model.capabilities.thinking_levels, full_reasoning_levels());
        assert_eq!(model.context_window, 250_000);
        assert_eq!(model.max_output, Some(32_000));
    }
}
