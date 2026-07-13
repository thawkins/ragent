//! Rig-backed completion provider adapter (FR-004 / FR-005 / FR-012).
//!
//! This module wires ragent's [`LlmClient`] trait onto Rig's
//! [`CompletionModel`] / [`StreamingCompletionModel`] traits. It is compiled
//! whenever at least one `provider-*` feature is enabled.
//!
//! # Architecture
//!
//! The flow (established in T-003) is:
//!
//! ```text
//! ragent ChatRequest
//!     │
//!     ▼  chat_request_to_rig()        ← pure mapping (FR-004)
//! Rig CompletionRequest
//!     │
//!     ▼  provider model.stream() / .completion()
//! Rig StreamingChoice | CompletionResponse
//!     │
//!     ▼  streaming_choice_to_chunk() / completion_response_to_chunks()
//! RigStreamChunk
//!     │
//!     ▼  chunk_to_stream_event()       ← pure mapping (FR-005 / FR-013)
//! StreamEvent
//! ```
//!
//! Two concrete types implement the internal [`CompletionBackend`] trait
//! (defined in [`crate::completion`]):
//!
//! * [`RigCompletionBackend`] — holds the provider alias and a boxed
//!   streaming closure that owns the concrete Rig provider model. The closure
//!   is constructed by a per-provider builder (e.g. [`build_openai_backend`])
//!   so that each provider's concrete `CompletionModel` type is handled at
//!   wiring time, avoiding the need for a (non-object-safe) `dyn
//!   CompletionModel`.
//! * [`RigLlmClient`] — a thin wrapper that implements ragent's
//!   [`LlmClient`] trait by delegating to a `Box<dyn CompletionBackend>` and
//!   mapping each [`RigStreamChunk`] onto a [`StreamEvent`] via
//!   [`crate::completion::chunk_to_stream_event`].

use std::pin::Pin;

use futures::{Stream, StreamExt};
use ragent_llm::llm::{ChatRequest, LlmClient, StreamEvent};
use ragent_types::event::FinishReason;
use ragent_types::llm::{ChatContent, ChatMessage, ContentPart};
use rig::completion::message::{
    Text as RigText, ToolCall as RigToolCall, ToolFunction as RigToolFunction,
    ToolResult as RigToolResult, ToolResultContent as RigToolResultContent, UserContent,
};
use rig::completion::{
    AssistantContent, CompletionModel as RigCompletionModel, CompletionRequest,
    Message as RigMessage,
};
use rig::one_or_many::OneOrMany;
use rig::streaming::StreamingChoice;
// `StreamingCompletionModel` is only needed by providers that implement the
// streaming sub-trait (Anthropic). Importing it unconditionally triggers an
// unused-import warning when only non-streaming providers are compiled.
#[cfg(feature = "provider-anthropic")]
use rig::streaming::StreamingCompletionModel;

use crate::completion::{CompletionBackend, RigStreamChunk, chunk_to_stream_event};
use crate::error::{Result, RigError};

// ── ragent → Rig request mapping ─────────────���─────────────────────────────

/// Convert ragent's [`ChatRequest`] into the pieces Rig needs: the system
/// preamble, the ordered chat history, and the final prompt message.
///
/// The last message in `request.messages` becomes the Rig `prompt`; every
/// preceding message becomes `chat_history`. The ragent `system` field maps
/// to the Rig `preamble`.
///
/// Tool definitions, sampling parameters, and provider options are returned
/// alongside so the caller can populate the [`CompletionRequest`] without
/// re-deriving them.
#[must_use]
pub fn chat_request_to_rig(
    request: &ChatRequest,
) -> (
    Option<String>,
    Vec<RigMessage>,
    RigMessage,
    Vec<rig::completion::ToolDefinition>,
    Option<f64>,
    Option<u64>,
    Option<serde_json::Value>,
) {
    let preamble = request.system.as_ref().map(|s| (*s).to_string());

    let messages: Vec<&ChatMessage> = request.messages.iter().collect();
    let (history_refs, prompt_ref): (&[&ChatMessage], &ChatMessage) = match messages.split_last() {
        Some((last, rest)) => (rest, *last),
        None => (
            &[] as &[&ChatMessage],
            &ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text(String::new()),
            },
        ),
    };

    let history = history_refs
        .iter()
        .map(|m| convert_chat_message(m))
        .collect::<Result<Vec<_>>>()
        .unwrap_or_default();

    let prompt = convert_chat_message(prompt_ref).unwrap_or_else(|_| RigMessage::user(""));

    let tools = request
        .tools
        .iter()
        .map(|t| rig::completion::ToolDefinition {
            name: t.name.clone(),
            description: t.description.clone(),
            parameters: t.parameters.clone(),
        })
        .collect();

    let temperature = request.temperature.map(f64::from);
    let max_tokens = request.max_tokens.map(u64::from);
    let additional_params = if request.options.is_empty() {
        None
    } else {
        Some(serde_json::to_value(&request.options).unwrap_or(serde_json::Value::Null))
    };

    (
        preamble,
        history,
        prompt,
        tools,
        temperature,
        max_tokens,
        additional_params,
    )
}

/// Convert a single ragent [`ChatMessage`] into a Rig [`Message`].
fn convert_chat_message(msg: &ChatMessage) -> Result<RigMessage> {
    match msg.content.clone() {
        ChatContent::Text(text) => match msg.role.as_str() {
            "assistant" => Ok(RigMessage::assistant(text)),
            _ => Ok(RigMessage::user(text)),
        },
        ChatContent::Parts(parts) => {
            // Partition parts into text / tool-use (assistant) or
            // text / tool-result (user).
            if msg.role == "assistant" {
                let mut assistant_parts: Vec<AssistantContent> = Vec::new();
                for part in parts {
                    match part {
                        ContentPart::Text { text } => {
                            assistant_parts.push(AssistantContent::Text(RigText { text }));
                        }
                        ContentPart::ToolUse { id, name, input } => {
                            assistant_parts.push(AssistantContent::ToolCall(RigToolCall {
                                id,
                                function: RigToolFunction {
                                    name,
                                    arguments: input,
                                },
                            }));
                        }
                        ContentPart::ToolResult { .. } | ContentPart::ImageUrl { .. } => {
                            // Tool results and images are not valid in
                            // assistant messages; skip them defensively.
                        }
                    }
                }
                if assistant_parts.is_empty() {
                    return Ok(RigMessage::assistant(""));
                }
                let content = OneOrMany::many(assistant_parts)
                    .map_err(|e| RigError::BackendError(format!("empty assistant content: {e}")))?;
                Ok(RigMessage::Assistant { content })
            } else {
                // user / tool role → user message with text + tool results.
                let mut user_parts: Vec<UserContent> = Vec::new();
                for part in parts {
                    match part {
                        ContentPart::Text { text } => {
                            user_parts.push(UserContent::Text(RigText { text }));
                        }
                        ContentPart::ToolResult {
                            tool_use_id,
                            content,
                        } => {
                            user_parts.push(UserContent::ToolResult(RigToolResult {
                                id: tool_use_id,
                                content: OneOrMany::one(RigToolResultContent::Text(RigText {
                                    text: (*content).to_string(),
                                })),
                            }));
                        }
                        ContentPart::ToolUse { .. } | ContentPart::ImageUrl { .. } => {
                            // Tool uses and images in a user message are
                            // ignored for now; they are rare and not all Rig
                            // providers support them.
                        }
                    }
                }
                if user_parts.is_empty() {
                    return Ok(RigMessage::user(""));
                }
                let content = OneOrMany::many(user_parts)
                    .map_err(|e| RigError::BackendError(format!("empty user content: {e}")))?;
                Ok(RigMessage::User { content })
            }
        }
    }
}

// ── Rig response → RigStreamChunk mapping ──────────────────────────────────

/// Map a Rig streaming [`StreamingChoice`] onto one or more
/// [`RigStreamChunk`]s.
///
/// `StreamingChoice::Message` becomes a single [`RigStreamChunk::TextDelta`].
/// `StreamingChoice::ToolCall` is expanded into a start/delta/end triple so
/// that ragent's tool-call state machine receives the same lifecycle events
/// that native providers emit.
#[must_use]
pub fn streaming_choice_to_chunks(choice: StreamingChoice) -> Vec<RigStreamChunk> {
    match choice {
        StreamingChoice::Message(text) => vec![RigStreamChunk::TextDelta { text }],
        StreamingChoice::ToolCall(name, id, params) => {
            let args_json = params.to_string();
            vec![
                RigStreamChunk::ToolCallStart {
                    id: id.clone(),
                    name,
                },
                RigStreamChunk::ToolCallDelta {
                    id: id.clone(),
                    args_json,
                },
                RigStreamChunk::ToolCallEnd { id },
            ]
        }
    }
}

/// Map a non-streaming Rig completion response's `choice` (one or more
/// [`AssistantContent`]) onto a sequence of [`RigStreamChunk`]s, ending with
/// a [`RigStreamChunk::Finish`].
///
/// Text content becomes one or more `TextDelta`s; each tool call becomes a
/// start/delta/end triple. This lets the adapter use the non-streaming
/// `CompletionModel::completion()` path for providers that do not implement
/// [`StreamingCompletionModel`] (e.g. OpenAI in rig-core 0.9.x).
#[must_use]
pub fn completion_response_to_chunks(choice: OneOrMany<AssistantContent>) -> Vec<RigStreamChunk> {
    let mut chunks = Vec::new();
    for content in choice {
        match content {
            AssistantContent::Text(RigText { text }) => {
                if !text.is_empty() {
                    chunks.push(RigStreamChunk::TextDelta { text });
                }
            }
            AssistantContent::ToolCall(RigToolCall {
                id,
                function: RigToolFunction { name, arguments },
            }) => {
                let args_json = arguments.to_string();
                chunks.push(RigStreamChunk::ToolCallStart {
                    id: id.clone(),
                    name,
                });
                chunks.push(RigStreamChunk::ToolCallDelta {
                    id: id.clone(),
                    args_json,
                });
                chunks.push(RigStreamChunk::ToolCallEnd { id });
            }
        }
    }
    chunks.push(RigStreamChunk::Finish {
        reason: if chunks
            .iter()
            .any(|c| matches!(c, RigStreamChunk::ToolCallEnd { .. }))
        {
            FinishReason::ToolUse
        } else {
            FinishReason::Stop
        },
    });
    chunks
}

// ── CompletionBackend impl ──────────────────────────────────────────────────

/// A boxed streaming closure that owns a concrete Rig provider model and
/// produces [`RigStreamChunk`]s for a given [`ChatRequest`].
///
/// `pub(crate)` so the `testing` module (T-014) can build a
/// [`RigCompletionBackend`] from a mock model without re-declaring the type.
pub(crate) type StreamFn = Box<
    dyn Fn(ChatRequest) -> Pin<Box<dyn Stream<Item = Result<RigStreamChunk>> + Send>> + Send + Sync,
>;

/// A Rig-backed completion backend that wraps a provider-specific streaming
/// closure.
///
/// The closure is constructed by a per-provider builder (e.g.
/// [`build_openai_backend`]) so that each provider's concrete
/// [`RigCompletionModel`] type is handled at wiring time. This avoids the need
/// for a (non-object-safe) `dyn CompletionModel`.
pub struct RigCompletionBackend {
    alias: String,
    streaming: bool,
    stream_fn: StreamFn,
}

impl std::fmt::Debug for RigCompletionBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RigCompletionBackend")
            .field("alias", &self.alias)
            .field("streaming", &self.streaming)
            .finish_non_exhaustive()
    }
}

impl RigCompletionBackend {
    /// Construct a streaming backend from a provider alias and a streaming
    /// closure.
    ///
    /// The closure receives a [`ChatRequest`] and returns a stream of
    /// [`RigStreamChunk`]s. Use the per-provider builders
    /// ([`build_openai_backend`], [`build_anthropic_backend`], …) to construct
    /// a closure backed by a concrete Rig provider model.
    #[must_use]
    pub fn new_streaming(alias: String, stream_fn: StreamFn) -> Self {
        Self {
            alias,
            streaming: true,
            stream_fn,
        }
    }

    /// Construct a non-streaming backend from a provider alias and a closure.
    ///
    /// Non-streaming backends still return a [`RigStreamChunk`] stream, but
    /// the stream is produced from a single non-streaming `completion()` call
    /// (see [`completion_response_to_chunks`]). The `streaming` flag is
    /// `false` so callers can report that streaming is not supported.
    #[must_use]
    pub fn new_non_streaming(alias: String, stream_fn: StreamFn) -> Self {
        Self {
            alias,
            streaming: false,
            stream_fn,
        }
    }
}

impl CompletionBackend for RigCompletionBackend {
    fn complete(
        &self,
        request: &ChatRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<RigStreamChunk>> + Send>> {
        // The closure owns the provider model; clone the request and hand it
        // over. The request is cheap to clone (Arc<Vec<...>>).
        (self.stream_fn)(request.clone())
    }

    fn alias(&self) -> &str {
        &self.alias
    }

    fn supports_streaming(&self) -> bool {
        self.streaming
    }
}

// ── LlmClient wrapper ───────────────────────────────────────────────────────

/// A ragent [`LlmClient`] backed by a Rig [`CompletionBackend`].
///
/// This is the type that gets registered in ragent's `ProviderRegistry`
/// (T-006) so that a Rig-backed provider is indistinguishable from a native
/// provider to the agent loop, TUI, and server (FR-012).
pub struct RigLlmClient {
    backend: Box<dyn CompletionBackend>,
}

impl std::fmt::Debug for RigLlmClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RigLlmClient")
            .field("alias", &self.backend.alias())
            .field("streaming", &self.backend.supports_streaming())
            .finish_non_exhaustive()
    }
}

impl RigLlmClient {
    /// Construct a `RigLlmClient` from any [`CompletionBackend`].
    #[must_use]
    pub fn new(backend: Box<dyn CompletionBackend>) -> Self {
        Self { backend }
    }

    /// Returns the provider alias this client was constructed with.
    pub fn alias(&self) -> &str {
        self.backend.alias()
    }

    /// Returns whether the underlying backend supports true streaming.
    pub fn supports_streaming(&self) -> bool {
        self.backend.supports_streaming()
    }
}

#[async_trait::async_trait]
impl LlmClient for RigLlmClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>> {
        if !self.backend.supports_streaming() {
            // Non-streaming backends still work — they emit all chunks at
            // once — but log at debug so users know it is a synthesised
            // stream.
            tracing::debug!(
                alias = self.backend.alias(),
                "Rig backend does not support native streaming; synthesising stream"
            );
        }
        let chunk_stream = self.backend.complete(&request);
        // Map Result<RigStreamChunk> → StreamEvent, converting errors into
        // StreamEvent::Error so the agent loop can react uniformly.
        let event_stream = chunk_stream.map(|item| match item {
            Ok(chunk) => chunk_to_stream_event(chunk),
            Err(e) => StreamEvent::Error {
                message: e.to_string(),
            },
        });
        Ok(Box::pin(event_stream))
    }
}

// ── Per-provider builders ──────────────────────────────────────────────────
//
// Each builder constructs the concrete Rig provider client + model, then
// returns a `RigCompletionBackend` whose closure owns the model. Providers
// that implement `StreamingCompletionModel` use `.stream()`; others use the
// non-streaming `.completion()` path and synthesise chunks.

/// Build a Rig-backed OpenAI completion backend.
///
/// OpenAI's rig-core `CompletionModel` does not implement
/// `StreamingCompletionModel` in 0.9.x, so this backend uses the non-streaming
/// `completion()` path and synthesises a chunk stream from the response.
///
/// # Errors
///
/// Returns [`RigError::BackendError`] if the Rig client or model cannot be
/// constructed.
#[cfg(feature = "provider-openai")]
pub fn build_openai_backend(
    alias: String,
    api_key: &str,
    base_url: Option<&str>,
    model: String,
) -> Result<RigCompletionBackend> {
    use rig::providers::openai;

    let client = match base_url {
        Some(url) => openai::Client::from_url(api_key, url),
        None => openai::Client::new(api_key),
    };
    let rig_model = client.completion_model(&model);

    let stream_fn: StreamFn = Box::new(move |req: ChatRequest| {
        let model = rig_model.clone();
        Box::pin(async_stream::stream! {
            let (preamble, history, prompt, tools, temp, max_tokens, params) =
                chat_request_to_rig(&req);
            let rig_req = CompletionRequest {
                prompt,
                preamble,
                chat_history: history,
                documents: Vec::new(),
                tools,
                temperature: temp,
                max_tokens,
                additional_params: params,
            };
            match model.completion(rig_req).await {
                Ok(resp) => {
                    for chunk in completion_response_to_chunks(resp.choice) {
                        yield Ok(chunk);
                    }
                }
                Err(e) => {
                    yield Err(RigError::BackendError(e.to_string()));
                }
            }
        })
    });
    Ok(RigCompletionBackend::new_non_streaming(alias, stream_fn))
}

/// Build a Rig-backed Anthropic completion backend.
///
/// Anthropic's rig-core `CompletionModel` implements
/// `StreamingCompletionModel`, so this backend uses `.stream()` and maps each
/// [`StreamingChoice`] onto [`RigStreamChunk`]s.
///
/// # Errors
///
/// Returns [`RigError::BackendError`] if the Rig client or model cannot be
/// constructed.
#[cfg(feature = "provider-anthropic")]
pub fn build_anthropic_backend(
    alias: String,
    api_key: &str,
    base_url: Option<&str>,
    model: String,
) -> Result<RigCompletionBackend> {
    use rig::providers::anthropic;

    let client = match base_url {
        Some(url) => anthropic::Client::new(api_key, url, None, "2023-06-01"),
        None => anthropic::Client::from_env(),
    };
    let rig_model = client.completion_model(&model);

    let stream_fn: StreamFn = Box::new(move |req: ChatRequest| {
        let model = rig_model.clone();
        Box::pin(async_stream::stream! {
            let (preamble, history, prompt, tools, temp, max_tokens, params) =
                chat_request_to_rig(&req);
            // Anthropic requires max_tokens; default to a reasonable cap.
            let max_tokens = max_tokens.unwrap_or(4096);
            let rig_req = CompletionRequest {
                prompt,
                preamble,
                chat_history: history,
                documents: Vec::new(),
                tools,
                temperature: temp,
                max_tokens: Some(max_tokens),
                additional_params: params,
            };
            match model.stream(rig_req).await {
                Ok(mut stream) => {
                    while let Some(item) = stream.next().await {
                        match item {
                            Ok(choice) => {
                                for chunk in streaming_choice_to_chunks(choice) {
                                    yield Ok(chunk);
                                }
                            }
                            Err(e) => {
                                yield Err(RigError::BackendError(e.to_string()));
                                break;
                            }
                        }
                    }
                    // Rig does not always emit an explicit finish; append one.
                    yield Ok(RigStreamChunk::Finish {
                        reason: FinishReason::Stop,
                    });
                }
                Err(e) => {
                    yield Err(RigError::BackendError(e.to_string()));
                }
            }
        })
    });
    Ok(RigCompletionBackend::new_streaming(alias, stream_fn))
}

/// Build a Rig-backed Gemini completion backend (non-streaming path).
#[cfg(feature = "provider-gemini")]
pub fn build_gemini_backend(
    alias: String,
    api_key: &str,
    base_url: Option<&str>,
    model: String,
) -> Result<RigCompletionBackend> {
    use rig::providers::gemini;
    let client = match base_url {
        Some(url) => gemini::Client::from_url(api_key, url),
        None => gemini::Client::new(api_key),
    };
    let rig_model = client.completion_model(&model);
    let stream_fn: StreamFn = Box::new(move |req: ChatRequest| {
        let model = rig_model.clone();
        Box::pin(async_stream::stream! {
            let (preamble, history, prompt, tools, temp, max_tokens, params) =
                chat_request_to_rig(&req);
            let rig_req = CompletionRequest {
                prompt, preamble, chat_history: history, documents: Vec::new(),
                tools, temperature: temp, max_tokens, additional_params: params,
            };
            match model.completion(rig_req).await {
                Ok(resp) => {
                    for chunk in completion_response_to_chunks(resp.choice) {
                        yield Ok(chunk);
                    }
                }
                Err(e) => { yield Err(RigError::BackendError(e.to_string())); }
            }
        })
    });
    Ok(RigCompletionBackend::new_non_streaming(alias, stream_fn))
}

/// Build a Rig-backed Ollama completion backend (non-streaming path).
#[cfg(feature = "provider-ollama")]
pub fn build_ollama_backend(
    alias: String,
    api_key: &str,
    base_url: Option<&str>,
    model: String,
) -> Result<RigCompletionBackend> {
    use rig::providers::ollama;
    let _ = api_key;
    let client = match base_url {
        Some(url) => ollama::Client::from_url(url),
        None => ollama::Client::new(),
    };
    let rig_model = client.completion_model(&model);
    let stream_fn: StreamFn = Box::new(move |req: ChatRequest| {
        let model = rig_model.clone();
        Box::pin(async_stream::stream! {
            let (preamble, history, prompt, tools, temp, max_tokens, params) =
                chat_request_to_rig(&req);
            let rig_req = CompletionRequest {
                prompt, preamble, chat_history: history, documents: Vec::new(),
                tools, temperature: temp, max_tokens, additional_params: params,
            };
            match model.completion(rig_req).await {
                Ok(resp) => {
                    for chunk in completion_response_to_chunks(resp.choice) {
                        yield Ok(chunk);
                    }
                }
                Err(e) => { yield Err(RigError::BackendError(e.to_string())); }
            }
        })
    });
    Ok(RigCompletionBackend::new_non_streaming(alias, stream_fn))
}

/// Dispatch table: construct a backend by Rig provider name.
///
/// Used by T-006 wiring to turn a `RigProviderConfig` into a concrete
/// [`RigCompletionBackend`] without the caller needing to know which feature
/// flag gates which builder.
///
/// # Errors
///
/// Returns [`RigError::ProviderNotEnabled`] if the requested provider's
/// feature flag is not compiled in, or [`RigError::InvalidConfiguration`] if
/// the provider name is not recognised.
pub fn build_backend_by_provider(
    alias: String,
    provider: &str,
    api_key: &str,
    base_url: Option<&str>,
    model: String,
) -> Result<RigCompletionBackend> {
    match provider {
        #[cfg(feature = "provider-openai")]
        "openai" => build_openai_backend(alias, api_key, base_url, model),
        #[cfg(feature = "provider-anthropic")]
        "anthropic" => build_anthropic_backend(alias, api_key, base_url, model),
        #[cfg(feature = "provider-gemini")]
        "gemini" => build_gemini_backend(alias, api_key, base_url, model),
        #[cfg(feature = "provider-ollama")]
        "ollama" => build_ollama_backend(alias, api_key, base_url, model),
        #[cfg(not(all(
            feature = "provider-openai",
            feature = "provider-anthropic",
            feature = "provider-gemini",
            feature = "provider-ollama",
        )))]
        other => {
            // If the provider is known but its feature is off, report it as
            // not enabled. Unknown names are an invalid configuration.
            if matches!(
                other,
                "openai"
                    | "anthropic"
                    | "gemini"
                    | "ollama"
                    | "cohere"
                    | "deepseek"
                    | "groq"
                    | "huggingface"
                    | "mistral"
                    | "perplexity"
                    | "together"
                    | "xai"
            ) {
                Err(RigError::ProviderNotEnabled(other.to_owned()))
            } else {
                Err(RigError::InvalidConfiguration(format!(
                    "unknown Rig provider: {other}"
                )))
            }
        }
        #[cfg(all(
            feature = "provider-openai",
            feature = "provider-anthropic",
            feature = "provider-gemini",
            feature = "provider-ollama",
        ))]
        other => Err(RigError::InvalidConfiguration(format!(
            "unsupported Rig provider: {other}"
        ))),
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    use ragent_types::llm::{ChatContent, ChatMessage, ContentPart, ToolDefinition};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn make_request(role: &str, text: &str) -> ChatRequest {
        ChatRequest {
            model: "rig-test".to_owned(),
            messages: Arc::new(vec![ChatMessage {
                role: role.to_owned(),
                content: ChatContent::Text(text.to_owned()),
            }]),
            tools: Arc::new(Vec::new()),
            temperature: Some(0.5),
            top_p: None,
            max_tokens: Some(128),
            system: Some(Arc::from("you are helpful")),
            options: HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: None,
            thinking: None,
        }
    }

    #[test]
    fn chat_request_to_rig_splits_prompt_and_history() {
        let request = ChatRequest {
            model: "m".into(),
            messages: Arc::new(vec![
                ChatMessage {
                    role: "user".into(),
                    content: ChatContent::Text("first".into()),
                },
                ChatMessage {
                    role: "assistant".into(),
                    content: ChatContent::Text("hi".into()),
                },
                ChatMessage {
                    role: "user".into(),
                    content: ChatContent::Text("second".into()),
                },
            ]),
            tools: Arc::new(Vec::new()),
            temperature: None,
            top_p: None,
            max_tokens: None,
            system: Some(Arc::from("sys")),
            options: HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: None,
            thinking: None,
        };
        let (preamble, history, prompt, _tools, temp, max_tokens, _params) =
            chat_request_to_rig(&request);
        assert_eq!(preamble.as_deref(), Some("sys"));
        assert_eq!(history.len(), 2);
        assert!(matches!(prompt, RigMessage::User { .. }));
        assert_eq!(temp, None);
        assert_eq!(max_tokens, None);
    }

    #[test]
    fn convert_user_text_message() {
        let msg = ChatMessage {
            role: "user".into(),
            content: ChatContent::Text("hello".into()),
        };
        let rig = convert_chat_message(&msg).expect("convert");
        assert!(matches!(rig, RigMessage::User { .. }));
    }

    #[test]
    fn convert_assistant_text_message() {
        let msg = ChatMessage {
            role: "assistant".into(),
            content: ChatContent::Text("hi".into()),
        };
        let rig = convert_chat_message(&msg).expect("convert");
        assert!(matches!(rig, RigMessage::Assistant { .. }));
    }

    #[test]
    fn convert_assistant_tool_use_parts() {
        let msg = ChatMessage {
            role: "assistant".into(),
            content: ChatContent::Parts(vec![
                ContentPart::Text {
                    text: "calling".into(),
                },
                ContentPart::ToolUse {
                    id: "c1".into(),
                    name: "read".into(),
                    input: serde_json::json!({"path": "x"}),
                },
            ]),
        };
        let rig = convert_chat_message(&msg).expect("convert");
        match rig {
            RigMessage::Assistant { content } => {
                let items: Vec<&AssistantContent> = content.iter().collect();
                assert_eq!(items.len(), 2);
                assert!(matches!(items[0], AssistantContent::Text(_)));
                assert!(matches!(items[1], AssistantContent::ToolCall(_)));
            }
            other => panic!("expected Assistant, got {other:?}"),
        }
    }

    #[test]
    fn convert_user_tool_result_parts() {
        let msg = ChatMessage {
            role: "user".into(),
            content: ChatContent::Parts(vec![ContentPart::ToolResult {
                tool_use_id: "c1".into(),
                content: Arc::from("result text"),
            }]),
        };
        let rig = convert_chat_message(&msg).expect("convert");
        match rig {
            RigMessage::User { content } => {
                let items: Vec<&UserContent> = content.iter().collect();
                assert_eq!(items.len(), 1);
                assert!(matches!(items[0], UserContent::ToolResult(_)));
            }
            other => panic!("expected User, got {other:?}"),
        }
    }

    #[test]
    fn streaming_message_choice_becomes_text_delta() {
        let chunks = streaming_choice_to_chunks(StreamingChoice::Message("hi".into()));
        assert_eq!(chunks.len(), 1);
        assert!(matches!(chunks[0], RigStreamChunk::TextDelta { .. }));
    }

    #[test]
    fn streaming_tool_call_choice_becomes_lifecycle_triple() {
        let chunks = streaming_choice_to_chunks(StreamingChoice::ToolCall(
            "read".into(),
            "c1".into(),
            serde_json::json!({"path": "x"}),
        ));
        assert_eq!(chunks.len(), 3);
        assert!(matches!(chunks[0], RigStreamChunk::ToolCallStart { .. }));
        assert!(matches!(chunks[1], RigStreamChunk::ToolCallDelta { .. }));
        assert!(matches!(chunks[2], RigStreamChunk::ToolCallEnd { .. }));
    }

    /// FR-013 / FR-024: a multi-chunk streaming sequence (text deltas
    /// followed by a tool call) maps onto the ragent event stream in order,
    /// with the tool-call lifecycle triple appearing after the text deltas.
    #[test]
    fn streaming_choice_sequence_preserves_order() {
        let mut all_chunks = Vec::new();
        all_chunks.extend(streaming_choice_to_chunks(StreamingChoice::Message(
            "Hello".into(),
        )));
        all_chunks.extend(streaming_choice_to_chunks(StreamingChoice::Message(
            " world".into(),
        )));
        all_chunks.extend(streaming_choice_to_chunks(StreamingChoice::ToolCall(
            "read".into(),
            "c1".into(),
            serde_json::json!({"path": "x"}),
        )));
        // 2 text deltas + 3 tool-call lifecycle chunks
        assert_eq!(all_chunks.len(), 5);
        assert!(matches!(all_chunks[0], RigStreamChunk::TextDelta { ref text } if text == "Hello"));
        assert!(
            matches!(all_chunks[1], RigStreamChunk::TextDelta { ref text } if text == " world")
        );
        assert!(matches!(
            all_chunks[2],
            RigStreamChunk::ToolCallStart { .. }
        ));
        assert!(matches!(
            all_chunks[3],
            RigStreamChunk::ToolCallDelta { .. }
        ));
        assert!(matches!(all_chunks[4], RigStreamChunk::ToolCallEnd { .. }));
        // Every chunk must map to a StreamEvent without panic (FR-013).
        for chunk in all_chunks {
            let _event = chunk_to_stream_event(chunk);
        }
    }

    /// FR-013: every `RigStreamChunk` variant produced by
    /// `streaming_choice_to_chunks` maps onto the ragent `StreamEvent` enum.
    /// This guards against a future Rig `StreamingChoice` variant being added
    /// without a corresponding `RigStreamChunk` / `StreamEvent` mapping.
    #[test]
    fn streaming_chunks_all_map_to_stream_events() {
        let chunks = streaming_choice_to_chunks(StreamingChoice::Message("hi".into()));
        for chunk in chunks {
            let event = chunk_to_stream_event(chunk);
            assert!(matches!(event, StreamEvent::TextDelta { .. }));
        }
        let chunks = streaming_choice_to_chunks(StreamingChoice::ToolCall(
            "read".into(),
            "c1".into(),
            serde_json::json!({}),
        ));
        let events: Vec<StreamEvent> = chunks.into_iter().map(chunk_to_stream_event).collect();
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], StreamEvent::ToolCallStart { .. }));
        assert!(matches!(events[1], StreamEvent::ToolCallDelta { .. }));
        assert!(matches!(events[2], StreamEvent::ToolCallEnd { .. }));
    }

    #[test]
    fn completion_response_text_becomes_text_delta_plus_finish() {
        let choice = OneOrMany::one(AssistantContent::Text(RigText {
            text: "hello".into(),
        }));
        let chunks = completion_response_to_chunks(choice);
        assert_eq!(chunks.len(), 2);
        assert!(matches!(chunks[0], RigStreamChunk::TextDelta { .. }));
        assert!(matches!(
            chunks[1],
            RigStreamChunk::Finish {
                reason: FinishReason::Stop
            }
        ));
    }

    #[test]
    fn completion_response_tool_call_finishes_with_tool_use_reason() {
        let choice = OneOrMany::one(AssistantContent::ToolCall(RigToolCall {
            id: "c1".into(),
            function: RigToolFunction {
                name: "read".into(),
                arguments: serde_json::json!({"path": "x"}),
            },
        }));
        let chunks = completion_response_to_chunks(choice);
        // start + delta + end + finish
        assert_eq!(chunks.len(), 4);
        assert!(matches!(chunks[3], RigStreamChunk::Finish { .. }));
        match &chunks[3] {
            RigStreamChunk::Finish { reason } => assert_eq!(*reason, FinishReason::ToolUse),
            _ => unreachable!(),
        }
    }

    #[test]
    fn build_backend_by_provider_rejects_unknown_provider() {
        let err = build_backend_by_provider(
            "alias".into(),
            "not-a-provider",
            "key",
            None,
            "model".into(),
        )
        .expect_err("expected error");
        assert!(matches!(err, RigError::InvalidConfiguration(_)));
    }

    #[test]
    fn rig_llm_client_reports_alias_and_streaming_flag() {
        let backend = RigCompletionBackend::new_streaming(
            "rig-test".to_owned(),
            Box::new(|_req| {
                Box::pin(stream::iter(vec![Ok(RigStreamChunk::Finish {
                    reason: FinishReason::Stop,
                })]))
            }),
        );
        let client = RigLlmClient::new(Box::new(backend));
        assert_eq!(client.alias(), "rig-test");
        assert!(client.supports_streaming());
    }

    #[tokio::test]
    async fn rig_llm_client_maps_chunks_to_stream_events() {
        use futures::StreamExt;
        let backend = RigCompletionBackend::new_streaming(
            "rig-test".to_owned(),
            Box::new(|_req| {
                Box::pin(stream::iter(vec![
                    Ok(RigStreamChunk::TextDelta { text: "hi".into() }),
                    Ok(RigStreamChunk::Finish {
                        reason: FinishReason::Stop,
                    }),
                ]))
            }),
        );
        let client = RigLlmClient::new(Box::new(backend));
        let mut stream = client
            .chat(make_request("user", "hello"))
            .await
            .expect("chat");
        let mut events = Vec::new();
        while let Some(ev) = stream.next().await {
            events.push(ev);
        }
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], StreamEvent::TextDelta { .. }));
        assert!(matches!(events[1], StreamEvent::Finish { .. }));
    }

    #[tokio::test]
    async fn rig_llm_client_maps_backend_errors_to_stream_error_events() {
        use futures::StreamExt;
        let backend = RigCompletionBackend::new_streaming(
            "rig-test".to_owned(),
            Box::new(|_req| {
                Box::pin(stream::iter(vec![Err(RigError::BackendError(
                    "boom".into(),
                ))]))
            }),
        );
        let client = RigLlmClient::new(Box::new(backend));
        let mut stream = client
            .chat(make_request("user", "hello"))
            .await
            .expect("chat");
        let ev = stream.next().await.expect("one event");
        match ev {
            StreamEvent::Error { message } => assert!(message.contains("boom")),
            other => panic!("expected Error event, got {other:?}"),
        }
    }

    #[test]
    fn tool_definitions_map_to_rig() {
        let request = ChatRequest {
            model: "m".into(),
            messages: Arc::new(vec![ChatMessage {
                role: "user".into(),
                content: ChatContent::Text("hi".into()),
            }]),
            tools: Arc::new(vec![ToolDefinition {
                name: "read".into(),
                description: "read a file".into(),
                parameters: serde_json::json!({"type": "object"}),
            }]),
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
        let (_preamble, _history, _prompt, tools, _temp, _max, _params) =
            chat_request_to_rig(&request);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "read");
        assert_eq!(tools[0].description, "read a file");
    }
}
