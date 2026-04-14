//! Ollama Cloud provider implementation.
//!
//! Connects to Ollama Cloud at `https://ollama.com` using the native
//! `/api/chat` and `/api/tags` endpoints with bearer-token authentication.

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

const DEFAULT_OLLAMA_CLOUD_HOST: &str = "https://ollama.com";

/// Provider implementation for Ollama Cloud.
pub struct OllamaCloudProvider {
    base_url: String,
}

impl OllamaCloudProvider {
    /// Creates a provider for the Ollama Cloud API.
    #[must_use]
    pub fn new() -> Self {
        Self::with_url(DEFAULT_OLLAMA_CLOUD_HOST)
    }

    /// Creates a provider pointing at a custom Ollama Cloud-compatible host.
    #[must_use]
    pub fn with_url(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    async fn discover_models(&self, api_key: &str) -> Result<Vec<OllamaModelEntry>> {
        if api_key.is_empty() {
            bail!("Ollama Cloud requires an API key.");
        }

        let url = format!("{}/api/tags", self.base_url);
        let response = reqwest::Client::new()
            .get(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .context("Failed to connect to Ollama Cloud")?;

        if !response.status().is_success() {
            bail!(
                "Ollama Cloud API returned status {} from {}",
                response.status(),
                url
            );
        }

        let body: OllamaTagsResponse = response
            .json()
            .await
            .context("Failed to parse Ollama Cloud tags response")?;

        Ok(body.models)
    }

    /// Fetches detailed model information via /api/show endpoint.
    /// Returns context_length and vision capability if available.
    async fn show_model(&self, api_key: &str, model_name: &str) -> Option<OllamaShowResponse> {
        let url = format!("{}/api/show", self.base_url);
        let response = reqwest::Client::new()
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&json!({ "model": model_name }))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .ok()?;

        if !response.status().is_success() {
            return None;
        }

        response.json().await.ok()
    }
}

impl Default for OllamaCloudProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelEntry>,
}

#[derive(Debug, Deserialize)]
struct OllamaModelEntry {
    name: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    details: OllamaModelDetails,
}

#[derive(Debug, Default, Deserialize)]
struct OllamaModelDetails {
    #[serde(default)]
    parameter_size: String,
    #[serde(default)]
    #[allow(dead_code)]
    family: String,
}

/// Response from /api/show endpoint containing model details including context length.
#[derive(Debug, Deserialize)]
struct OllamaShowResponse {
    #[serde(default)]
    model_info: HashMap<String, Value>,
    #[serde(default)]
    capabilities: Vec<String>,
}

impl OllamaShowResponse {
    /// Extracts the context length from model_info.
    /// Looks for keys matching `<architecture>.context_length`.
    fn context_length(&self) -> Option<usize> {
        for (key, value) in &self.model_info {
            if key.ends_with(".context_length") {
                if let Some(len) = value.as_u64() {
                    return Some(len as usize);
                }
            }
        }
        None
    }

    /// Checks if the model has vision capability.
    fn has_vision(&self) -> bool {
        self.capabilities.iter().any(|c| c == "vision")
    }
}

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

fn format_model_name(name: &str, details: &OllamaModelDetails) -> String {
    let param_size = if details.parameter_size.is_empty() {
        infer_parameter_size(name)
    } else {
        Some(details.parameter_size.clone())
    };

    match param_size {
        Some(size) if !size.is_empty() => format!("{name} ({size})"),
        _ => name.to_string(),
    }
}

fn infer_parameter_size(name: &str) -> Option<String> {
    let tag = name.split_once(':')?.1;
    let token = tag.split(['-', '_']).next().unwrap_or(tag);
    let mut digits = String::new();
    let mut saw_digit = false;
    for c in token.chars() {
        if c.is_ascii_digit() || (!saw_digit && c == '.') {
            digits.push(c);
            saw_digit = true;
        } else {
            break;
        }
    }
    if digits.is_empty() {
        return None;
    }
    if token[digits.len()..].starts_with('b') || token[digits.len()..].starts_with('B') {
        Some(format!("{digits}B"))
    } else {
        Some(digits)
    }
}

#[async_trait::async_trait]
impl Provider for OllamaCloudProvider {
    fn id(&self) -> &'static str {
        "ollama_cloud"
    }

    fn name(&self) -> &'static str {
        "Ollama Cloud"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        vec![]
    }

    async fn create_client(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        _options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        let key = if api_key.is_empty() {
            bail!("Ollama Cloud requires an API key.");
        } else {
            api_key.to_string()
        };

        let client = OllamaCloudClient {
            api_key: key,
            base_url: base_url
                .unwrap_or(&self.base_url)
                .trim_end_matches('/')
                .to_string(),
            http: reqwest::Client::builder()
                .tcp_keepalive(std::time::Duration::from_secs(30))
                .connect_timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        };
        Ok(Box::new(client))
    }
}struct OllamaCloudClient {
    api_key: String,
    base_url: String,
    http: reqwest::Client,
}

impl OllamaCloudClient {
    fn build_request_body(&self, request: &ChatRequest, tools: &[ToolDefinition]) -> Value {
        let mut messages = Vec::new();

        // Build a map of tool_use_id → tool_name so we can include both
        // `tool_call_id` (OpenAI format) and `tool_name` (native Ollama format)
        // in tool result messages, satisfying whichever format the model expects.
        let mut tool_id_to_name: HashMap<String, String> = HashMap::new();
        for msg in &request.messages {
            if let ChatContent::Parts(parts) = &msg.content {
                for part in parts {
                    if let ContentPart::ToolUse { id, name, .. } = part {
                        tool_id_to_name.insert(id.clone(), name.clone());
                    }
                }
            }
        }

        if let Some(system) = &request.system {
            messages.push(json!({
                "role": "system",
                "content": system
            }));
        }

        for msg in &request.messages {
            // Ollama Cloud requires content to always be a plain string.
            // Images must go in a separate "images" array as raw base64 (no data-URL prefix).
            let (content_str, images): (String, Vec<String>) = match &msg.content {
                ChatContent::Text(text) => (text.clone(), vec![]),
                ChatContent::Parts(parts) => {
                    let mut texts: Vec<String> = Vec::new();
                    let mut imgs: Vec<String> = Vec::new();
                    for part in parts {
                        match part {
                            ContentPart::Text { text } => texts.push(text.clone()),
                            ContentPart::ImageUrl { url } => {
                                // Strip data-URL prefix: "data:image/png;base64,<data>"
                                let b64 = if let Some(idx) = url.find(";base64,") {
                                    url[idx + 8..].to_string()
                                } else {
                                    url.clone()
                                };
                                imgs.push(b64);
                            }
                            ContentPart::ToolUse { .. } | ContentPart::ToolResult { .. } => {}
                        }
                    }
                    (texts.join("\n"), imgs)
                }
            };
            let content = json!(content_str);

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
                        // Native Ollama /api/chat format: arguments is a JSON object (not a string),
                        // and there is no top-level `id` or `type` wrapper.
                        let tool_calls: Vec<Value> = tool_uses
                            .iter()
                            .map(|p| match p {
                                ContentPart::ToolUse { name, input, .. } => json!({
                                    "function": {
                                        "name": name,
                                        "arguments": input
                                    }
                                }),
                                _ => unreachable!(),
                            })
                            .collect();
                        messages.push(json!({
                            "role": "assistant",
                            "content": "",
                            "tool_calls": tool_calls
                        }));
                    } else if !tool_results.is_empty() {
                        for result in tool_results {
                            if let ContentPart::ToolResult {
                                tool_use_id,
                                content,
                            } = result
                            {
                                // Native Ollama /api/chat format: tool results use `name`, not `tool_call_id`.
                                let name = tool_id_to_name
                                    .get(tool_use_id)
                                    .map_or("", std::string::String::as_str);
                                messages.push(json!({
                                    "role": "tool",
                                    "name": name,
                                    "content": content
                                }));
                            }
                        }
                    } else {
                        let mut msg_json = json!({
                            "role": msg.role,
                            "content": content
                        });
                        if !images.is_empty() {
                            msg_json["images"] = json!(images);
                        }
                        messages.push(msg_json);
                    }
                }
                _ => {
                    let mut msg_json = json!({
                        "role": msg.role,
                        "content": content
                    });
                    if !images.is_empty() {
                        msg_json["images"] = json!(images);
                    }
                    messages.push(msg_json);
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

        if let Some(thinking_val) = request.options.get("thinking")
            && thinking_val.as_str() == Some("disabled")
        {
            body["think"] = json!(false);
        }

        body
    }
}

#[async_trait::async_trait]
impl LlmClient for OllamaCloudClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        let url = format!("{}/api/chat", self.base_url);
        let body = self.build_request_body(&request, &request.tools);

        tracing::debug!(
            url = %url,
            model = %request.model,
            has_tools = !request.tools.is_empty(),
            tool_count = request.tools.len(),
            "Ollama Cloud request"
        );

        // Log the full request body at warn level so it appears in the log even without RUST_LOG=debug
        if tracing::enabled!(tracing::Level::DEBUG) || !request.tools.is_empty() {
            let body_preview = serde_json::to_string(&body).unwrap_or_default();
            let preview_len = body_preview.len().min(800);
            tracing::debug!(body = %&body_preview[..preview_len], "Ollama Cloud request body (truncated)");
        }

        let timeout_secs = request.stream_timeout_secs.unwrap_or(600);
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            self.http
                .post(&url)
                .header("content-type", "application/json")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(&body)
                .send(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Ollama Cloud: initial response timed out after {timeout_secs}s"))?
        .context("Failed to connect to Ollama Cloud")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            // Log the full request body and error at warn level for diagnostics
            let body_str = serde_json::to_string_pretty(&body).unwrap_or_default();
            tracing::warn!(
                url = %url,
                model = %request.model,
                status = %status,
                error = %error_body,
                request_body = %body_str,
                "Ollama Cloud API error — full request logged"
            );
            bail!("Ollama Cloud API error ({status}): {error_body}");
        }

        let stream = response.bytes_stream();
        let model_name = request.model.clone();
        let event_stream = async_stream::stream! {
            let mut buffer = String::new();
            let mut open_tool_calls: HashMap<String, String> = HashMap::new();
            let mut stream_done = false;
            let mut line_count = 0usize;

            futures::pin_mut!(stream);

            while !stream_done {
                let chunk_result = match tokio::time::timeout(
                    std::time::Duration::from_secs(timeout_secs),
                    stream.next(),
                )
                .await
                {
                    Ok(Some(r)) => r,
                    Ok(None) => break,
                    Err(_) => {
                        yield StreamEvent::Error {
                            message: format!("Ollama Cloud: stream stalled — no data received for {timeout_secs}s"),
                        };
                        break;
                    }
                };
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

                    let data = line.strip_prefix("data: ").unwrap_or(line).trim();
                    if data == "[DONE]" {
                        stream_done = true;
                        break;
                    }

                    let parsed: Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::warn!(model=%model_name, line=%data, error=%e, "Ollama Cloud: failed to parse stream line");
                            continue;
                        }
                    };

                    // Log key stream lines for diagnostics (first 5 + any with tool_calls or done)
                    line_count += 1;
                    let has_tool_calls = parsed
                        .get("message")
                        .and_then(|m| m.get("tool_calls"))
                        .is_some();
                    let is_done = parsed.get("done").and_then(serde_json::Value::as_bool) == Some(true);
                    if line_count <= 3 || has_tool_calls || is_done {
                        tracing::info!(
                            model = %model_name,
                            line = line_count,
                            done = is_done,
                            has_tool_calls,
                            done_reason = parsed.get("done_reason").and_then(|v| v.as_str()).unwrap_or(""),
                            "Ollama Cloud stream line"
                        );
                    }

                    if let Some(message) = parsed.get("message") {
                        // Handle thinking/reasoning content (qwen3 and similar models)
                        if let Some(thinking) = message.get("thinking").and_then(|v| v.as_str())
                            && !thinking.is_empty()
                        {
                            tracing::debug!(model=%model_name, chars=thinking.len(), "Ollama Cloud: thinking content");
                        }

                        if let Some(content) = message.get("content").and_then(|v| v.as_str())
                            && !content.is_empty()
                        {
                            yield StreamEvent::TextDelta { text: content.to_string() };
                        }

                        if let Some(tool_calls) = message.get("tool_calls").and_then(|v| v.as_array())
                        {
                            for (idx, tool_call) in tool_calls.iter().enumerate() {
                                let tool_call_id = tool_call
                                    .get("id")
                                    .and_then(|v| v.as_str()).map_or_else(|| format!("ollama_cloud_tc_{idx}"), ToString::to_string);
                                let function = tool_call.get("function").unwrap_or(tool_call);
                                let name = function
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("tool")
                                    .to_string();

                                if !open_tool_calls.contains_key(&tool_call_id) {
                                    open_tool_calls.insert(tool_call_id.clone(), name.clone());
                                    yield StreamEvent::ToolCallStart { id: tool_call_id.clone(), name: name.clone() };
                                }

                                if let Some(args) = function.get("arguments") {
                                    let args_json = if let Some(s) = args.as_str() {
                                        s.to_string()
                                    } else {
                                        args.to_string()
                                    };
                                    if !args_json.is_empty() {
                                        yield StreamEvent::ToolCallDelta {
                                            id: tool_call_id.clone(),
                                            args_json,
                                        };
                                    }
                                }
                            }
                        }
                    }

                    if let Some(response) = parsed.get("response").and_then(|v| v.as_str())
                        && !response.is_empty()
                    {
                        yield StreamEvent::TextDelta { text: response.to_string() };
                    }

                    if parsed.get("done").and_then(serde_json::Value::as_bool) == Some(true) {
                        // Log full done frame so we can see if tool_calls appear there
                        let done_preview = serde_json::to_string(&parsed).unwrap_or_default();
                        let preview_len = done_preview.len().min(500);
                        tracing::info!(
                            model = %model_name,
                            open_tool_calls = open_tool_calls.len(),
                            done_frame = %&done_preview[..preview_len],
                            "Ollama Cloud: done frame received"
                        );

                        if let Some(prompt_tokens) = parsed.get("prompt_eval_count").and_then(serde_json::Value::as_u64)
                        {
                            let output_tokens = parsed.get("eval_count").and_then(serde_json::Value::as_u64).unwrap_or(0);
                            if prompt_tokens > 0 || output_tokens > 0 {
                                yield StreamEvent::Usage {
                                    input_tokens: prompt_tokens,
                                    output_tokens,
                                };
                            }
                        }

                        // Also check done frame for tool_calls (some Ollama versions
                        // batch all tool calls into the final done=true message)
                        if let Some(msg) = parsed.get("message")
                            && let Some(tool_calls_arr) = msg.get("tool_calls").and_then(|v| v.as_array()) {
                                tracing::info!(
                                    model = %model_name,
                                    count = tool_calls_arr.len(),
                                    "Ollama Cloud: tool_calls found in done frame"
                                );
                                for (idx, tool_call) in tool_calls_arr.iter().enumerate() {
                                    let tool_call_id = tool_call
                                        .get("id")
                                        .and_then(|v| v.as_str()).map_or_else(|| format!("ollama_cloud_done_tc_{idx}"), ToString::to_string);
                                    let function = tool_call.get("function").unwrap_or(tool_call);
                                    let name = function
                                        .get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("tool")
                                        .to_string();
                                    if !open_tool_calls.contains_key(&tool_call_id) {
                                        open_tool_calls.insert(tool_call_id.clone(), name.clone());
                                        yield StreamEvent::ToolCallStart { id: tool_call_id.clone(), name: name.clone() };
                                    }
                                    if let Some(args) = function.get("arguments") {
                                        let args_json = if let Some(s) = args.as_str() {
                                            s.to_string()
                                        } else {
                                            args.to_string()
                                        };
                                        if !args_json.is_empty() {
                                            yield StreamEvent::ToolCallDelta { id: tool_call_id.clone(), args_json };
                                        }
                                    }
                                }
                            }

                        for (id, _name) in open_tool_calls.drain() {
                            yield StreamEvent::ToolCallEnd { id };
                        }

                        let reason = match parsed
                            .get("done_reason")
                            .and_then(|v| v.as_str())
                            .unwrap_or("stop")
                        {
                            "tool_calls" => FinishReason::ToolUse,
                            "length" => FinishReason::Length,
                            "content_filter" => FinishReason::ContentFilter,
                            _ => FinishReason::Stop,
                        };
                        yield StreamEvent::Finish { reason };
                        stream_done = true;
                    }
                }
            }
        };

        Ok(Box::pin(event_stream))
    }
}

/// Queries Ollama Cloud for available models.
/// Fetches model details via /api/show to get accurate context window sizes.
pub async fn list_ollama_cloud_models(
    api_key: &str,
    base_url: Option<&str>,
) -> Result<Vec<ModelInfo>> {
    let provider = match base_url {
        Some(url) => OllamaCloudProvider::with_url(url),
        None => OllamaCloudProvider::new(),
    };

    let entries = provider
        .discover_models(api_key)
        .await
        .context("Could not discover Ollama Cloud models")?;

    // Fetch detailed info for each model in parallel
    let model_names: Vec<_> = entries
        .iter()
        .map(|entry| entry.model.clone().unwrap_or_else(|| entry.name.clone()))
        .collect();

    let show_futures: Vec<_> = model_names
        .iter()
        .map(|model_name| provider.show_model(api_key, model_name))
        .collect();

    let show_results = futures::future::join_all(show_futures).await;

    Ok(entries
        .into_iter()
        .zip(show_results)
        .map(|(entry, show_info)| {
            let model_id = entry.model.clone().unwrap_or_else(|| entry.name.clone());
            let display_name = format_model_name(&entry.name, &entry.details);
            let display_name = if model_id == entry.name {
                display_name
            } else {
                format!("{display_name} ({model_id})")
            };

            // Use context_length from /api/show if available, otherwise fall back to estimate
            let ctx = show_info
                .as_ref()
                .and_then(|info| info.context_length())
                .unwrap_or_else(|| estimate_context_window(&entry.details.parameter_size));

            // Check vision capability from /api/show
            let has_vision = show_info.as_ref().is_some_and(|info| info.has_vision());

            ModelInfo {
                id: model_id,
                provider_id: "ollama_cloud".to_string(),
                name: display_name,
                cost: Cost {
                    input: 0.0,
                    output: 0.0,
                },
                capabilities: Capabilities {
                    reasoning: false,
                    streaming: true,
                    vision: has_vision,
                    tool_use: true,
                },
                context_window: ctx,
                max_output: None,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_defaults() {
        let provider = OllamaCloudProvider::new();
        assert_eq!(provider.id(), "ollama_cloud");
        assert_eq!(provider.name(), "Ollama Cloud");
        assert!(provider.default_models().is_empty());
    }

    #[test]
    fn test_with_custom_url() {
        let provider = OllamaCloudProvider::with_url("https://example.com/");
        assert_eq!(provider.base_url, "https://example.com");
    }
}
