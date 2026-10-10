//! OpenAI-compatible inbound HTTP surface (spec `openhands` FR-003, FR-011,
//! FR-012, FR-024, FR-038).
//!
//! This module is a thin adapter over the existing [`SessionProcessor`] agent
//! loop. It exposes the two endpoints an OpenAI client expects:
//!
//! - `GET  /v1/models`              - the advertised model list (FR-003)
//! - `POST /v1/chat/completions`   - a chat completion bridged to one agent
//!   turn; `stream:false` returns an OpenAI-shaped JSON object (FR-011) and
//!   `stream:true` returns `chat.completion.chunk` `text/event-stream` chunks
//!   terminated by `data: [DONE]` (FR-012)
//!
//! The native REST+SSE API stays canonical (spec assumption A4): this surface
//! does not fork the agent loop, the tool registry, or the permission system.
//! It is mounted inside the auth-protected router so it inherits the same
//! bearer-token check as every other route (FR-024), and it drives the turn
//! through [`SessionProcessor::process_message`], which runs every tool call
//! through the same 7-layer bash validation, containment guard, and permission
//! prompts as a native request. The surface never sets the processor's
//! `auto_approve` flag, so a tool that would require confirmation through the
//! native API requires it here too - and its prompt times out to a denial
//! because no interactive client is attached (FR-038).

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    response::sse::{Event as SseEvent, KeepAlive, Sse},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::error::RecvError;

use ragent_agent::agent::{self, AgentInfo};
use ragent_agent::event::{Event, FinishReason};
use ragent_agent::sanitize::redact_secrets;
use ragent_agent::session::processor::SessionProcessor;

use super::AppState;

/// Upper bound on how long the usage collector waits for the terminal
/// `RunCostSummary` after `process_message` has already returned.
///
/// The collector normally finishes as soon as the loop publishes its final
/// `RunCostSummary`, which happens before `process_message` resolves. The
/// timeout is a safety net against a loop that terminates on an early-return
/// path without publishing one; on expiry the usage already observed is used.
const USAGE_COLLECT_TIMEOUT: Duration = Duration::from_secs(2);

/// Build the OpenAI-compatible sub-router.
///
/// The returned router carries no middleware of its own; it is merged into the
/// auth-protected router by [`super::router`] so both endpoints require the
/// same bearer token as the native API.
pub fn openai_routes() -> Router<AppState> {
    Router::new()
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
}

// -- Response shapes --------------------------------------------------------

/// One entry in the `GET /v1/models` payload.
#[derive(Debug, Serialize)]
struct ModelObject {
    /// Fully-qualified id (`"<provider>/<model>"`) a client echoes back.
    id: String,
    /// Always `"model"` (the OpenAI object discriminator).
    object: &'static str,
    /// Unix creation time; `0` because ragent has no per-model creation time.
    created: i64,
    /// The provider id that owns the model.
    owned_by: String,
}

/// The `GET /v1/models` list envelope.
#[derive(Debug, Serialize)]
struct ModelList {
    /// Always `"list"`.
    object: &'static str,
    /// The advertised models.
    data: Vec<ModelObject>,
}

/// One assistant choice in a non-streaming completion.
#[derive(Debug, Serialize)]
struct Choice {
    /// Zero-based choice index (always `0`).
    index: u32,
    /// The assistant message.
    message: ChatMessage,
    /// Why generation stopped (`"stop"`).
    finish_reason: &'static str,
}

/// An assistant message in an OpenAI completion.
#[derive(Debug, Serialize)]
struct ChatMessage {
    /// Always `"assistant"`.
    role: &'static str,
    /// The reply text.
    content: String,
}

/// Token accounting for a completion.
///
/// The field names are fixed by the OpenAI wire format (`prompt_tokens` /
/// `completion_tokens` / `total_tokens`); the shared `_tokens` postfix is
/// therefore deliberate, not an accident.
#[allow(clippy::struct_field_names)]
#[derive(Debug, Serialize)]
struct Usage {
    /// Prompt (input) tokens.
    prompt_tokens: u64,
    /// Completion (output) tokens.
    completion_tokens: u64,
    /// Sum of the two.
    total_tokens: u64,
}

/// The non-streaming `chat.completion` object (FR-011).
#[derive(Debug, Serialize)]
struct ChatCompletion {
    /// Completion id (`"chatcmpl-<uuid>"`).
    id: String,
    /// Always `"chat.completion"`.
    object: &'static str,
    /// Unix creation time.
    created: i64,
    /// The model that produced the reply.
    model: String,
    /// The single assistant choice.
    choices: Vec<Choice>,
    /// Token usage, best-effort (zero when the provider reported none).
    usage: Usage,
}

/// One streaming choice delta in a `chat.completion.chunk` (FR-012).
#[derive(Debug, Serialize)]
struct ChunkChoice {
    /// Zero-based choice index (always `0`).
    index: u32,
    /// The incremental delta carried by this chunk.
    delta: ChunkDelta,
    /// Set to `"stop"` on the terminal chunk; `null` on every delta chunk.
    finish_reason: Option<&'static str>,
}

/// The incremental assistant delta of a streaming chunk.
///
/// The `role` field is emitted only on the first chunk of a choice (OpenAI
/// semantics); later chunks carry `content` alone and omit `role`.
#[derive(Debug, Default, Serialize)]
struct ChunkDelta {
    /// `"assistant"` on the opening chunk; omitted afterwards.
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<&'static str>,
    /// The assistant text fragment for this chunk (`""` when none).
    content: String,
}

/// The OpenAI `chat.completion.chunk` object streamed for `stream:true`
/// (FR-012).
#[derive(Debug, Serialize)]
struct ChatCompletionChunk {
    /// Completion id (`"chatcmpl-<uuid>"`), shared by every chunk of the run.
    id: String,
    /// Always `"chat.completion.chunk"`.
    object: &'static str,
    /// Unix creation time, shared by every chunk of the run.
    created: i64,
    /// The model that produced the reply.
    model: String,
    /// The single assistant choice delta.
    choices: Vec<ChunkChoice>,
}

/// The OpenAI-shaped error envelope.
#[derive(Debug, Serialize)]
struct OpenAiError {
    /// Error detail.
    error: OpenAiErrorBody,
}

/// Detail block of an [`OpenAiError`].
#[derive(Debug, Serialize)]
struct OpenAiErrorBody {
    /// Human-readable message.
    message: String,
    /// Error class (`"invalid_request_error"` / `"server_error"`).
    r#type: &'static str,
}

/// Build an OpenAI-shaped error response.
fn error(status: StatusCode, kind: &'static str, message: impl Into<String>) -> impl IntoResponse {
    (
        status,
        Json(OpenAiError {
            error: OpenAiErrorBody {
                message: message.into(),
                r#type: kind,
            },
        }),
    )
}

// -- Request shapes ---------------------------------------------------------

/// A `POST /v1/chat/completions` request body.
///
/// Extra OpenAI fields are ignored; only the fields needed to run a turn are
/// read.
#[derive(Debug, Deserialize)]
struct ChatCompletionRequest {
    /// Model id (`"provider/model"` or a bare model name).
    #[serde(default)]
    model: Option<String>,
    /// Conversation messages; the last user message becomes the turn prompt.
    messages: Vec<RequestMessage>,
    /// When `true` the reply is streamed as `chat.completion.chunk` SSE events
    /// (FR-012).
    #[serde(default)]
    stream: bool,
    /// Optional sampling temperature override.
    #[serde(default)]
    temperature: Option<f32>,
    /// Optional nucleus-sampling override.
    #[serde(default)]
    top_p: Option<f32>,
}

/// One request message. `content` is a string or an array of typed parts.
#[derive(Debug, Deserialize)]
struct RequestMessage {
    /// `"system"`, `"user"`, or `"assistant"`.
    #[serde(default)]
    role: String,
    /// Message content (string, or an array of `{type,text}` parts).
    #[serde(default)]
    content: Option<serde_json::Value>,
}

/// Extract the plain text of a request message.
///
/// Handles the two OpenAI content encodings: a bare string, and the typed
/// array form (`[{"type":"text","text":"..."}]`). Non-text parts (e.g. images)
/// are skipped.
fn message_text(content: Option<&serde_json::Value>) -> std::borrow::Cow<'_, str> {
    match content {
        Some(serde_json::Value::String(s)) => std::borrow::Cow::Borrowed(s.as_str()),
        Some(serde_json::Value::Array(parts)) => {
            let mut out = String::new();
            for part in parts {
                if let Some(text) = part.get("text").and_then(serde_json::Value::as_str) {
                    out.push_str(text);
                }
            }
            std::borrow::Cow::Owned(out)
        }
        _ => std::borrow::Cow::Borrowed(""),
    }
}

/// Unix timestamp in seconds, or `0` when the clock is unavailable.
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

// -- Handlers ---------------------------------------------------------------

/// `GET /v1/models` - advertise the configured models (FR-003).
///
/// The list is built from the live provider registry: every registered provider
/// contributes each of its default models under the fully-qualified
/// `"<provider>/<model>"` id. An empty registry yields a well-formed empty
/// list rather than an error.
async fn list_models(State(state): State<AppState>) -> impl IntoResponse {
    let mut data = Vec::new();
    for provider in state.session_processor.provider_registry.list() {
        for model in provider.models {
            data.push(ModelObject {
                id: format!("{}/{}", provider.id, model.id),
                object: "model",
                created: 0,
                owned_by: provider.id.clone(),
            });
        }
    }
    Json(ModelList {
        object: "list",
        data,
    })
}

/// Token usage observed for one session during a single completion.
#[derive(Debug, Default, Clone, Copy)]
struct UsageAccum {
    /// Prompt (input) tokens.
    prompt_tokens: u64,
    /// Completion (output) tokens.
    completion_tokens: u64,
}

/// Spawn a task that records the run's token usage from its `RunCostSummary`.
///
/// Subscribing *before* the turn starts closes the window in which a fast turn
/// could publish its summary before the collector is listening. The returned
/// handle is awaited (with a bound) once the turn has completed.
fn spawn_usage_collector(
    processor: &Arc<SessionProcessor>,
    session_id: &str,
) -> (tokio::task::JoinHandle<()>, Arc<Mutex<UsageAccum>>) {
    let usage = Arc::new(Mutex::new(UsageAccum::default()));
    let mut rx = processor.event_bus.subscribe();
    let sid = session_id.to_string();
    let sink = Arc::clone(&usage);
    let handle = tokio::spawn(async move {
        // Break only once a summary has been observed *and* the turn has ended, so
        // a `MessageEnd` that outruns its `RunCostSummary` does not report zero
        // usage. The caller awaits this handle with a bound, so a turn that never
        // publishes a summary still stops.
        let mut seen_summary = false;
        loop {
            match rx.recv().await {
                Ok(Event::RunCostSummary {
                    session_id,
                    input_tokens,
                    output_tokens,
                    ..
                }) if session_id == sid => {
                    let mut acc = sink
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    acc.prompt_tokens = input_tokens;
                    acc.completion_tokens = output_tokens;
                    seen_summary = true;
                }
                Ok(Event::MessageEnd { session_id, .. }) if session_id == sid => {
                    if seen_summary {
                        break;
                    }
                }
                Ok(_) => {}
                // A lagged subscriber may have dropped intermediate events, but
                // keeping reading lets it observe the newest summary/end pair.
                Err(RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    });
    (handle, usage)
}

/// Resolve the [`AgentInfo`] to run for a request, honouring a `model` override.
///
/// The configured default agent is resolved with its default model; a request
/// `model` in `"provider/model"` form overrides the model when the provider is
/// registered (an unknown provider is ignored so the turn does not fail on a
/// stray model name). `temperature` and `top_p` are applied as overrides.
async fn resolve_agent(state: &AppState, request: &ChatCompletionRequest) -> Arc<AgentInfo> {
    let cfg = state.config.read().await;
    let mut resolved = agent::resolve_agent_with_model(
        &cfg.default_agent,
        &cfg,
        state.session_processor.provider_registry.as_ref(),
    )
    .unwrap_or_else(|e| {
        tracing::warn!(
            error = %redact_secrets(&e.to_string()),
            "OpenAI surface: agent resolution failed; falling back to the general agent"
        );
        Arc::new(AgentInfo::new("general", "General-purpose agent"))
    });
    drop(cfg);

    let info = Arc::make_mut(&mut resolved);
    if let Some(model) = request.model.as_deref()
        && let Some((provider_id, model_id)) = model.split_once('/')
        && state
            .session_processor
            .provider_registry
            .get(provider_id)
            .is_some()
    {
        info.model = Some(agent::ModelRef {
            provider_id: provider_id.to_string(),
            model_id: model_id.to_string(),
        });
        info.model_pinned = true;
    }
    if let Some(temperature) = request.temperature {
        info.temperature = Some(temperature);
    }
    if let Some(top_p) = request.top_p {
        info.top_p = Some(top_p);
    }
    resolved
}

/// The resolved inputs for one OpenAI completion turn.
struct PreparedTurn {
    /// The freshly created local session the turn runs against.
    session_id: String,
    /// The last user message, used as the turn prompt.
    prompt: String,
    /// The agent (and any `model`/`temperature`/`top_p` overrides) to run.
    agent: Arc<AgentInfo>,
    /// The `"<provider>/<model>"` label echoed into the completion objects.
    model_label: String,
}

/// `POST /v1/chat/completions` - run a completion (FR-003, FR-011, FR-012,
/// FR-038).
///
/// Dispatch on the request's `stream` flag: `stream:false` returns one
/// OpenAI `chat.completion` object ([`run_completion`], FR-011); `stream:true`
/// returns a `chat.completion.chunk` `text/event-stream` ending with
/// `data: [DONE]` ([`stream_completion`], FR-012).
///
/// The last user message is bridged to one
/// [`SessionProcessor::process_message`] turn against a freshly created
/// session rooted at the server working directory. Permission checks run
/// unmodified: `auto_approve` is never forced, so a tool that needs
/// confirmation raises the normal `PermissionRequested` prompt and, with no
/// interactive client attached to answer it, times out to a denial.
async fn chat_completions(
    State(state): State<AppState>,
    Json(request): Json<ChatCompletionRequest>,
) -> axum::response::Response {
    let prepared = match prepare_turn(&state, &request).await {
        Ok(prepared) => prepared,
        Err(response) => return *response,
    };

    if request.stream {
        stream_completion(state, prepared)
    } else {
        run_completion(state, prepared).await
    }
}

/// Resolve the turn to run: validate the request, create a session at the
/// server working directory, and resolve the agent plus its model label.
///
/// Returns the caller-facing error response (boxed to keep the `Result` small)
/// on failure: a missing/blank user message is `400`, and an unresolvable
/// working directory or a session-creation failure is `500`. Splitting this out
/// lets the streaming and non-streaming paths share one validation and setup
/// path so they cannot drift.
async fn prepare_turn(
    state: &AppState,
    request: &ChatCompletionRequest,
) -> Result<PreparedTurn, Box<axum::response::Response>> {
    // The turn prompt is the last user message; conversation history is not
    // replayed through this adapter (the native API remains the way to resume
    // a multi-turn session).
    let prompt = request
        .messages
        .iter()
        .rev()
        .find(|m| m.role.eq_ignore_ascii_case("user"))
        .map(|m| message_text(m.content.as_ref()));
    let Some(prompt) = prompt.filter(|p| !p.trim().is_empty()) else {
        return Err(Box::new(
            error(
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                "no non-empty user message supplied",
            )
            .into_response(),
        ));
    };

    // Root the ephemeral session at the server's working directory. OpenAI
    // clients carry no working-directory field; the server process owns the
    // project root, matching the native API's `create_session` contract.
    let directory = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            tracing::error!(error = %e, "OpenAI surface: cannot resolve working directory");
            return Err(Box::new(
                error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "server_error",
                    "cannot resolve the server working directory",
                )
                .into_response(),
            ));
        }
    };

    let session = match state
        .session_processor
        .session_manager
        .create_session(directory)
    {
        Ok(session) => session,
        Err(e) => {
            tracing::error!(
                error = %redact_secrets(&e.to_string()),
                "OpenAI surface: session creation failed"
            );
            return Err(Box::new(
                error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "server_error",
                    "failed to create a session",
                )
                .into_response(),
            ));
        }
    };

    let agent = resolve_agent(state, request).await;
    let model_label = agent.model.as_ref().map_or_else(
        || "ragent".to_string(),
        |m| format!("{}/{}", m.provider_id, m.model_id),
    );

    Ok(PreparedTurn {
        session_id: session.id,
        prompt: prompt.into_owned(),
        agent,
        model_label,
    })
}

/// Run the prepared turn to completion and return an OpenAI `chat.completion`
/// object (FR-011).
async fn run_completion(state: AppState, prepared: PreparedTurn) -> axum::response::Response {
    let processor = Arc::clone(&state.session_processor);
    let (usage_handle, usage) = spawn_usage_collector(&processor, &prepared.session_id);

    // FR-038: never force auto-approval. `auto_approve` stays `false` so every
    // tool call runs through the full permission layer exactly as in a native
    // request. A prompt with no attached client times out to a denial.
    let result = processor
        .process_message(
            &prepared.session_id,
            &prepared.prompt,
            &prepared.agent,
            Arc::new(AtomicBool::new(false)),
        )
        .await;

    // The collector ends on the terminal `RunCostSummary`; bound the wait so a
    // turn that resolves on an early-return path cannot stall the response.
    // A hit timeout only means the loop published no summary, not an error
    // (FR-011: usage is best-effort), so the outcome is deliberately dropped.
    // INTENTIONAL: usage collection is best-effort; a timeout just means no summary was published
    let _ = tokio::time::timeout(USAGE_COLLECT_TIMEOUT, usage_handle).await;
    let (prompt_tokens, completion_tokens) = {
        let acc = usage
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (acc.prompt_tokens, acc.completion_tokens)
    };

    match result {
        Ok(message) => {
            let completion = ChatCompletion {
                id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
                object: "chat.completion",
                created: unix_now(),
                model: prepared.model_label,
                choices: vec![Choice {
                    index: 0,
                    message: ChatMessage {
                        role: "assistant",
                        content: message.text_content(),
                    },
                    finish_reason: "stop",
                }],
                usage: Usage {
                    prompt_tokens,
                    completion_tokens,
                    total_tokens: prompt_tokens.saturating_add(completion_tokens),
                },
            };
            (StatusCode::OK, Json(completion)).into_response()
        }
        Err(e) => {
            tracing::error!(
                session_id = %prepared.session_id,
                error = %redact_secrets(&e.to_string()),
                "OpenAI surface: agent turn failed"
            );
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "server_error",
                "the agent turn failed",
            )
            .into_response()
        }
    }
}

/// Map a ragent loop [`FinishReason`] to its OpenAI `finish_reason` string.
fn map_finish_reason(reason: &FinishReason) -> &'static str {
    match reason {
        FinishReason::Stop | FinishReason::Cancelled => "stop",
        FinishReason::ToolUse => "tool_calls",
        FinishReason::Length | FinishReason::Truncation => "length",
        FinishReason::ContentFilter => "content_filter",
    }
}

/// Build one SSE frame carrying an OpenAI `chat.completion.chunk` (FR-012).
fn chunk_frame(
    id: &str,
    created: i64,
    model: &str,
    role: Option<&'static str>,
    content: String,
    finish_reason: Option<&'static str>,
) -> SseEvent {
    let payload = ChatCompletionChunk {
        id: id.to_string(),
        object: "chat.completion.chunk",
        created,
        model: model.to_string(),
        choices: vec![ChunkChoice {
            index: 0,
            delta: ChunkDelta { role, content },
            finish_reason,
        }],
    };
    // A serialisation failure is impossible for this flat struct; log and fall
    // back to an empty data line rather than panicking on a user-facing path, so
    // a real regression is still visible in the logs.
    let data = serde_json::to_string(&payload).unwrap_or_else(|error| {
        tracing::warn!(%error, "OpenAI surface: failed to serialize a stream chunk; sending an empty object");
        "{}".to_string()
    });
    SseEvent::default().data(data)
}

/// Stream the prepared turn as OpenAI `chat.completion.chunk` `text/event-stream`
/// chunks terminated by `data: [DONE]` (FR-012).
///
/// The turn runs in a spawned task so the SSE response returns immediately; the
/// stream mirrors the session's `TextDelta` events as content chunks and maps
/// the terminal `MessageEnd` reason onto the final chunk's `finish_reason`. The
/// turn's completion is signalled over a oneshot so a turn that fails without
/// publishing a `MessageEnd` still terminates the stream rather than hanging.
fn stream_completion(state: AppState, prepared: PreparedTurn) -> axum::response::Response {
    let processor = Arc::clone(&state.session_processor);
    let id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = unix_now();
    let session_id = prepared.session_id;
    let model_label = prepared.model_label;
    let prompt = prepared.prompt;
    let agent = prepared.agent;

    let body = async_stream::stream! {
        let mut rx = processor.event_bus.subscribe();
        let (turn_tx, mut turn_rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
        {
            let turn_processor = Arc::clone(&processor);
            let turn_session = session_id.clone();
            let turn_agent = Arc::clone(&agent);
            let turn_prompt = prompt.clone();
            tokio::spawn(async move {
                // FR-038: never force auto-approval on the streaming path either.
                let outcome = turn_processor
                    .process_message(
                        &turn_session,
                        &turn_prompt,
                        &turn_agent,
                        Arc::new(AtomicBool::new(false)),
                    )
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string());
                let _ = turn_tx.send(outcome);
            });
        }

        // Opening chunk: carries `delta.role = "assistant"` and empty content,
        // matching the OpenAI streaming contract.
        yield Ok::<_, std::convert::Infallible>(chunk_frame(
            &id,
            created,
            &model_label,
            Some("assistant"),
            String::new(),
            None,
        ));

        let mut finish = "stop";
        // The outcome branch always breaks after draining, so it can be selected
        // at most once per stream: no "already done" guard is needed. Listing it
        // first makes it win the race against the terminal `MessageEnd` event.
        'turn: loop {
            tokio::select! {
                outcome = &mut turn_rx => {
                    match outcome {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => tracing::error!(
                            session_id = %session_id,
                            error = %redact_secrets(&e),
                            "OpenAI surface: streaming turn failed"
                        ),
                        Err(_) => {}
                    }
                    // The turn is over, so every event it will ever publish is
                    // already queued on the bus. Drain them: mirror any deltas
                    // not yet emitted and capture the terminal `MessageEnd`
                    // reason, then end the stream. Draining rather than waiting
                    // is required because the completion signal and the final
                    // `MessageEnd` race - whichever `select!` branch is ready
                    // first - and the turn publishes nothing after `MessageEnd`.
                    // A `Lagged` error means intermediate events were dropped but
                    // the newest are still readable, so keep draining rather than
                    // stopping early and reporting a default `finish_reason`.
                    loop {
                        match rx.try_recv() {
                            Ok(Event::TextDelta { session_id: sid, text }) if sid == session_id => {
                                if !text.is_empty() {
                                    yield Ok(chunk_frame(&id, created, &model_label, None, text, None));
                                }
                            }
                            Ok(Event::MessageEnd { session_id: sid, reason, .. }) if sid == session_id => {
                                finish = map_finish_reason(&reason);
                            }
                            Ok(_) => {}
                            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => continue,
                            Err(_) => break,
                        }
                    }
                    break 'turn;
                }
                event = rx.recv() => {
                    match event {
                        Ok(Event::TextDelta { session_id: sid, text }) if sid == session_id => {
                            if !text.is_empty() {
                                yield Ok(chunk_frame(&id, created, &model_label, None, text, None));
                            }
                        }
                        Ok(Event::MessageEnd { session_id: sid, reason, .. }) if sid == session_id => {
                            finish = map_finish_reason(&reason);
                            break 'turn;
                        }
                        Ok(_) => {}
                        // A lagged subscriber dropped intermediate events; keep
                        // reading so the newest delta/end pair is still observed.
                        Err(RecvError::Lagged(_)) => continue,
                        Err(_) => break 'turn,
                    }
                }
            }
        }

        // Terminal chunk carries the mapped `finish_reason`, then the OpenAI
        // `[DONE]` sentinel closes the stream (FR-012).
        yield Ok(chunk_frame(
            &id,
            created,
            &model_label,
            None,
            String::new(),
            Some(finish),
        ));
        yield Ok(SseEvent::default().data("[DONE]"));
    };

    Sse::new(body)
        .keep_alive(KeepAlive::default())
        .into_response()
}
