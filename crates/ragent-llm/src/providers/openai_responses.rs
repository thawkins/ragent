//! OpenAI Responses API provider implementation.
//!
//! Implements the [`Provider`] trait for the OpenAI Responses API, which is the
//! recommended API for reasoning models (GPT-5.x, o1, o3, etc.). The Responses API
//! provides better support for:
//! - Reasoning tokens and summaries
//! - Multi-turn conversation continuity via `previous_response_id`
//! - Cache write tracking for cost optimization
//! - 409 Conflict retry logic for concurrent modifications
//!
//! Unlike the Chat Completions API, the Responses API:
//! - Uses `input` instead of `messages`
//! - Returns `output` array with structured content
//! - Supports `reasoning.effort` and `reasoning.mode` parameters
//! - Exposes reasoning summaries (not raw tokens)
//! - Tracks `cache_write_tokens` separately in usage

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::pin::Pin;
use std::time::Duration;

use super::http_client::{create_http_client, create_streaming_http_client};
use super::thinking::openai_thinking_levels_for_model;
use crate::llm::{ChatContent, ChatRequest, ContentPart, LlmClient, StreamEvent};
use crate::{ModelInfo, Provider};
use ragent_config::{Capabilities, Cost};
use ragent_types::event::FinishReason;
use ragent_types::thinking::ThinkingLevel;

/// Default API base URL for OpenAI Responses API.
pub const RESPONSES_API_BASE: &str = "https://api.openai.com/v1";

/// Returns the default OpenAI Responses API model catalog.
///
/// These models are optimized for the Responses API format.
#[must_use]
pub fn responses_api_default_models(provider_id: &str) -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "gpt-5.6".to_string(),
            provider_id: provider_id.to_string(),
            name: "GPT-5.6".to_string(),
            cost: Cost {
                input: 1.25,
                output: 10.0,
            },
            capabilities: Capabilities {
                reasoning: true,
                streaming: true,
                vision: true,
                tool_use: true,
                thinking_levels: openai_thinking_levels_for_model("gpt-5.6"),
            },
            context_window: 400_000,
            max_output: Some(128_000),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "gpt-5.6-sol".to_string(),
            provider_id: provider_id.to_string(),
            name: "GPT-5.6 Sol (Pro)".to_string(),
            cost: Cost {
                input: 2.50,
                output: 20.0,
            },
            capabilities: Capabilities {
                reasoning: true,
                streaming: true,
                vision: true,
                tool_use: true,
                thinking_levels: openai_thinking_levels_for_model("gpt-5.6-sol"),
            },
            context_window: 400_000,
            max_output: Some(128_000),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "o1".to_string(),
            provider_id: provider_id.to_string(),
            name: "o1".to_string(),
            cost: Cost {
                input: 15.0,
                output: 60.0,
            },
            capabilities: Capabilities {
                reasoning: true,
                streaming: true,
                vision: true,
                tool_use: true,
                thinking_levels: openai_thinking_levels_for_model("o1"),
            },
            context_window: 200_000,
            max_output: Some(100_000),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "o3".to_string(),
            provider_id: provider_id.to_string(),
            name: "o3".to_string(),
            cost: Cost {
                input: 10.0,
                output: 40.0,
            },
            capabilities: Capabilities {
                reasoning: true,
                streaming: true,
                vision: true,
                tool_use: true,
                thinking_levels: openai_thinking_levels_for_model("o3"),
            },
            context_window: 200_000,
            max_output: Some(100_000),
            request_multiplier: None,
            thinking_config: None,
        },
    ]
}

/// Provider implementation for the OpenAI Responses API.
pub struct ResponsesApiProvider;

#[async_trait::async_trait]
impl Provider for ResponsesApiProvider {
    /// Returns `"openai_responses"`.
    fn id(&self) -> &'static str {
        "openai_responses"
    }

    /// Returns `"OpenAI Responses API"`.
    fn name(&self) -> &'static str {
        "OpenAI Responses API"
    }

    fn as_any_static(&self) -> &dyn std::any::Any {
        self
    }

    /// Returns the default Responses API model catalog.
    fn default_models(&self) -> Vec<ModelInfo> {
        responses_api_default_models("openai_responses")
    }

    /// Discover available models from the OpenAI `/v1/models` endpoint.
    async fn discover_models(&self) -> Result<Vec<ModelInfo>> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|k| !k.is_empty())
            .context("OpenAI Responses API model discovery requires OPENAI_API_KEY")?;

        let client = create_http_client();
        let url = format!("{}/models", RESPONSES_API_BASE);

        let response = client
            .get(&url)
            .bearer_auth(&api_key)
            .send()
            .await
            .context("Failed to fetch OpenAI models")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("OpenAI model discovery failed with {}: {}", status, body);
        }

        let data: Value = response
            .json()
            .await
            .context("Failed to parse models response")?;
        let models_array = data
            .get("data")
            .and_then(|v| v.as_array())
            .context("Invalid models response format")?;

        let mut models = Vec::new();
        for model_value in models_array {
            if let Some(model_id) = model_value.get("id").and_then(|v| v.as_str()) {
                // Only include reasoning models
                if model_id.starts_with("gpt-5") || model_id.starts_with("o") {
                    let base_models = responses_api_default_models("openai_responses");
                    if let Some(base) = base_models.iter().find(|m| m.id == model_id) {
                        models.push(base.clone());
                    } else {
                        // Unknown model, add with defaults
                        models.push(ModelInfo {
                            id: model_id.to_string(),
                            provider_id: "openai_responses".to_string(),
                            name: model_id.to_string(),
                            cost: Cost {
                                input: 1.0,
                                output: 5.0,
                            },
                            capabilities: Capabilities {
                                reasoning: true,
                                streaming: true,
                                vision: true,
                                tool_use: true,
                                thinking_levels: openai_thinking_levels_for_model(model_id),
                            },
                            context_window: 200_000,
                            max_output: Some(50_000),
                            request_multiplier: None,
                            thinking_config: None,
                        });
                    }
                }
            }
        }

        Ok(models)
    }

    async fn create_client(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        _options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        let resolved_base = base_url
            .unwrap_or(RESPONSES_API_BASE)
            .trim_end_matches('/')
            .to_string();
        let client = ResponsesApiClient::new(api_key, &resolved_base);
        tracing::info!(
            responses_endpoint = %format!("{}/responses", resolved_base),
            "OpenAI Responses API provider initialized"
        );
        Ok(Box::new(client))
    }
}

/// Client for the OpenAI Responses API.
pub struct ResponsesApiClient {
    api_key: String,
    base_url: String,
    http_client: reqwest::Client,
}

impl ResponsesApiClient {
    /// Creates a new Responses API client.
    pub fn new(api_key: &str, base_url: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            base_url: base_url.to_string(),
            http_client: create_streaming_http_client(),
        }
    }

    /// Builds the request body for the Responses API.
    pub fn build_request_body(&self, request: &ChatRequest) -> Value {
        // Map thinking config to reasoning effort
        let reasoning_effort = request
            .thinking
            .as_ref()
            .map(|t| match t.level {
                ThinkingLevel::Low => "low",
                ThinkingLevel::Medium => "medium",
                ThinkingLevel::High => "high",
                ThinkingLevel::Auto => "medium",
                ThinkingLevel::Off => "none",
            })
            .unwrap_or("medium");

        // Build input array from messages
        let input_array: Vec<Value> = request
            .messages
            .iter()
            .filter_map(|msg| {
                let role = match msg.role.as_str() {
                    "user" => "user",
                    "assistant" => "assistant",
                    "system" => "system",
                    "developer" => "developer",
                    _ => return None,
                };

                let content = match &msg.content {
                    ChatContent::Text(text) => Value::String(text.clone()),
                    ChatContent::Parts(parts) => {
                        let parts_array: Vec<Value> = parts
                            .iter()
                            .filter_map(|part| match part {
                                ContentPart::Text { text } => {
                                    Some(json!({ "type": "text", "text": text }))
                                }
                                _ => None, // Responses API doesn't support images yet
                            })
                            .collect();
                        Value::Array(parts_array)
                    }
                };

                Some(json!({
                    "role": role,
                    "content": content,
                }))
            })
            .collect();

        // Build tools array
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                })
            })
            .collect();

        let mut body = json!({
            "model": request.model,
            "input": input_array,
            "reasoning": {
                "effort": reasoning_effort,
            },
            "stream": true,
        });

        if !tools.is_empty() {
            body["tools"] = Value::Array(tools);
        }

        if let Some(max_tokens) = request.max_tokens {
            body["max_output_tokens"] = Value::Number(max_tokens.into());
        }

        if let Some(temp) = request.temperature {
            body["temperature"] = json!(temp);
        }

        if let Some(top_p) = request.top_p {
            body["top_p"] = json!(top_p);
        }

        // Include system prompt as instructions if present
        if let Some(system) = &request.system {
            body["instructions"] = Value::String(system.to_string());
        }

        // Add reasoning mode if specified in options
        if let Some(mode) = request
            .options
            .get("reasoning_mode")
            .and_then(|v| v.as_str())
        {
            body["reasoning"]["mode"] = Value::String(mode.to_string());
        }

        // Add reasoning context if specified
        if let Some(context) = request
            .options
            .get("reasoning_context")
            .and_then(|v| v.as_str())
        {
            body["reasoning"]["context"] = Value::String(context.to_string());
        }

        // Add previous_response_id if present for conversation continuity
        if let Some(prev_id) = request
            .options
            .get("previous_response_id")
            .and_then(|v| v.as_str())
        {
            body["previous_response_id"] = Value::String(prev_id.to_string());
        }

        body
    }

    /// Parses the SSE stream from the Responses API.
    pub(crate) fn parse_sse_stream(
        &self,
        response: reqwest::Response,
    ) -> Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>> {
        use futures::stream::StreamExt;

        let status = response.status();

        Box::pin(async_stream::stream! {
            // Check for error status first
            if !status.is_success() {
                let body = match response.text().await {
                    Ok(b) => b,
                    Err(e) => format!("Failed to read error body: {}", e),
                };

                // Check for 409 Conflict - retryable error
                if status == reqwest::StatusCode::CONFLICT {
                    yield StreamEvent::Error {
                        message: format!("HTTP 409 Conflict: Concurrent modification detected. Please retry the request."),
                    };
                } else {
                    yield StreamEvent::Error {
                        message: format!("HTTP {}: {}", status, body),
                    };
                }
                return;
            }

            let stream = response.bytes_stream();
            let mut buffer = String::new();
            futures::pin_mut!(stream);

            while let Some(chunk_result) = stream.next().await {
                let chunk = match chunk_result {
                    Ok(c) => c,
                    Err(e) => {
                        yield StreamEvent::Error { message: format!("Stream error: {}", e) };
                        break;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(newline_pos) = buffer.find('\n') {
                    let line = buffer[..newline_pos].to_string();
                    buffer = buffer[newline_pos + 1..].to_string();

                    let line = line.trim();
                    if line.is_empty() || !line.starts_with("data: ") {
                        continue;
                    }

                    let data = &line[6..]; // Remove "data: " prefix
                    if data == "[DONE]" {
                        yield StreamEvent::Finish { reason: FinishReason::Stop };
                        return;
                    }

                    let event: Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::warn!(error = %e, data = %data, "Failed to parse SSE event");
                            continue;
                        }
                    };

                    // Parse Responses API SSE events
                    if let Some(event_type) = event.get("type").and_then(|v| v.as_str()) {
                        match event_type {
                            "response.created" => {
                                // Response started
                            }
                            "response.in_progress" => {
                                // Response is being generated
                            }
                            "response.output_text.delta" => {
                                if let Some(delta) = event.get("delta").and_then(|v| v.as_str()) {
                                    yield StreamEvent::TextDelta { text: delta.to_string() };
                                }
                            }
                            "response.output_text.done" => {
                                // Text output complete
                            }
                            "response.reasoning_summary_text.delta" => {
                                yield StreamEvent::ReasoningStart;
                                if let Some(delta) = event.get("delta").and_then(|v| v.as_str()) {
                                    yield StreamEvent::ReasoningDelta { text: delta.to_string() };
                                }
                            }
                            "response.reasoning_summary_text.done" => {
                                yield StreamEvent::ReasoningEnd;
                            }
                            "response.function_call_arguments.delta" => {
                                if let Some(call_id) = event.get("call_id").and_then(|v| v.as_str()) {
                                    if let Some(delta) = event.get("delta").and_then(|v| v.as_str()) {
                                        yield StreamEvent::ToolCallDelta {
                                            id: call_id.to_string(),
                                            args_json: delta.to_string(),
                                        };
                                    }
                                }
                            }
                            "response.function_call_arguments.done" => {
                                if let Some(call_id) = event.get("call_id").and_then(|v| v.as_str()) {
                                    yield StreamEvent::ToolCallEnd {
                                        id: call_id.to_string(),
                                    };
                                }
                            }
                            "response.completed" => {
                                // Extract usage information
                                if let Some(usage) = event.get("response").and_then(|r| r.get("usage")) {
                                    let input_tokens = usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let output_tokens = usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);

                                    // Extract cache write tokens if present
                                    let cache_write_tokens = usage
                                        .get("cache_write_tokens")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0);

                                    if cache_write_tokens > 0 {
                                        tracing::info!(
                                            cache_write_tokens = cache_write_tokens,
                                            "OpenAI Responses API cache write tokens"
                                        );
                                    }

                                    yield StreamEvent::Usage {
                                        input_tokens,
                                        output_tokens,
                                    };
                                }

                                yield StreamEvent::Finish { reason: FinishReason::Stop };
                            }
                            "error" => {
                                if let Some(error_msg) = event.get("error").and_then(|e| e.get("message").and_then(|v| v.as_str())) {
                                    yield StreamEvent::Error { message: error_msg.to_string() };
                                }
                            }
                            _ => {
                                tracing::debug!(event_type = %event_type, "Unknown Responses API event type");
                            }
                        }
                    }
                }
            }
        })
    }
}

#[async_trait::async_trait]
impl LlmClient for ResponsesApiClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> anyhow::Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        let url = format!("{}/responses", self.base_url);
        let body = self.build_request_body(&request);

        tracing::debug!(
            url = %url,
            model = %request.model,
            "Sending OpenAI Responses API request"
        );

        // Try initial request and handle 409 Conflict with retries
        let mut attempt = 0;
        loop {
            attempt += 1;

            let response = self
                .http_client
                .post(&url)
                .bearer_auth(&self.api_key)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .context("Failed to send Responses API request")?;

            // Check for 409 Conflict
            if response.status() == reqwest::StatusCode::CONFLICT {
                let response_body = response
                    .text()
                    .await
                    .unwrap_or_else(|e| format!("Failed to read body: {}", e));

                tracing::warn!(
                    body = %response_body,
                    attempt = %attempt,
                    "OpenAI Responses API returned 409 Conflict - concurrent modification"
                );

                // Retry with exponential backoff (up to 3 total attempts)
                if attempt < 3 {
                    let delay = Duration::from_millis(100 * 2u64.pow(attempt - 1));
                    tracing::info!(attempt = %attempt, delay_ms = %delay.as_millis(), "Retrying after 409 Conflict");
                    tokio::time::sleep(delay).await;
                    continue;
                } else {
                    // Final attempt exhausted - return error
                    return Err(anyhow::anyhow!(
                        "OpenAI Responses API returned 409 Conflict after {} attempts",
                        attempt
                    ));
                }
            }

            // Success - parse the stream
            return Ok(self.parse_sse_stream(response));
        }
    }
}

/// Usage details specific to Responses API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesApiUsage {
    /// Number of input tokens.
    pub input_tokens: u64,
    /// Number of output tokens (includes reasoning tokens).
    pub output_tokens: u64,
    /// Number of reasoning tokens used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
    /// Number of tokens written to cache.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<u64>,
    /// Number of tokens read from cache.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{ChatMessage, ToolDefinition};
    use std::sync::Arc;

    #[test]
    fn test_provider_id_and_name() {
        let provider = ResponsesApiProvider;
        assert_eq!(provider.id(), "openai_responses");
        assert_eq!(provider.name(), "OpenAI Responses API");
    }

    #[test]
    fn test_default_models() {
        let provider = ResponsesApiProvider;
        let models = provider.default_models();

        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "gpt-5.6"));
        assert!(models.iter().any(|m| m.id == "o1"));

        // All models should have reasoning capability
        for model in &models {
            assert!(model.capabilities.reasoning);
        }
    }

    #[test]
    fn test_build_request_body_basic() {
        let client = ResponsesApiClient::new("test-key", "https://api.openai.com");
        let request = ChatRequest {
            model: "gpt-5.6".to_string(),
            messages: Arc::new(vec![ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("Hello".to_string()),
            }]),
            tools: Arc::new(vec![]),
            temperature: None,
            top_p: None,
            max_tokens: None,
            system: None,
            options: HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: None,
            thinking: None,
        };

        let body = client.build_request_body(&request);

        assert_eq!(body["model"], "gpt-5.6");
        assert_eq!(body["reasoning"]["effort"], "medium");
        assert!(body["stream"].as_bool().unwrap());

        let input = body["input"].as_array().unwrap();
        assert_eq!(input.len(), 1);
        assert_eq!(input[0]["role"], "user");
        assert_eq!(input[0]["content"], "Hello");
    }

    #[test]
    fn test_build_request_body_with_tools() {
        let client = ResponsesApiClient::new("test-key", "https://api.openai.com");
        let tools = vec![ToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters: json!({"type": "object"}),
        }];

        let request = ChatRequest {
            model: "gpt-5.6".to_string(),
            messages: Arc::new(vec![ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("Test".to_string()),
            }]),
            tools: Arc::new(tools),
            temperature: None,
            top_p: None,
            max_tokens: Some(1000),
            system: Some("You are helpful".into()),
            options: HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: None,
            thinking: None,
        };

        let body = client.build_request_body(&request);

        assert!(body["tools"].is_array());
        assert_eq!(body["tools"].as_array().unwrap().len(), 1);
        assert_eq!(body["max_output_tokens"], 1000);
        assert_eq!(body["instructions"], "You are helpful");
    }

    #[test]
    fn test_build_request_body_with_thinking() {
        use ragent_types::thinking::{ThinkingConfig, ThinkingLevel};

        let client = ResponsesApiClient::new("test-key", "https://api.openai.com");
        let request = ChatRequest {
            model: "gpt-5.6".to_string(),
            messages: Arc::new(vec![ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("Test".to_string()),
            }]),
            tools: Arc::new(vec![]),
            temperature: None,
            top_p: None,
            max_tokens: None,
            system: None,
            options: HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: None,
            thinking: Some(ThinkingConfig {
                enabled: true,
                level: ThinkingLevel::High,
                budget_tokens: None,
                display: None,
            }),
        };

        let body = client.build_request_body(&request);

        assert_eq!(body["reasoning"]["effort"], "high");
    }
}
