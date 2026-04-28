//! Google Gemini provider implementation.
//!
//! Implements the [`Provider`] trait for the Google Gemini API, supporting
//! streaming responses, tool calls, and vision capabilities.

use anyhow::{Context, Result, bail};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::pin::Pin;

use super::thinking::{
    full_reasoning_levels, gemini_thinking_config_from_request, gemini_thinking_levels_for_model,
};
use crate::event::FinishReason;
use crate::llm::{ChatContent, ChatRequest, ContentPart, LlmClient, StreamEvent};
use crate::{ModelInfo, Provider};
use ragent_config::{Capabilities, Cost};

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
                thinking_levels: gemini_thinking_levels_for_model("gemini-2.5-flash-preview-05-20"),
            },
            context_window: 1_048_576,
            max_output: Some(65_536),
            request_multiplier: None,
            thinking_config: None,
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
                thinking_levels: gemini_thinking_levels_for_model("gemini-2.5-pro-preview-05-06"),
            },
            context_window: 1_048_576,
            max_output: Some(65_536),
            request_multiplier: None,
            thinking_config: None,
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
                thinking_levels: gemini_thinking_levels_for_model("gemini-2.0-flash"),
            },
            context_window: 1_048_576,
            max_output: Some(8_192),
            request_multiplier: None,
            thinking_config: None,
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
                thinking_levels: gemini_thinking_levels_for_model("gemini-2.0-flash-lite"),
            },
            context_window: 1_048_576,
            max_output: Some(8_192),
            request_multiplier: None,
            thinking_config: None,
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
                thinking_levels: gemini_thinking_levels_for_model("gemini-1.5-flash"),
            },
            context_window: 1_048_576,
            max_output: Some(8_192),
            request_multiplier: None,
            thinking_config: None,
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
                thinking_levels: gemini_thinking_levels_for_model("gemini-1.5-pro"),
            },
            context_window: 2_097_152,
            max_output: Some(8_192),
            request_multiplier: None,
            thinking_config: None,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiModelsResponse {
    #[serde(default)]
    models: Vec<GeminiDiscoveredModel>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiDiscoveredModel {
    name: String,
    #[serde(default)]
    base_model_id: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    input_token_limit: Option<u64>,
    #[serde(default)]
    output_token_limit: Option<u64>,
    #[serde(default)]
    supported_generation_methods: Vec<String>,
    #[serde(default)]
    thinking: Option<bool>,
}

fn gemini_discovered_model_to_info(
    model: GeminiDiscoveredModel,
    defaults: &HashMap<String, ModelInfo>,
) -> Option<ModelInfo> {
    let id = model
        .base_model_id
        .clone()
        .or_else(|| model.name.strip_prefix("models/").map(ToOwned::to_owned))
        .unwrap_or(model.name.clone());

    let supports_generate_content = model.supported_generation_methods.iter().any(|method| {
        matches!(
            method.as_str(),
            "generateContent"
                | "streamGenerateContent"
                | "GenerateContent"
                | "StreamGenerateContent"
        )
    });
    if !supports_generate_content {
        return None;
    }

    let default = defaults.get(&id);
    let thinking_levels = if model.thinking.unwrap_or(false) {
        let heuristic = gemini_thinking_levels_for_model(&id);
        if heuristic.is_empty() {
            full_reasoning_levels()
        } else {
            heuristic
        }
    } else {
        Vec::new()
    };

    let capabilities = default.map_or_else(
        || Capabilities {
            reasoning: !thinking_levels.is_empty(),
            streaming: true,
            vision: true,
            tool_use: true,
            thinking_levels: thinking_levels.clone(),
        },
        |existing| {
            let mut capabilities = existing.capabilities.clone();
            capabilities.reasoning = !thinking_levels.is_empty();
            capabilities.thinking_levels = thinking_levels.clone();
            capabilities
        },
    );

    Some(ModelInfo {
        id: id.clone(),
        provider_id: "gemini".to_string(),
        name: model
            .display_name
            .or_else(|| default.map(|existing| existing.name.clone()))
            .unwrap_or(id),
        cost: default
            .map(|existing| existing.cost.clone())
            .unwrap_or(Cost {
                input: 0.0,
                output: 0.0,
            }),
        capabilities,
        context_window: model
            .input_token_limit
            .and_then(|limit| usize::try_from(limit).ok())
            .or_else(|| default.map(|existing| existing.context_window))
            .unwrap_or(1_048_576),
        max_output: model
            .output_token_limit
            .and_then(|limit| usize::try_from(limit).ok())
            .or_else(|| default.and_then(|existing| existing.max_output)),
        request_multiplier: default.and_then(|existing| existing.request_multiplier),
        thinking_config: default.and_then(|existing| existing.thinking_config.clone()),
    })
}

/// Queries Gemini's `/v1beta/models` endpoint and converts the response into
/// `ModelInfo` rows, using the live `thinking` capability flag when present.
///
/// # Errors
///
/// Returns an error if the HTTP request fails or the response cannot be parsed.
pub async fn list_gemini_models(api_key: &str, base_url: Option<&str>) -> Result<Vec<ModelInfo>> {
    let base_url = base_url.unwrap_or(GEMINI_API_BASE).trim_end_matches('/');
    let url = format!("{base_url}/v1beta/models?key={api_key}");
    let http = crate::provider::http_client::create_http_client();
    let resp = http
        .get(&url)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .context("Failed to fetch Gemini models")?;

    if !resp.status().is_success() {
        bail!("Gemini models endpoint returned HTTP {}", resp.status());
    }

    let payload: GeminiModelsResponse = resp
        .json()
        .await
        .context("Failed to parse Gemini models response")?;
    let defaults = gemini_default_models("gemini");
    let defaults_by_id = defaults
        .into_iter()
        .map(|model| (model.id.clone(), model))
        .collect::<HashMap<_, _>>();

    let mut models: Vec<ModelInfo> = payload
        .models
        .into_iter()
        .filter_map(|model| gemini_discovered_model_to_info(model, &defaults_by_id))
        .collect();

    models.sort_by(|a, b| a.name.cmp(&b.name));
    models.dedup_by(|a, b| a.id == b.id);
    Ok(models)
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
                            ContentPart::Text { text } => Some(json!({ "text": text })),
                            ContentPart::ImageUrl { url } => {
                                // Handle data URIs for images
                                if url.starts_with("data:") {
                                    let mime = url
                                        .strip_prefix("data:")
                                        .and_then(|s| s.split(';').next())
                                        .unwrap_or("image/jpeg");
                                    let base64_data =
                                        url.find(",").map(|i| &url[i + 1..]).unwrap_or(url);
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

        if generation_config
            .as_object()
            .map(|o| !o.is_empty())
            .unwrap_or(false)
        {
            body["generationConfig"] = generation_config;
        }

        if let Some(thinking_config) = gemini_thinking_config_from_request(request) {
            if body.get("generationConfig").is_none() || body["generationConfig"].is_null() {
                body["generationConfig"] = json!({});
            }
            body["generationConfig"]["thinkingConfig"] = thinking_config;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gemini_discovered_model_to_info_uses_live_thinking_flag() {
        let defaults = gemini_default_models("gemini")
            .into_iter()
            .map(|model| (model.id.clone(), model))
            .collect::<HashMap<_, _>>();
        let model = GeminiDiscoveredModel {
            name: "models/gemini-2.5-pro-preview-05-06".to_string(),
            base_model_id: Some("gemini-2.5-pro-preview-05-06".to_string()),
            display_name: Some("Gemini 2.5 Pro Preview".to_string()),
            input_token_limit: Some(2_000_000),
            output_token_limit: Some(65_536),
            supported_generation_methods: vec!["generateContent".to_string()],
            thinking: Some(true),
        };

        let model = gemini_discovered_model_to_info(model, &defaults).expect("model info");
        assert!(model.capabilities.reasoning);
        assert_eq!(
            model.capabilities.thinking_levels,
            gemini_thinking_levels_for_model("gemini-2.5-pro-preview-05-06")
        );
        assert_eq!(model.context_window, 2_000_000);
        assert_eq!(model.max_output, Some(65_536));
    }
}
