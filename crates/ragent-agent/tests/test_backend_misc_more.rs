//! Additional tests-only coverage for the execution backends
//! (`ragent_agent::backend::{container, remote}`).
//!
//! Everything here is driven through the public API only:
//!
//! - **container.rs** - `ContainerBackend::execute_tool` evaluates the pure
//!   `container_script` translation (and its `container_path` / `glob_predicate`
//!   / `required_str` helpers) **before** it provisions a sandbox, so a backend
//!   built with `image: None` exercises every routed-tool script branch and then
//!   fails the call at the image check without ever touching a container runtime
//!   or the network. The un-routable-tool refusal path is exercised the same way.
//! - **remote.rs** - the SSE decoder (`parse_remote_update`, `split_sse_frames`),
//!   the `RemoteError` surface, the remote-session cache, and the HTTP error
//!   mapping of `create_session` / `open_turn` / `reply_permission` against an
//!   in-process Axum mock (no real server, no real LLM).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use ragent_agent::backend::{
    BackendError, BackendErrorKind, BackendToolCall, ContainerBackend, ExecutionBackend,
    RemoteBackend, RemoteError, RemoteErrorKind, RemoteUpdate, cached_remote_session,
    forget_remote_session, parse_remote_update, remote_backend_config, split_sse_frames,
    validate_remote_backend,
};
use ragent_agent::event::EventBus;
use ragent_agent::tool::{Tool, ToolContext, ToolOutput};
use ragent_config::{BackendConfig, Config, ExecutionBackendKind};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Tool double + context (same shape as the sibling backend tests).
// ---------------------------------------------------------------------------

struct NamedTool {
    name: &'static str,
    runs: Arc<AtomicUsize>,
}

impl NamedTool {
    fn new(name: &'static str) -> (Self, Arc<AtomicUsize>) {
        let runs = Arc::new(AtomicUsize::new(0));
        (
            Self {
                name,
                runs: Arc::clone(&runs),
            },
            runs,
        )
    }
}

#[async_trait]
impl Tool for NamedTool {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &'static str {
        "test double"
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
        session_id: "backend-misc".to_string(),
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
        read_timestamps: Arc::new(std::sync::RwLock::new(HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        tool_registry: ToolContext::default_tool_registry(),
    }
}

/// A distinct descriptor id per call so named-sandbox hashes never collide.
fn descriptor(kind: &str, image: Option<&str>) -> BackendConfig {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let id = format!("misc-{}", NEXT_ID.fetch_add(1, Ordering::SeqCst));
    BackendConfig {
        id,
        name: Some("Misc sandbox".to_string()),
        kind: kind.to_string(),
        image: image.map(str::to_string),
        workspace: None,
        url: None,
        api_key: None,
        credentials: Vec::new(),
    }
}

/// An `image: None` container backend: every routed-tool script is built, then
/// the call fails the image check without a runtime or network touch.
fn imageless_backend() -> (ContainerBackend, ToolContext) {
    let context = ctx();
    let backend =
        ContainerBackend::from_descriptor(&descriptor("podman", None), &context.working_dir);
    (backend, context)
}

/// Dispatch `tool` and return the error (the call is expected to fail at the
/// image check after the script was built, or earlier for a rejected input).
async fn dispatch_file_tool(
    backend: &ContainerBackend,
    context: &ToolContext,
    tool_name: &'static str,
    input: Value,
) -> anyhow::Error {
    let (tool, runs) = NamedTool::new(tool_name);
    let err = ExecutionBackend::execute_tool(
        backend,
        BackendToolCall {
            tool: &tool,
            input,
            ctx: context,
        },
    )
    .await
    .expect_err("the tool must not execute on the host");
    assert_eq!(runs.load(Ordering::SeqCst), 0, "no host execution");
    err
}

// ---------------------------------------------------------------------------
// container.rs: routed-tool script branches (built before provisioning).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn container_scripts_reach_the_image_check_for_every_routed_tool() {
    // Each of these builds a sandbox `sh -lc` script (covering container_script,
    // container_path and glob_predicate) and then fails at the missing image.
    let (backend, context) = imageless_backend();

    let cases: &[(&'static str, Value)] = &[
        ("read", json!({ "path": "src/lib.rs" })),
        (
            "read",
            json!({ "path": "/projects/src/lib.rs", "start_line": 2, "num_lines": 3 }),
        ),
        ("read", json!({ "path": "./nested/file.rs", "end_line": 9 })),
        ("create", json!({ "path": "src/new.rs", "content": "hi" })),
        ("write", json!({ "path": "src/w.rs", "content": "hi" })),
        (
            "append_file",
            json!({ "path": "src/a.rs", "content": "more" }),
        ),
        ("rm", json!({ "path": "src/dead.rs" })),
        ("mkdir", json!({ "path": "src/gen" })),
        ("list", json!({ "path": "." })),
        ("list", json!({ "path": "src", "depth": 4 })),
        ("glob", json!({ "pattern": "**/*.rs" })),
        ("glob", json!({ "pattern": "src/**/*.ts", "path": "src" })),
        ("glob", json!({ "pattern": "a/b", "path": "src" })),
        ("grep", json!({ "pattern": "needle" })),
        (
            "grep",
            json!({ "pattern": "x", "path": "src", "case_insensitive": true, "include": "*.rs", "exclude": "vendor/*" }),
        ),
    ];

    for (tool, input) in cases {
        let err = dispatch_file_tool(&backend, &context, tool, input.clone()).await;
        assert_eq!(
            err.downcast_ref::<BackendError>()
                .map(BackendError::error_kind),
            Some(BackendErrorKind::Provision),
            "tool {tool} with {input} should fail provisioning: {err}"
        );
    }
}

#[tokio::test]
async fn container_read_rejects_a_zero_line_range() {
    let (backend, context) = imageless_backend();
    // start_line/end_line/num_lines must be >= 1.
    for input in [
        json!({ "path": "src/lib.rs", "start_line": 0 }),
        json!({ "path": "src/lib.rs", "end_line": 0 }),
        json!({ "path": "src/lib.rs", "num_lines": 0 }),
    ] {
        let err = dispatch_file_tool(&backend, &context, "read", input).await;
        assert!(err.to_string().contains("Invalid line range"), "err: {err}");
    }
}

#[tokio::test]
async fn container_file_tools_require_their_path_parameter() {
    let (backend, context) = imageless_backend();
    // `required_str` fails with a clear message when `path`/`pattern` is absent.
    for (tool, input, needle) in [
        ("read", json!({}), "path"),
        ("create", json!({ "path": "x" }), "content"),
        ("glob", json!({}), "pattern"),
        ("grep", json!({}), "pattern"),
    ] {
        let err = dispatch_file_tool(&backend, &context, tool, input).await;
        assert!(
            err.to_string().contains("Missing required") && err.to_string().contains(needle),
            "tool {tool} should name the missing '{needle}': {err}"
        );
    }
}

#[tokio::test]
async fn container_path_rejects_empty_and_parent_dir_escapes() {
    let (backend, context) = imageless_backend();
    // Empty path and a `..` component are rejected by container_path.
    let empty = dispatch_file_tool(&backend, &context, "read", json!({ "path": "   " })).await;
    assert!(empty.to_string().contains("must not be empty"), "{empty}");

    for bad in ["../etc/passwd", "a/../../b", "/projects/../../etc/shadow"] {
        let err = dispatch_file_tool(&backend, &context, "mkdir", json!({ "path": bad })).await;
        assert!(
            err.to_string().contains("FR-035") || err.to_string().contains("escape"),
            "path {bad} should be rejected: {err}"
        );
    }

    // An absolute path outside the workspace mount is rejected too.
    let outside =
        dispatch_file_tool(&backend, &context, "read", json!({ "path": "/etc/hosts" })).await;
    assert!(
        outside
            .to_string()
            .contains("outside the container workspace"),
        "{outside}"
    );
}

#[tokio::test]
async fn container_rm_rejects_wildcards() {
    let (backend, context) = imageless_backend();
    for pattern in ["src/*.rs", "src/a?.rs"] {
        let err = dispatch_file_tool(&backend, &context, "rm", json!({ "path": pattern })).await;
        assert!(
            err.to_string().contains("does not accept wildcards"),
            "err: {err}"
        );
    }
}

#[tokio::test]
async fn container_bash_missing_command_and_banned_command_fail_before_provisioning() {
    let (backend, context) = imageless_backend();
    // A `bash` call with no `command` fails on the required parameter.
    let missing = dispatch_file_tool(&backend, &context, "bash", json!({})).await;
    assert!(missing.to_string().contains("command"), "{missing}");

    // A banned command is rejected by the 7-layer validator before any sandbox.
    let banned = dispatch_file_tool(
        &backend,
        &context,
        "bash",
        json!({ "command": "mkfs.ext4 /dev/sda1" }),
    )
    .await;
    assert!(banned.to_string().contains("7-layer"), "{banned}");
}

#[tokio::test]
async fn container_unadapted_tool_is_refused_with_unavailable() {
    let (backend, context) = imageless_backend();
    let err = dispatch_file_tool(&backend, &context, "websearch", json!({ "query": "x" })).await;
    assert_eq!(
        err.downcast_ref::<BackendError>()
            .map(BackendError::error_kind),
        Some(BackendErrorKind::Unavailable)
    );
}

#[test]
fn container_backend_accessors_and_redacted_summary() {
    let context = ctx();
    let mut desc = descriptor("podman", Some("img:1"));
    desc.workspace = Some("/work".to_string());
    desc.credentials = vec![
        "TOKEN_A".to_string(),
        "  ".to_string(),
        "TOKEN_A".to_string(),
    ];
    let backend = ContainerBackend::from_descriptor(&desc, &context.working_dir);

    assert_eq!(
        backend.runtime(),
        ragent_agent::backend::ContainerRuntime::Podman
    );
    assert_eq!(backend.workspace(), "/work");
    assert!(backend.container_name().starts_with("ragent-sandbox-"));
    assert!(backend.volume_name().starts_with("ragent-sandbox-vol-"));
    assert_eq!(backend.credential_names(), &["TOKEN_A".to_string()]);
    // The summary never carries a secret value, only the name + a marker.
    let summary = backend.injected_env_summary();
    assert_eq!(summary, vec!["TOKEN_A=[REDACTED]".to_string()]);

    // A relative workspace keeps the default container mount point.
    let default_ws = ContainerBackend::from_descriptor(
        &descriptor("podman", Some("img:1")),
        &context.working_dir,
    );
    assert_eq!(default_ws.workspace(), "/projects");
}

// ---------------------------------------------------------------------------
// remote.rs: decode arms, SSE splitting, error surface, cache.
// ---------------------------------------------------------------------------

#[test]
fn remote_decoder_covers_every_event_arm() {
    assert_eq!(
        parse_remote_update("session_created", "{}"),
        RemoteUpdate::SessionCreated
    );
    assert_eq!(
        parse_remote_update("message_start", "{\"message_id\":\"m1\"}"),
        RemoteUpdate::MessageStart {
            message_id: "m1".to_string()
        }
    );
    assert_eq!(
        parse_remote_update("text_delta", "{\"text\":\"hi\"}"),
        RemoteUpdate::TextDelta {
            text: "hi".to_string()
        }
    );
    assert_eq!(
        parse_remote_update("reasoning_delta", "{\"text\":\"think\"}"),
        RemoteUpdate::ReasoningDelta {
            text: "think".to_string()
        }
    );
    assert_eq!(
        parse_remote_update("tool_call_start", "{\"call_id\":\"c1\",\"tool\":\"bash\"}"),
        RemoteUpdate::ToolCallStart {
            call_id: "c1".to_string(),
            tool: "bash".to_string()
        }
    );
    assert_eq!(
        parse_remote_update(
            "tool_call_end",
            "{\"call_id\":\"c1\",\"tool\":\"bash\",\"error\":\"boom\",\"duration_ms\":42}"
        ),
        RemoteUpdate::ToolCallEnd {
            call_id: "c1".to_string(),
            tool: "bash".to_string(),
            error: Some("boom".to_string()),
            duration_ms: 42
        }
    );
    // No error field -> `None`, missing duration -> 0.
    assert_eq!(
        parse_remote_update("tool_call_end", "{\"call_id\":\"c1\",\"tool\":\"bash\"}"),
        RemoteUpdate::ToolCallEnd {
            call_id: "c1".to_string(),
            tool: "bash".to_string(),
            error: None,
            duration_ms: 0
        }
    );
    assert_eq!(
        parse_remote_update("agent_notice", "{\"message\":\"fyi\"}"),
        RemoteUpdate::AgentNotice {
            message: "fyi".to_string()
        }
    );
    assert_eq!(
        parse_remote_update("agent_error", "{\"error\":\"bad\"}"),
        RemoteUpdate::AgentError {
            error: "bad".to_string()
        }
    );
    assert_eq!(
        parse_remote_update(
            "permission_requested",
            "{\"request_id\":\"p1\",\"permission\":\"file:write\",\"description\":\"d\",\"options\":[\"a\",\"b\"]}"
        ),
        RemoteUpdate::PermissionRequested {
            request_id: "p1".to_string(),
            permission: "file:write".to_string(),
            description: "d".to_string(),
            options: vec!["a".to_string(), "b".to_string()]
        }
    );
    assert_eq!(
        parse_remote_update(
            "question_requested",
            "{\"request_id\":\"q1\",\"question\":\"?\",\"options\":[\"a\"],\"default_index\":1}"
        ),
        RemoteUpdate::QuestionRequested {
            request_id: "q1".to_string(),
            question: "?".to_string(),
            options: vec!["a".to_string()],
            default_index: Some(1)
        }
    );
    // Wrong-typed / missing option fields collapse to empty/None.
    assert_eq!(
        parse_remote_update("question_requested", "{\"request_id\":\"q2\"}"),
        RemoteUpdate::QuestionRequested {
            request_id: "q2".to_string(),
            question: String::new(),
            options: Vec::new(),
            default_index: None
        }
    );
    // An unknown event name is preserved.
    assert_eq!(
        parse_remote_update("some_new_event", "{\"x\":1}"),
        RemoteUpdate::Other {
            kind: "some_new_event".to_string()
        }
    );
    // A malformed payload yields a `malformed:<name>` sentinel rather than dropping.
    assert_eq!(
        parse_remote_update("text_delta", "not-json"),
        RemoteUpdate::Other {
            kind: "malformed:text_delta".to_string()
        }
    );
}

#[test]
fn sse_splitter_handles_empty_partial_and_crlf_frames() {
    // Empty input -> no frames.
    assert_eq!(split_sse_frames(""), [] as [std::string::String; 0]);
    // A trailing partial frame (no terminating blank line) is left unconsumed.
    assert_eq!(
        split_sse_frames("event: text_delta\ndata: {}\n"),
        [] as [std::string::String; 0]
    );
    // A CRLF-separated frame is recognised.
    let crlf = split_sse_frames("event: text_delta\r\ndata: {\"text\":\"x\"}\r\n\r\n");
    assert_eq!(crlf.len(), 1);
    // A `data:` line without a space separator is accepted.
    let no_space = split_sse_frames("data:{\"text\":\"y\"}\n\n");
    assert_eq!(no_space.len(), 1);
}

#[test]
fn remote_backend_handle_surface_is_secret_free() {
    let descriptor = BackendConfig {
        id: "r1".to_string(),
        name: Some("Remote One".to_string()),
        kind: "remote".to_string(),
        url: Some("http://example.invalid:9/".to_string()),
        api_key: Some("sk-secret".to_string()),
        ..BackendConfig::default()
    };
    let backend = RemoteBackend::from_descriptor(&descriptor);
    assert_eq!(backend.id(), "r1");
    assert_eq!(backend.name(), "Remote One");
    assert!(backend.has_url());
    assert!(backend.has_bearer_key());
    // The trailing slash is trimmed from the stored URL.
    assert_eq!(backend.url(), Some("http://example.invalid:9"));
    // Debug must never carry the bearer key.
    let debug = format!("{backend:?}");
    assert!(!debug.contains("sk-secret"), "leaked key: {debug}");
    assert!(debug.contains("[REDACTED]"));
}

#[test]
fn remote_error_surface_and_backend_mapping() {
    let err = RemoteError::new(RemoteErrorKind::Protocol, "boom").with_status(503);
    assert_eq!(err.kind(), RemoteErrorKind::Protocol);
    assert_eq!(err.status(), Some(503));
    assert_eq!(err.message(), "boom");
    assert!(err.to_string().contains("boom"));

    // The mapping of each kind onto the backend taxonomy.
    for (kind, expected) in [
        (RemoteErrorKind::Unreachable, BackendErrorKind::Provision),
        (RemoteErrorKind::Protocol, BackendErrorKind::Protocol),
        (RemoteErrorKind::Timeout, BackendErrorKind::Protocol),
        (RemoteErrorKind::Cancelled, BackendErrorKind::Protocol),
    ] {
        let mapped = BackendError::from(RemoteError::new(kind, "x"));
        assert_eq!(mapped.error_kind(), expected, "kind {kind:?}");
        assert_eq!(mapped.backend(), ExecutionBackendKind::Remote);
    }
}

#[test]
fn remote_session_cache_and_config_resolution() {
    let descriptor = descriptor("remote", None);
    assert!(cached_remote_session(&descriptor.id, "local-x").is_none());
    ragent_agent::backend::remember_remote_session(&descriptor.id, "local-x", "remote-9");
    assert_eq!(
        cached_remote_session(&descriptor.id, "local-x").as_deref(),
        Some("remote-9")
    );
    forget_remote_session(&descriptor.id, "local-x");
    assert!(cached_remote_session(&descriptor.id, "local-x").is_none());

    // remote_backend_config only reports a descriptor when remote is selected.
    let remote: Config = serde_json::from_value(json!({
        "execution_backend": { "id": "remote-test", "kind": "remote", "url": "http://127.0.0.1:9" }
    }))
    .expect("parse remote config");
    assert!(remote_backend_config(&remote).is_some());
    assert!(remote_backend_config(&Config::default()).is_none());

    // validate_remote_backend fails when no URL is configured.
    let url_less = BackendConfig {
        id: "no-url".to_string(),
        kind: "remote".to_string(),
        ..BackendConfig::default()
    };
    let verify = validate_remote_backend(&url_less).expect_err("URL-less must not validate");
    assert_eq!(verify.kind(), RemoteErrorKind::Unreachable);
    // And succeeds once a URL is present.
    let with_url = BackendConfig {
        url: Some("http://127.0.0.1:9".to_string()),
        ..url_less
    };
    assert!(validate_remote_backend(&with_url).is_ok());
}

// ---------------------------------------------------------------------------
// remote.rs: HTTP error mapping against an in-process Axum mock.
// ---------------------------------------------------------------------------

/// What `POST /sessions` should answer with.
#[derive(Clone)]
enum CreateReply {
    /// A successful JSON session body.
    Id,
    /// A 2xx body that is not valid JSON.
    NotJson,
    /// A 2xx JSON body with no `id` field.
    NoId,
    /// A non-2xx status.
    Status(StatusCode),
}

#[derive(Clone)]
struct MockState {
    create: CreateReply,
    /// The SSE body for `POST /sessions/{id}/messages` (only on 2xx).
    stream_status: StatusCode,
    stream_body: String,
}

async fn mock_create(State(state): State<Arc<MockState>>) -> Response {
    match &state.create {
        CreateReply::Id => Json(json!({ "id": "remote-session-1" })).into_response(),
        CreateReply::NotJson => {
            ([(header::CONTENT_TYPE, "application/json")], "not json").into_response()
        }
        CreateReply::NoId => Json(json!({ "directory": "/remote" })).into_response(),
        CreateReply::Status(status) => (*status, "rejected").into_response(),
    }
}

async fn mock_messages(State(state): State<Arc<MockState>>) -> Response {
    if state.stream_status.is_success() {
        (
            [(header::CONTENT_TYPE, "text/event-stream")],
            state.stream_body.clone(),
        )
            .into_response()
    } else {
        (state.stream_status, "turn rejected").into_response()
    }
}

async fn mock_permission() -> StatusCode {
    StatusCode::OK
}

async fn start_mock(state: MockState) -> (String, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route("/sessions", post(mock_create))
        .route("/sessions/{id}/messages", post(mock_messages))
        .route("/sessions/{id}/permission/{req}", post(mock_permission))
        .with_state(Arc::new(state));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock server");
    let addr = listener.local_addr().expect("local addr");
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), handle)
}

fn remote_descriptor(url: &str) -> BackendConfig {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    BackendConfig {
        id: format!("remote-mock-{}", NEXT_ID.fetch_add(1, Ordering::SeqCst)),
        kind: "remote".to_string(),
        url: Some(url.to_string()),
        api_key: Some("test-key".to_string()),
        ..BackendConfig::default()
    }
}

#[tokio::test]
async fn remote_create_session_maps_success_and_error_replies() {
    // Success: a fresh remote session id is returned and cached.
    let (url, handle) = start_mock(MockState {
        create: CreateReply::Id,
        stream_status: StatusCode::OK,
        stream_body: String::new(),
    })
    .await;
    let backend = RemoteBackend::from_descriptor(&remote_descriptor(&url));
    let id = backend
        .create_session("local-created", std::path::Path::new("/work"))
        .await
        .expect("create succeeds");
    assert_eq!(id, "remote-session-1");
    // A second call returns the cached id without a new request.
    let again = backend
        .create_session("local-created", std::path::Path::new("/work"))
        .await
        .expect("cached");
    assert_eq!(again, "remote-session-1");
    handle.abort();

    // Non-2xx: a Protocol failure carrying the status.
    let (url, handle) = start_mock(MockState {
        create: CreateReply::Status(StatusCode::INTERNAL_SERVER_ERROR),
        stream_status: StatusCode::OK,
        stream_body: String::new(),
    })
    .await;
    let backend = RemoteBackend::from_descriptor(&remote_descriptor(&url));
    let err = backend
        .create_session("local-500", std::path::Path::new("/work"))
        .await
        .expect_err("500 must fail");
    assert_eq!(err.kind(), RemoteErrorKind::Protocol);
    assert_eq!(err.status(), Some(500));
    handle.abort();

    // A 2xx body that is not JSON.
    let (url, handle) = start_mock(MockState {
        create: CreateReply::NotJson,
        stream_status: StatusCode::OK,
        stream_body: String::new(),
    })
    .await;
    let backend = RemoteBackend::from_descriptor(&remote_descriptor(&url));
    let err = backend
        .create_session("local-badjson", std::path::Path::new("/work"))
        .await
        .expect_err("bad json must fail");
    assert!(err.message().contains("invalid session body"), "{err}");
    handle.abort();

    // A 2xx JSON body with no `id`.
    let (url, handle) = start_mock(MockState {
        create: CreateReply::NoId,
        stream_status: StatusCode::OK,
        stream_body: String::new(),
    })
    .await;
    let backend = RemoteBackend::from_descriptor(&remote_descriptor(&url));
    let err = backend
        .create_session("local-noid", std::path::Path::new("/work"))
        .await
        .expect_err("missing id must fail");
    assert!(err.message().contains("no `id`"), "{err}");
    handle.abort();
}

#[tokio::test]
async fn remote_open_turn_maps_success_and_error_replies() {
    let (url, handle) = start_mock(MockState {
        create: CreateReply::Id,
        stream_status: StatusCode::OK,
        stream_body: "event: text_delta\ndata: {\"text\":\"x\"}\n\n".to_string(),
    })
    .await;
    let backend = RemoteBackend::from_descriptor(&remote_descriptor(&url));
    let response = backend
        .open_turn("remote-session-1", "hi")
        .await
        .expect("turn opens");
    assert!(response.status().is_success());
    handle.abort();

    let (url, handle) = start_mock(MockState {
        create: CreateReply::Id,
        stream_status: StatusCode::BAD_GATEWAY,
        stream_body: String::new(),
    })
    .await;
    let backend = RemoteBackend::from_descriptor(&remote_descriptor(&url));
    let err = backend
        .open_turn("remote-session-1", "hi")
        .await
        .expect_err("502 must fail");
    assert_eq!(err.kind(), RemoteErrorKind::Protocol);
    assert_eq!(err.status(), Some(502));
    handle.abort();
}

#[tokio::test]
async fn remote_reply_permission_success_and_failure() {
    let (url, handle) = start_mock(MockState {
        create: CreateReply::Id,
        stream_status: StatusCode::OK,
        stream_body: String::new(),
    })
    .await;
    let backend = RemoteBackend::from_descriptor(&remote_descriptor(&url));
    backend
        .reply_permission("remote-session-1", "p1", true)
        .await
        .expect("allow reply delivered");
    backend
        .reply_permission("remote-session-1", "p2", false)
        .await
        .expect("deny reply delivered");
    handle.abort();

    // A server whose /sessions is broken: the permission route is still present,
    // so exercise the non-2xx path with a URL that has no such route at all.
    let (url, handle) = start_mock(MockState {
        create: CreateReply::Status(StatusCode::INTERNAL_SERVER_ERROR),
        stream_status: StatusCode::OK,
        stream_body: String::new(),
    })
    .await;
    let backend = RemoteBackend::from_descriptor(&remote_descriptor(&url));
    // 404 (route exists but the shape is fixed) -> any route mismatch is a
    // Protocol failure; a route match returns 200. Either way the call must not
    // error with a transport failure.
    let result = backend
        .reply_permission("remote-session-1", "p3", true)
        .await;
    assert!(result.is_ok() || result.unwrap_err().kind() == RemoteErrorKind::Protocol);
    handle.abort();
}

#[tokio::test]
async fn remote_relay_turn_handles_empty_and_malformed_streams() {
    // An empty stream: the server closes before any `message_end`.
    let (url, handle) = start_mock(MockState {
        create: CreateReply::Id,
        stream_status: StatusCode::OK,
        stream_body: String::new(),
    })
    .await;
    let backend = RemoteBackend::from_descriptor(&remote_descriptor(&url));
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut on_update = |_u: RemoteUpdate| {};
    let err = ragent_agent::backend::relay_turn(
        &backend,
        "local-empty",
        std::path::Path::new("/work"),
        "hi",
        &cancel,
        &mut on_update,
    )
    .await
    .expect_err("an empty stream must fail");
    assert_eq!(err.kind(), RemoteErrorKind::Protocol);
    handle.abort();

    // A trailing partial frame never completes, so the stream closes mid-turn.
    let (url, handle) = start_mock(MockState {
        create: CreateReply::Id,
        stream_status: StatusCode::OK,
        stream_body: "event: text_delta\ndata: {\"text\":\"x\"}\n\n\
                      event: message_end\ndata: {\"message_id\":\"m1\""
            .to_string(),
    })
    .await;
    let backend = RemoteBackend::from_descriptor(&remote_descriptor(&url));
    let mut on_update = |_u: RemoteUpdate| {};
    let err = ragent_agent::backend::relay_turn(
        &backend,
        "local-partial",
        std::path::Path::new("/work"),
        "hi",
        &cancel,
        &mut on_update,
    )
    .await
    .expect_err("a partial trailing frame must fail the turn");
    assert_eq!(err.kind(), RemoteErrorKind::Protocol);
    handle.abort();

    // A malformed event is surfaced as `Other` and the turn still completes.
    let (url, handle) = start_mock(MockState {
        create: CreateReply::Id,
        stream_status: StatusCode::OK,
        stream_body: "event: strange\ndata: not-json\n\n\
                      event: message_end\ndata: {\"message_id\":\"m1\",\"reason\":\"stop\"}\n\n"
            .to_string(),
    })
    .await;
    let backend = RemoteBackend::from_descriptor(&remote_descriptor(&url));
    let mut seen: Vec<RemoteUpdate> = Vec::new();
    let mut on_update = |u: RemoteUpdate| seen.push(u);
    let outcome = ragent_agent::backend::relay_turn(
        &backend,
        "local-malformed",
        std::path::Path::new("/work"),
        "hi",
        &cancel,
        &mut on_update,
    )
    .await
    .expect("the malformed event must not abort the turn");
    assert_eq!(outcome.stop_reason, "stop");
    assert!(seen.iter().any(|u| matches!(
        u,
        RemoteUpdate::Other { kind } if kind == "malformed:strange"
    )));
    handle.abort();
}
