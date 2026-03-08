//! Anthropic (Claude) provider implementation.
//!
//! Implements the [`Provider`] trait for the Anthropic Messages API, supporting
//! streaming chat completions, tool use, and extended thinking.

use anyhow::{Context, Result, bail};
use futures::StreamExt;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::pin::Pin;

use crate::config::{Capabilities, Cost};
use crate::event::FinishReason;
use crate::llm::{ChatContent, ChatRequest, ContentPart, LlmClient, StreamEvent};
use crate::provider::{ModelInfo, Provider};

/// Provider implementation for the Anthropic Claude API.
pub struct AnthropicProvider;

impl AnthropicProvider {
    const API_BASE: &'static str = "https://api.anthropic.com";
}

#[async_trait::async_trait]
impl Provider for AnthropicProvider {
    /// Returns `"anthropic"`.
    fn id(&self) -> &str {
        "anthropic"
    }

    /// Returns `"Anthropic"`.
    fn name(&self) -> &str {
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
                },
                context_window: 200_000,
                max_output: Some(64_000),
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
                },
                context_window: 200_000,
                max_output: Some(8_192),
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
            http: reqwest::Client::new(),
        };
        Ok(Box::new(client))
    }
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
            bail!("Anthropic API error ({}): {}", status, body);
        }

        let stream = response.bytes_stream();

        let event_stream = async_stream::stream! {
            let mut buffer = String::new();
            let mut current_event_type = String::new();
            let mut tool_call_args: HashMap<String, String> = HashMap::new();

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
