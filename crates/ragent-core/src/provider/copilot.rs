//! GitHub Copilot provider implementation.
//!
//! Connects to the GitHub Copilot API at `https://api.githubcopilot.com` using
//! its OpenAI-compatible `/v1/chat/completions` endpoint. Supports streaming
//! responses, tool calls, and dynamic model discovery via `/models`.
//!
//! # Authentication
//!
//! Copilot requires an OAuth token for authentication. The token can be supplied
//! via the `GITHUB_COPILOT_TOKEN` environment variable, or ragent will attempt
//! to read it from your IDE's Copilot configuration:
//!
//! - Linux/macOS: `~/.config/github-copilot/apps.json`
//! - Windows: `%LOCALAPPDATA%/github-copilot/apps.json`
//!
//! The token typically starts with `ghu_` or `gho_`.

use anyhow::{Context, Result, bail};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::pin::Pin;

use crate::config::{Capabilities, Cost};
use crate::event::FinishReason;
use crate::llm::{ChatContent, ChatRequest, ContentPart, LlmClient, StreamEvent, ToolDefinition};
use crate::provider::{ModelInfo, Provider};

/// Default GitHub Copilot API base URL.
const DEFAULT_COPILOT_API_BASE: &str = "https://api.githubcopilot.com";

/// Provider implementation for GitHub Copilot.
///
/// Uses the OpenAI-compatible chat completions endpoint exposed by the
/// Copilot API. Requires an active GitHub Copilot subscription.
pub struct CopilotProvider {
    /// Base URL of the Copilot API (without trailing slash).
    base_url: String,
}

impl CopilotProvider {
    /// Creates a new Copilot provider with the default API endpoint.
    pub fn new() -> Self {
        Self {
            base_url: DEFAULT_COPILOT_API_BASE.to_string(),
        }
    }

    /// Creates a provider pointing at a custom Copilot-compatible endpoint.
    pub fn with_url(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }
}

impl Default for CopilotProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Provider for CopilotProvider {
    /// Returns `"copilot"`.
    fn id(&self) -> &str {
        "copilot"
    }

    /// Returns `"GitHub Copilot"`.
    fn name(&self) -> &str {
        "GitHub Copilot"
    }

    /// Returns the default models available through GitHub Copilot.
    fn default_models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "gpt-4o".to_string(),
                provider_id: "copilot".to_string(),
                name: "GPT-4o (Copilot)".to_string(),
                cost: Cost {
                    input: 0.0,
                    output: 0.0,
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
                provider_id: "copilot".to_string(),
                name: "GPT-4o Mini (Copilot)".to_string(),
                cost: Cost {
                    input: 0.0,
                    output: 0.0,
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
                id: "claude-sonnet-4".to_string(),
                provider_id: "copilot".to_string(),
                name: "Claude Sonnet 4 (Copilot)".to_string(),
                cost: Cost {
                    input: 0.0,
                    output: 0.0,
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
                id: "o3-mini".to_string(),
                provider_id: "copilot".to_string(),
                name: "o3-mini (Copilot)".to_string(),
                cost: Cost {
                    input: 0.0,
                    output: 0.0,
                },
                capabilities: Capabilities {
                    reasoning: true,
                    streaming: true,
                    vision: false,
                    tool_use: true,
                },
                context_window: 200_000,
                max_output: Some(100_000),
            },
        ]
    }

    /// Creates a [`CopilotClient`] configured with the given API token.
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
        let url = base_url.unwrap_or(&self.base_url);
        let client = CopilotClient {
            token: api_key.to_string(),
            base_url: url.trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        };
        Ok(Box::new(client))
    }
}

/// HTTP client for the GitHub Copilot API with streaming SSE support.
///
/// Uses the OpenAI-compatible `/v1/chat/completions` endpoint at
/// `api.githubcopilot.com`. Authentication is via a Copilot OAuth token
/// sent as a Bearer token.
struct CopilotClient {
    /// Copilot OAuth token.
    token: String,
    /// Base URL of the Copilot API.
    base_url: String,
    /// Reusable HTTP client.
    http: reqwest::Client,
}

impl CopilotClient {
    /// Builds the JSON request body in OpenAI chat completions format.
    fn build_request_body(&self, request: &ChatRequest, tools: &[ToolDefinition]) -> Value {
        let mut messages = Vec::new();

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
            "stream": true
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
        if !tools.is_empty() {
            let tool_defs: Vec<Value> = tools
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
            body["tools"] = json!(tool_defs);
        }

        // Reasoning / thinking control via agent options
        if let Some(thinking_val) = request.options.get("thinking") {
            if thinking_val.as_str() == Some("disabled") {
                body["reasoning_effort"] = json!("none");
            }
        }

        body
    }
}

#[async_trait::async_trait]
impl LlmClient for CopilotClient {
    /// Sends a streaming chat completion request to GitHub Copilot.
    ///
    /// Uses the OpenAI-compatible `/v1/chat/completions` endpoint with SSE streaming.
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let body = self.build_request_body(&request, &request.tools);

        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("content-type", "application/json")
            .header("editor-version", "ragent/0.1.0")
            .json(&body)
            .send()
            .await
            .context("Failed to connect to GitHub Copilot API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            bail!("GitHub Copilot API error ({}): {}", status, error_body);
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

                    // Usage info
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

/// Attempts to locate the Copilot OAuth token from IDE configuration files.
///
/// Checks the following locations in order:
/// 1. `~/.config/github-copilot/apps.json` (Linux/macOS)
/// 2. `%LOCALAPPDATA%/github-copilot/apps.json` (Windows)
///
/// Returns `None` if no token is found.
pub fn find_copilot_token() -> Option<String> {
    let config_dirs = if cfg!(windows) {
        vec![dirs::data_local_dir().map(|d| d.join("github-copilot"))]
    } else {
        vec![dirs::config_dir().map(|d| d.join("github-copilot"))]
    };

    for dir_opt in config_dirs.into_iter().flatten() {
        let apps_file = dir_opt.join("apps.json");
        if let Ok(content) = std::fs::read_to_string(&apps_file) {
            if let Ok(parsed) = serde_json::from_str::<Value>(&content) {
                // apps.json is an object keyed by app ID, each with an "oauth_token" field
                if let Some(obj) = parsed.as_object() {
                    for (_key, entry) in obj {
                        if let Some(token) = entry.get("oauth_token").and_then(|t| t.as_str()) {
                            if !token.is_empty() {
                                return Some(token.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

/// Queries the GitHub Copilot API for available models.
///
/// # Errors
///
/// Returns an error if authentication fails or the server is unreachable.
pub async fn list_copilot_models(token: &str, base_url: Option<&str>) -> Result<Vec<ModelInfo>> {
    let url = format!(
        "{}/models",
        base_url.unwrap_or(DEFAULT_COPILOT_API_BASE)
    );

    let http = reqwest::Client::new();
    let response = http
        .get(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("editor-version", "ragent/0.1.0")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .context("Failed to connect to GitHub Copilot API")?;

    if !response.status().is_success() {
        bail!(
            "Copilot API returned {} — check your token",
            response.status()
        );
    }

    let body: CopilotModelsResponse = response
        .json()
        .await
        .context("Failed to parse Copilot models response")?;

    let models = body
        .data
        .into_iter()
        .map(|m| ModelInfo {
            id: m.id.clone(),
            provider_id: "copilot".to_string(),
            name: m.id,
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
            max_output: None,
        })
        .collect();

    Ok(models)
}

/// Response from the Copilot `/models` endpoint.
#[derive(Debug, Deserialize)]
struct CopilotModelsResponse {
    data: Vec<CopilotModelEntry>,
}

/// A single model entry from the Copilot models listing.
#[derive(Debug, Deserialize)]
struct CopilotModelEntry {
    id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_defaults() {
        let provider = CopilotProvider::new();
        assert_eq!(provider.id(), "copilot");
        assert_eq!(provider.name(), "GitHub Copilot");
        let models = provider.default_models();
        assert!(models.len() >= 4);
        assert!(models.iter().any(|m| m.id == "gpt-4o"));
        assert!(models.iter().any(|m| m.id == "claude-sonnet-4"));
    }

    #[test]
    fn test_with_custom_url() {
        let provider = CopilotProvider::with_url("https://proxy.example.com/");
        assert_eq!(provider.base_url, "https://proxy.example.com");
    }

    #[test]
    fn test_models_are_free() {
        let provider = CopilotProvider::new();
        for m in provider.default_models() {
            assert_eq!(m.cost.input, 0.0);
            assert_eq!(m.cost.output, 0.0);
        }
    }
}
