//! OpenAI provider implementation.
//!
//! Implements the [`Provider`] trait for the OpenAI Chat Completions API, supporting
//! streaming responses, tool calls, and usage tracking.

use anyhow::{Context, Result, bail};
use futures::StreamExt;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::pin::Pin;

use crate::config::{Capabilities, Cost};
use crate::event::FinishReason;
use crate::llm::{ChatContent, ChatRequest, ContentPart, LlmClient, StreamEvent};
use crate::provider::{ModelInfo, Provider};

/// Provider implementation for the OpenAI Chat Completions API.
pub struct OpenAiProvider;

impl OpenAiProvider {
    const API_BASE: &'static str = "https://api.openai.com";
}

#[async_trait::async_trait]
impl Provider for OpenAiProvider {
    /// Returns `"openai"`.
    fn id(&self) -> &str {
        "openai"
    }

    /// Returns `"OpenAI"`.
    fn name(&self) -> &str {
        "OpenAI"
    }

    /// Returns default OpenAI models (GPT-4o, GPT-4o Mini).
    fn default_models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "gpt-4o".to_string(),
                provider_id: "openai".to_string(),
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
                },
                context_window: 128_000,
                max_output: Some(16_384),
            },
            ModelInfo {
                id: "gpt-4o-mini".to_string(),
                provider_id: "openai".to_string(),
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
                },
                context_window: 128_000,
                max_output: Some(16_384),
            },
        ]
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
        let client = OpenAiClient {
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

/// HTTP client for the OpenAI Chat Completions API with streaming SSE support.
pub struct OpenAiClient {
    api_key: String,
    base_url: String,
    http: reqwest::Client,
}

impl OpenAiClient {
    fn build_request_body(&self, request: &ChatRequest) -> Value {
        let mut messages = Vec::new();

        // Add system message if present
        if let Some(system) = &request.system {
            messages.push(json!({
                "role": "system",
                "content": system
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
                            ContentPart::ToolResult {
                                tool_use_id: _,
                                content: _,
                            } => None,
                            ContentPart::ToolUse { .. } => None,
                        })
                        .collect();
                    if content_parts.len() == 1 {
                        // Unwrap single text to string
                        content_parts[0]["text"].clone()
                    } else {
                        json!(content_parts)
                    }
                }
            };

            // Handle tool results as separate messages in OpenAI format
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
                        // Assistant message with tool calls
                        let tool_calls: Vec<Value> = tool_uses
                            .iter()
                            .map(|p| match p {
                                ContentPart::ToolUse { id, name, input } => json!({
                                    "id": id,
                                    "type": "function",
                                    "function": {
                                        "name": name,
                                        "arguments": input.to_string()
                                    }
                                }),
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
                            "name": t.name,
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
impl LlmClient for OpenAiClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let body = self.build_request_body(&request);

        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to send request to OpenAI API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to read response body");
                String::new()
            });
            bail!("OpenAI API error ({}): {}", status, body);
        }

        let stream = response.bytes_stream();

        let event_stream = async_stream::stream! {
            let mut buffer = String::new();
            let mut tool_call_ids: HashMap<u64, String> = HashMap::new();
            let mut tool_call_names: HashMap<u64, String> = HashMap::new();

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

                    // Handle usage info (sent with stream_options.include_usage)
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
                                            .unwrap_or_else(|| format!("tc_{}", index));
                                        tool_call_names.insert(index, name.to_string());
                                        yield StreamEvent::ToolCallStart {
                                            id: tc_id,
                                            name: name.to_string(),
                                        };
                                    }

                                    if let Some(args) = function["arguments"].as_str()
                                        && !args.is_empty()
                                    {
                                        let tc_id = tool_call_ids.get(&index)
                                            .cloned()
                                            .unwrap_or_else(|| format!("tc_{}", index));
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
                            // End any pending tool calls
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
