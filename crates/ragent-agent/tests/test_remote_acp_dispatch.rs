//! External integration coverage for the remote-backend and ACP turn
//! dispatchers (`ragent_agent::session::{remote_dispatch, acp_dispatch}`).
//!
//! The dispatch entry points (`remote_backend_for_active`, `acp_agent_for_active`,
//! `process_remote_turn`, `process_acp_turn`, `publish_update`, `fail_turn`,
//! `describe`, `finish_reason_for`) are `pub(crate)` / private, so an external
//! `tests/` crate cannot call them directly. This harness drives them through the
//! public `SessionProcessor::process_user_message`, which routes a turn to ACP
//! (FR-022) and then to a remote backend (FR-021) before any local LLM work.
//!
//! Two hermetic fixtures are used:
//! - an in-process Axum mock of the ragent REST+SSE API for the remote relay; and
//! - a small Python subprocess implementing ACP JSON-RPC for the ACP relay.
//!
//! No network egress and no real LLM provider are involved.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    http::header,
    response::{IntoResponse, Response},
    routing::post,
};
use ragent_agent::agent::AgentInfo;
use ragent_agent::automation::{clear_session_backend, register_session_backend};
use ragent_agent::backend::{BackendError, BackendErrorKind};
use ragent_agent::event::{Event, EventBus, FinishReason};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::ProviderRegistry;
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool::create_default_registry;
use ragent_config::{AcpAgentConfig, AcpConfig, Config as RagentConfig, ExecutionBackend};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A process-unique suffix so backend ids, session ids, and remote-session cache
/// keys never collide across tests running in parallel.
fn uid() -> String {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    format!("rad-{}", NEXT.fetch_add(1, Ordering::SeqCst))
}

/// Build a `SessionProcessor` whose cached config is `config`, with in-memory
/// storage and no provider. The remote/ACP dispatch paths return before any
/// provider or model resolution, so no provider is needed.
fn make_processor(config: RagentConfig) -> SessionProcessor {
    let event_bus = Arc::new(EventBus::new(2048));
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));

    SessionProcessor {
        session_manager,
        provider_registry: Arc::new(ProviderRegistry::new()),
        tool_registry: Arc::new(create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![]))),
        event_bus,
        agent_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        team_context_cache: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        tool_repeat_guard: Arc::new(parking_lot::Mutex::new(HashMap::new())),
        mcp_client: std::sync::OnceLock::new(),
        connector_session: tokio::sync::RwLock::new(None),
        connector_statuses: tokio::sync::RwLock::new(None),
        code_index: std::sync::OnceLock::new(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(HashMap::new()),
        cached_config: parking_lot::Mutex::new(Some(CachedConfig {
            config: Arc::new(config),
            file_mtimes: Vec::new(),
            env_overrides_present: false,
        })),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::StreamConfig::default(),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
        read_timestamps: Arc::new(std::sync::RwLock::new(HashMap::new())),
        telemetry: Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        bg_service: std::sync::OnceLock::new(),
        activity_log: std::sync::OnceLock::new(),
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(HashMap::new()),
        pending_verdicts: ragent_agent::session::pending::PendingVerdicts::new(),
    }
}

/// Create a session rooted at `dir` and return its id.
fn new_session(processor: &SessionProcessor, dir: &Path) -> String {
    processor
        .session_manager
        .create_session(dir.to_path_buf())
        .expect("session should be created")
        .id
}

/// Drain every event published on the bus so far.
fn drain(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<Event> {
    let mut out = Vec::new();
    while let Ok(event) = rx.try_recv() {
        out.push(event);
    }
    out
}

/// The persisted assistant text for `session_id` (empty string when none).
fn assistant_text(processor: &SessionProcessor, session_id: &str) -> String {
    processor
        .session_manager
        .storage()
        .get_messages(session_id)
        .expect("messages")
        .into_iter()
        .filter(|m| m.role == ragent_agent::message::Role::Assistant)
        .map(|m| m.text_content())
        .next_back()
        .unwrap_or_default()
}

/// A `Config` whose active backend is an inline `remote` descriptor for `url`.
fn remote_config(id: &str, url: &str) -> RagentConfig {
    let mut config = RagentConfig::default();
    config.execution_backend = Some(ExecutionBackend::Config(ragent_config::BackendConfig {
        id: id.to_string(),
        name: Some("Test remote".to_string()),
        kind: "remote".to_string(),
        url: Some(url.to_string()),
        api_key: Some("test-key".to_string()),
        ..ragent_config::BackendConfig::default()
    }));
    config
}

/// A `Config` binding `agent_id` to a single enabled ACP agent running `script`.
fn acp_config(agent_id: &str, script: &Path) -> RagentConfig {
    let mut config = RagentConfig::default();
    let entry = AcpAgentConfig {
        id: agent_id.to_string(),
        name: None,
        enabled: true,
        command: "python3".to_string(),
        args: vec![script.to_string_lossy().to_string()],
        env: HashMap::new(),
        cwd: None,
        turn_timeout_secs: 10,
        system_prompt: None,
    };
    let mut agents = HashMap::new();
    agents.insert(agent_id.to_string(), entry);
    config.acp = Some(AcpConfig {
        agents,
        ..AcpConfig::default()
    });
    config
}

/// Write a scratch Python source file and return its absolute path.
///
/// The ACP subprocess is spawned with the session's working directory as cwd (a
/// tempdir), so the script path must be absolute to resolve.
fn write_script(name: &str, body: &str) -> PathBuf {
    let dir = Path::new("target/temp/acp-tests");
    std::fs::create_dir_all(dir).expect("create scratch dir");
    let path = dir.join(name);
    std::fs::write(&path, body).expect("write script");
    std::fs::canonicalize(&path).unwrap_or(path)
}

// ---------------------------------------------------------------------------
// In-process Axum mock of the ragent REST+SSE API
// ---------------------------------------------------------------------------

/// Bind a mock answering `POST /sessions` with a fixed id and
/// `POST /sessions/{id}/messages` with `body` as an SSE stream.
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

/// Bind a mock that rejects every request with HTTP 500.
async fn start_error_server() -> (String, tokio::task::JoinHandle<()>) {
    let app = Router::new().route(
        "/sessions",
        post(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "session refused") }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind error server");
    let addr = listener.local_addr().expect("local addr");
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), handle)
}

/// Reserve then release a loopback port, so nothing is listening on it.
fn dead_url() -> String {
    let reserved = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve port");
    let addr = reserved.local_addr().expect("addr");
    drop(reserved);
    format!("http://{addr}")
}

fn has_text_delta(events: &[Event], text: &str) -> bool {
    events
        .iter()
        .any(|e| matches!(e, Event::TextDelta { text: t, .. } if t == text))
}

fn has_permission_request(events: &[Event], request_id: &str) -> bool {
    events
        .iter()
        .any(|e| matches!(e, Event::PermissionRequested { request_id: r, .. } if r == request_id))
}

fn has_question_request(events: &[Event], request_id: &str) -> bool {
    events
        .iter()
        .any(|e| matches!(e, Event::QuestionRequested { request_id: r, .. } if r == request_id))
}

// ---------------------------------------------------------------------------
// Remote backend turn
// ---------------------------------------------------------------------------

/// An SSE body exercising every `RemoteUpdate` arm of `publish_update`.
const REMOTE_ALL_UPDATES: &str = "event: session_created\ndata: {\"session_id\":\"remote-session-1\"}\n\n\
     event: message_start\ndata: {\"message_id\":\"m-remote\"}\n\n\
     event: text_delta\ndata: {\"text\":\"Hello \"}\n\n\
     event: text_delta\ndata: {\"text\":\"world\"}\n\n\
     event: reasoning_delta\ndata: {\"text\":\"thinking\"}\n\n\
     event: tool_call_start\ndata: {\"call_id\":\"c1\",\"tool\":\"Bash\"}\n\n\
     event: tool_call_end\ndata: {\"call_id\":\"c1\",\"tool\":\"Bash\",\"duration_ms\":12}\n\n\
     event: tool_result\ndata: {\"call_id\":\"c1\",\"tool\":\"Bash\",\"content\":\"ok\",\"content_line_count\":3,\"success\":true}\n\n\
     event: agent_notice\ndata: {\"message\":\"a remote notice\"}\n\n\
     event: permission_requested\ndata: {\"request_id\":\"p1\",\"permission\":\"file:write\",\"description\":\"write a file\",\"options\":[\"allow\",\"deny\"]}\n\n\
     event: question_requested\ndata: {\"request_id\":\"q1\",\"question\":\"which?\",\"options\":[\"a\",\"b\"],\"default_index\":1}\n\n\
     event: agent_error\ndata: {\"error\":\"a remote warning\"}\n\n\
     event: some_new_event\ndata: {\"x\":1}\n\n\
     event: message_end\ndata: {\"message_id\":\"m-remote\",\"reason\":\"stop\"}\n\n";

#[tokio::test]
async fn remote_turn_mirrors_every_update_kind() {
    // FR-021: the remote turn is relayed over REST+SSE and every streamed event
    // is mirrored onto the local bus under the local session id.
    let (url, handle) = start_mock_server(REMOTE_ALL_UPDATES.to_string()).await;
    let processor = make_processor(remote_config(&uid(), &url));
    let dir = tempfile::tempdir().expect("tempdir");
    let session_id = new_session(&processor, dir.path());
    let agent = AgentInfo::new("general", "General");

    let mut rx = processor.event_bus.subscribe();
    let cancel = Arc::new(AtomicBool::new(false));
    let outcome = processor
        .process_message(&session_id, "say hello", &agent, cancel)
        .await;
    assert!(outcome.is_ok(), "a healthy relay must succeed: {outcome:?}");

    let events = drain(&mut rx);
    assert!(has_text_delta(&events, "Hello "));
    assert!(has_text_delta(&events, "world"));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::ReasoningDelta { text, .. } if text == "thinking"
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::ToolCallStart { tool, .. } if tool == "Bash"
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::ToolCallEnd { tool, error: None, duration_ms: 12, .. } if tool == "Bash"
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::ToolResult { content, success: true, .. } if content == "ok"
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::AgentNotice { message, .. } if message == "a remote notice"
    )));
    assert!(has_permission_request(&events, "p1"));
    assert!(has_question_request(&events, "q1"));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::AgentError { error, .. } if error == "a remote warning"
    )));
    // The mirrored MessageStart and the terminal MessageEnd both carry the local
    // session id.
    assert!(events.iter().any(|e| matches!(
        e,
        Event::MessageStart { message_id, .. } if message_id == "m-remote"
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::MessageEnd { reason: FinishReason::Stop, session_id: sid, .. } if sid == &session_id
    )));

    // The final assistant text is persisted from the mirrored deltas.
    assert_eq!(assistant_text(&processor, &session_id), "Hello world");
    handle.abort();
}

#[tokio::test]
async fn remote_turn_maps_a_non_default_finish_reason_label() {
    // `finish_reason_for` maps the terminal label; "cancelled" is not the default.
    let body = "event: text_delta\ndata: {\"text\":\"done\"}\n\n\
                event: message_end\ndata: {\"message_id\":\"m1\",\"reason\":\"cancelled\"}\n\n";
    let (url, handle) = start_mock_server(body.to_string()).await;
    let processor = make_processor(remote_config(&uid(), &url));
    let dir = tempfile::tempdir().expect("tempdir");
    let session_id = new_session(&processor, dir.path());
    let agent = AgentInfo::new("general", "General");

    let mut rx = processor.event_bus.subscribe();
    let cancel = Arc::new(AtomicBool::new(false));
    processor
        .process_message(&session_id, "go", &agent, cancel)
        .await
        .expect("relay succeeds");

    let events = drain(&mut rx);
    assert!(events.iter().any(|e| matches!(
        e,
        Event::MessageEnd {
            reason: FinishReason::Cancelled,
            ..
        }
    )));
    handle.abort();
}

#[tokio::test]
async fn remote_turn_failure_runs_fail_turn_and_describe() {
    // FR-034: a non-2xx provisioning answer fails the turn. `fail_turn` publishes
    // the `AgentError` (through `describe`) and the terminal `MessageEnd`.
    let (url, handle) = start_error_server().await;
    let processor = make_processor(remote_config(&uid(), &url));
    let dir = tempfile::tempdir().expect("tempdir");
    let session_id = new_session(&processor, dir.path());
    let agent = AgentInfo::new("general", "General");

    let mut rx = processor.event_bus.subscribe();
    let cancel = Arc::new(AtomicBool::new(false));
    let error = processor
        .process_message(&session_id, "go", &agent, cancel)
        .await
        .expect_err("a rejected session must fail the turn");

    let backend = error
        .downcast_ref::<BackendError>()
        .expect("structured backend failure");
    assert_eq!(backend.error_kind(), BackendErrorKind::Protocol);

    let events = drain(&mut rx);
    assert!(events.iter().any(|e| matches!(
        e,
        Event::AgentError { error, .. }
            if error == "remote backend 'Test remote': execution backend 'remote' failed (Protocol): the remote server rejected session creation (HTTP 500): session refused"
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::MessageEnd {
            reason: FinishReason::Stop,
            ..
        }
    )));
    // An assistant placeholder is persisted even on failure.
    assert_eq!(assistant_text(&processor, &session_id), "");
    handle.abort();
}

#[tokio::test]
async fn remote_turn_unreachable_server_is_a_provision_failure() {
    // FR-031: an unreachable server fails at provisioning, never on the host.
    let processor = make_processor(remote_config(&uid(), &dead_url()));
    let dir = tempfile::tempdir().expect("tempdir");
    let session_id = new_session(&processor, dir.path());
    let agent = AgentInfo::new("general", "General");

    let cancel = Arc::new(AtomicBool::new(false));
    let error = processor
        .process_message(&session_id, "go", &agent, cancel)
        .await
        .expect_err("an unreachable server must fail the turn");
    let backend = error
        .downcast_ref::<BackendError>()
        .expect("structured backend failure");
    assert_eq!(backend.error_kind(), BackendErrorKind::Provision);
}

#[tokio::test]
async fn remote_turn_reports_a_stream_that_drops_mid_turn() {
    // FR-034: a stream that closes before `message_end` fails the turn, but the
    // partial update that arrived is still mirrored.
    let body = "event: text_delta\ndata: {\"text\":\"partial\"}\n\n".to_string();
    let (url, handle) = start_mock_server(body).await;
    let processor = make_processor(remote_config(&uid(), &url));
    let dir = tempfile::tempdir().expect("tempdir");
    let session_id = new_session(&processor, dir.path());
    let agent = AgentInfo::new("general", "General");

    let mut rx = processor.event_bus.subscribe();
    let cancel = Arc::new(AtomicBool::new(false));
    let error = processor
        .process_message(&session_id, "go", &agent, cancel)
        .await
        .expect_err("a dropped stream must fail the turn");
    let backend = error
        .downcast_ref::<BackendError>()
        .expect("structured backend failure");
    assert_eq!(backend.error_kind(), BackendErrorKind::Protocol);

    let events = drain(&mut rx);
    assert!(has_text_delta(&events, "partial"));
    handle.abort();
}

#[tokio::test]
async fn remote_turn_cancellation_reports_the_cancelled_reason() {
    // FR-034: a cancelled relay is reported as a cancellation, not a hard error.
    let body = "event: text_delta\ndata: {\"text\":\"x\"}\n\n\
                event: message_end\ndata: {\"message_id\":\"m1\",\"reason\":\"stop\"}\n\n";
    let (url, handle) = start_mock_server(body.to_string()).await;
    let processor = make_processor(remote_config(&uid(), &url));
    let dir = tempfile::tempdir().expect("tempdir");
    let session_id = new_session(&processor, dir.path());
    let agent = AgentInfo::new("general", "General");

    let mut rx = processor.event_bus.subscribe();
    let cancel = Arc::new(AtomicBool::new(true));
    let error = processor
        .process_message(&session_id, "go", &agent, cancel)
        .await
        .expect_err("a cancelled turn must not report success");
    assert!(error.downcast_ref::<BackendError>().is_some());

    let events = drain(&mut rx);
    assert!(events.iter().any(|e| matches!(
        e,
        Event::MessageEnd {
            reason: FinishReason::Cancelled,
            ..
        }
    )));
    handle.abort();
}

#[tokio::test]
async fn remote_turn_uses_a_session_backend_override() {
    // FR-033: an automation run confined to the `remote` backend relays its turn
    // even when the global config selects the host backend.
    let (url, handle) = start_mock_server(REMOTE_ALL_UPDATES.to_string()).await;
    let processor = make_processor(RagentConfig::default());
    let dir = tempfile::tempdir().expect("tempdir");
    let session_id = new_session(&processor, dir.path());
    let agent = AgentInfo::new("general", "General");

    let descriptor = ragent_config::BackendConfig {
        id: uid(),
        name: Some("Override remote".to_string()),
        kind: "remote".to_string(),
        url: Some(url),
        api_key: Some("test-key".to_string()),
        ..ragent_config::BackendConfig::default()
    };
    register_session_backend(&session_id, descriptor);

    let cancel = Arc::new(AtomicBool::new(false));
    let result = processor
        .process_message(&session_id, "go", &agent, cancel)
        .await;
    clear_session_backend(&session_id);
    assert!(
        result.is_ok(),
        "the override must relay the turn: {result:?}"
    );
    assert_eq!(assistant_text(&processor, &session_id), "Hello world");
    handle.abort();
}

#[tokio::test]
async fn no_dispatch_when_local_and_unbound() {
    // `remote_backend_for_active` and `acp_agent_for_active` both resolve to
    // `None` for a default config with an unbound agent, so the turn falls
    // through to local preparation (which fails here for want of a model).
    let processor = make_processor(RagentConfig::default());
    let dir = tempfile::tempdir().expect("tempdir");
    let session_id = new_session(&processor, dir.path());
    let agent = AgentInfo::new("general", "General");

    let cancel = Arc::new(AtomicBool::new(false));
    let error = processor
        .process_message(&session_id, "go", &agent, cancel)
        .await
        .expect_err("no model => local prep fails");
    assert!(
        error.to_string().contains("no model configured"),
        "unexpected error: {error}"
    );
}

// ---------------------------------------------------------------------------
// ACP agent turn
// ---------------------------------------------------------------------------

/// A Python ACP agent that completes the handshake, then emits one of every
/// `session/update` kind before replying to `session/prompt`.
const ACP_MULTI_UPDATE_AGENT: &str = r#"
import sys, json

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

def update(body):
    send({"jsonrpc": "2.0", "method": "session/update",
          "params": {"sessionId": "s-1", "update": body}})

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    method = req.get("method")
    rid = req.get("id")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": rid, "result": {"protocolVersion": 1}})
    elif method == "session/new":
        send({"jsonrpc": "2.0", "id": rid, "result": {"sessionId": "s-1"}})
    elif method == "session/prompt":
        update({"sessionUpdate": "agent_message_chunk",
                "content": {"type": "text", "text": "Hello from ACP"}})
        update({"sessionUpdate": "agent_thought_chunk",
                "content": {"type": "text", "text": "reasoning"}})
        update({"sessionUpdate": "tool_call", "toolCallId": "tc-1",
                "title": "run tests", "status": "completed"})
        update({"sessionUpdate": "tool_call", "toolCallId": "tc-2",
                "title": "failing tool", "status": "failed"})
        update({"sessionUpdate": "tool_call", "toolCallId": "tc-3",
                "title": "started", "status": "in_progress"})
        update({"sessionUpdate": "tool_call_update", "toolCallId": "tc-3",
                "title": "started", "status": "completed"})
        update({"sessionUpdate": "tool_call_update", "toolCallId": "tc-3"})
        update({"sessionUpdate": "plan", "entries": [{"content": "a"}, {"content": "b"}]})
        update({"sessionUpdate": "some_new_kind", "foo": 1})
        send({"jsonrpc": "2.0", "id": rid, "result": {"stopReason": "end_turn"}})
        sys.stdout.flush()
"#;

#[tokio::test]
async fn acp_turn_renders_every_update_kind() {
    // FR-022: the turn is relayed to the bound ACP subprocess and each streamed
    // `session/update` is bridged onto the local bus.
    let script = write_script("rad_multi_update_agent.py", ACP_MULTI_UPDATE_AGENT);
    let processor = make_processor(acp_config("rad-acp", &script));
    let dir = tempfile::tempdir().expect("tempdir");
    let session_id = new_session(&processor, dir.path());
    let agent = AgentInfo::new("rad-acp", "Rad ACP");

    let mut rx = processor.event_bus.subscribe();
    let cancel = Arc::new(AtomicBool::new(false));
    processor
        .process_message(&session_id, "hi", &agent, cancel)
        .await
        .expect("the ACP relay must succeed");

    let events = drain(&mut rx);
    assert!(has_text_delta(&events, "Hello from ACP"));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::ReasoningDelta { text, .. } if text == "reasoning"
    )));
    let starts = events
        .iter()
        .filter(|e| matches!(e, Event::ToolCallStart { .. }))
        .count();
    assert_eq!(starts, 3, "three tool_call notifications start calls");
    let ends = events
        .iter()
        .filter(|e| matches!(e, Event::ToolCallEnd { .. }))
        .count();
    assert_eq!(
        ends, 3,
        "completed/failed tool calls and one status update end"
    );
    assert!(events.iter().any(|e| matches!(
        e,
        Event::ToolCallEnd { error: Some(msg), .. } if msg == "tool call failed"
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::AgentNotice { message, .. }
            if message == "ACP agent proposed a plan with 2 step(s)"
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::AgentNotice { message, .. }
            if message == "ACP agent update: some_new_kind"
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::MessageEnd {
            reason: FinishReason::Stop,
            ..
        }
    )));

    assert_eq!(assistant_text(&processor, &session_id), "Hello from ACP");
}

#[tokio::test]
async fn acp_turn_failure_on_subprocess_exit_publishes_describe() {
    // FR-036: a subprocess that exits before replying fails the turn; the failure
    // is published through `describe` and the turn returns `Err`.
    let script = write_script(
        "rad_exit_agent.py",
        "import sys\nsys.stderr.write('boom\\n')\nsys.stderr.flush()\nsys.exit(7)\n",
    );
    let processor = make_processor(acp_config("rad-acp-exit", &script));
    let dir = tempfile::tempdir().expect("tempdir");
    let session_id = new_session(&processor, dir.path());
    let agent = AgentInfo::new("rad-acp-exit", "Rad ACP Exit");

    let mut rx = processor.event_bus.subscribe();
    let cancel = Arc::new(AtomicBool::new(false));
    let error = processor
        .process_message(&session_id, "hi", &agent, cancel)
        .await
        .expect_err("an exiting subprocess must fail the turn");
    assert!(
        error
            .downcast_ref::<ragent_agent::acp::AcpError>()
            .is_some(),
        "the cause must be an AcpError: {error}"
    );

    let events = drain(&mut rx);
    assert!(events.iter().any(|e| matches!(e, Event::AgentError { .. })));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::MessageEnd {
            reason: FinishReason::Stop,
            ..
        }
    )));
    assert_eq!(assistant_text(&processor, &session_id), "");
}

#[tokio::test]
async fn acp_turn_failure_on_missing_command_is_a_spawn_error() {
    // FR-036: a command that cannot be spawned fails the turn with `describe`.
    let script = write_script("rad_unused_agent.py", "pass\n");
    // Build the config with a bogus command directly.
    let mut config = acp_config("rad-acp-missing", &script);
    if let Some(acp) = config.acp.as_mut()
        && let Some(entry) = acp.agents.get_mut("rad-acp-missing")
    {
        entry.command = "/nonexistent/acp-agent-does-not-exist".to_string();
        entry.args = Vec::new();
    }
    let processor = make_processor(config);
    let dir = tempfile::tempdir().expect("tempdir");
    let session_id = new_session(&processor, dir.path());
    let agent = AgentInfo::new("rad-acp-missing", "Rad ACP Missing");

    let cancel = Arc::new(AtomicBool::new(false));
    let error = processor
        .process_message(&session_id, "hi", &agent, cancel)
        .await
        .expect_err("a missing command must fail the turn");
    let acp_error = error
        .downcast_ref::<ragent_agent::acp::AcpError>()
        .expect("structured ACP failure");
    assert_eq!(acp_error.kind(), ragent_agent::acp::AcpErrorKind::Spawn);
}
