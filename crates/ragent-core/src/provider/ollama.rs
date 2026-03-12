//! Ollama provider implementation.
//!
//! Connects to a local or remote [Ollama](https://ollama.com) server using
//! its OpenAI-compatible `/v1/chat/completions` endpoint. Supports streaming
//! responses, tool calls, and dynamic model discovery via `/api/tags`.
//!
//! # Configuration
//!
//! Set the `OLLAMA_HOST` environment variable to override the default base URL
//! (`http://localhost:11434`). No API key is required for local servers.

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

/// Default Ollama server address.
const DEFAULT_OLLAMA_HOST: &str = "http://localhost:11434";

/// Provider implementation for Ollama servers.
///
/// Ollama exposes an OpenAI-compatible chat completions API at `/v1/chat/completions`
/// and a model listing API at `/api/tags`. This provider works with both local
/// and remote Ollama instances.
pub struct OllamaProvider {
    /// Base URL of the Ollama server (without trailing slash).
    base_url: String,
}

impl OllamaProvider {
    /// Creates a new Ollama provider with the default or environment-configured host.
    ///
    /// Checks `OLLAMA_HOST` environment variable first, then falls back to
    /// `http://localhost:11434`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ragent_core::provider::ollama::OllamaProvider;
    ///
    /// let provider = OllamaProvider::new();
    /// ```
    pub fn new() -> Self {
        let base_url = std::env::var("OLLAMA_HOST")
            .unwrap_or_else(|_| DEFAULT_OLLAMA_HOST.to_string())
            .trim_end_matches('/')
            .to_string();
        Self { base_url }
    }

    /// Creates a provider pointing at a specific Ollama server URL.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ragent_core::provider::ollama::OllamaProvider;
    ///
    /// let provider = OllamaProvider::with_url("http://gpu-server:11434");
    /// ```
    pub fn with_url(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// Queries the Ollama `/api/tags` endpoint to discover locally available models.
    async fn discover_models(&self) -> Result<Vec<OllamaModelEntry>> {
        let url = format!("{}/api/tags", self.base_url);
        let http = reqwest::Client::new();

        let response = http
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .context("Failed to connect to Ollama server")?;

        if !response.status().is_success() {
            bail!(
                "Ollama API returned status {} from {}",
                response.status(),
                url
            );
        }

        let body: OllamaTagsResponse = response
            .json()
            .await
            .context("Failed to parse Ollama tags response")?;

        Ok(body.models)
    }
}

impl Default for OllamaProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Response from the Ollama `/api/tags` endpoint.
#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelEntry>,
}

/// A single model entry returned by Ollama's `/api/tags`.
#[derive(Debug, Deserialize)]
struct OllamaModelEntry {
    /// Model name (e.g. `"llama3.3:latest"`).
    name: String,
    /// Details about the model.
    #[serde(default)]
    details: OllamaModelDetails,
}

/// Model details from the Ollama tags response.
#[derive(Debug, Default, Deserialize)]
struct OllamaModelDetails {
    /// Parameter size string (e.g. `"70B"`, `"8B"`).
    #[serde(default)]
    parameter_size: String,
    /// Model family (e.g. `"llama"`, `"qwen"`).
    #[serde(default)]
    #[allow(dead_code)]
    family: String,
}

/// Estimates the context window size based on parameter count.
fn estimate_context_window(parameter_size: &str) -> usize {
    let size = parameter_size
        .trim_end_matches('B')
        .trim_end_matches('b')
        .parse::<f64>()
        .unwrap_or(7.0);

    if size >= 70.0 {
        131_072
    } else if size >= 30.0 {
        65_536
    } else if size >= 7.0 {
        32_768
    } else {
        8_192
    }
}

#[async_trait::async_trait]
impl Provider for OllamaProvider {
    /// Returns `"ollama"`.
    fn id(&self) -> &str {
        "ollama"
    }

    /// Returns `"Ollama"`.
    fn name(&self) -> &str {
        "Ollama"
    }

    /// Returns a placeholder model list.
    ///
    /// Since Ollama model availability depends on the running server, this
    /// returns an empty list. Use [`OllamaProvider::discover_models`] or the
    /// `ragent models --provider ollama` command to query available models.
    fn default_models(&self) -> Vec<ModelInfo> {
        // Return a generic entry — actual models are discovered at runtime
        vec![ModelInfo {
            id: "llama3.2".to_string(),
            provider_id: "ollama".to_string(),
            name: "Llama 3.2 (default)".to_string(),
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
            context_window: 131_072,
            max_output: None,
        }]
    }

    /// Creates an [`OllamaClient`] configured with the given base URL.
    ///
    /// The `api_key` parameter is accepted but ignored for local Ollama servers.
    /// For remote servers behind authentication, it will be sent as a Bearer token.
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
        let client = OllamaClient {
            api_key: if api_key.is_empty() {
                None
            } else {
                Some(api_key.to_string())
            },
            base_url: url.trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        };
        Ok(Box::new(client))
    }
}

/// HTTP client for the Ollama OpenAI-compatible API with streaming SSE support.
///
/// Communicates with Ollama's `/v1/chat/completions` endpoint, which mirrors
/// the OpenAI API format. Supports text streaming, tool calls, and
/// optional Bearer token authentication for remote servers.
struct OllamaClient {
    /// Optional API key for remote/authenticated Ollama servers.
    api_key: Option<String>,
    /// Base URL of the Ollama server.
    base_url: String,
    /// Reusable HTTP client.
    http: reqwest::Client,
}

impl OllamaClient {
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
                body["think"] = json!(false);
            }
        }

        body
    }
}

#[async_trait::async_trait]
impl LlmClient for OllamaClient {
    /// Sends a streaming chat completion request to Ollama.
    ///
    /// Uses the OpenAI-compatible `/v1/chat/completions` endpoint with SSE streaming.
    /// Handles text deltas, tool calls, usage reporting, and finish reasons.
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        let url = format!("{}/v1/chat/completions", self.base_url);
        let body = self.build_request_body(&request, &request.tools);

        let mut req_builder = self
            .http
            .post(&url)
            .header("content-type", "application/json");

        // Only send auth header if we have a key (remote servers)
        if let Some(ref key) = self.api_key {
            req_builder = req_builder.header("Authorization", format!("Bearer {key}"));
        }

        let response = req_builder
            .json(&body)
            .send()
            .await
            .context("Failed to connect to Ollama server — is it running?")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            bail!("Ollama API error ({}): {}", status, error_body);
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

/// Queries the Ollama server at the given base URL for available models.
///
/// Returns model metadata suitable for display, including estimated context
/// window sizes and parameter counts. This is called by the CLI `models` command.
///
/// # Errors
///
/// Returns an error if the server is unreachable or returns an invalid response.
///
/// # Examples
///
/// ```no_run
/// use ragent_core::provider::ollama::list_ollama_models;
///
/// # async fn example() -> anyhow::Result<()> {
/// // Use the default Ollama host.
/// let models = list_ollama_models(None).await?;
/// for m in &models {
///     println!("{}: {}", m.id, m.name);
/// }
///
/// // Use a custom Ollama host.
/// let models = list_ollama_models(Some("http://gpu-server:11434")).await?;
/// # Ok(())
/// # }
/// ```
pub async fn list_ollama_models(base_url: Option<&str>) -> Result<Vec<ModelInfo>> {
    let provider = match base_url {
        Some(url) => OllamaProvider::with_url(url),
        None => OllamaProvider::new(),
    };

    let entries = provider
        .discover_models()
        .await
        .context("Could not discover Ollama models")?;

    let models = entries
        .into_iter()
        .map(|entry| {
            let display_name = format_model_name(&entry.name, &entry.details);
            let ctx = estimate_context_window(&entry.details.parameter_size);

            ModelInfo {
                id: entry.name.clone(),
                provider_id: "ollama".to_string(),
                name: display_name,
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
                context_window: ctx,
                max_output: None,
            }
        })
        .collect();

    Ok(models)
}

/// Formats a human-readable model name from its tag and details.
fn format_model_name(name: &str, details: &OllamaModelDetails) -> String {
    let base = name.split(':').next().unwrap_or(name);
    let capitalized = base
        .split(|c: char| c == '-' || c == '_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    format!("{upper}{}", chars.as_str())
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    if details.parameter_size.is_empty() {
        capitalized
    } else {
        format!("{capitalized} ({})", details.parameter_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_model_name() {
        let details = OllamaModelDetails {
            parameter_size: "70B".to_string(),
            family: "llama".to_string(),
        };
        assert_eq!(
            format_model_name("llama3.3:latest", &details),
            "Llama3.3 (70B)"
        );
    }

    #[test]
    fn test_estimate_context_window() {
        assert_eq!(estimate_context_window("70B"), 131_072);
        assert_eq!(estimate_context_window("8B"), 32_768);
        assert_eq!(estimate_context_window("3B"), 8_192);
        assert_eq!(estimate_context_window("32B"), 65_536);
    }

    #[test]
    fn test_provider_defaults() {
        let provider = OllamaProvider::new();
        assert_eq!(provider.id(), "ollama");
        assert_eq!(provider.name(), "Ollama");
        assert!(!provider.default_models().is_empty());
    }

    #[test]
    fn test_with_custom_url() {
        let provider = OllamaProvider::with_url("http://remote:11434/");
        assert_eq!(provider.base_url, "http://remote:11434");
    }
}
