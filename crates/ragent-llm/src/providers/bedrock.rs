//! Amazon Bedrock provider implementation.
//!
//! Implements the [`Provider`] trait for Amazon Bedrock, supporting both
//! Anthropic Claude models (via the Bedrock Messages API) and non-Anthropic
//! models (via the Bedrock Converse API). Authentication uses AWS Signature
//! Version 4 rather than static API keys.
//!
//! # Configuration
//!
//! Configure via `ragent.json`:
//! ```jsonc
//! {
//!   "provider": {
//!     "bedrock": {
//!       "env": ["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY"],
//!       "options": {
//!         "region": "us-east-1",
//!         "profile": "my-dev-profile"
//!       }
//!     }
//!   }
//! }
//! ```
//!
//! Or use environment variables exclusively:
//! ```bash
//! export AWS_ACCESS_KEY_ID="AKIAIOSFODNN7EXAMPLE"
//! export AWS_SECRET_ACCESS_KEY="wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
//! export AWS_REGION="eu-west-1"
//! ragent --model bedrock/anthropic.claude-sonnet-4-20250514-v1:0
//! ```

use anyhow::{Context, Result, bail};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::pin::Pin;

use super::bedrock_credentials::{AwsCredentials, resolve_aws_credentials};

use super::thinking::{
    anthropic_thinking_levels_for_model, anthropic_thinking_payload_from_request,
    request_uses_unsupported_anthropic_display,
};
use crate::event::FinishReason;
use crate::llm::{ChatContent, ChatRequest, ContentPart, LlmClient, StreamEvent, ToolDefinition};
use crate::{ModelInfo, Provider};
use ragent_config::{Capabilities, Cost};

// ---------------------------------------------------------------------------
// Bedrock model ID helpers
// ---------------------------------------------------------------------------

/// Returns `true` if the model ID indicates an Anthropic Claude model.
///
/// Anthropic models on Bedrock have IDs starting with `anthropic.claude`.
fn is_anthropic_model(model_id: &str) -> bool {
    model_id.starts_with("anthropic.claude")
}

/// Strips the `@bedrock` suffix from a model ID (FR-013).
///
/// Example: `claude-sonnet-4-20250514@bedrock` → `claude-sonnet-4-20250514`
fn strip_bedrock_suffix(model_id: &str) -> String {
    model_id
        .split_once('@')
        .map(|(base, _)| base)
        .unwrap_or(model_id)
        .to_string()
}

/// Maps a user-friendly model ID to the Bedrock model ID format.
///
/// The user can specify either:
/// - The full Bedrock model ID: `anthropic.claude-sonnet-4-20250514-v1:0`
/// - A short alias: `claude-sonnet-4-20250514` (mapped to the Bedrock ID)
/// - With `@bedrock` suffix: `claude-sonnet-4-20250514@bedrock`
fn resolve_bedrock_model_id(model_id: &str) -> String {
    let stripped = strip_bedrock_suffix(model_id);

    // If already a full Bedrock model ID, return as-is
    if stripped.contains('.') {
        return stripped;
    }

    // Map short aliases to Bedrock model IDs
    match stripped.as_str() {
        "claude-sonnet-4-20250514" => "anthropic.claude-sonnet-4-20250514-v1:0".to_string(),
        "claude-opus-4-20250514" => "anthropic.claude-opus-4-20250514-v1:0".to_string(),
        "claude-3-5-haiku-20241022" => "anthropic.claude-3-5-haiku-20241022-v1:0".to_string(),
        "claude-3-5-sonnet-20241022" => "anthropic.claude-3-5-sonnet-20241022-v1:0".to_string(),
        "claude-3-haiku-20240307" => "anthropic.claude-3-haiku-20240307-v1:0".to_string(),
        "claude-3-opus-20240229" => "anthropic.claude-3-opus-20240229-v1:0".to_string(),
        "nova-pro" => "amazon.nova-pro-v1:0".to_string(),
        "nova-lite" => "amazon.nova-lite-v1:0".to_string(),
        "nova-micro" => "amazon.nova-micro-v1:0".to_string(),
        "llama4-maverick" => "meta.llama4-maverick-17b-instruct-v1:0".to_string(),
        "mistral-large" => "mistral.mistral-large-2407-v1:0".to_string(),
        _ => stripped,
    }
}

// ---------------------------------------------------------------------------
// BedrockProvider
// ---------------------------------------------------------------------------

/// Provider implementation for Amazon Bedrock.
pub struct BedrockProvider;

#[async_trait::async_trait]
impl Provider for BedrockProvider {
    /// Returns `"bedrock"` (FR-008).
    fn id(&self) -> &'static str {
        "bedrock"
    }

    /// Returns `"Amazon Bedrock"` (FR-008).
    fn name(&self) -> &'static str {
        "Amazon Bedrock"
    }

    /// Returns the default Bedrock model catalog (FR-010).
    fn default_models(&self) -> Vec<ModelInfo> {
        bedrock_default_models()
    }

    /// Creates a Bedrock client for the given model.
    ///
    /// Resolves AWS credentials, determines the API type (Anthropic Messages
    /// or Converse) based on the model ID, and returns the appropriate client.
    ///
    /// # Errors
    ///
    /// Returns an error if AWS credentials cannot be resolved (FR-002).
    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        let credentials = resolve_aws_credentials(options)?;

        // Determine model ID from options (injected by the session processor)
        let raw_model_id = options
            .get("model_id")
            .and_then(Value::as_str)
            .unwrap_or("anthropic.claude-sonnet-4-20250514-v1:0");
        let model_id = resolve_bedrock_model_id(raw_model_id);

        // FR-007: Custom endpoint URL for VPC endpoints
        let endpoint_url = options
            .get("endpoint_url")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(String::from);

        let base_url = build_bedrock_base_url(&credentials.region, endpoint_url.as_deref());

        if is_anthropic_model(&model_id) {
            tracing::info!(
                chat_endpoint = %format!("{base_url}/model/{model_id}/invoke-with-response-stream"),
                model_id = %model_id,
                api = "anthropic_messages",
                "Bedrock (Anthropic) provider connected"
            );
            Ok(Box::new(BedrockAnthropicClient {
                credentials,
                base_url,
                model_id,
                http: crate::provider::http_client::create_streaming_http_client(),
            }))
        } else {
            tracing::info!(
                chat_endpoint = %format!("{base_url}/model/{model_id}/converse-stream"),
                model_id = %model_id,
                api = "converse",
                "Bedrock (Converse) provider connected"
            );
            Ok(Box::new(BedrockConverseClient {
                credentials,
                base_url,
                model_id,
                http: crate::provider::http_client::create_streaming_http_client(),
            }))
        }
    }
}

/// Constructs the Bedrock API base URL for a given region and optional custom endpoint.
///
/// Default: `https://bedrock.{region}.amazonaws.com`
/// Custom: uses the provided endpoint_url directly (FR-007).
fn build_bedrock_base_url(region: &str, endpoint_url: Option<&str>) -> String {
    if let Some(url) = endpoint_url {
        return url.trim_end_matches('/').to_string();
    }
    format!("https://bedrock.{region}.amazonaws.com")
}

/// Returns the default Bedrock model catalog (FR-010).
pub fn bedrock_default_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "anthropic.claude-sonnet-4-20250514-v1:0".to_string(),
            provider_id: "bedrock".to_string(),
            name: "Bedrock Claude Sonnet 4".to_string(),
            cost: Cost {
                input: 3.0,
                output: 15.0,
            },
            capabilities: Capabilities {
                reasoning: true,
                streaming: true,
                vision: true,
                tool_use: true,
                thinking_levels: anthropic_thinking_levels_for_model("claude-sonnet-4-20250514"),
            },
            context_window: 200_000,
            max_output: Some(8_192),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "anthropic.claude-opus-4-20250514-v1:0".to_string(),
            provider_id: "bedrock".to_string(),
            name: "Bedrock Claude Opus 4".to_string(),
            cost: Cost {
                input: 15.0,
                output: 75.0,
            },
            capabilities: Capabilities {
                reasoning: true,
                streaming: true,
                vision: true,
                tool_use: true,
                thinking_levels: anthropic_thinking_levels_for_model("claude-opus-4-20250514"),
            },
            context_window: 200_000,
            max_output: Some(32_000),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "anthropic.claude-3-5-haiku-20241022-v1:0".to_string(),
            provider_id: "bedrock".to_string(),
            name: "Bedrock Claude 3.5 Haiku".to_string(),
            cost: Cost {
                input: 0.8,
                output: 4.0,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: true,
                tool_use: true,
                thinking_levels: Vec::new(),
            },
            context_window: 200_000,
            max_output: Some(8_192),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "anthropic.claude-3-5-sonnet-20241022-v1:0".to_string(),
            provider_id: "bedrock".to_string(),
            name: "Bedrock Claude 3.5 Sonnet".to_string(),
            cost: Cost {
                input: 3.0,
                output: 15.0,
            },
            capabilities: Capabilities {
                reasoning: true,
                streaming: true,
                vision: true,
                tool_use: true,
                thinking_levels: anthropic_thinking_levels_for_model("claude-3-5-sonnet-20241022"),
            },
            context_window: 200_000,
            max_output: Some(8_192),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "amazon.nova-pro-v1:0".to_string(),
            provider_id: "bedrock".to_string(),
            name: "Bedrock Amazon Nova Pro".to_string(),
            cost: Cost {
                input: 0.8,
                output: 3.2,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: true,
                tool_use: true,
                thinking_levels: Vec::new(),
            },
            context_window: 300_000,
            max_output: Some(5_000),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "amazon.nova-lite-v1:0".to_string(),
            provider_id: "bedrock".to_string(),
            name: "Bedrock Amazon Nova Lite".to_string(),
            cost: Cost {
                input: 0.06,
                output: 0.24,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: true,
                tool_use: true,
                thinking_levels: Vec::new(),
            },
            context_window: 300_000,
            max_output: Some(5_000),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "amazon.nova-micro-v1:0".to_string(),
            provider_id: "bedrock".to_string(),
            name: "Bedrock Amazon Nova Micro".to_string(),
            cost: Cost {
                input: 0.035,
                output: 0.14,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: Vec::new(),
            },
            context_window: 128_000,
            max_output: Some(5_000),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "meta.llama4-maverick-17b-instruct-v1:0".to_string(),
            provider_id: "bedrock".to_string(),
            name: "Bedrock Llama 4 Maverick".to_string(),
            cost: Cost {
                input: 0.24,
                output: 0.24,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: true,
                tool_use: true,
                thinking_levels: Vec::new(),
            },
            context_window: 128_000,
            max_output: Some(4_096),
            request_multiplier: None,
            thinking_config: None,
        },
        ModelInfo {
            id: "mistral.mistral-large-2407-v1:0".to_string(),
            provider_id: "bedrock".to_string(),
            name: "Bedrock Mistral Large".to_string(),
            cost: Cost {
                input: 2.0,
                output: 6.0,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: Vec::new(),
            },
            context_window: 128_000,
            max_output: Some(8_192),
            request_multiplier: None,
            thinking_config: None,
        },
    ]
}

// ---------------------------------------------------------------------------
// BedrockAnthropicClient — Messages API for Claude models (FR-011)
// ---------------------------------------------------------------------------

/// Extract the MIME type from a `data:<mime>;base64,<data>` URI.
fn extract_mime_from_data_uri(uri: &str) -> Option<&str> {
    uri.strip_prefix("data:").and_then(|s| s.split(';').next())
}

/// Extract the raw base64 payload from a `data:<mime>;base64,<data>` URI.
fn extract_base64_from_data_uri(uri: &str) -> Option<&str> {
    uri.find(",base64,")
        .map(|i| &uri[i + 8..])
        .or_else(|| uri.find(',').map(|i| &uri[i + 1..]))
}

/// HTTP client for Anthropic models on Bedrock using the Messages API.
///
/// Routes to `/model/{model_id}/invoke-with-response-stream` (FR-011) and
/// signs requests with AWS SigV4 instead of the `x-api-key` header.
pub struct BedrockAnthropicClient {
    credentials: AwsCredentials,
    base_url: String,
    model_id: String,
    http: reqwest::Client,
}

impl BedrockAnthropicClient {
    /// Builds the Anthropic Messages request body for Bedrock.
    ///
    /// This is the same format as the direct Anthropic Messages API, but
    /// sent to the Bedrock endpoint.
    fn build_request_body(&self, request: &ChatRequest) -> Value {
        let mut messages = Vec::new();

        for msg in request.messages.iter() {
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
                            ContentPart::ToolResult { tool_use_id, content } => json!({
                                "type": "tool_result",
                                "tool_use_id": tool_use_id,
                                "content": content
                            }),
                            // FR-024: Image support for Anthropic models
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
            "anthropic_version": "bedrock-2023-06-01",
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

        // FR-020: Tool use support
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

        // FR-025: Thinking/reasoning support for Claude models
        if request_uses_unsupported_anthropic_display(request) {
            tracing::warn!(
                model = %request.model,
                "Bedrock Anthropic: summarized thinking display not supported yet; falling back to standard thinking output"
            );
        }

        if let Some(thinking) = anthropic_thinking_payload_from_request(request) {
            body["thinking"] = thinking;
        }

        body
    }
}

#[async_trait::async_trait]
impl LlmClient for BedrockAnthropicClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        // FR-011: Route to Bedrock Messages API
        let url = format!(
            "{}/model/{}/invoke-with-response-stream",
            self.base_url, self.model_id
        );

        let body = self.build_request_body(&request);
        let body_bytes = serde_json::to_vec(&body)
            .context("Failed to serialize Bedrock Anthropic request body")?;

        // Sign the request with SigV4
        let mut headers: Vec<(String, String)> = Vec::new();
        headers.push(("content-type".to_string(), "application/json".to_string()));
        headers.push(("anthropic-version".to_string(), "2023-06-01".to_string()));
        headers.push(("accept".to_string(), "text/event-stream".to_string()));

        super::bedrock_sigv4::sign_request(
            "POST",
            &url,
            &mut headers,
            &body_bytes,
            &self.credentials,
        )
        .map_err(|e| anyhow::anyhow!("Failed to sign Bedrock request: {e}"))?;

        // Build the HTTP request with signed headers
        let mut request_builder = self.http.post(&url);
        for (key, value) in &headers {
            request_builder = request_builder.header(key.as_str(), value.as_str());
        }

        let response = request_builder
            .body(body_bytes)
            .send()
            .await
            .inspect_err(|e| {
                tracing::warn!(url = %url, error = %e, "Bedrock Anthropic chat request failed");
            })
            .with_context(|| format!("Failed to send request to Bedrock at {url}"))?;

        // FR-027, FR-028, FR-029: Error handling
        if !response.status().is_success() {
            return handle_bedrock_error(response).await;
        }

        let stream = response.bytes_stream();

        // Parse the Anthropic SSE stream — same format as direct Anthropic API
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

                        // Check for Bedrock-level errors in the event
                        if let Some(error) = parsed.get("error") {
                            let message = error
                                .get("message")
                                .and_then(Value::as_str)
                                .unwrap_or("Unknown Bedrock error");
                            yield StreamEvent::Error { message: message.to_string() };
                            continue;
                        }

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
                                if let Some((id, _)) = tool_call_args.iter().last() {
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

// ---------------------------------------------------------------------------
// BedrockConverseClient — Converse API for non-Anthropic models (FR-012)
// ---------------------------------------------------------------------------

/// HTTP client for non-Anthropic models on Bedrock using the Converse API.
///
/// Routes to `/model/{model_id}/converse-stream` (FR-012) and translates
/// ragent `ChatRequest` into the Bedrock Converse request format.
pub struct BedrockConverseClient {
    credentials: AwsCredentials,
    base_url: String,
    model_id: String,
    http: reqwest::Client,
}

impl BedrockConverseClient {
    /// Builds the Bedrock Converse API request body.
    ///
    /// Translates `ChatRequest` into the Converse format, including tool
    /// definitions (FR-021) and image content (FR-024).
    fn build_request_body(&self, request: &ChatRequest) -> Value {
        let mut messages = Vec::new();

        // Add system prompt as a separate top-level field
        // (Converse API uses `system` not a system message)

        for msg in request.messages.iter() {
            if msg.role == "system" {
                continue; // Handled separately
            }

            let role = match msg.role.as_str() {
                "user" => "user",
                "assistant" => "assistant",
                _ => continue, // Skip unknown roles
            };

            let content = match &msg.content {
                ChatContent::Text(text) => {
                    vec![json!({
                        "text": text
                    })]
                }
                ChatContent::Parts(parts) => {
                    parts
                        .iter()
                        .map(|part| match part {
                            ContentPart::Text { text } => json!({
                                "text": text
                            }),
                            // FR-024: Image support for Converse API
                            ContentPart::ImageUrl { url } => {
                                let mime = extract_mime_from_data_uri(url).unwrap_or("image/png");
                                let data =
                                    extract_base64_from_data_uri(url).unwrap_or(url.as_str());
                                json!({
                                    "image": {
                                        "format": mime_to_converse_format(mime),
                                        "source": {
                                            "bytes": data
                                        }
                                    }
                                })
                            }
                            // FR-021: Tool use in Converse format
                            ContentPart::ToolUse { id, name, input } => json!({
                                "toolUse": {
                                    "toolUseId": id,
                                    "name": name,
                                    "input": input
                                }
                            }),
                            ContentPart::ToolResult {
                                tool_use_id,
                                content,
                            } => json!({
                                "toolResult": {
                                    "toolUseId": tool_use_id,
                                    "content": [{
                                        "text": content
                                    }],
                                    "status": "success"
                                }
                            }),
                        })
                        .collect()
                }
            };

            messages.push(json!({
                "role": role,
                "content": content
            }));
        }

        let mut body = json!({
            "messages": messages,
        });

        // System prompt
        if let Some(system) = &request.system {
            body["system"] = json!([{
                "text": system
            }]);
        }

        // Inference configuration
        let mut inference_config = json!({});
        if let Some(temp) = request.temperature {
            inference_config["temperature"] = json!(temp);
        }
        if let Some(top_p) = request.top_p {
            inference_config["topP"] = json!(top_p);
        }
        if let Some(max_tokens) = request.max_tokens {
            inference_config["maxTokens"] = json!(max_tokens);
        }
        if inference_config
            .as_object()
            .map(|o| !o.is_empty())
            .unwrap_or(false)
        {
            body["inferenceConfig"] = inference_config;
        }

        // FR-021: Tool definitions in Converse format
        if !request.tools.is_empty() {
            let tool_config = build_converse_tool_config(&request.tools);
            body["toolConfig"] = tool_config;
        }

        body
    }
}

/// Builds the `toolConfig` object for the Converse API (FR-021).
fn build_converse_tool_config(tools: &[ToolDefinition]) -> Value {
    let tool_specs: Vec<Value> = tools
        .iter()
        .map(|t| {
            json!({
                "toolSpec": {
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": {
                        "json": t.parameters
                    }
                }
            })
        })
        .collect();

    json!({
        "tools": tool_specs
    })
}

/// Converts a MIME type to the Converse API image format string.
///
/// E.g. `image/png` → `png`, `image/jpeg` → `jpeg`
fn mime_to_converse_format(mime: &str) -> &str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpeg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "png", // Default
    }
}

#[async_trait::async_trait]
impl LlmClient for BedrockConverseClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        // FR-012: Route to Bedrock Converse API
        let url = format!("{}/model/{}/converse-stream", self.base_url, self.model_id);

        let body = self.build_request_body(&request);
        let body_bytes = serde_json::to_vec(&body)
            .context("Failed to serialize Bedrock Converse request body")?;

        // Sign the request with SigV4
        let mut headers: Vec<(String, String)> = Vec::new();
        headers.push(("content-type".to_string(), "application/json".to_string()));
        headers.push(("accept".to_string(), "text/event-stream".to_string()));

        super::bedrock_sigv4::sign_request(
            "POST",
            &url,
            &mut headers,
            &body_bytes,
            &self.credentials,
        )
        .map_err(|e| anyhow::anyhow!("Failed to sign Bedrock request: {e}"))?;

        let mut request_builder = self.http.post(&url);
        for (key, value) in &headers {
            request_builder = request_builder.header(key.as_str(), value.as_str());
        }

        let response = request_builder
            .body(body_bytes)
            .send()
            .await
            .inspect_err(|e| {
                tracing::warn!(url = %url, error = %e, "Bedrock Converse chat request failed");
            })
            .with_context(|| format!("Failed to send request to Bedrock Converse at {url}"))?;

        // Error handling
        if !response.status().is_success() {
            return handle_bedrock_error(response).await;
        }

        let stream = response.bytes_stream();

        // Parse the Converse API event stream
        // The Converse API uses a different event format than Anthropic Messages
        let event_stream = async_stream::stream! {
            let mut buffer = String::new();
            let mut current_event_type = String::new();
            let mut active_tool_call_id = String::new();
            let _active_tool_call_name = String::new();

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

                        // Check for Bedrock-level errors
                        if let Some(error) = parsed.get("error") {
                            let message = error
                                .get("message")
                                .and_then(Value::as_str)
                                .unwrap_or("Unknown Bedrock error");
                            yield StreamEvent::Error { message: message.to_string() };
                            continue;
                        }

                        match current_event_type.as_str() {
                            "contentBlockStart" => {
                                if let Some(start) = parsed.get("start") {
                                    if let Some(tool_use) = start.get("toolUse") {
                                        active_tool_call_id = tool_use["toolUseId"].as_str().unwrap_or("").to_string();
                                        let name = tool_use["name"].as_str().unwrap_or("").to_string();
                                                                              yield StreamEvent::ToolCallStart {
                                                                                      id: active_tool_call_id.clone(),
                                                                                      name,
                                                                                  };                                    }
                                }
                            }
                            "contentBlockDelta" => {
                                if let Some(delta) = parsed.get("delta") {
                                    if let Some(text_delta) = delta.get("text") {
                                        if let Some(text) = text_delta.as_str() {
                                            yield StreamEvent::TextDelta { text: text.to_string() };
                                        }
                                    }
                                    if let Some(tool_delta) = delta.get("toolUse") {
                                        if let Some(input) = tool_delta.get("input") {
                                            if let Some(json_str) = input.as_str() {
                                                yield StreamEvent::ToolCallDelta {
                                                    id: active_tool_call_id.clone(),
                                                    args_json: json_str.to_string(),
                                                };
                                            }
                                        }
                                    }
                                }
                            }
                            "contentBlockStop" => {
                                if !active_tool_call_id.is_empty() {
                                    yield StreamEvent::ToolCallEnd {
                                        id: active_tool_call_id.clone(),
                                    };
                                    active_tool_call_id = String::new();
                                }
                            }
                            "messageStop" => {
                                if let Some(stop_reason) = parsed.get("stopReason").and_then(Value::as_str) {
                                    let reason = match stop_reason {
                                        "tool_use" => FinishReason::ToolUse,
                                        "max_tokens" => FinishReason::Length,
                                        _ => FinishReason::Stop,
                                    };
                                    yield StreamEvent::Finish { reason };
                                } else {
                                    yield StreamEvent::Finish { reason: FinishReason::Stop };
                                }
                            }
                            "messageStart" => {
                                if let Some(usage) = parsed.get("usage") {
                                    let input_tokens = usage["inputTokens"].as_u64().unwrap_or(0);
                                    yield StreamEvent::Usage {
                                        input_tokens,
                                        output_tokens: 0,
                                    };
                                }
                            }
                            "metadata" => {
                                if let Some(usage) = parsed.get("usage") {
                                    let output_tokens = usage["outputTokens"].as_u64().unwrap_or(0);
                                    yield StreamEvent::Usage {
                                        input_tokens: 0,
                                        output_tokens,
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

// ---------------------------------------------------------------------------
// Error handling (FR-027, FR-028, FR-029, FR-030)
// ---------------------------------------------------------------------------

/// Handles a non-success HTTP response from the Bedrock API.
///
/// Maps Bedrock-specific error types to actionable error messages:
/// - `ThrottlingException` → retryable (FR-027)
/// - `ValidationException` → descriptive (FR-028)
/// - `AccessDeniedException` → permissions (FR-029)
/// - Never exposes raw AWS keys in errors (FR-030)
async fn handle_bedrock_error(
    response: reqwest::Response,
) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
    let status = response.status();
    let body_text = response.text().await.unwrap_or_default();

    // Try to parse as a Bedrock error JSON
    let error_json: Value = serde_json::from_str(&body_text).unwrap_or_default();

    let error_type = error_json
        .get("error")
        .and_then(|e| e.get("type"))
        .or_else(|| error_json.get("__type"))
        .and_then(Value::as_str)
        .unwrap_or("Unknown");

    let message = error_json
        .get("message")
        .or_else(|| error_json.get("error").and_then(|e| e.get("message")))
        .and_then(Value::as_str)
        .unwrap_or(&body_text);

    match error_type {
        // FR-027: Throttling → retryable error
        "ThrottlingException" | "ServiceUnavailableException" => {
            tracing::warn!(status = %status, error_type = %error_type, message = %message, "Bedrock throttling error");
            bail!(
                "Bedrock rate limited ({error_type}): {message}. Please retry after a short wait."
            );
        }
        // FR-029: Access denied → permissions error
        "AccessDeniedException" => {
            tracing::error!(status = %status, message = %message, "Bedrock access denied");
            bail!(
                "Bedrock access denied: {message}. \
                 Ensure your AWS principal has 'bedrock:InvokeModel' permission for the requested model."
            );
        }
        // FR-028: Validation → descriptive error
        "ValidationException" => {
            tracing::warn!(status = %status, message = %message, "Bedrock validation error");
            bail!("Bedrock validation error: {message}");
        }
        _ => {
            tracing::error!(status = %status, error_type = %error_type, message = %message, "Bedrock API error");
            bail!("Bedrock API error ({status}): [{error_type}] {message}");
        }
    }
}

// ---------------------------------------------------------------------------
// Model discovery (FR-022, FR-023)
// ---------------------------------------------------------------------------

/// Response from the Bedrock `ListFoundationModels` API.
#[derive(Debug, Deserialize)]
struct ListFoundationModelsResponse {
    #[serde(default, rename = "modelSummaries")]
    model_summaries: Vec<BedrockModelSummary>,
}

/// A single model from the `ListFoundationModels` response.
#[derive(Debug, Deserialize)]
struct BedrockModelSummary {
    #[serde(default, rename = "modelId")]
    model_id: String,
    #[serde(default, rename = "modelName")]
    model_name: String,
    #[serde(default, rename = "providerName")]
    _provider_name: String,
    #[serde(default)]
    streaming: Option<bool>,
    #[serde(default, rename = "inputModalities")]
    input_modalities: Vec<String>,
    #[serde(default, rename = "outputModalities")]
    _output_modalities: Vec<String>,
    #[serde(default, rename = "responseStreamingSupported")]
    response_streaming_supported: Option<bool>,
}

/// Discovers available models from the Bedrock `ListFoundationModels` API (FR-022).
///
/// Falls back to the static default catalog on error (FR-023).
pub async fn discover_bedrock_models(
    credentials: &AwsCredentials,
    endpoint_url: Option<&str>,
) -> Vec<ModelInfo> {
    let base_url = build_bedrock_base_url(&credentials.region, endpoint_url);
    let url = format!("{base_url}/foundation-models");

    // Sign the GET request
    let mut headers: Vec<(String, String)> = Vec::new();
    headers.push(("accept".to_string(), "application/json".to_string()));

    if let Err(e) = super::bedrock_sigv4::sign_request("GET", &url, &mut headers, b"", credentials)
    {
        tracing::warn!(error = %e, "Failed to sign Bedrock model discovery request; using static catalog");
        return bedrock_default_models();
    }

    let http = crate::provider::http_client::create_http_client();

    let mut request_builder = http.get(&url);
    for (key, value) in &headers {
        request_builder = request_builder.header(key.as_str(), value.as_str());
    }

    match request_builder
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            match response.json::<ListFoundationModelsResponse>().await {
                Ok(data) => {
                    let models: Vec<ModelInfo> = data
                        .model_summaries
                        .into_iter()
                        .filter(|m| {
                            // Only include models that support streaming
                            m.streaming.unwrap_or(false)
                                || m.response_streaming_supported.unwrap_or(false)
                        })
                        .map(|m| {
                            let supports_vision = m.input_modalities.contains(&"Image".to_string());
                            let is_anthropic = m.model_id.starts_with("anthropic.claude");

                            ModelInfo {
                                id: m.model_id.clone(),
                                provider_id: "bedrock".to_string(),
                                name: format!("Bedrock {}", m.model_name),
                                cost: Cost {
                                    input: 0.0,
                                    output: 0.0,
                                },
                                capabilities: Capabilities {
                                    reasoning: is_anthropic,
                                    streaming: true,
                                    vision: supports_vision,
                                    tool_use: true,
                                    thinking_levels: if is_anthropic {
                                        anthropic_thinking_levels_for_model(&m.model_id)
                                    } else {
                                        Vec::new()
                                    },
                                },
                                context_window: 128_000,
                                max_output: None,
                                request_multiplier: None,
                                thinking_config: None,
                            }
                        })
                        .collect();

                    if models.is_empty() {
                        tracing::warn!("Bedrock returned empty model list; using static catalog");
                        bedrock_default_models()
                    } else {
                        models
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to parse Bedrock models response; using static catalog");
                    bedrock_default_models()
                }
            }
        }
        Ok(response) => {
            let status = response.status();
            tracing::warn!(status = %status, "Bedrock model discovery returned non-success; using static catalog");
            bedrock_default_models()
        }
        Err(e) => {
            tracing::warn!(error = %e, "Bedrock model discovery request failed; using static catalog");
            bedrock_default_models()
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bedrock_provider_id_and_name() {
        let provider = BedrockProvider;
        assert_eq!(provider.id(), "bedrock");
        assert_eq!(provider.name(), "Amazon Bedrock");
    }

    #[test]
    fn test_bedrock_default_models_non_empty() {
        let models = bedrock_default_models();
        assert!(!models.is_empty());
        assert!(models.len() >= 8, "Expected at least 8 default models");

        // All models should have bedrock provider_id
        for model in &models {
            assert_eq!(model.provider_id, "bedrock");
        }
    }

    #[test]
    fn test_is_anthropic_model() {
        assert!(is_anthropic_model(
            "anthropic.claude-sonnet-4-20250514-v1:0"
        ));
        assert!(is_anthropic_model(
            "anthropic.claude-3-5-haiku-20241022-v1:0"
        ));
        assert!(!is_anthropic_model("amazon.nova-pro-v1:0"));
        assert!(!is_anthropic_model(
            "meta.llama4-maverick-17b-instruct-v1:0"
        ));
        assert!(!is_anthropic_model("mistral.mistral-large-2407-v1:0"));
    }

    #[test]
    fn test_strip_bedrock_suffix() {
        assert_eq!(
            strip_bedrock_suffix("claude-sonnet-4-20250514@bedrock"),
            "claude-sonnet-4-20250514"
        );
        assert_eq!(
            strip_bedrock_suffix("anthropic.claude-sonnet-4-20250514-v1:0"),
            "anthropic.claude-sonnet-4-20250514-v1:0"
        );
        assert_eq!(strip_bedrock_suffix("no-suffix"), "no-suffix");
    }

    #[test]
    fn test_resolve_bedrock_model_id_short_aliases() {
        assert_eq!(
            resolve_bedrock_model_id("claude-sonnet-4-20250514"),
            "anthropic.claude-sonnet-4-20250514-v1:0"
        );
        assert_eq!(
            resolve_bedrock_model_id("claude-opus-4-20250514"),
            "anthropic.claude-opus-4-20250514-v1:0"
        );
        assert_eq!(resolve_bedrock_model_id("nova-pro"), "amazon.nova-pro-v1:0");
        assert_eq!(
            resolve_bedrock_model_id("nova-lite"),
            "amazon.nova-lite-v1:0"
        );
        assert_eq!(
            resolve_bedrock_model_id("nova-micro"),
            "amazon.nova-micro-v1:0"
        );
    }

    #[test]
    fn test_resolve_bedrock_model_id_full_id_passthrough() {
        // Full Bedrock model IDs should pass through unchanged
        assert_eq!(
            resolve_bedrock_model_id("anthropic.claude-sonnet-4-20250514-v1:0"),
            "anthropic.claude-sonnet-4-20250514-v1:0"
        );
    }

    #[test]
    fn test_resolve_bedrock_model_id_with_suffix() {
        assert_eq!(
            resolve_bedrock_model_id("claude-sonnet-4-20250514@bedrock"),
            "anthropic.claude-sonnet-4-20250514-v1:0"
        );
    }

    #[test]
    fn test_build_bedrock_base_url_default() {
        let url = build_bedrock_base_url("us-east-1", None);
        assert_eq!(url, "https://bedrock.us-east-1.amazonaws.com");
    }

    #[test]
    fn test_build_bedrock_base_url_custom_endpoint() {
        let url =
            build_bedrock_base_url("us-east-1", Some("https://my-vpc-endbedrock.example.com"));
        assert_eq!(url, "https://my-vpc-endbedrock.example.com");
    }

    #[test]
    fn test_build_bedrock_base_url_trailing_slash() {
        let url = build_bedrock_base_url("eu-west-1", Some("https://endpoint.example.com/"));
        assert_eq!(url, "https://endpoint.example.com");
    }

    #[test]
    fn test_converse_tool_config() {
        let tools = vec![ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get weather for a location".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "location": { "type": "string" }
                }
            }),
        }];

        let config = build_converse_tool_config(&tools);

        // Should have tools array with toolSpec
        let tool_specs = config["tools"].as_array().unwrap();
        assert_eq!(tool_specs.len(), 1);
        assert_eq!(tool_specs[0]["toolSpec"]["name"], "get_weather");
        assert_eq!(
            tool_specs[0]["toolSpec"]["description"],
            "Get weather for a location"
        );
        // inputSchema should wrap parameters in "json" key
        assert!(tool_specs[0]["toolSpec"]["inputSchema"]["json"].is_object());
    }

    #[test]
    fn test_mime_to_converse_format() {
        assert_eq!(mime_to_converse_format("image/png"), "png");
        assert_eq!(mime_to_converse_format("image/jpeg"), "jpeg");
        assert_eq!(mime_to_converse_format("image/gif"), "gif");
        assert_eq!(mime_to_converse_format("image/webp"), "webp");
        assert_eq!(mime_to_converse_format("image/unknown"), "png"); // Default
    }

    #[test]
    fn test_bedrock_anthropic_request_body() {
        let creds = AwsCredentials {
            access_key: "test".to_string(),
            secret_key: "test".to_string(),
            session_token: None,
            region: "us-east-1".to_string(),
        };
        let client = BedrockAnthropicClient {
            credentials: creds,
            base_url: "https://bedrock.us-east-1.amazonaws.com".to_string(),
            model_id: "anthropic.claude-sonnet-4-20250514-v1:0".to_string(),
            http: crate::provider::http_client::create_streaming_http_client(),
        };

        let request = ChatRequest {
            model: "anthropic.claude-sonnet-4-20250514-v1:0".to_string(),
            messages: std::sync::Arc::new(vec![crate::llm::ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("Hello".to_string()),
            }]),
            tools: std::sync::Arc::new(vec![]),
            temperature: None,
            top_p: None,
            max_tokens: Some(1024),
            system: Some("You are helpful".to_string()),
            options: HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: None,
            thinking: None,
        };

        let body = client.build_request_body(&request);

        // Should have anthropic_version for Bedrock
        assert_eq!(body["anthropic_version"], "bedrock-2023-06-01");
        // Should have system prompt
        assert_eq!(body["system"], "You are helpful");
        // Should have max_tokens
        assert_eq!(body["max_tokens"], 1024);
        // Should have stream enabled
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn test_bedrock_converse_request_body() {
        let creds = AwsCredentials {
            access_key: "test".to_string(),
            secret_key: "test".to_string(),
            session_token: None,
            region: "us-east-1".to_string(),
        };
        let client = BedrockConverseClient {
            credentials: creds,
            base_url: "https://bedrock.us-east-1.amazonaws.com".to_string(),
            model_id: "amazon.nova-pro-v1:0".to_string(),
            http: crate::provider::http_client::create_streaming_http_client(),
        };

        let request = ChatRequest {
            model: "amazon.nova-pro-v1:0".to_string(),
            messages: std::sync::Arc::new(vec![crate::llm::ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("Hello".to_string()),
            }]),
            tools: std::sync::Arc::new(vec![ToolDefinition {
                name: "get_time".to_string(),
                description: "Get current time".to_string(),
                parameters: json!({"type": "object"}),
            }]),
            temperature: Some(0.7),
            top_p: None,
            max_tokens: Some(2048),
            system: Some("Be concise".to_string()),
            options: HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: None,
            thinking: None,
        };

        let body = client.build_request_body(&request);

        // Should have system as array with text
        let system = body["system"].as_array().unwrap();
        assert_eq!(system[0]["text"], "Be concise");

        // Should have inferenceConfig
        // Temperature is f32, JSON float may have precision loss
        let temp = body["inferenceConfig"]["temperature"].as_f64().unwrap();
        assert!((temp - 0.7).abs() < 0.01, "Expected ~0.7, got {temp}");
        assert_eq!(body["inferenceConfig"]["maxTokens"], 2048);

        // Should have toolConfig with toolSpec
        let tools_arr = body["toolConfig"]["tools"].as_array().unwrap();
        assert_eq!(tools_arr.len(), 1);
        assert_eq!(tools_arr[0]["toolSpec"]["name"], "get_time");
    }

    #[test]
    fn test_no_api_key_header_in_signed_request() {
        // FR-016: Verify that SigV4 signing does not produce x-api-key or Bearer auth
        let creds = AwsCredentials {
            access_key: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: None,
            region: "us-east-1".to_string(),
        };

        let mut headers: Vec<(String, String)> = Vec::new();
        super::super::bedrock_sigv4::sign_request(
            "POST",
            "https://bedrock.us-east-1.amazonaws.com/model/test/invoke",
            &mut headers,
            b"{}",
            &creds,
        )
        .unwrap();

        // No x-api-key header
        assert!(!headers.iter().any(|(k, _)| k == "x-api-key"));
        // No Bearer auth
        assert!(
            !headers
                .iter()
                .any(|(k, v)| k == "Authorization" && v.starts_with("Bearer"))
        );
        // Must have AWS4-HMAC-SHA256 auth
        assert!(
            headers
                .iter()
                .any(|(k, v)| k == "Authorization" && v.starts_with("AWS4-HMAC-SHA256"))
        );
    }

    #[test]
    fn test_credentials_not_in_error_messages() {
        // FR-030: Verify that error messages don't contain raw keys
        // We test by checking the resolve function returns credential-related
        // errors without exposing actual key values
        let options = HashMap::new();
        let result = resolve_aws_credentials(&options);
        if let Err(e) = result {
            let msg = e.to_string();
            // The error message should NOT contain actual AWS key patterns
            assert!(!msg.contains("AKIA"));
            assert!(!msg.contains("wJalr"));
        }
    }
}
