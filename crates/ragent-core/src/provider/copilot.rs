//! GitHub Copilot provider implementation.
//!
//! Connects to the GitHub Copilot API at `https://api.githubcopilot.com` using
//! its OpenAI-compatible `/v1/chat/completions` endpoint. Supports streaming
//! responses, tool calls, and dynamic model discovery via `/models`.
//!
//! # Authentication
//!
//! Copilot uses a two-step authentication flow:
//!
//! 1. A GitHub token (OAuth `ghu_`/`gho_`, or fine-grained PAT `github_pat_`)
//!    is obtained from an environment variable, the IDE config, or manual entry.
//! 2. That token is **exchanged** at `api.github.com/copilot_internal/v2/token`
//!    for a short-lived Copilot session token (JWT).
//! 3. The session token is used as `Authorization: Bearer <jwt>` for all
//!    Copilot API requests.
//!
//! Token sources (checked in priority order):
//!
//! - `GITHUB_COPILOT_TOKEN` environment variable
//! - IDE config: `~/.config/github-copilot/apps.json` (Linux/macOS)
//!   or `%LOCALAPPDATA%/github-copilot/apps.json` (Windows)
//! - Manually stored via `/provider` setup (database)

use anyhow::{Context, Result, bail};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Mutex;

use crate::config::{Capabilities, Cost};
use crate::event::FinishReason;
use crate::llm::{ChatContent, ChatRequest, ContentPart, LlmClient, StreamEvent, ToolDefinition};
use crate::provider::{ModelInfo, Provider};

/// Default GitHub Copilot API base URL (used with IDE OAuth tokens after
/// token exchange).
const DEFAULT_COPILOT_API_BASE: &str = "https://api.githubcopilot.com";

/// GitHub Models inference endpoint (used as fallback when the Copilot
/// internal token exchange is unavailable, e.g. for `gh` CLI OAuth tokens).
const GITHUB_MODELS_API_BASE: &str = "https://models.inference.ai.azure.com";

/// GitHub API endpoint for exchanging a GitHub token for a Copilot session token.
const COPILOT_TOKEN_ENDPOINT: &str = "https://api.github.com/copilot_internal/v2/token";

/// GitHub OAuth client ID for the "GitHub Copilot" GitHub App.
///
/// This is the same client ID used by VS Code, Neovim, and other third-party
/// Copilot integrations for the device flow OAuth.
const COPILOT_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";

/// Safety margin (seconds) before considering a cached session token expired.
const TOKEN_EXPIRY_BUFFER_SECS: i64 = 60;

/// Resolved authentication for the Copilot API.
///
/// Depending on the source token type, this may either be a session JWT
/// (from the internal token exchange) or the raw GitHub OAuth token (used
/// directly with the GitHub Models API).
pub struct CopilotAuth {
    /// The token to send as `Authorization: Bearer`.
    pub token: String,
    /// The API base URL to use (varies by auth path).
    pub base_url: String,
}

/// Cached Copilot session token with its expiry time.
struct CachedSessionToken {
    /// The short-lived Copilot session JWT.
    token: String,
    /// Unix timestamp (seconds) at which this token expires.
    expires_at: i64,
    /// The GitHub token that was exchanged to produce this session token,
    /// so we can detect when the source token changes.
    source_hash: u64,
    /// API base URL returned by the token exchange endpoint (if any).
    api_base: Option<String>,
}

/// Global cache for the exchanged Copilot session token.
static SESSION_TOKEN_CACHE: Mutex<Option<CachedSessionToken>> = Mutex::new(None);

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
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ragent_core::provider::copilot::CopilotProvider;
    ///
    /// let provider = CopilotProvider::new();
    /// ```
    pub fn new() -> Self {
        Self {
            base_url: DEFAULT_COPILOT_API_BASE.to_string(),
        }
    }

    /// Creates a provider pointing at a custom Copilot-compatible endpoint.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ragent_core::provider::copilot::CopilotProvider;
    ///
    /// let provider = CopilotProvider::with_url("https://my-proxy.example.com/copilot");
    /// ```
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
    /// The `api_key` should be a GitHub token (OAuth or fine-grained PAT).
    /// Authentication is resolved via [`resolve_copilot_auth`], which tries
    /// the internal token exchange first and falls back to the GitHub Models
    /// API when the exchange is unavailable.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be constructed or
    /// authentication fails entirely.
    async fn create_client(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        _options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        let auth = resolve_copilot_auth(api_key, base_url).await?;
        let client = CopilotClient {
            token: auth.token,
            base_url: auth.base_url.trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        };
        Ok(Box::new(client))
    }
}

/// HTTP client for the GitHub Copilot API with streaming SSE support.
///
/// Uses the OpenAI-compatible chat completions endpoint. The base URL
/// and token are resolved at construction time via [`resolve_copilot_auth`].
struct CopilotClient {
    /// Bearer token for API requests.
    token: String,
    /// Base URL of the API.
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
    /// Adapts the URL path based on the base URL:
    /// - GitHub Models API → `/chat/completions`
    /// - Copilot individual/default API → `/chat/completions`
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        let url = format!("{}/chat/completions", self.base_url);
        tracing::info!(url = %url, model = %request.model, "copilot chat request");
        let body = self.build_request_body(&request, &request.tools);

        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("content-type", "application/json")
            .header("editor-version", "vscode/1.96.0")
            .header("editor-plugin-version", "copilot-chat/0.24.0")
            .header("user-agent", "GitHubCopilotChat/0.24.0")
            .header("copilot-integration-id", "vscode-chat")
            .header("openai-organization", "github-copilot")
            .header("openai-intent", "conversation-panel")
            .header("x-github-api-version", "2023-07-07")
            .json(&body)
            .send()
            .await
            .context("Failed to connect to GitHub Copilot API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            tracing::error!(status = %status, body = %error_body, "copilot chat error");
            // Extract a clean message from the JSON error response if possible
            let clean_msg =
                parse_api_error_message(&error_body).unwrap_or_else(|| format!("HTTP {status}"));
            bail!("{clean_msg}");
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
///
/// # Examples
///
/// ```no_run
/// use ragent_core::provider::copilot::find_copilot_token;
///
/// if let Some(token) = find_copilot_token() {
///     println!("Found Copilot token: {}", &token[..8]);
/// }
/// ```
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

/// Attempts to get a GitHub OAuth token from the `gh` CLI.
///
/// Runs `gh auth token` and returns the token if available. Only returns
/// OAuth tokens (`gho_` / `ghu_`); fine-grained PATs are filtered out
/// since they cannot be used with the Copilot internal API.
///
/// # Examples
///
/// ```no_run
/// use ragent_core::provider::copilot::find_gh_cli_token;
///
/// if let Some(token) = find_gh_cli_token() {
///     println!("Got token from gh CLI: {}", &token[..8]);
/// }
/// ```
pub fn find_gh_cli_token() -> Option<String> {
    let output = std::process::Command::new("gh")
        .args(["auth", "token"])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        return None;
    }
    // Only accept OAuth tokens; PATs can't use copilot_internal
    if token.starts_with("github_pat_") || token.starts_with("ghp_") {
        return None;
    }
    Some(token)
}

/// Returns `true` if the token looks like a fine-grained or classic PAT
/// (which cannot be used with the Copilot internal API).
///
/// # Examples
///
/// ```no_run
/// use ragent_core::provider::copilot::is_pat_token;
///
/// assert!(is_pat_token("github_pat_abc123"));
/// assert!(is_pat_token("ghp_xxxx"));
/// assert!(!is_pat_token("ghu_xxxx"));
/// ```
pub fn is_pat_token(token: &str) -> bool {
    token.starts_with("github_pat_") || token.starts_with("ghp_")
}

/// Resolves a Copilot-compatible GitHub token from all available sources.
///
/// Priority: env var → IDE auto-discover → `gh` CLI → database.
/// Returns `None` if no valid token is found.
///
/// # Examples
///
/// ```no_run
/// use ragent_core::provider::copilot::resolve_copilot_github_token;
///
/// // Resolve without a database lookup fallback.
/// let token = resolve_copilot_github_token(None);
///
/// // Resolve with a database lookup fallback.
/// let db_lookup = || Some("ghu_my_stored_token".to_string());
/// let token = resolve_copilot_github_token(Some(&db_lookup));
/// ```
pub fn resolve_copilot_github_token(
    db_lookup: Option<&dyn Fn() -> Option<String>>,
) -> Option<String> {
    // 1. GITHUB_COPILOT_TOKEN env var
    if let Ok(token) = std::env::var("GITHUB_COPILOT_TOKEN") {
        if !token.is_empty() {
            return Some(token);
        }
    }
    // 2. IDE auto-discover (apps.json)
    if let Some(token) = find_copilot_token() {
        return Some(token);
    }
    // 3. gh CLI
    if let Some(token) = find_gh_cli_token() {
        return Some(token);
    }
    // 4. Database (if provided)
    if let Some(lookup) = db_lookup {
        if let Some(token) = lookup() {
            if !token.is_empty() {
                return Some(token);
            }
        }
    }
    None
}

/// Extracts a human-readable error message from a JSON API error body.
///
/// Handles the common `{"error":{"message":"..."}}` format returned by
/// the GitHub Models and Copilot APIs. Returns `None` if the body is not
/// valid JSON or has no recognisable message field.
fn parse_api_error_message(body: &str) -> Option<String> {
    let parsed: Value = serde_json::from_str(body).ok()?;
    // Try {"error":{"message":"..."}}
    if let Some(msg) = parsed
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
    {
        return Some(msg.to_string());
    }
    // Try {"message":"..."}
    if let Some(msg) = parsed.get("message").and_then(|m| m.as_str()) {
        return Some(msg.to_string());
    }
    // Try {"error":"..."}
    if let Some(msg) = parsed.get("error").and_then(|e| e.as_str()) {
        return Some(msg.to_string());
    }
    None
}

// ── Device Flow OAuth ────────────────────────────────────────────

/// Response from `github.com/login/device/code`.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceFlowStart {
    /// Opaque device code used when polling for the access token.
    pub device_code: String,
    /// Short code the user enters at the verification URL.
    pub user_code: String,
    /// URL the user must visit to authorise (typically `https://github.com/login/device`).
    pub verification_uri: String,
    /// How many seconds before this device code expires.
    pub expires_in: u64,
    /// Minimum polling interval in seconds.
    pub interval: u64,
}

/// Initiates the GitHub device flow for Copilot OAuth.
///
/// Returns the device code and user-facing instructions. The caller should
/// display `user_code` and `verification_uri` to the user, then poll with
/// [`poll_copilot_device_flow`].
///
/// # Examples
///
/// ```no_run
/// use ragent_core::provider::copilot::start_copilot_device_flow;
///
/// # async fn example() -> anyhow::Result<()> {
/// let flow = start_copilot_device_flow().await?;
/// println!("Go to {} and enter code: {}", flow.verification_uri, flow.user_code);
/// # Ok(())
/// # }
/// ```
pub async fn start_copilot_device_flow() -> Result<DeviceFlowStart> {
    let http = reqwest::Client::new();
    let resp = http
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[("client_id", COPILOT_CLIENT_ID), ("scope", "")])
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .context("Failed to start Copilot device flow")?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        bail!("Device flow initiation failed: {body}");
    }

    resp.json::<DeviceFlowStart>()
        .await
        .context("Failed to parse device flow response")
}

/// Polls the GitHub OAuth token endpoint until the user authorises (or the
/// code expires).
///
/// Returns the access token on success. Callers should respect `interval`
/// from [`DeviceFlowStart`] between calls.
///
/// # Returns
///
/// - `Ok(Some(token))` — user authorised, token is ready
/// - `Ok(None)` — user hasn't authorised yet, keep polling
/// - `Err(...)` — the code expired, was denied, or a network error occurred
///
/// # Examples
///
/// ```no_run
/// use ragent_core::provider::copilot::poll_copilot_device_flow;
///
/// # async fn example() -> anyhow::Result<()> {
/// let device_code = "abc123";
/// match poll_copilot_device_flow(device_code).await? {
///     Some(token) => println!("Authorised! Token: {}", &token[..8]),
///     None => println!("Still waiting for user authorisation..."),
/// }
/// # Ok(())
/// # }
/// ```
pub async fn poll_copilot_device_flow(device_code: &str) -> Result<Option<String>> {
    let http = reqwest::Client::new();
    let resp = http
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", COPILOT_CLIENT_ID),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .context("Device flow poll failed")?;

    let body: Value = resp.json().await.context("Failed to parse poll response")?;

    // Success → token granted
    if let Some(token) = body.get("access_token").and_then(|t| t.as_str()) {
        return Ok(Some(token.to_string()));
    }

    // Still waiting
    let error = body
        .get("error")
        .and_then(|e| e.as_str())
        .unwrap_or("unknown");
    match error {
        "authorization_pending" | "slow_down" => Ok(None),
        "expired_token" => bail!("Device code expired. Please try again."),
        "access_denied" => bail!("Authorization was denied."),
        other => bail!("Device flow error: {other}"),
    }
}

/// Simple hash for comparing whether the source token has changed.
fn hash_token(token: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    token.hash(&mut hasher);
    hasher.finish()
}

/// Result of a Copilot token exchange containing the session JWT and
/// optionally the plan-specific API base URL.
struct TokenExchangeResult {
    token: String,
    api_base: Option<String>,
}

/// Tries the Copilot internal token exchange.
///
/// Returns the session JWT and, when available, the plan-specific API base
/// URL extracted from the exchange response's `endpoints` field.
async fn try_copilot_token_exchange(github_token: &str) -> Result<TokenExchangeResult> {
    let source_hash = hash_token(github_token);

    // Check the cache first
    {
        let cache = SESSION_TOKEN_CACHE.lock().unwrap();
        if let Some(cached) = cache.as_ref() {
            let now = chrono::Utc::now().timestamp();
            if cached.source_hash == source_hash
                && now < cached.expires_at - TOKEN_EXPIRY_BUFFER_SECS
            {
                return Ok(TokenExchangeResult {
                    token: cached.token.clone(),
                    api_base: cached.api_base.clone(),
                });
            }
        }
    }

    let http = reqwest::Client::new();
    let response = http
        .get(COPILOT_TOKEN_ENDPOINT)
        .header("Authorization", format!("token {github_token}"))
        .header("editor-version", "vscode/1.96.0")
        .header("editor-plugin-version", "copilot-chat/0.24.0")
        .header("user-agent", "GitHubCopilotChat/0.24.0")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .context("Copilot token exchange request failed")?;

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_default();
        bail!("Copilot token exchange failed (HTTP {status}): {error_body}");
    }

    let body: CopilotTokenResponse = response
        .json()
        .await
        .context("Failed to parse Copilot token exchange response")?;

    let exchange_api_base = body
        .endpoints
        .and_then(|e| e.api)
        .map(|u| u.trim_end_matches('/').to_string());

    tracing::info!(
        exchange_api_base = ?exchange_api_base,
        "Copilot token exchange endpoints"
    );

    // Cache the new session token (including discovered API base)
    {
        let mut cache = SESSION_TOKEN_CACHE.lock().unwrap();
        *cache = Some(CachedSessionToken {
            token: body.token.clone(),
            expires_at: body.expires_at,
            source_hash,
            api_base: exchange_api_base.clone(),
        });
    }

    Ok(TokenExchangeResult {
        token: body.token,
        api_base: exchange_api_base,
    })
}

/// Resolves Copilot authentication from a GitHub token.
///
/// 1. Tries the internal token exchange (`copilot_internal/v2/token`).
///    Uses the API base from the exchange response's `endpoints` field if
///    available; otherwise falls back to `stored_api_base`, then discovery
///    via `copilot_internal/user` (trying alternative token sources such as
///    the `gh` CLI when the primary token lacks scope), then the default.
/// 2. On failure (e.g. `gh` CLI tokens that lack the internal scope),
///    falls back to using the raw token directly with the GitHub Models
///    API at `models.inference.ai.azure.com`.
///
/// # Examples
///
/// ```no_run
/// use ragent_core::provider::copilot::resolve_copilot_auth;
///
/// # async fn example() -> anyhow::Result<()> {
/// let auth = resolve_copilot_auth("ghu_xxxxxxxxxxxx", None).await?;
/// println!("Using API at {} with token {}", auth.base_url, &auth.token[..8]);
/// # Ok(())
/// # }
/// ```
pub async fn resolve_copilot_auth(
    github_token: &str,
    stored_api_base: Option<&str>,
) -> Result<CopilotAuth> {
    // Try the internal token exchange first
    match try_copilot_token_exchange(github_token).await {
        Ok(exchange) => {
            // Priority: exchange endpoints > non-default stored > discovery > default
            let base_url = if let Some(base) = exchange.api_base {
                base
            } else if let Some(base) = stored_api_base {
                if base == DEFAULT_COPILOT_API_BASE {
                    // The default URL may be a stale fallback; try discovery.
                    // The device flow token often lacks scope for
                    // copilot_internal/user, so also try the gh CLI token.
                    discover_api_base_multi_source(github_token)
                        .await
                        .unwrap_or_else(|| base.to_string())
                } else {
                    base.to_string()
                }
            } else {
                discover_api_base_multi_source(github_token)
                    .await
                    .unwrap_or_else(|| DEFAULT_COPILOT_API_BASE.to_string())
            };
            tracing::info!(base_url = %base_url, "Copilot auth resolved");
            Ok(CopilotAuth {
                token: exchange.token,
                base_url,
            })
        }
        Err(_) => {
            // Fall back to GitHub Models API (no exchange needed)
            Ok(CopilotAuth {
                token: github_token.to_string(),
                base_url: GITHUB_MODELS_API_BASE.to_string(),
            })
        }
    }
}

/// Tries to discover the plan-specific Copilot API base URL using multiple
/// token sources.  First attempts `copilot_internal/user` with the given
/// token, then falls back to the `gh` CLI token (which typically has broader
/// scope).
async fn discover_api_base_multi_source(primary_token: &str) -> Option<String> {
    if let Some(base) = discover_copilot_api_base(primary_token).await {
        return Some(base);
    }
    // The primary token (e.g. device flow) may lack scope; try gh CLI
    if let Some(gh_token) = find_gh_cli_token() {
        if gh_token != primary_token {
            if let Some(base) = discover_copilot_api_base(&gh_token).await {
                return Some(base);
            }
        }
    }
    None
}

/// Response from the Copilot token exchange endpoint.
#[derive(Debug, Deserialize)]
struct CopilotTokenResponse {
    /// Short-lived Copilot session JWT.
    token: String,
    /// Unix timestamp when the token expires.
    expires_at: i64,
    /// Plan-specific API endpoints (may include `api` base URL).
    #[serde(default)]
    endpoints: Option<CopilotTokenEndpoints>,
}

/// Endpoints returned in the Copilot token exchange response.
#[derive(Debug, Deserialize)]
struct CopilotTokenEndpoints {
    /// Base API URL (e.g. `https://api.individual.githubcopilot.com`).
    #[serde(default)]
    api: Option<String>,
}

/// Checks whether a GitHub token can authenticate to a Copilot-compatible API.
///
/// Tries the plan-specific API endpoint first (via `copilot_internal/user`),
/// then falls back to the GitHub Models API. Returns `true` if any models
/// endpoint responds successfully.
///
/// # Examples
///
/// ```no_run
/// use ragent_core::provider::copilot::check_copilot_health;
///
/// # async fn example() {
/// let healthy = check_copilot_health("ghu_xxxxxxxxxxxx").await;
/// println!("Copilot API healthy: {healthy}");
/// # }
/// ```
pub async fn check_copilot_health(github_token: &str) -> bool {
    let http = reqwest::Client::new();

    // Try the user's plan-specific API first
    if let Some(api_base) = discover_copilot_api_base(github_token).await {
        let url = format!("{api_base}/models");
        if let Ok(resp) = http
            .get(&url)
            .header("Authorization", format!("Bearer {github_token}"))
            .header("editor-version", "ragent/0.1.0")
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            if resp.status().is_success() {
                return true;
            }
        }
    }

    // Fall back to GitHub Models API
    let url = format!("{GITHUB_MODELS_API_BASE}/models");
    http.get(&url)
        .header("Authorization", format!("Bearer {github_token}"))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
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
    name: Option<String>,
    #[serde(default)]
    model_picker_enabled: bool,
    vendor: Option<String>,
    #[serde(default)]
    capabilities: Option<CopilotModelCapabilities>,
}

/// Capability metadata for a Copilot model.
#[derive(Debug, Deserialize)]
struct CopilotModelCapabilities {
    #[serde(default)]
    limits: Option<CopilotModelLimits>,
    #[serde(default)]
    supports: Option<CopilotModelSupports>,
    /// Model type: `"chat"`, `"embeddings"`, etc.
    #[serde(rename = "type")]
    model_type: Option<String>,
}

/// Token limits for a Copilot model.
#[derive(Debug, Deserialize)]
struct CopilotModelLimits {
    #[serde(default)]
    max_context_window_tokens: usize,
    #[serde(default)]
    max_output_tokens: usize,
}

/// Feature support flags for a Copilot model.
#[derive(Debug, Deserialize)]
struct CopilotModelSupports {
    #[serde(default)]
    tool_calls: bool,
    #[serde(default)]
    vision: bool,
    #[serde(default)]
    streaming: bool,
    #[serde(default)]
    reasoning_effort: Option<Vec<String>>,
}

/// Response from the `copilot_internal/user` endpoint.
#[derive(Debug, Deserialize)]
struct CopilotUserInfo {
    #[serde(default)]
    endpoints: Option<CopilotEndpoints>,
}

/// API endpoints for the user's Copilot plan.
#[derive(Debug, Deserialize)]
struct CopilotEndpoints {
    api: Option<String>,
}

/// Discovers the Copilot API base URL for the current user's plan.
///
/// Queries `api.github.com/copilot_internal/user` and returns the
/// plan-specific API endpoint (e.g. `api.individual.githubcopilot.com`
/// for Pro Plus subscribers).
///
/// # Examples
///
/// ```no_run
/// use ragent_core::provider::copilot::discover_copilot_api_base;
///
/// # async fn example() {
/// if let Some(base) = discover_copilot_api_base("ghu_xxxxxxxxxxxx").await {
///     println!("Plan-specific API base: {base}");
/// }
/// # }
/// ```
pub async fn discover_copilot_api_base(github_token: &str) -> Option<String> {
    let http = reqwest::Client::new();
    let resp = http
        .get("https://api.github.com/copilot_internal/user")
        .header("Authorization", format!("token {github_token}"))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        tracing::debug!(
            status = %resp.status(),
            "copilot_internal/user discovery failed"
        );
        return None;
    }
    let info: CopilotUserInfo = resp.json().await.ok()?;
    let result = info
        .endpoints
        .and_then(|e| e.api)
        .map(|u| u.trim_end_matches('/').to_string());
    tracing::debug!(api_base = ?result, "copilot API base discovery result");
    result
}

/// Fetches the list of available Copilot models dynamically from the API.
///
/// Discovers the user's plan-specific API endpoint, then queries `/models`
/// to get the full catalogue. Only returns chat-capable models that are
/// enabled in the model picker. Falls back to `default_models()` on failure.
///
/// # Examples
///
/// ```no_run
/// use ragent_core::provider::copilot::list_copilot_models;
///
/// # async fn example() -> anyhow::Result<()> {
/// let models = list_copilot_models("ghu_xxxxxxxxxxxx").await?;
/// for m in &models {
///     println!("{}: {}", m.id, m.name);
/// }
/// # Ok(())
/// # }
/// ```
pub async fn list_copilot_models(github_token: &str) -> Result<Vec<ModelInfo>> {
    // Discover the user-specific API base, fall back to default
    let api_base = discover_api_base_multi_source(github_token)
        .await
        .unwrap_or_else(|| DEFAULT_COPILOT_API_BASE.to_string());

    let url = format!("{api_base}/models");
    let http = reqwest::Client::new();
    let resp = http
        .get(&url)
        .header("Authorization", format!("Bearer {github_token}"))
        .header("editor-version", "ragent/0.1.0")
        .header("editor-plugin-version", "ragent/0.1.0")
        .header("user-agent", "ragent/0.1.0")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .context("Failed to fetch Copilot models")?;

    if !resp.status().is_success() {
        bail!("Copilot models endpoint returned HTTP {}", resp.status());
    }

    let listing: CopilotModelsResponse = resp
        .json()
        .await
        .context("Failed to parse Copilot models response")?;

    let mut models: Vec<ModelInfo> = listing
        .data
        .into_iter()
        .filter(|entry| {
            // Only include chat models enabled in the model picker
            let is_chat = entry
                .capabilities
                .as_ref()
                .and_then(|c| c.model_type.as_deref())
                .map_or(false, |t| t == "chat");
            is_chat && entry.model_picker_enabled
        })
        .map(|entry| {
            let caps = entry.capabilities.as_ref();
            let limits = caps.and_then(|c| c.limits.as_ref());
            let supports = caps.and_then(|c| c.supports.as_ref());

            let context_window = limits.map_or(128_000, |l| {
                if l.max_context_window_tokens > 0 {
                    l.max_context_window_tokens
                } else {
                    128_000
                }
            });
            let max_output = limits.map(|l| l.max_output_tokens).filter(|&t| t > 0);

            let has_reasoning = supports
                .and_then(|s| s.reasoning_effort.as_ref())
                .map_or(false, |efforts| !efforts.is_empty());

            let display_name = entry.name.clone().unwrap_or_else(|| entry.id.clone());

            let vendor_suffix = entry
                .vendor
                .as_deref()
                .map(|v| format!(" ({v})"))
                .unwrap_or_default();

            ModelInfo {
                id: entry.id,
                provider_id: "copilot".to_string(),
                name: format!("{display_name}{vendor_suffix}"),
                cost: Cost {
                    input: 0.0,
                    output: 0.0,
                },
                capabilities: Capabilities {
                    reasoning: has_reasoning,
                    streaming: supports.map_or(true, |s| s.streaming),
                    vision: supports.map_or(false, |s| s.vision),
                    tool_use: supports.map_or(true, |s| s.tool_calls),
                },
                context_window,
                max_output,
            }
        })
        .collect();

    // Sort: larger context / newer models first
    models.sort_by(|a, b| {
        b.context_window
            .cmp(&a.context_window)
            .then_with(|| a.name.cmp(&b.name))
    });

    // Deduplicate by model ID (API sometimes returns duplicates)
    models.dedup_by(|a, b| a.id == b.id);

    Ok(models)
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
