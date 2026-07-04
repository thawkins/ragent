//! Azure Resource Provider — file-based model catalog.
//!
//! Reads model definitions from `azureresources.json` so users can register
//! Azure-hosted endpoints (e.g. Azure OpenAI, Azure AI Foundry, custom
//! endpoints) without rebuilding the application.
//!
//! # `azureresources.json` schema
//!
//! ```json
//! {
//!   "version": "1",
//!   "resources": [
//!     {
//!       "id": "my-gpt-4o",
//!       "name": "My Azure GPT-4o",
//!       "endpoint": "https://my-resource.openai.azure.com",
//!       "api_key_env": "MY_AOAI_KEY",
//!       "context_window": 128000,
//!       "capabilities": ["streaming", "vision", "tool_use"],
//!       "thinking": { "enabled": false }
//!     }
//!   ]
//! }
//! ```
//!
//! The file is searched in this order:
//! 1. `~/.config/ragent/azureresources.json`
//! 2. `.ragent/azureresources.json` (current working directory)

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::llm::LlmClient;
use crate::provider::anthropic::AnthropicClient;
use crate::provider::azure_foundry::AzureFoundryClient;
use crate::{ModelInfo, Provider};
use ragent_config::{Capabilities, Cost};

/// A single Azure resource entry parsed from `azureresources.json`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AzureResourceEntry {
    /// Unique identifier for this resource (used as model ID).
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Base endpoint URL for the Azure resource.
    pub endpoint: String,
    /// Optional inline API key (discouraged — prefer `api_key_env`).
    #[serde(default)]
    pub api_key: Option<String>,
    /// Name of the environment variable that holds the API key.
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// Maximum context-window size in tokens.
    #[serde(default)]
    pub context_window: Option<usize>,
    /// Optional capability tags (e.g. `"streaming"`, `"vision"`, `"tool_use"`).
    #[serde(default)]
    pub capabilities: Option<Vec<String>>,
    /// Optional thinking / reasoning configuration.
    #[serde(default)]
    pub thinking: Option<ragent_types::ThinkingConfig>,
    /// API type: "openai" or "anthropic". Defaults to "openai".
    #[serde(default, rename = "api_type")]
    pub api_type: Option<String>,
}

/// Top-level structure of `azureresources.json`.
#[derive(Debug, Deserialize)]
struct AzureResourcesFile {
    version: String,
    resources: Vec<AzureResourceEntry>,
}

/// Parse an `azureresources.json` file at `path` and return validated entries.
///
/// Validation rules:
/// * `version` must be `"1"`.
/// * Each entry must have non-empty `id`, `name`, and `endpoint`.
/// * Each entry must have at least one of `api_key` or `api_key_env`.
/// * Entries missing required fields are skipped with a `tracing::warn!` log.
/// * Duplicate IDs are deduplicated (first wins) with a warning.
///
/// Returns an empty `Vec` on any fatal error (missing file, malformed JSON,
/// etc.).
pub fn parse_azure_resources(path: &Path) -> Result<Vec<AzureResourceEntry>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let file: AzureResourcesFile = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON in {}", path.display()))?;

    if file.version != "1" {
        anyhow::bail!(
            "Unsupported azureresources.json version: expected '1', got '{}'",
            file.version
        );
    }

    let mut seen_ids = HashMap::new();
    let mut entries = Vec::new();

    for entry in file.resources {
        // Validate mandatory fields
        if entry.id.trim().is_empty() {
            tracing::warn!("Skipping Azure resource entry: missing 'id'");
            continue;
        }
        if entry.name.trim().is_empty() {
            tracing::warn!("Skipping Azure resource entry: missing 'name'");
            continue;
        }
        if entry.endpoint.trim().is_empty() {
            tracing::warn!("Skipping Azure resource entry: missing 'endpoint'");
            continue;
        }
        if entry.api_key.is_none() && entry.api_key_env.is_none() {
            tracing::warn!(
                resource_id = %entry.id,
                "Skipping Azure resource entry: neither 'api_key' nor 'api_key_env' provided"
            );
            continue;
        }

        let api_type = entry.api_type.as_deref().unwrap_or("openai");
        if api_type != "openai" && api_type != "anthropic" {
            tracing::warn!(
                resource_id = %entry.id,
                api_type = %api_type,
                "Skipping Azure resource entry: unsupported api_type"
            );
            continue;
        }

        // Deduplicate IDs
        if seen_ids.contains_key(&entry.id) {
            tracing::warn!(
                resource_id = %entry.id,
                "Skipping duplicate Azure resource entry"
            );
            continue;
        }
        seen_ids.insert(entry.id.clone(), ());
        entries.push(entry);
    }

    Ok(entries)
}

/// Resolve the path to `azureresources.json`.
///
/// Tries, in order:
/// 1. `~/.config/ragent/azureresources.json`
/// 2. `.ragent/azureresources.json` in the current working directory
fn resolve_config_path() -> Option<PathBuf> {
    // 1. User config directory
    if let Some(home) = dirs::home_dir() {
        let p = home
            .join(".config")
            .join("ragent")
            .join("azureresources.json");
        if p.exists() {
            return Some(p);
        }
    }

    // 2. Project-local .ragent directory
    let p = PathBuf::from(".ragent").join("azureresources.json");
    if p.exists() {
        return Some(p);
    }

    None
}

/// Provider implementation backed by a user-supplied `azureresources.json` file.
///
/// Models are loaded dynamically from the file; if the file is absent or
/// malformed the provider advertises an empty catalog.
pub struct AzureResourceProvider {
    config_path: PathBuf,
    /// Parsed entries cached after first successful load, used for `create_client`
    /// branching on `api_type`.
    cached_entries: Option<Vec<AzureResourceEntry>>,
}

impl AzureResourceProvider {
    /// Creates a new provider using the default config-file resolution.
    #[must_use]
    pub fn new() -> Self {
        let config_path = resolve_config_path()
            .unwrap_or_else(|| PathBuf::from(".ragent").join("azureresources.json"));
        let cached_entries = parse_azure_resources(&config_path).ok();
        Self {
            config_path,
            cached_entries,
        }
    }

    /// Creates a new provider with an explicit config file path.
    #[must_use]
    pub fn with_path(path: PathBuf) -> Self {
        let cached_entries = parse_azure_resources(&path).ok();
        Self {
            config_path: path,
            cached_entries,
        }
    }

    /// Returns the raw parsed entries from `azureresources.json`.
    ///
    /// This preserves `endpoint`, `api_key`, `api_key_env`, and `api_type`
    /// which are lost when converting to [`ModelInfo`] via [`Provider::default_models`].
    pub fn entries(&self) -> Vec<AzureResourceEntry> {
        match &self.cached_entries {
            Some(entries) => entries.clone(),
            None => parse_azure_resources(&self.config_path).unwrap_or_default(),
        }
    }

    /// Look up the `api_type` for a given model id among cached entries.
    fn api_type_for_model(&self, model_id: &str) -> Option<String> {
        self.entries()
            .into_iter()
            .find(|e| e.id == model_id)
            .and_then(|e| e.api_type)
    }
}

impl Default for AzureResourceProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Provider for AzureResourceProvider {
    fn id(&self) -> &str {
        "azure_resource"
    }

    fn name(&self) -> &str {
        "Azure Resource (File)"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        match parse_azure_resources(&self.config_path) {
            Ok(entries) => entries.into_iter().map(entry_to_model_info).collect(),
            Err(e) => {
                tracing::debug!(
                    path = %self.config_path.display(),
                    error = %e,
                    "AzureResourceProvider: no model catalog loaded"
                );
                Vec::new()
            }
        }
    }

    async fn create_client(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        let resolved = base_url
            .unwrap_or_default()
            .trim_end_matches('/')
            .to_string();
        // Determine api_type from options (model_id) or fallback to openai
        let api_type = if let Some(model_id) = options.get("model_id").and_then(Value::as_str) {
            self.api_type_for_model(model_id)
                .unwrap_or_else(|| "openai".to_string())
        } else {
            "openai".to_string()
        };

        if api_type == "anthropic" {
            let client = crate::provider::anthropic::AnthropicClient {
                api_key: api_key.to_string(),
                base_url: resolved.clone(),
                http: crate::provider::http_client::create_streaming_http_client(),
            };
            tracing::info!(
                chat_endpoint = %format!("{}/anthropic/v1/messages", resolved),
                "Azure Resource (Anthropic) connected"
            );
            Ok(Box::new(AzureAnthropicClient {
                inner: client,
                api_key: api_key.to_string(),
            }))
        } else {
            let client = AzureFoundryClient::new(api_key, &resolved);
            tracing::info!(
                chat_endpoint = %format!("{}/openai/v1/chat/completions", resolved),
                "Azure Resource (OpenAI) connected"
            );
            Ok(Box::new(client))
        }
    }
}

/// Convert an [`AzureResourceEntry`] into a [`ModelInfo`].
fn entry_to_model_info(entry: AzureResourceEntry) -> ModelInfo {
    let caps = entry.capabilities.as_ref();
    ModelInfo {
        id: entry.id.clone(),
        provider_id: "azure_resource".to_string(),
        name: entry.name,
        cost: Cost {
            input: 0.0,
            output: 0.0,
        },
        capabilities: {
            if let Some(c) = caps {
                // When capabilities are explicitly listed, only enable the ones listed
                Capabilities {
                    reasoning: c.contains(&"reasoning".to_string()),
                    streaming: c.contains(&"streaming".to_string()),
                    vision: c.contains(&"vision".to_string()),
                    tool_use: c.contains(&"tool_use".to_string()),
                    thinking_levels: Vec::new(),
                }
            } else {
                // When capabilities are not specified, use sensible defaults
                Capabilities {
                    reasoning: false,
                    streaming: true,
                    vision: false,
                    tool_use: true,
                    thinking_levels: Vec::new(),
                }
            }
        },
        context_window: entry.context_window.unwrap_or(128_000),
        max_output: None,
        request_multiplier: None,
        thinking_config: entry.thinking,
    }
}

/// Wrapper around [`AnthropicClient`] that overrides the auth header to use
/// Azure-style `api-key` (instead of the standard `x-api-key`) and targets the
/// Azure-hosted Anthropic endpoint path (`anthropic/v1/messages`).
pub(crate) struct AzureAnthropicClient {
    inner: AnthropicClient,
    api_key: String,
}

#[async_trait::async_trait]
impl LlmClient for AzureAnthropicClient {
    async fn chat(
        &self,
        request: crate::llm::ChatRequest,
    ) -> Result<std::pin::Pin<Box<dyn futures::Stream<Item = crate::llm::StreamEvent> + Send>>>
    {
        let url = format!("{}/anthropic/v1/messages", self.inner.base_url);
        let body = self.inner.build_request_body(&request);

        let response = self
            .inner
            .http
            .post(&url)
            .header("api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .inspect_err(|e| {
                tracing::warn!(url = %url, error = %e, "Azure Anthropic chat request failed");
            })
            .with_context(|| format!("Failed to send request to Azure Anthropic at {url}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to read response body");
                String::new()
            });
            tracing::warn!(
                url = %url,
                model = %request.model,
                status = %status,
                error = %body_text,
                "Azure Anthropic API error"
            );
            anyhow::bail!("Azure Anthropic API error ({status}): {body_text}");
        }

        // Reuse the Anthropic SSE stream parser by delegating to the inner client.
        // The AnthropicClient::chat method is what we want, but it hardcodes
        // "x-api-key" and its own URL.  Instead, we inline the response handling
        // below — but since the response format is identical, we can call the
        // private helper logic by constructing an identical response and passing
        // it through.
        //
        // Simplification: we build an identical async_stream here that mirrors
        // AnthropicClient::chat after the HTTP response is received.
        use crate::llm::StreamEvent;
        use futures::StreamExt;
        use ragent_types::event::FinishReason;
        use serde_json::Value;
        use std::collections::HashMap;

        let rate_limit_event =
            crate::provider::anthropic::parse_anthropic_rate_limit_headers(response.headers());
        let stream = response.bytes_stream();

        let event_stream = async_stream::stream! {
            let mut buffer = String::new();
            let mut current_event_type = String::new();
            let mut tool_call_args: HashMap<String, String> = HashMap::new();

            if let Some(ev) = rate_limit_event {
                yield ev;
            }

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
                                            let _idx = parsed["index"].as_u64().unwrap_or(0);
                                            if let Some((id, _args)) = tool_call_args.iter_mut().last() {
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
