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

// Ollama Cloud supports the `think` parameter on the native `/api/chat`
// endpoint, mirroring local Ollama. We import the binary-thinking helpers
// to populate model metadata and set the `think` flag in requests.
use super::thinking::{
    binary_thinking_levels_for_model, model_supports_binary_thinking, think_flag_from_request,
};
use super::tool_cache::{ToolFormat, cached_tools};
use crate::llm::{ChatContent, ChatRequest, ContentPart, LlmClient, StreamEvent, ToolDefinition};
use crate::{ModelInfo, Provider};
use ragent_config::{Capabilities, Cost};
use ragent_types::ThinkingConfig;
use ragent_types::event::FinishReason;

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

    async fn discover_models(&self, _api_key: &str) -> Result<Vec<OllamaModelEntry>> {
        let url = format!("{}/api/tags", self.base_url);
        let response = crate::provider::http_client::create_http_client()
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .inspect_err(|e| {
                tracing::warn!(url = %url, error = %e, "Ollama Cloud model discovery failed");
            })
            .with_context(|| format!("Failed to connect to Ollama Cloud at {url}"))?;
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
    /// The endpoint is public for listing model metadata, so the Authorization
    /// header is omitted when no API key is available.
    async fn show_model(&self, api_key: &str, model_name: &str) -> Option<OllamaShowResponse> {
        let url = format!("{}/api/show", self.base_url);
        let mut request = crate::provider::http_client::create_http_client()
            .post(&url)
            .json(&json!({ "model": model_name }))
            .timeout(std::time::Duration::from_secs(5));
        if !api_key.is_empty() {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }
        let response = request.send().await.ok()?;

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
    #[serde(default)]
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
    /// Model family (e.g. `"llama"`, `"qwen"`).
    ///
    /// Currently parsed but not used for context-window estimation, which relies
    /// on `parameter_size`. Kept as a documented field so future routing or
    /// display logic can use it without changing the API response schema.
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
    #[serde(flatten)]
    extra: HashMap<String, Value>,
}

impl OllamaShowResponse {
    /// Extracts the context length from model_info.
    /// Looks for common top-level fields first, then `<architecture>.*` model_info keys.
    fn context_length(&self) -> Option<usize> {
        for key in [
            "context_length",
            "context_window",
            "num_ctx",
            "max_position_embeddings",
            "max_sequence_length",
        ] {
            if let Some(value) = self.extra.get(key).and_then(parse_usize_value) {
                return Some(value);
            }
        }

        for (key, value) in &self.model_info {
            if (key.ends_with(".context_length")
                || key.ends_with(".context_window")
                || key.ends_with(".num_ctx")
                || key.ends_with(".n_ctx")
                || key.ends_with(".max_position_embeddings")
                || key.ends_with(".max_sequence_length"))
                && let Some(len) = parse_usize_value(value)
            {
                return Some(len);
            }
        }
        None
    }

    /// Checks if the model has vision capability.
    fn has_vision(&self) -> bool {
        self.capabilities.iter().any(|c| c == "vision")
    }

    /// Checks if the model supports thinking/reasoning.
    ///
    /// Detects thinking support from two sources:
    /// 1. The `capabilities` array — Ollama may include a "thinking" capability
    ///    for models whose template contains `{{--think}}` tags.
    /// 2. The `template` field — models with `<!-- think -->` markers in their
    ///    Modelfile template support the `think` parameter.
    fn has_thinking(&self) -> bool {
        // Check the capabilities array first (structured detection).
        if self.capabilities.iter().any(|c| c == "thinking") {
            return true;
        }
        // Fallback: inspect the template for thinking markers.
        if let Some(template) = self.extra.get("template").and_then(|v| v.as_str())
            && (template.contains("<!-- think -->") || template.contains("{{--think}}"))
        {
            return true;
        }
        false
    }
}

fn parse_usize_value(value: &Value) -> Option<usize> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .or_else(|| {
            value
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .and_then(|s| s.parse::<usize>().ok())
        })
}

/// Estimates the context window size based on parameter count.
///
/// Modern local/remote Ollama models (Llama 3.2 1B/3B, Phi4-mini, Qwen2.5,
/// Gemma 2, etc.) commonly support 128k-token contexts regardless of
/// parameter size. The old size-based tiers under-reported capacity for
/// current small models and caused the TUI context panel to display
/// nonsensical ">100% full" percentages. We default to 128k for any model
/// with at least 1B parameters, falling back to 32k only for sub-1B models.
/// When the server returns explicit `context_length`/`num_ctx` metadata in
/// `OllamaShowResponse`, that value takes precedence over this heuristic.
fn estimate_context_window(parameter_size: &str) -> usize {
    let size = parameter_size
        .trim_end_matches('B')
        .trim_end_matches('b')
        .parse::<f64>()
        .unwrap_or_else(|_| {
            tracing::warn!(
                parameter_size,
                "failed to parse Ollama Cloud parameter size; defaulting context window to 128k"
            );
            7.0
        });

    if size >= 1.0 { 131_072 } else { 32_768 }
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

    fn as_any_static(&self) -> &dyn std::any::Any {
        self
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        vec![]
    }

    /// Queries Ollama Cloud `/api/tags` for live model discovery.
    /// Authentication is not required to list public models, so discovery works
    /// even when `OLLAMA_API_KEY` is not configured.
    async fn discover_models(&self) -> Result<Vec<ModelInfo>> {
        let api_key = std::env::var("OLLAMA_API_KEY").unwrap_or_default();
        let models = list_ollama_cloud_models(&api_key, Some(&self.base_url))
            .await
            .with_context(|| "Ollama Cloud model discovery failed")?;
        Ok(models)
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

        let url = base_url
            .unwrap_or(&self.base_url)
            .trim_end_matches('/')
            .to_string();
        let client = OllamaCloudClient {
            api_key: key,
            base_url: url.clone(),
            http: crate::provider::http_client::create_streaming_http_client(),
        };
        tracing::info!(chat_endpoint = %format!("{}/api/chat", url), models_endpoint = %format!("{}/api/tags", url), "Ollama Cloud provider connected");
        Ok(Box::new(client))
    }
}
struct OllamaCloudClient {
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
        for msg in request.messages.iter() {
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
                "content": &**system
            }));
        }

        // Only the most recent user message should carry an `images` array.
        // Historical images remain in the chat history as `ContentPart::ImageUrl`,
        // but sending them to a non-vision model (or on older turns) causes
        // Ollama Cloud to reject the request with "this model does not support
        // image input". Compute the index once and only attach images there.
        let last_user_idx = request.messages.iter().rposition(|m| m.role == "user");

        for (idx, msg) in request.messages.iter().enumerate() {
            // Ollama Cloud requires content to always be a plain string.
            // Images must go in a separate "images" array as raw base64 (no data-URL prefix).
            // For any message other than the most recent user message, discard
            // image parts so they are not sent to a model that cannot handle them.
            let is_last_user = last_user_idx == Some(idx);
            let (content_str, images): (String, Vec<String>) = match &msg.content {
                ChatContent::Text(text) => (text.clone(), vec![]),
                ChatContent::Parts(parts) => {
                    let mut texts: Vec<String> = Vec::new();
                    let mut imgs: Vec<String> = Vec::new();
                    for part in parts {
                        match part {
                            ContentPart::Text { text } => texts.push(text.clone()),
                            ContentPart::ImageUrl { url } => {
                                if is_last_user {
                                    // Strip data-URL prefix: "data:image/png;base64,<data>"
                                    let b64 = if let Some(idx) = url.find(";base64,") {
                                        url[idx + 8..].to_string()
                                    } else {
                                        url.clone()
                                    };
                                    imgs.push(b64);
                                }
                                // For non-current user messages, silently drop the
                                // image so a non-vision model is not rejected for
                                // historical image attachments.
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
            // H2: reuse the cached serialised OpenAI-compatible tool list
            // instead of building a fresh `Vec<Value>` on every call.
            let cached = cached_tools(ToolFormat::OpenAi, tools);
            body["tools"] = cached.openai_tools_array();
        }
        // Ollama Cloud supports the `think` boolean parameter on its
        // native `/api/chat` endpoint, just like local Ollama.
        if let Some(think) = think_flag_from_request(request) {
            body["think"] = json!(think);
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
        ); // Log the full request body at debug level (visible when RUST_LOG=debug or when tools are present).
        if tracing::enabled!(tracing::Level::DEBUG) || !request.tools.is_empty() {
            let body_preview = serde_json::to_string(&body).unwrap_or_default();
            let preview_len = body_preview.len().min(800);
            tracing::debug!(body = %&body_preview[..preview_len], "Ollama Cloud request body (truncated)");
        }

        let timeout_secs = request.stream_timeout_secs.unwrap_or(600);
        let body_bytes =
            serde_json::to_vec(&body).context("serialise Ollama Cloud request body")?;
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            self.http
                .post(&url)
                .header("content-type", "application/json")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .body(body_bytes)
                .send(),
        )
        .await
        .inspect_err(|e| {
            tracing::warn!(url = %url, error = %e, "Ollama Cloud chat request timed out");
        })
        .map_err(|_| {
            anyhow::anyhow!("Ollama Cloud: initial response timed out after {timeout_secs}s")
        })?
        .inspect_err(|e| {
            tracing::warn!(url = %url, error = %e, "Ollama Cloud chat request failed");
        })
        .with_context(|| format!("Failed to connect to Ollama Cloud at {url}"))?;
        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            tracing::warn!(
                url = %url,
                model = %request.model,
                status = %status,
                error = %error_body,
                "Ollama Cloud API error"
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
                    std::time::Duration::from_secs(
                        super::http_client::STREAM_CHUNK_IDLE_TIMEOUT_SECS,
                    ),
                    stream.next(),
                )
                .await
                {
                    Ok(Some(r)) => r,
                    Ok(None) => break,
                    Err(_) => {
                        yield StreamEvent::Error {
                            message: format!(
                                "Ollama Cloud: stream stalled — no data received for {}s",
                                super::http_client::STREAM_CHUNK_IDLE_TIMEOUT_SECS
                            ),
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

                while let Some(line) = super::http_client::take_sse_line(&mut buffer) {
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
                    };                      // Log key stream lines for diagnostics (first 3 + any with tool_calls or done)
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
                            yield StreamEvent::ReasoningStart;
                            yield StreamEvent::ReasoningDelta {
                                text: thinking.to_string(),
                            };
                            yield StreamEvent::ReasoningEnd;
                        }

                        let has_tool_calls = message
                            .get("tool_calls")
                            .and_then(|v| v.as_array())
                            .is_some_and(|a| !a.is_empty());

                        if let Some(content) = message.get("content").and_then(|v| v.as_str())
                            && !content.is_empty()
                            && !has_tool_calls
                        {
                            yield StreamEvent::TextDelta { text: content.to_string() };
                        }

                        if has_tool_calls
                            && let Some(tool_calls) = message.get("tool_calls").and_then(|v| v.as_array())
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
            // Check thinking capability: prefer /api/show structured detection,
            // fall back to model-name heuristics.
            let has_thinking = show_info.as_ref().is_some_and(|info| info.has_thinking())
                || model_supports_binary_thinking(&model_id);
            let reasoning = has_thinking;
            let thinking_levels = if has_thinking {
                binary_thinking_levels_for_model(&model_id)
            } else {
                Vec::new()
            };
            let thinking_config = if has_thinking {
                Some(ThinkingConfig::new(ragent_types::ThinkingLevel::Low))
            } else {
                None
            };

            ModelInfo {
                id: model_id,
                provider_id: "ollama_cloud".to_string(),
                name: display_name,
                cost: Cost {
                    input: 0.0,
                    output: 0.0,
                },
                capabilities: Capabilities {
                    reasoning,
                    streaming: true,
                    vision: has_vision,
                    tool_use: true,
                    thinking_levels,
                },
                context_window: ctx,
                max_output: None,
                request_multiplier: None,
                thinking_config,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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

    #[test]
    fn test_context_length_parses_top_level_string_fields() {
        let response: OllamaShowResponse = serde_json::from_value(json!({
            "context_length": "1048576",
            "capabilities": []
        }))
        .expect("show response should parse");

        assert_eq!(response.context_length(), Some(1_048_576));
    }

    #[test]
    fn test_context_length_parses_alternate_model_info_keys() {
        let response: OllamaShowResponse = serde_json::from_value(json!({
            "model_info": {
                "llama.context_window": 1_048_576
            },
            "capabilities": []
        }))
        .expect("show response should parse");

        assert_eq!(response.context_length(), Some(1_048_576));
    }

    #[test]
    fn test_has_thinking_from_capabilities() {
        let response: OllamaShowResponse = serde_json::from_value(json!({
            "capabilities": ["vision", "thinking"]
        }))
        .expect("show response should parse");

        assert!(response.has_thinking());
    }

    #[test]
    fn test_has_thinking_from_template_markers() {
        // Template with <!-- think --> marker
        let response: OllamaShowResponse = serde_json::from_value(json!({
            "template": "Some text <!-- think --> thinking block {{ .Content }}",
            "capabilities": []
        }))
        .expect("show response should parse");

        assert!(response.has_thinking());
    }

    #[test]
    fn test_has_thinking_from_template_go_template() {
        // Template with Go template {{--think}} marker
        let response: OllamaShowResponse = serde_json::from_value(json!({
            "template": "{{--think}}\n{{ .Content }}",
            "capabilities": []
        }))
        .expect("show response should parse");

        assert!(response.has_thinking());
    }

    #[test]
    fn test_has_thinking_false_when_no_indicators() {
        let response: OllamaShowResponse = serde_json::from_value(json!({
            "template": "{{ .Content }}",
            "capabilities": ["vision"]
        }))
        .expect("show response should parse");

        assert!(!response.has_thinking());
    }
}
