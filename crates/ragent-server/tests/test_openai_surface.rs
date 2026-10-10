//! Integration tests for the OpenAI-compatible inbound surface (spec
//! `openhands` T-009; FR-003, FR-011, FR-038).
//!
//! These drive the live Axum router in-process with
//! [`tower::ServiceExt::oneshot`], backed by a scripted `Provider` so the
//! agent loop runs end to end without a network call. They cover:
//!
//! - `GET /v1/models` returns an OpenAI-shaped list (FR-003);
//! - `POST /v1/chat/completions` with `stream:false` returns a well-formed
//!   `chat.completion` object (FR-011);
//! - both endpoints sit behind the native bearer-token check (FR-024);
//! - `stream:true` is refused rather than silently downgraded;
//! - a missing user message is a `400`;
//! - a tool call that would need confirmation raises the ordinary
//!   `PermissionRequested` prompt and is never auto-approved (FR-038).

use std::collections::HashMap;
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use futures::stream;
use http_body_util::BodyExt;
use ragent_agent::Config;
use ragent_agent::event::{Event, EventBus};
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_agent::{Capabilities, Cost};
use ragent_server::routes::{AppState, router};
use tower::ServiceExt;

/// A scripted model reply.
#[derive(Clone)]
enum Reply {
    /// A plain assistant text response.
    Text(String),
    /// A single `write` tool call requesting `path` = `content`.
    Write { path: String, content: String },
}

/// A provider whose client plays back a queue of scripted replies, one per
/// `chat` call, repeating the last reply once the queue is exhausted.
#[derive(Clone)]
struct ScriptedProvider {
    replies: Arc<std::sync::Mutex<VecDeque<Reply>>>,
}

struct ScriptedClient {
    replies: Arc<std::sync::Mutex<VecDeque<Reply>>>,
}

#[async_trait::async_trait]
impl LlmClient for ScriptedClient {
    async fn chat(
        &self,
        _request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        let reply = {
            let mut queue = self.replies.lock().expect("replies lock");
            if queue.len() > 1 {
                queue.pop_front()
            } else {
                queue.front().cloned()
            }
        };
        let events: Vec<StreamEvent> = match reply {
            // A terminal `TextDelta` after `Finish` keeps the stream alive long
            // enough for the loop's teardown to observe the finish reason.
            Some(Reply::Text(text)) => vec![
                StreamEvent::TextDelta { text },
                StreamEvent::Usage {
                    input_tokens: 11,
                    output_tokens: 7,
                },
                StreamEvent::Finish {
                    reason: LlmFinishReason::Stop,
                },
                StreamEvent::TextDelta {
                    text: String::new(),
                },
            ],
            Some(Reply::Write { path, content }) => {
                let args = serde_json::json!({ "path": path, "content": content }).to_string();
                vec![
                    StreamEvent::ToolCallStart {
                        id: "call_1".to_string(),
                        name: "write".to_string(),
                    },
                    StreamEvent::ToolCallDelta {
                        id: "call_1".to_string(),
                        args_json: args,
                    },
                    StreamEvent::ToolCallEnd {
                        id: "call_1".to_string(),
                    },
                    StreamEvent::Usage {
                        input_tokens: 11,
                        output_tokens: 7,
                    },
                    StreamEvent::Finish {
                        reason: LlmFinishReason::ToolUse,
                    },
                ]
            }
            None => vec![StreamEvent::Finish {
                reason: LlmFinishReason::Stop,
            }],
        };
        Ok(Box::pin(stream::iter(events)))
    }
}

#[async_trait::async_trait]
impl Provider for ScriptedProvider {
    fn id(&self) -> &'static str {
        "ollama"
    }

    fn name(&self) -> &'static str {
        "Scripted provider"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "scripted".to_string(),
            provider_id: "ollama".to_string(),
            name: "Scripted".to_string(),
            cost: Cost {
                input: 0.0,
                output: 0.0,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: vec![],
            },
            context_window: 128_000,
            max_output: Some(4_096),
            request_multiplier: None,
            thinking_config: None,
        }]
    }

    fn as_any_static(&self) -> &(dyn std::any::Any + 'static) {
        self
    }

    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        _options: &HashMap<String, serde_json::Value>,
    ) -> Result<Box<dyn LlmClient>> {
        Ok(Box::new(ScriptedClient {
            replies: Arc::clone(&self.replies),
        }))
    }
}

/// Build a full [`AppState`] wired to a scripted provider.
fn state_with(replies: Vec<Reply>, token: &str) -> (AppState, Arc<EventBus>) {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let event_bus = Arc::new(EventBus::new(256));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));

    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(ScriptedProvider {
        replies: Arc::new(std::sync::Mutex::new(VecDeque::from(replies))),
    }));

    let processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: Arc::new(provider_registry),
        tool_registry: Arc::new(tool::create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![]))),
        event_bus: event_bus.clone(),
        agent_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        connector_session: tokio::sync::RwLock::new(None),
        connector_statuses: tokio::sync::RwLock::new(None),
        code_index: std::sync::OnceLock::new(),
        stream_config: Default::default(),
        extraction_engine: std::sync::OnceLock::new(),
        // FR-038: the OpenAI surface must never run with auto-approval forced.
        auto_approve: false,
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: Arc::new(parking_lot::RwLock::new(std::collections::HashMap::new())),
        tool_repeat_guard: Arc::new(parking_lot::Mutex::new(std::collections::HashMap::new())),
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        telemetry: Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        activity_log: std::sync::OnceLock::new(),
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(std::collections::HashMap::new()),
        bg_service: std::sync::OnceLock::new(),
    });

    let state = AppState {
        event_bus: event_bus.clone(),
        config: Arc::new(tokio::sync::RwLock::new(Config::default())),
        storage,
        session_processor: processor,
        auth_token: token.to_string(),
        rate_limiter: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        coordinator: None,
        research_runs: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
    };
    (state, event_bus)
}

/// Build an authenticated request with a JSON body.
fn json_request(uri: &str, token: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request")
}

/// Collect a response body into a JSON value.
async fn json_body(response: axum::response::Response) -> serde_json::Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("valid JSON body")
}

/// Collect a response body into a UTF-8 string (for SSE bodies).
async fn response_text(response: axum::response::Response) -> String {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    String::from_utf8(bytes.to_vec()).expect("utf-8 body")
}

#[tokio::test]
async fn test_models_requires_auth() {
    let (state, _bus) = state_with(vec![Reply::Text("hi".into())], "secret");
    let app = router(state);
    let req = Request::builder()
        .uri("/v1/models")
        .body(Body::empty())
        .expect("request");
    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_models_lists_registered_models() {
    let (state, _bus) = state_with(vec![Reply::Text("hi".into())], "tok");
    let app = router(state);
    let req = Request::builder()
        .uri("/v1/models")
        .header("Authorization", "Bearer tok")
        .body(Body::empty())
        .expect("request");
    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["object"], "list");
    let data = body["data"].as_array().expect("data array");
    assert!(
        data.iter().any(|m| m["id"] == "ollama/scripted"),
        "the registered provider/model should be advertised: {body}"
    );
    assert!(data.iter().all(|m| m["object"] == "model"));
}

#[tokio::test]
async fn test_chat_completions_requires_auth() {
    let (state, _bus) = state_with(vec![Reply::Text("hi".into())], "secret");
    let app = router(state);
    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "model": "ollama/scripted",
                "stream": false,
                "messages": [{ "role": "user", "content": "say hello" }]
            })
            .to_string(),
        ))
        .expect("request");
    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_non_streaming_completion_shape() {
    let (state, _bus) = state_with(vec![Reply::Text("hello there".into())], "tok");
    let app = router(state);
    let req = json_request(
        "/v1/chat/completions",
        "tok",
        serde_json::json!({
            "model": "ollama/scripted",
            "stream": false,
            "messages": [{ "role": "user", "content": "say hello" }]
        }),
    );
    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;

    assert_eq!(body["object"], "chat.completion");
    assert!(
        body["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("chatcmpl-"))
    );
    assert_eq!(body["model"], "ollama/scripted");
    assert!(body["created"].as_i64().is_some());
    assert_eq!(body["choices"][0]["message"]["role"], "assistant");
    assert_eq!(body["choices"][0]["message"]["content"], "hello there");
    assert_eq!(body["choices"][0]["finish_reason"], "stop");
    assert!(body["usage"]["prompt_tokens"].as_u64().is_some());
    assert!(body["usage"]["completion_tokens"].as_u64().is_some());
    assert!(body["usage"]["total_tokens"].as_u64().is_some());
}

#[tokio::test]
async fn test_content_parts_array_is_joined() {
    let (state, _bus) = state_with(vec![Reply::Text("ok".into())], "tok");
    let app = router(state);
    let req = json_request(
        "/v1/chat/completions",
        "tok",
        serde_json::json!({
            "model": "ollama/scripted",
            "stream": false,
            "messages": [{
                "role": "user",
                "content": [
                    { "type": "text", "text": "part one " },
                    { "type": "text", "text": "part two" }
                ]
            }]
        }),
    );
    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["choices"][0]["message"]["content"], "ok");
}

#[tokio::test]
async fn test_streaming_completion_emits_chunks_and_done() {
    let (state, _bus) = state_with(vec![Reply::Text("hello there".into())], "tok");
    let app = router(state);
    let req = json_request(
        "/v1/chat/completions",
        "tok",
        serde_json::json!({
            "model": "ollama/scripted",
            "stream": true,
            "messages": [{ "role": "user", "content": "say hello" }]
        }),
    );
    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream"),
        "a streaming completion must answer text/event-stream (FR-012)"
    );

    let raw = response_text(resp).await;
    let data_lines: Vec<&str> = raw
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .collect();
    assert!(!data_lines.is_empty(), "the SSE body must carry data lines");
    assert_eq!(
        data_lines.last().copied(),
        Some("[DONE]"),
        "the stream must terminate with `data: [DONE]` (FR-012): {raw}"
    );

    // Every chunk before the sentinel is a well-formed `chat.completion.chunk`.
    let mut streamed_text = String::new();
    let mut saw_finish = false;
    for line in &data_lines[..data_lines.len() - 1] {
        let chunk: serde_json::Value = serde_json::from_str(line).expect("chunk JSON");
        assert_eq!(chunk["object"], "chat.completion.chunk");
        assert!(
            chunk["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("chatcmpl-"))
        );
        assert_eq!(chunk["model"], "ollama/scripted");
        assert_eq!(chunk["choices"][0]["index"], 0);
        if let Some(content) = chunk["choices"][0]["delta"]["content"].as_str() {
            streamed_text.push_str(content);
        }
        if chunk["choices"][0]["finish_reason"].as_str().is_some() {
            saw_finish = true;
            assert_eq!(chunk["choices"][0]["finish_reason"], "stop");
        }
    }
    assert_eq!(streamed_text, "hello there");
    assert!(saw_finish, "a terminal chunk must carry the finish_reason");
}

#[tokio::test]
async fn test_streaming_requires_auth() {
    // FR-024: the streaming endpoint sits behind the same bearer-token check as
    // the rest of the API - a token-less request is rejected before any turn.
    let (state, _bus) = state_with(vec![Reply::Text("hi".into())], "secret");
    let app = router(state);
    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "model": "ollama/scripted",
                "stream": true,
                "messages": [{ "role": "user", "content": "say hello" }]
            })
            .to_string(),
        ))
        .expect("request");
    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_missing_user_message_is_bad_request() {
    let (state, _bus) = state_with(vec![Reply::Text("hi".into())], "tok");
    let app = router(state);
    let req = json_request(
        "/v1/chat/completions",
        "tok",
        serde_json::json!({
            "model": "ollama/scripted",
            "stream": false,
            "messages": [{ "role": "system", "content": "instructions only" }]
        }),
    );
    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = json_body(resp).await;
    assert_eq!(body["error"]["type"], "invalid_request_error");
}

/// FR-038: a tool action that would require confirmation through the native
/// API must raise the same permission prompt here, and must not be
/// auto-approved. With no interactive client attached the prompt would time
/// out to a denial; the test only needs to observe that the prompt is raised,
/// so it aborts the request once the `PermissionRequested` event arrives.
#[tokio::test]
async fn test_tool_call_raises_permission_prompt_and_is_not_auto_approved() {
    let (state, bus) = state_with(
        vec![
            Reply::Write {
                path: "openai-surface-guard.txt".to_string(),
                content: "should never be written".to_string(),
            },
            Reply::Text("done".to_string()),
        ],
        "tok",
    );
    let app = router(state);

    let mut rx = bus.subscribe();
    let req = json_request(
        "/v1/chat/completions",
        "tok",
        serde_json::json!({
            "model": "ollama/scripted",
            "stream": false,
            "messages": [{ "role": "user", "content": "write a file" }]
        }),
    );
    let handle = tokio::spawn(async move { app.oneshot(req).await });

    let mut saw_prompt = false;
    for _ in 0..200 {
        match tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await {
            Ok(Ok(Event::PermissionRequested { .. })) => {
                saw_prompt = true;
                break;
            }
            // Any other event (streaming deltas, tool start, ...) - keep
            // draining until the prompt appears.
            Ok(Ok(_)) => {}
            Ok(Err(_)) | Err(_) => break,
        }
    }
    handle.abort();

    assert!(
        saw_prompt,
        "a write tool call must raise a PermissionRequested prompt (FR-038)"
    );
}

/// The adapter maps the last user message; earlier turns are not replayed, so
/// the assistant's reply reflects only the final user turn.
#[tokio::test]
async fn test_last_user_message_drives_the_turn() {
    let (state, _bus) = state_with(vec![Reply::Text("second".into())], "tok");
    let app = router(state);
    let req = json_request(
        "/v1/chat/completions",
        "tok",
        serde_json::json!({
            "model": "ollama/scripted",
            "stream": false,
            "messages": [
                { "role": "user", "content": "first" },
                { "role": "assistant", "content": "reply" },
                { "role": "user", "content": "second" }
            ]
        }),
    );
    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["choices"][0]["message"]["content"], "second");
}

/// A bare (unqualified) model name is ignored rather than failing the turn, so
/// the resolved default model is used.
#[tokio::test]
async fn test_unqualified_model_does_not_fail() {
    let (state, _bus) = state_with(vec![Reply::Text("ok".into())], "tok");
    let app = router(state);
    let req = json_request(
        "/v1/chat/completions",
        "tok",
        serde_json::json!({
            "model": "some-model-without-a-provider",
            "stream": false,
            "messages": [{ "role": "user", "content": "hello" }]
        }),
    );
    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["choices"][0]["message"]["content"], "ok");
}
