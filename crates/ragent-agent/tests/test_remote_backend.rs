//! Integration tests for the remote execution backend (spec `openhands` T-003;
//! FR-001, FR-004, FR-009, FR-021, FR-031, FR-034).
//!
//! The tests split into two groups:
//!
//! - **Hermetic** cases that need no server: config resolution to a
//!   [`RemoteBackend`] (FR-001, FR-004), the fail-without-fallback path for a
//!   missing URL (FR-031), the refusal of local tool dispatch (FR-021, FR-031),
//!   the SSE frame decoder, and the remote-session cache (FR-009).
//! - **Live** cases driven against an in-process Axum mock of the ragent REST+SSE
//!   API: a turn is relayed and its streamed events are mirrored (FR-021), an
//!   unreachable server fails the turn (FR-031), a stream that drops part-way
//!   through the turn is surfaced without local re-execution (FR-034), and a
//!   cancellation is honoured.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
    routing::post,
};
use ragent_agent::backend::{
    BackendError, BackendErrorKind, BackendToolCall, ExecutionBackend, RemoteBackend,
    RemoteErrorKind, RemoteUpdate, cached_remote_session, forget_remote_session,
    parse_remote_update, relay_turn, remote_backend_config, resolve_backend,
    resolve_backend_for_config, split_sse_frames,
};
use ragent_agent::event::EventBus;
use ragent_agent::tool::{Tool, ToolContext, ToolOutput};
use ragent_config::{BackendConfig, Config, ExecutionBackendKind};
use serde_json::{Value, json};

/// A tool double that only counts how many times it actually runs.
struct CountingTool {
    runs: Arc<AtomicUsize>,
}

#[async_trait]
impl Tool for CountingTool {
    fn name(&self) -> &'static str {
        "counting_tool"
    }

    fn description(&self) -> &'static str {
        "test double that counts its invocations"
    }

    fn parameters_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    fn permission_category(&self) -> &'static str {
        "none"
    }

    async fn execute(&self, _input: Value, _ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        Ok(ToolOutput {
            content: "ran on host".to_string(),
            metadata: None,
        })
    }
}

fn ctx() -> ToolContext {
    ToolContext {
        session_id: "remote-test".to_string(),
        working_dir: std::env::current_dir().expect("cwd"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        allowed_roots: Vec::new(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        tool_registry: ToolContext::default_tool_registry(),
    }
}

/// A remote backend descriptor with a distinct id per call, so tests that use
/// the process-wide remote-session cache do not collide.
fn descriptor(url: Option<&str>) -> BackendConfig {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let id = format!("remote-{}", NEXT_ID.fetch_add(1, Ordering::SeqCst));
    BackendConfig {
        id,
        name: Some("Test remote".to_string()),
        kind: "remote".to_string(),
        image: None,
        workspace: None,
        url: url.map(str::to_string),
        api_key: Some("test-key".to_string()),
        credentials: Vec::new(),
    }
}

/// A config whose active backend is the `remote` descriptor for `url`.
fn remote_config(url: &str) -> Config {
    serde_json::from_value(json!({
        "execution_backend": {
            "id": "remote-test",
            "kind": "remote",
            "url": url,
            "api_key": "test-key"
        }
    }))
    .expect("parse remote config")
}

// ---------------------------------------------------------------------------
// Hermetic cases: resolution, refusal, cache, decoder.
// ---------------------------------------------------------------------------

#[test]
fn resolved_remote_kind_selects_a_remote_backend() {
    // FR-001: `remote` is a selectable kind and resolves to the remote adapter.
    let backend = resolve_backend(ExecutionBackendKind::Remote);
    assert_eq!(backend.kind(), ExecutionBackendKind::Remote);
    // Docker/podman keep their placeholder adapter until a descriptor is read
    // from config (they need an image and workspace).
    assert_eq!(
        resolve_backend(ExecutionBackendKind::Docker).kind(),
        ExecutionBackendKind::Docker
    );
}

#[test]
fn config_resolution_builds_a_remote_backend_for_a_remote_descriptor() {
    // FR-001, FR-009: a configured `remote` entry resolves to a RemoteBackend.
    let config = remote_config("http://127.0.0.1:9");
    assert_eq!(
        config.effective_execution_backend(),
        ExecutionBackendKind::Remote
    );
    let backend = resolve_backend_for_config(&config, std::path::Path::new("."));
    assert_eq!(backend.kind(), ExecutionBackendKind::Remote);

    // A `local` config still resolves to the host adapter (FR-019).
    let local = Config::default();
    let backend = resolve_backend_for_config(&local, std::path::Path::new("."));
    assert_eq!(backend.kind(), ExecutionBackendKind::Local);
}

#[test]
fn remote_backend_config_is_present_only_when_remote_is_active() {
    // FR-004: the descriptor is available for registry/health use only when the
    // remote backend is selected.
    let config = remote_config("http://example.invalid:1234");
    let descriptor = remote_backend_config(&config).expect("remote config exposes a descriptor");
    assert_eq!(descriptor.display_name(), "remote-test");
    assert_eq!(
        descriptor.url.as_deref(),
        Some("http://example.invalid:1234")
    );

    let local = Config::default();
    assert!(remote_backend_config(&local).is_none());
}

#[test]
fn remote_config_without_a_url_is_a_provisioning_failure() {
    // FR-031: a remote backend with no URL fails at provisioning and never
    // resolves to host execution.
    let backend = RemoteBackend::from_descriptor(&BackendConfig {
        id: "no-url".to_string(),
        kind: "remote".to_string(),
        ..BackendConfig::default()
    });
    assert!(!backend.has_url());

    let error = ragent_agent::backend::validate_remote_backend(&BackendConfig {
        id: "no-url".to_string(),
        kind: "remote".to_string(),
        ..BackendConfig::default()
    })
    .expect_err("a URL-less remote backend must not validate");
    assert_eq!(error.kind(), RemoteErrorKind::Unreachable);

    let mapped = BackendError::from(error);
    assert_eq!(mapped.error_kind(), BackendErrorKind::Provision);
    assert!(mapped.to_string().contains("remote"));
}

#[tokio::test]
async fn remote_backend_never_executes_a_tool_on_the_host() {
    // FR-021, FR-031: a tool dispatch that reaches the remote backend is refused;
    // the tool is never run locally.
    let runs = Arc::new(AtomicUsize::new(0));
    let tool = CountingTool {
        runs: Arc::clone(&runs),
    };
    let ctx = ctx();
    let backend = RemoteBackend::from_descriptor(&descriptor(Some("http://127.0.0.1:9")));

    let error = ExecutionBackend::execute_tool(
        &backend,
        BackendToolCall {
            tool: &tool,
            input: json!({}),
            ctx: &ctx,
        },
    )
    .await
    .expect_err("the remote backend must refuse a local tool dispatch");

    assert_eq!(runs.load(Ordering::SeqCst), 0, "no host execution");
    let backend_error = error
        .downcast_ref::<BackendError>()
        .expect("structured backend failure");
    assert_eq!(backend_error.backend(), ExecutionBackendKind::Remote);
    assert_eq!(backend_error.error_kind(), BackendErrorKind::Unavailable);
    assert!(error.to_string().contains("FR-021") || error.to_string().contains("FR-031"));
}

#[test]
fn backend_error_maps_remote_failure_kinds() {
    // FR-031/FR-034: an unreachable remote is a provisioning failure; a
    // mid-stream protocol fault is a protocol failure.
    let unreachable = BackendError::from(ragent_agent::backend::RemoteError::new(
        RemoteErrorKind::Unreachable,
        "no route to host",
    ));
    assert_eq!(unreachable.error_kind(), BackendErrorKind::Provision);

    let protocol = BackendError::from(ragent_agent::backend::RemoteError::new(
        RemoteErrorKind::Protocol,
        "stream dropped",
    ));
    assert_eq!(protocol.error_kind(), BackendErrorKind::Protocol);
}

#[test]
fn remote_session_cache_remembers_and_forgets() {
    // FR-009: a second turn reuses the same remote conversation.
    let backend = descriptor(Some("http://127.0.0.1:9"));
    assert!(cached_remote_session(&backend.id, "local-a").is_none());
    ragent_agent::backend::remember_remote_session(&backend.id, "local-a", "remote-42");
    assert_eq!(
        cached_remote_session(&backend.id, "local-a").as_deref(),
        Some("remote-42")
    );
    // A different local session under the same backend is independent.
    assert!(cached_remote_session(&backend.id, "local-b").is_none());
    forget_remote_session(&backend.id, "local-a");
    assert!(cached_remote_session(&backend.id, "local-a").is_none());
}

#[test]
fn sse_frames_split_and_decode() {
    let body = "event: text_delta\ndata: {\"session_id\":\"s\",\"text\":\"Hello \"}\n\n\
                event: text_delta\ndata: {\"session_id\":\"s\",\"text\":\"world\"}\n\n\
                event: message_end\ndata: {\"message_id\":\"m1\",\"reason\":\"stop\"}\n\n";
    let frames = split_sse_frames(body);
    assert_eq!(frames.len(), 3, "three complete frames");

    let updates: Vec<RemoteUpdate> = frames
        .iter()
        .map(|f| {
            let (name, data) = split_name_data(f);
            parse_remote_update(&name, &data)
        })
        .collect();
    assert_eq!(
        updates[0],
        RemoteUpdate::TextDelta {
            text: "Hello ".to_string()
        }
    );
    assert_eq!(
        updates[1],
        RemoteUpdate::TextDelta {
            text: "world".to_string()
        }
    );
    assert_eq!(
        updates[2],
        RemoteUpdate::MessageEnd {
            message_id: "m1".to_string(),
            reason: "stop".to_string()
        }
    );
}

#[test]
fn sse_decoder_handles_multiline_data_and_unknown_events() {
    // The SSE grammar joins multiple `data:` lines with `\n`; an unknown event is
    // preserved rather than dropped.
    let body = "event: tool_result\n\
                data: {\"call_id\":\"c1\",\"tool\":\"bash\",\n\
                data: \"content\":\"ok\",\"success\":true}\n\n\
                event: some_new_event\ndata: {\"x\":1}\n\n";
    let frames = split_sse_frames(body);
    assert_eq!(frames.len(), 2);
    assert_eq!(
        parse_remote_update(
            "tool_result",
            "{\"call_id\":\"c1\",\"tool\":\"bash\",\"content\":\"ok\",\"success\":true}"
        ),
        RemoteUpdate::ToolResult {
            call_id: "c1".to_string(),
            tool: "bash".to_string(),
            content: "ok".to_string(),
            content_line_count: 0,
            metadata: None,
            success: true,
        }
    );
    assert_eq!(
        parse_remote_update("some_new_event", "{\"x\":1}"),
        RemoteUpdate::Other {
            kind: "some_new_event".to_string()
        }
    );
}

/// Split one SSE frame into its `(event, data)` pair, mirroring the transport.
fn split_name_data(frame: &str) -> (String, String) {
    let mut event = String::new();
    let mut data = Vec::new();
    for line in frame.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("data:") {
            data.push(rest.strip_prefix(' ').unwrap_or(rest).to_string());
        }
    }
    (event, data.join("\n"))
}

// ---------------------------------------------------------------------------
// Live cases against an in-process mock of the ragent REST+SSE API.
// ---------------------------------------------------------------------------

/// Start an in-process Axum mock that answers `POST /sessions` with a fixed id
/// and `POST /sessions/{id}/messages` with `body` as an SSE stream.
async fn start_mock_server(body: String) -> (String, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route("/sessions", post(mock_create_session))
        .route("/sessions/{id}/messages", post(mock_stream))
        .with_state(Arc::new(body));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock server");
    let addr = listener.local_addr().expect("local addr");
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), handle)
}

async fn mock_create_session(Json(_body): Json<Value>) -> Json<Value> {
    Json(json!({ "id": "remote-session-1", "directory": "/remote" }))
}

async fn mock_stream(State(body): State<Arc<String>>) -> Response {
    (
        [(header::CONTENT_TYPE, "text/event-stream")],
        (*body).clone(),
    )
        .into_response()
}

#[tokio::test]
async fn relay_turn_streams_and_mirrors_updates() {
    // FR-021: a turn is driven over the remote REST+SSE API and each streamed
    // event is decoded in order.
    let body = "event: session_created\ndata: {\"session_id\":\"remote-session-1\"}\n\n\
                event: message_start\ndata: {\"message_id\":\"m1\"}\n\n\
                event: text_delta\ndata: {\"text\":\"Hello \"}\n\n\
                event: text_delta\ndata: {\"text\":\"from remote\"}\n\n\
                event: tool_call_start\ndata: {\"call_id\":\"c1\",\"tool\":\"bash\"}\n\n\
                event: message_end\ndata: {\"message_id\":\"m1\",\"reason\":\"stop\"}\n\n"
        .to_string();
    let (url, handle) = start_mock_server(body).await;

    let backend = RemoteBackend::from_descriptor(&descriptor(Some(&url)));
    let cancel = AtomicBool::new(false);
    let mut seen: Vec<RemoteUpdate> = Vec::new();
    let mut on_update = |update: RemoteUpdate| seen.push(update);

    let outcome = relay_turn(
        &backend,
        "local-stream",
        std::path::Path::new("/work"),
        "say hello",
        &cancel,
        &mut on_update,
    )
    .await
    .expect("the relay must succeed against the mock");

    assert_eq!(outcome.text, "Hello from remote");
    assert_eq!(outcome.stop_reason, "stop");
    assert!(outcome.updates >= 5, "each frame is delivered");
    assert!(matches!(seen[0], RemoteUpdate::SessionCreated));
    assert!(
        seen.iter()
            .any(|u| matches!(u, RemoteUpdate::ToolCallStart { tool, .. } if tool == "bash"))
    );
    handle.abort();
}

#[tokio::test]
async fn relay_turn_reuses_the_remote_session_across_turns() {
    // FR-009: the first turn creates the remote session; a second turn reuses it.
    let body = "event: text_delta\ndata: {\"text\":\"ok\"}\n\n\
                event: message_end\ndata: {\"message_id\":\"m1\",\"reason\":\"stop\"}\n\n"
        .to_string();
    let (url, handle) = start_mock_server(body).await;
    let backend = RemoteBackend::from_descriptor(&descriptor(Some(&url)));
    let cancel = AtomicBool::new(false);

    for _ in 0..2 {
        let mut on_update = |_u: RemoteUpdate| {};
        relay_turn(
            &backend,
            "local-reuse",
            std::path::Path::new("/work"),
            "hi",
            &cancel,
            &mut on_update,
        )
        .await
        .expect("relay succeeds");
    }
    assert_eq!(
        cached_remote_session(&backend.id(), "local-reuse").as_deref(),
        Some("remote-session-1")
    );
    handle.abort();
}

#[tokio::test]
async fn relay_turn_fails_when_the_server_is_unreachable() {
    // FR-031, FR-034: an unreachable server fails the turn and never falls back
    // to local execution.
    // Reserve a port, then release it so nothing is listening.
    let reserved = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve port");
    let addr = reserved.local_addr().expect("addr");
    drop(reserved);

    let backend = RemoteBackend::from_descriptor(&descriptor(Some(&format!("http://{addr}"))));
    let cancel = AtomicBool::new(false);
    let mut on_update = |_u: RemoteUpdate| {};

    let error = relay_turn(
        &backend,
        "local-unreachable",
        std::path::Path::new("/work"),
        "hello",
        &cancel,
        &mut on_update,
    )
    .await
    .expect_err("an unreachable server must fail the turn");
    assert_eq!(error.kind(), RemoteErrorKind::Unreachable);
}

#[tokio::test]
async fn relay_turn_reports_a_stream_that_drops_mid_turn() {
    // FR-034: a stream that closes before `message_end` is surfaced as a failure;
    // the turn's tools are not re-executed locally.
    let body = "event: text_delta\ndata: {\"text\":\"partial\"}\n\n".to_string();
    let (url, handle) = start_mock_server(body).await;
    let backend = RemoteBackend::from_descriptor(&descriptor(Some(&url)));
    let cancel = AtomicBool::new(false);
    let mut seen: Vec<RemoteUpdate> = Vec::new();
    let mut on_update = |update: RemoteUpdate| seen.push(update);

    let error = relay_turn(
        &backend,
        "local-dropped",
        std::path::Path::new("/work"),
        "hello",
        &cancel,
        &mut on_update,
    )
    .await
    .expect_err("a dropped stream must fail the turn");
    assert_eq!(error.kind(), RemoteErrorKind::Protocol);
    // The partial update that did arrive was still mirrored.
    assert!(
        seen.iter()
            .any(|u| matches!(u, RemoteUpdate::TextDelta { text } if text == "partial"))
    );
    handle.abort();
}

#[tokio::test]
async fn relay_turn_honours_cancellation() {
    // FR-034: a turn cancelled by the user stops promptly rather than hanging.
    let body = "event: text_delta\ndata: {\"text\":\"x\"}\n\n\
                event: message_end\ndata: {\"message_id\":\"m1\",\"reason\":\"stop\"}\n\n"
        .to_string();
    let (url, handle) = start_mock_server(body).await;
    let backend = RemoteBackend::from_descriptor(&descriptor(Some(&url)));
    let cancel = AtomicBool::new(true);
    let mut on_update = |_u: RemoteUpdate| {};

    let error = relay_turn(
        &backend,
        "local-cancel",
        std::path::Path::new("/work"),
        "hello",
        &cancel,
        &mut on_update,
    )
    .await
    .expect_err("a cancelled turn must not report success");
    assert_eq!(error.kind(), RemoteErrorKind::Cancelled);
    handle.abort();
}
