//! Google Gemini provider implementation.
//!
//! Implements the [`Provider`] trait for the Google Gemini API, supporting
//! streaming responses, tool calls, and vision capabilities.

use anyhow::{Context, Result, bail};
use futures::StreamExt;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::pin::Pin;

use crate::config::{Capabilities, Cost};
use crate::event::FinishReason;
use crate::llm::{ChatContent, ChatRequest, ContentPart, LlmClient, StreamEvent};
use crate::provider::{ModelInfo, Provider};

/// Default API base URL for Google Gemini API.
pub const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com";

/// Returns the default Gemini model catalog with `provider_id` attached.
#[must_use]
pub fn gemini_default_models(provider_id: &str) -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "gemini-2.5-flash-preview-05-20".to_string(),
            provider_id: provider_id.to_string(),
            name: "Gemini 2.5 Flash Preview".to_string(),
            cost: Cost {
                input: 0.15,
                output: 0.60,
            },
            capabilities: Capabilities {
                reasoning: true,
                streaming: true,
                vision: true,
                tool_use: true,
            },
            context_window: 1_048_576,
            max_output: Some(65_536),
        },
        ModelInfo {
            id: "gemini-2.5-pro-preview-05-06".to_string(),
            provider_id: provider_id.to_string(),
            name: "Gemini 2.5 Pro Preview".to_string(),
            cost: Cost {
                input: 1.25,
                output: 10.0,
            },
            capabilities: Capabilities {
                reasoning: true,
                streaming: true,
                vision: true,
                tool_use: true,
            },
            context_window: 1_048_576,
            max_output: Some(65_536),
        },
        ModelInfo {
            id: "gemini-2.0-flash".to_string(),
            provider_id: provider_id.to_string(),
            name: "Gemini 2.0 Flash".to_string(),
            cost: Cost {
                input: 0.10,
                output: 0.40,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: true,
                tool_use: true,
            },
            context_window: 1_048_576,
            max_output: Some(8_192),
        },
        ModelInfo {
            id: "gemini-2.0-flash-lite".to_string(),
            provider_id: provider_id.to_string(),
            name: "Gemini 2.0 Flash Lite".to_string(),
            cost: Cost {
                input: 0.075,
                output: 0.30,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: true,
                tool_use: true,
            },
            context_window: 1_048_576,
            max_output: Some(8_192),
        },
        ModelInfo {
            id: "gemini-1.5-flash".to_string(),
            provider_id: provider_id.to_string(),
            name: "Gemini 1.5 Flash".to_string(),
            cost: Cost {
                input: 0.075,
                output: 0.30,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: true,
                tool_use: true,
            },
            context_window: 1_048_576,
            max_output: Some(8_192),
        },
        ModelInfo {
            id: "gemini-1.5-pro".to_string(),
            provider_id: provider_id.to_string(),
            name: "Gemini 1.5 Pro".to_string(),
            cost: Cost {
                input: 1.25,
                output: 5.0,
            },
            capabilities: Capabilities {
                reasoning: true,
                streaming: true,
                vision: true,
                tool_use: true,
            },
            context_window: 2_097_152,
            max_output: Some(8_192),
        },
    ]
}

/// Provider implementation for the Google Gemini API.
pub struct GeminiProvider;

#[async_trait::async_trait]
impl Provider for GeminiProvider {
    /// Returns `"gemini"`.
    fn id(&self) -> &'static str {
        "gemini"
    }

    /// Returns `"Google Gemini"`.
    fn name(&self) -> &'static str {
        "Google Gemini"
    }

    /// Returns default Gemini models.
    fn default_models(&self) -> Vec<ModelInfo> {
        gemini_default_models("gemini")
    }

    /// Creates a [`GeminiClient`] configured with the given API key and optional base URL.
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
        let client = GeminiClient::new(api_key, base_url.unwrap_or(GEMINI_API_BASE));
        Ok(Box::new(client))
    }
}

/// HTTP client for the Google Gemini API with streaming SSE support.
pub(crate) struct GeminiClient {
    api_key: String,
    base_url: String,
    http: reqwest::Client,
}

impl GeminiClient {
    pub(crate) fn new(api_key: &str, base_url: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            http: crate::provider::http_client::create_streaming_http_client(),
        }
    }

    /// Build the JSON request body for the Gemini API.
    fn build_request_body(&self, request: &ChatRequest) -> Value {
        let mut contents = Vec::new();

        // System instruction is handled separately in Gemini API
        let mut system_instruction: Option<Value> = None;
        if let Some(system) = &request.system {
            system_instruction = Some(json!({
                "parts": [{ "text": system }],
                "role": "user"
            }));
        }

        for msg in &request.messages {
            let role = match msg.role.as_str() {
                "assistant" => "model",
                _ => "user",
            };

            let parts = match &msg.content {
                ChatContent::Text(text) => {
                    vec![json!({ "text": text })]
                }
                ChatContent::Parts(content_parts) => {
                    content_parts
                        .iter()
                        .filter_map(|part| match part {
                            ContentPart::Text { text } => {
                                Some(json!({ "text": text }))
                            }
                            ContentPart::ImageUrl { url } => {
                                // Handle data URIs for images
                                if url.starts_with("data:") {
                                    let mime = url
                                        .strip_prefix("data:")
                                        .and_then(|s| s.split(';').next())
                                        .unwrap_or("image/jpeg");
                                    let base64_data = url
                                        .find(",")
                                        .map(|i| &url[i + 1..])
                                        .unwrap_or(url);
                                    Some(json!({
                                        "inlineData": {
                                            "mimeType": mime,
                                            "data": base64_data
                                        }
                                    }))
                                } else {
                                    // For non-data URLs, we can't directly use them in Gemini
                                    // Gemini requires inline data or Google Cloud Storage URIs
                                    Some(json!({ "text": format!("[Image: {}]", url) }))
                                }
                            }
                            ContentPart::ToolResult {
                                tool_use_id,
                                content,
                            } => {
                                // Gemini uses functionResponse for tool results
                                Some(json!({
                                    "functionResponse": {
                                        "name": tool_use_id.split('_').next().unwrap_or("tool"),
                                        "response": {
                                            "result": content
                                        }
                                    }
                                }))
                            }
                            ContentPart::ToolUse { id: _, name, input } => {
                                // Gemini uses functionCall for tool invocations
                                Some(json!({
                                    "functionCall": {
                                        "name": name,
                                        "args": input
                                    }
                                }))
                            }
                        })
                        .collect()
                }
            };

            contents.push(json!({
                "role": role,
                "parts": parts
            }));
        }

        let mut body = json!({
            "contents": contents,
        });

        if let Some(system) = system_instruction {
            body["systemInstruction"] = system;
        }

        // Add generation config
        let mut generation_config = json!({});

        if let Some(temp) = request.temperature {
            generation_config["temperature"] = json!(temp);
        }
        if let Some(top_p) = request.top_p {
            generation_config["topP"] = json!(top_p);
        }
        if let Some(max_tokens) = request.max_tokens {
            generation_config["maxOutputTokens"] = json!(max_tokens);
        }

        if generation_config.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
            body["generationConfig"] = generation_config;
        }

        // Add tools if present
        if !request.tools.is_empty() {
            let tools: Vec<Value> = request
                .tools
                .iter()
                .map(|t| {
                    json!({
                        "functionDeclarations": [{
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters
                        }]
                    })
                })
                .collect();
            body["tools"] = json!(tools);
        }

        body
    }
}

#[async_trait::async_trait]
impl LlmClient for GeminiClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        let body = self.build_request_body(&request);
        
        // Use streaming endpoint
        let url = format!(
            "{}/v1beta/models/{}:streamGenerateContent?key={}",
            self.base_url, request.model, self.api_key
        );

        let response = self
            .http
            .post(&url)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to send request to Gemini API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to read Gemini error response body");
                String::new()
            });
            tracing::warn!(
                url = %url,
                model = %request.model,
                status = %status,
                error = %error_body,
                "Gemini API error"
            );
            bail!("Gemini API error ({}): {}", status, error_body);
        }

        let stream = response.bytes_stream();

        let event_stream = async_stream::stream! {
            let mut buffer = String::new();
            let mut pending_tool_calls: Vec<(String, String, String)> = Vec::new(); // (id, name, args)

            futures::pin_mut!(stream);

            while let Some(chunk_result) = stream.next().await {
                let chunk = match chunk_result {
                    Ok(c) => c,
                    Err(e) => {
                        yield StreamEvent::Error { message: e.to_string() };
                        break;
                    }
                };

                // Gemini streams JSON objects, not SSE
                buffer.push_str(&String::from_utf8_lossy(&chunk));

                // Try to parse complete JSON objects from buffer
                // Gemini returns a stream of JSON objects, each on its own line or as NDJSON
                while let Some(line_end) = buffer.find('\n') {
                    let line = buffer[..line_end].trim().to_string();
                    buffer = buffer[line_end + 1..].to_string();

                    if line.is_empty() || line == "[,]" {
                        continue;
                    }

                    // Remove trailing comma if present (JSON array formatting)
                    let line = line.trim_end_matches(',').trim();
                    
                    // Remove array brackets if present
                    let line = line.trim_start_matches('[').trim_start();
                    let line = line.trim_end_matches(']').trim_end();

                    if line.is_empty() {
                        continue;
                    }

                    let parsed: Value = match serde_json::from_str(line) {
                        Ok(v) => v,
                        Err(_) => {
                            // Try to accumulate more data
                            buffer = format!("{}\n{}", line, buffer);
                            continue;
                        }
                    };

                    // Handle usage metadata (usually in the last chunk)
                    if let Some(usage) = parsed.get("usageMetadata") {
                        let input_tokens = usage["promptTokenCount"].as_u64().unwrap_or(0);
                        let output_tokens = usage["candidatesTokenCount"].as_u64().unwrap_or(0);
                        if input_tokens > 0 || output_tokens > 0 {
                            yield StreamEvent::Usage { input_tokens, output_tokens };
                        }
                    }

                    // Handle candidates
                    if let Some(candidates) = parsed["candidates"].as_array() {
                        for candidate in candidates {
                            let content = &candidate["content"];
                            
                            // Handle finish reason
                            if let Some(finish_reason) = candidate["finishReason"].as_str() {
                                // Emit any pending tool calls
                                for (id, name, args) in &pending_tool_calls {
                                    yield StreamEvent::ToolCallStart {
                                        id: id.clone(),
                                        name: name.clone(),
                                    };
                                    yield StreamEvent::ToolCallDelta {
                                        id: id.clone(),
                                        args_json: args.clone(),
                                    };
                                    yield StreamEvent::ToolCallEnd { id: id.clone() };
                                }
                                pending_tool_calls.clear();

                                let reason = match finish_reason {
                                    "STOP" => FinishReason::Stop,
                                    "MAX_TOKENS" => FinishReason::Length,
                                    "SAFETY" | "RECITATION" => FinishReason::ContentFilter,
                                    "OTHER" => FinishReason::Stop,
                                    _ => FinishReason::Stop,
                                };
                                yield StreamEvent::Finish { reason };
                            }

                            // Handle content parts
                            if let Some(parts) = content["parts"].as_array() {
                                for part in parts {
                                    // Text content
                                    if let Some(text) = part["text"].as_str() {
                                        if !text.is_empty() {
                                            yield StreamEvent::TextDelta { text: text.to_string() };
                                        }
                                    }

                                    // Function calls (tool use)
                                    if let Some(function_call) = part.get("functionCall") {
                                        let name = function_call["name"].as_str()
                                            .unwrap_or("unknown")
                                            .to_string();
                                        let args = function_call["args"]
                                            .as_object()
                                            .map(|o| json!(o).to_string())
                                            .unwrap_or_else(|| "{}".to_string());
                                        let id = format!("fc_{}_{}", name, pending_tool_calls.len());
                                        
                                        // Buffer tool call to emit at end
                                        pending_tool_calls.push((id, name, args));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Handle any remaining data in buffer
            if !buffer.trim().is_empty() {
                let line = buffer.trim().trim_end_matches(',').trim();
                if !line.is_empty() && line != "[,]" {
                    if let Ok(parsed) = serde_json::from_str::<Value>(line) {
                        if let Some(usage) = parsed.get("usageMetadata") {
                            let input_tokens = usage["promptTokenCount"].as_u64().unwrap_or(0);
                            let output_tokens = usage["candidatesTokenCount"].as_u64().unwrap_or(0);
                            if input_tokens > 0 || output_tokens > 0 {
                                yield StreamEvent::Usage { input_tokens, output_tokens };
                            }
                        }
                    }
                }
            }

            // Ensure we emit a finish event if not already done
            // (Gemini might end without explicit finishReason in some cases)
        };

        Ok(Box::pin(event_stream))
    }
}
