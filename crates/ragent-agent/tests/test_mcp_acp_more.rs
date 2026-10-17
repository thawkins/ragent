//! Additional public-surface coverage for `mcp/mod.rs`, `mcp/discovery.rs`,
//! and `acp/mod.rs` (tests only; no production edits).
//!
//! Three areas are targeted:
//!
//! - **MCP client seams** (`mcp/mod.rs`): the [`McpClientBackend`] trait
//!   delegation, the `*_for_tests` server-list seams, the `discover()` /
//!   `discover_locations()` static wrappers, `Default`, and the full
//!   HTTP-transport lifecycle (`connect` -> `call_tool` -> `refresh` ->
//!   `disconnect_all` -> `shutdown`) driven through the public API against an
//!   in-process `axum` Streamable-HTTP mock.
//! - **MCP discovery** (`mcp/discovery.rs`): a fake `npm` on `PATH` plus a
//!   redirected `XDG_CONFIG_HOME` populate the npm-global and ragent-native
//!   registry rows so `scan_npm_global` / `scan_npm_mcp_scope` /
//!   `try_npm_mcp_package` and `scan_mcp_registry` / `try_mcp_registry_entry` /
//!   `try_mcp_json_config` all run against real files.
//! - **ACP transport** (`acp/mod.rs`): a small Python JSON-RPC subprocess drives
//!   `AcpClient::spawn`/`initialize`/`create_session`/`prompt`/`shutdown` and
//!   `relay_turn`, covering every streamed update variant, the client-directed
//!   request and unknown-notification frame paths, a `session/new` reply with no
//!   `sessionId`, and the cancellation poll.
//!
//! `PATH` and `XDG_CONFIG_HOME` are process-global; the two discovery tests are
//! `#[serial_test::serial]` and this target opts into `unsafe` for the env
//! mutation (Rust 2024).
#![allow(unsafe_code)]

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::post;
use ragent_agent::acp::{ACP_PROTOCOL_VERSION, AcpClient, AcpErrorKind, AcpUpdate, relay_turn};
use ragent_agent::mcp::discovery::{discover, discover_locations};
use ragent_agent::mcp::{McpClient, McpClientBackend, McpStatus, McpToolDef};
use ragent_config::{AcpAgentConfig, McpNotificationMode, McpServerConfig, McpTransport};
use serde_json::json;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// A scratch directory under `target/temp` (never `/tmp`).
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/mcp-acp-more")
        .join(name);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Write `body` to an absolute, canonical scratch file and return that path.
///
/// The ACP child runs with its cwd set to the session working directory, so the
/// script path handed to `python3` must be absolute.
fn write_script(name: &str, body: &str) -> PathBuf {
    let path = scratch("scripts").join(name);
    std::fs::write(&path, body).expect("write script");
    std::fs::canonicalize(&path).expect("canonicalize script")
}

// ---------------------------------------------------------------------------
// MCP: HTTP-transport lifecycle + client seams
// ---------------------------------------------------------------------------

/// A minimal Streamable-HTTP MCP server that answers `initialize`,
/// `tools/list`, and `tools/call`.
#[derive(Clone, Default)]
struct Seen {
    initializes: Arc<AtomicUsize>,
    lists: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
}

async fn handler(State(seen): State<Seen>, headers: HeaderMap, body: String) -> impl IntoResponse {
    let request: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let id = request.get("id").cloned().unwrap_or(json!(1));
    let sse = |frame: serde_json::Value| {
        (
            [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
            format!("event: message\ndata: {frame}\n\n"),
        )
            .into_response()
    };
    match request.get("method").and_then(|m| m.as_str()).unwrap_or("") {
        "initialize" => {
            seen.initializes.fetch_add(1, Ordering::SeqCst);
            let mut response = sse(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "protocolVersion": "2024-11-05" }
            }));
            response.headers_mut().insert(
                "mcp-session-id",
                axum::http::HeaderValue::from_static("more-session"),
            );
            response
        }
        "tools/list" => {
            assert!(
                headers.contains_key("mcp-session-id"),
                "later requests must replay the negotiated session"
            );
            seen.lists.fetch_add(1, Ordering::SeqCst);
            sse(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": [{
                        "name": "echo",
                        "description": "Echo a value",
                        "inputSchema": { "type": "object" }
                    }]
                }
            }))
        }
        "tools/call" => {
            seen.calls.fetch_add(1, Ordering::SeqCst);
            let name = request
                .get("params")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            sse(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": format!("called {name}") }],
                    "isError": false
                }
            }))
        }
        other => panic!("unexpected MCP method {other}"),
    }
}

async fn spawn_http_server(seen: Seen) -> SocketAddr {
    let router = Router::new().route("/mcp", post(handler)).with_state(seen);
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("bind test server");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    addr
}

fn http_config(addr: SocketAddr) -> McpServerConfig {
    McpServerConfig {
        type_: McpTransport::Http,
        url: Some(format!("http://{addr}/mcp")),
        headers: std::collections::HashMap::from([("X-Test".to_string(), "1".to_string())]),
        ..McpServerConfig::default()
    }
}

/// The whole HTTP lifecycle through the public surface: the `McpClientBackend`
/// trait delegates to the inherent methods, `call_tool` routes by server id,
/// `refresh_tools` re-queries, and `disconnect_all` / `shutdown` tear the server
/// down without error.
#[tokio::test]
async fn mcp_http_lifecycle_and_backend_trait() {
    let seen = Seen::default();
    let addr = spawn_http_server(seen.clone()).await;

    let mut client = McpClient::new();
    // `Default` must agree with `new()`.
    assert_eq!(McpClient::default().servers().len(), 0);

    client
        .connect("http-server", http_config(addr))
        .await
        .expect("connect the http server");
    // `connect` probes first and adopts the live endpoint, so exactly one
    // `initialize` handshake runs on the wire (the probe).
    assert_eq!(seen.initializes.load(Ordering::SeqCst), 1, "probe only");

    // -- McpClientBackend trait delegation --
    let backend: &dyn McpClientBackend = &client;
    let all = backend.list_tools().await;
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].name, "echo");
    assert_eq!(backend.list_tools_for_server("http-server").await.len(), 1);
    assert_eq!(backend.list_tools_for_server("absent").await.len(), 0);

    let result = backend
        .call_tool("http-server", "echo", json!({ "value": "hi" }))
        .await
        .expect("call by server id");
    assert_eq!(result["content"][0]["text"], "called echo");
    let by_name = backend
        .call_tool_by_name("echo", json!({ "value": "x" }))
        .await
        .expect("call by name");
    assert_eq!(by_name["content"][0]["text"], "called echo");
    assert_eq!(seen.calls.load(Ordering::SeqCst), 2);

    // `refresh_tools` (all) and `refresh_tools_for_server` (one) re-list.
    let mut client = client;
    client.refresh_tools().await.expect("refresh all");
    let one = client
        .refresh_tools_for_server("http-server")
        .await
        .expect("refresh one");
    assert_eq!(one.len(), 1);
    assert_eq!(
        seen.lists.load(Ordering::SeqCst),
        3,
        "connect + 2 refreshes"
    );

    // A tool call routed to an unknown server names the server in the error.
    let err = client
        .call_tool("ghost", "echo", json!({}))
        .await
        .expect_err("unknown server must error");
    assert!(err.to_string().contains("not connected"), "got: {err}");

    // Teardown: disconnect_all then shutdown must both be idempotent no-ops.
    client.disconnect_all().await.expect("disconnect_all");
    client
        .disconnect_all()
        .await
        .expect("disconnect_all repeat");
    client.shutdown().await;
    client.shutdown().await;
    assert!(
        client
            .servers()
            .iter()
            .all(|s| s.status != McpStatus::Connected),
        "shutdown parks every server as not-connected"
    );
}

/// The `*_for_tests` seams mutate the recorded server list directly: connected
/// registration also indexes tools, `push_server_for_tests` leaves the index
/// untouched, and `servers_mut_for_tests` lets a caller flip a status.
#[tokio::test]
async fn mcp_test_seams_register_and_mutate_servers() {
    let mut client = McpClient::new();
    let tool = McpToolDef {
        name: "ping".to_string(),
        description: "Ping".to_string(),
        parameters: json!({ "type": "object" }),
    };
    client.register_connected_for_tests("s1", vec![tool]);
    assert_eq!(client.servers().len(), 1);
    assert_eq!(client.servers()[0].status, McpStatus::Connected);
    assert_eq!(client.list_tools_for_server("s1").len(), 1);

    // The tool index was rebuilt, so a name lookup resolves to s1 and then fails
    // at dispatch because s1 has no live connection (call_tool's error path).
    let err = client
        .call_tool_by_name("ping", json!({}))
        .await
        .expect_err("no live connection for s1");
    assert!(err.to_string().contains("not connected"), "got: {err}");

    // `push_server_for_tests` accepts a caller-built entry and does not index it.
    client.push_server_for_tests(ragent_agent::mcp::McpServer {
        id: "s2".to_string(),
        config: McpServerConfig {
            type_: McpTransport::Http,
            url: Some("http://127.0.0.1:1/mcp".to_string()),
            disabled: true,
            notification: McpNotificationMode::None,
            ..McpServerConfig::default()
        },
        status: McpStatus::Failed {
            error: "boom".to_string(),
        },
        tools: Vec::new(),
    });
    assert_eq!(client.servers().len(), 2);
    assert!(client.list_tools_for_server("s2").is_empty());

    // `servers_mut_for_tests` lets a caller clear a status (as `disconnect` does).
    for server in client.servers_mut_for_tests() {
        if server.id == "s1" {
            server.status = McpStatus::Disabled;
            server.tools.clear();
        }
    }
    assert_eq!(client.servers()[0].status, McpStatus::Disabled);
}

/// The `McpClient` static discovery wrappers delegate to the `discovery` module.
///
/// Serial: a sibling test mutates `PATH` / `XDG_CONFIG_HOME` process-wide, so a
/// concurrent scan here would observe the temp tree.
#[serial_test::serial]
#[tokio::test]
async fn mcp_static_discovery_wrappers_delegate() {
    // Both wrappers must return the same kind of de-duplicated result.
    let via_client = McpClient::discover().await;
    let via_module = discover().await;
    let mut client_ids: Vec<&str> = via_client.iter().map(|s| s.id.as_str()).collect();
    let mut module_ids: Vec<&str> = via_module.iter().map(|s| s.id.as_str()).collect();
    client_ids.sort_unstable();
    module_ids.sort_unstable();
    client_ids.dedup();
    assert_eq!(
        client_ids.len(),
        via_client.len(),
        "the client wrapper de-duplicates by id"
    );
    assert_eq!(client_ids, module_ids, "both wrappers agree");

    let locations = McpClient::discover_locations().await;
    assert!(!locations.is_empty());
    assert!(locations.iter().any(|l| l.label.starts_with("PATH")));

    // A disabled server connects without any transport handshake.
    let mut client = McpClient::new();
    client
        .connect(
            "off",
            McpServerConfig {
                disabled: true,
                ..McpServerConfig::default()
            },
        )
        .await
        .expect("register disabled");
    assert_eq!(client.servers()[0].status, McpStatus::Disabled);
}

// ---------------------------------------------------------------------------
// MCP discovery: npm-global + ragent-native registry rows
// ---------------------------------------------------------------------------

/// A fake `npm` that reports a temp prefix, plus a redirected `XDG_CONFIG_HOME`
/// holding a ragent-native registry with both a directory entry (`server.json`)
/// and a standalone JSON config. Both the npm and registry scan paths then run
/// against real files.
#[serial_test::serial]
#[tokio::test]
async fn discovery_scans_npm_global_and_registry_dirs() {
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let original_xdg = std::env::var_os("XDG_CONFIG_HOME");

    // -- npm global tree ----------------------------------------------------
    let prefix = scratch("npm-prefix");
    let node_modules = prefix.join("lib").join("node_modules");
    let scope_pkg = node_modules.join("@modelcontextprotocol").join("server-fs");
    let plain_pkg = node_modules.join("mcp-server-plain");
    std::fs::create_dir_all(&scope_pkg).expect("scope pkg dir");
    std::fs::create_dir_all(&plain_pkg).expect("plain pkg dir");
    std::fs::write(scope_pkg.join("index.js"), b"// scope entry\n").expect("scope entry");
    std::fs::write(plain_pkg.join("cli.js"), b"// plain entry\n").expect("plain entry");
    std::fs::write(
        scope_pkg.join("package.json"),
        r#"{"name":"@modelcontextprotocol/server-fs","description":"FS server","bin":"index.js"}"#,
    )
    .expect("scope package.json");
    std::fs::write(
        plain_pkg.join("package.json"),
        r#"{"name":"mcp-server-plain","bin":{"plain":"cli.js"}}"#,
    )
    .expect("plain package.json");

    // A fake `npm` that prints the prefix; placed first on PATH.
    let fake_bin = scratch("fakebin");
    let npm = fake_bin.join("npm");
    std::fs::write(&npm, format!("#!/bin/sh\necho '{}'\n", prefix.display()))
        .expect("write fake npm");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&npm, std::fs::Permissions::from_mode(0o755))
            .expect("chmod fake npm");
    }

    // -- ragent-native registry --------------------------------------------
    let xdg = scratch("xdg");
    let servers_dir = xdg.join("ragent").join("servers");
    let dir_entry = servers_dir.join("dir-entry");
    std::fs::create_dir_all(&dir_entry).expect("registry dir entry");
    std::fs::write(
        dir_entry.join("server.json"),
        r#"{"command":"/bin/true","name":"Dir Entry","args":["--x"],"env":{"K":"V"}}"#,
    )
    .expect("server.json");
    std::fs::write(
        servers_dir.join("standalone.json"),
        r#"{"command":"/bin/echo","args":["hi"]}"#,
    )
    .expect("standalone.json");

    let mut entries = vec![fake_bin.clone()];
    entries.extend(std::env::split_paths(&original_path));
    let joined = std::env::join_paths(entries).expect("join PATH");
    // SAFETY: process-global mutation, serialised by `serial_test::serial`.
    unsafe {
        std::env::set_var("PATH", &joined);
        std::env::set_var("XDG_CONFIG_HOME", &xdg);
    }

    let locations = discover_locations().await;
    // `discover()` de-duplicates the same servers into one flat list; it must run
    // while the temp env is still in place.
    let flat = discover().await;

    // SAFETY: restore before asserting so a panic cannot leak the temp env.
    unsafe {
        std::env::set_var("PATH", &original_path);
        match &original_xdg {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }

    // npm global row carries the two npm packages.
    let npm_row = locations
        .iter()
        .find(|l| l.label == "npm global")
        .expect("npm global row");
    assert!(npm_row.exists, "the fake prefix has a node_modules tree");
    let npm_ids: Vec<&str> = npm_row.servers.iter().map(|s| s.id.as_str()).collect();
    assert!(npm_ids.contains(&"fs"), "scoped package id: {npm_ids:?}");
    assert!(npm_ids.contains(&"plain"), "plain package id: {npm_ids:?}");
    assert_eq!(
        npm_row
            .servers
            .iter()
            .find(|s| s.id == "fs")
            .map(|s| s.name.as_str()),
        Some("FS server")
    );

    // ragent-native registry row carries the directory + standalone entries.
    let native_row = locations
        .iter()
        .find(|l| l.label == "ragent-native")
        .expect("ragent-native row");
    assert!(native_row.exists, "the registry dir exists");
    let native_ids: Vec<&str> = native_row.servers.iter().map(|s| s.id.as_str()).collect();
    assert!(
        native_ids.contains(&"dir-entry"),
        "registry dir id: {native_ids:?}"
    );
    assert!(
        native_ids.contains(&"standalone"),
        "standalone json id: {native_ids:?}"
    );

    // `discover()` de-duplicates the same servers into one flat list.
    let flat_ids: Vec<&str> = flat.iter().map(|s| s.id.as_str()).collect();
    assert!(flat_ids.contains(&"fs"));
    assert!(flat_ids.contains(&"dir-entry"));

    // A registry-sourced server converts to a disabled stdio config with env.
    let dir_server = native_row
        .servers
        .iter()
        .find(|s| s.id == "dir-entry")
        .expect("dir-entry");
    let config = dir_server.to_config();
    assert!(config.disabled);
    assert_eq!(config.type_, McpTransport::Stdio);
    assert_eq!(config.env.get("K"), Some(&"V".to_string()));
    assert_eq!(config.args, vec!["--x".to_string()]);
}

// ---------------------------------------------------------------------------
// ACP: a scripted JSON-RPC subprocess
// ---------------------------------------------------------------------------

/// A small ACP agent whose behaviour is selected by its first argument.
const ACP_AGENT: &str = r#"
import sys, json
behavior = sys.argv[1] if len(sys.argv) > 1 else "variants"

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except Exception:
        continue
    method = req.get("method")
    rid = req.get("id")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": rid, "result": {"protocolVersion": 1}})
    elif method == "session/new":
        if behavior == "nosession":
            send({"jsonrpc": "2.0", "id": rid, "result": {}})
        else:
            send({"jsonrpc": "2.0", "id": rid, "result": {"sessionId": "s-1"}})
    elif method == "session/prompt":
        if behavior == "stall":
            continue
        if behavior == "clientreq":
            # A request directed at the client, an unknown notification, and a
            # reply for an id the client never issued.
            send({"jsonrpc": "2.0", "id": 999, "method": "fs/read_text_file", "params": {}})
            send({"jsonrpc": "2.0", "method": "custom/unknown"})
            send({"jsonrpc": "2.0", "id": 424242, "result": {}})
        if behavior == "variants":
            updates = [
                {"sessionUpdate": "agent_message_chunk",
                 "content": {"type": "text", "text": "hello "}},
                {"sessionUpdate": "agent_thought_chunk",
                 "content": [{"type": "text", "text": "why"}]},
                {"sessionUpdate": "tool_call", "toolCallId": "c1",
                 "title": "Read", "status": "in_progress"},
                {"sessionUpdate": "tool_call_update", "toolCallId": "c1",
                 "status": "completed"},
                {"sessionUpdate": "plan", "entries": [{"s": 1}, {"s": 2}]},
                {"sessionUpdate": "future_kind"},
            ]
            for u in updates:
                send({"jsonrpc": "2.0", "method": "session/update",
                      "params": {"sessionId": "s-1", "update": u}})
        send({"jsonrpc": "2.0", "id": rid, "result": {"stopReason": "end_turn"}})
"#;

/// An `AcpAgentConfig` running the scripted agent in `behavior` mode.
fn acp_agent(behavior: &str) -> AcpAgentConfig {
    let script = write_script("more_agent.py", ACP_AGENT);
    AcpAgentConfig {
        id: "more".to_string(),
        name: None,
        enabled: true,
        command: "python3".to_string(),
        args: vec![script.to_string_lossy().into_owned(), behavior.to_string()],
        env: std::collections::HashMap::new(),
        cwd: None,
        turn_timeout_secs: 10,
        system_prompt: None,
    }
}

/// `AcpClient` driven directly: accessors, the handshake, a full turn streaming
/// every update variant, and a repeatable `shutdown`.
#[tokio::test]
async fn acp_client_streams_every_update_variant() {
    let agent = acp_agent("variants");
    let mut client = AcpClient::spawn(&agent, Path::new("."))
        .await
        .expect("spawn the scripted agent");
    assert_eq!(client.agent_id(), "more");
    assert_eq!(client.display_name(), "more");
    assert!(client.is_alive());

    let init = client.initialize().await.expect("initialize");
    assert_eq!(
        init.get("protocolVersion")
            .and_then(serde_json::Value::as_u64),
        Some(ACP_PROTOCOL_VERSION)
    );
    let session_id = client
        .create_session(Path::new("."))
        .await
        .expect("create session");
    assert_eq!(session_id, "s-1");

    let cancel = AtomicBool::new(false);
    let mut rendered: Vec<AcpUpdate> = Vec::new();
    let mut on_update = |u: AcpUpdate| rendered.push(u);
    let outcome = client
        .prompt(&session_id, "hi", &cancel, &mut on_update)
        .await
        .expect("the turn must complete");
    assert_eq!(outcome.stop_reason, "end_turn");
    assert_eq!(outcome.text, "hello ");
    assert_eq!(outcome.updates, 6, "all six streamed updates are delivered");
    assert_eq!(rendered.len(), 6);
    assert_eq!(
        rendered[0],
        AcpUpdate::AgentMessageChunk {
            text: "hello ".to_string()
        }
    );
    assert!(matches!(rendered[2], AcpUpdate::ToolCall { .. }));
    assert!(matches!(rendered[4], AcpUpdate::Plan { entries: 2 }));
    assert_eq!(
        rendered[5],
        AcpUpdate::Other {
            kind: "future_kind".to_string()
        }
    );

    client.shutdown().await;
    client.shutdown().await; // idempotent
    assert!(!client.is_alive());
}

/// A subprocess that also sends a client-directed request, an unknown
/// notification, and a stray reply frame must still complete the turn (the
/// client answers the request and ignores the rest).
#[tokio::test]
async fn acp_relay_tolerates_client_directed_and_unknown_frames() {
    let agent = acp_agent("clientreq");
    let cancel = AtomicBool::new(false);
    let mut count = 0usize;
    let mut on_update = |_u: AcpUpdate| count += 1;
    let outcome = relay_turn(&agent, Path::new("."), "hi", &cancel, &mut on_update)
        .await
        .expect("stray frames must not fail the turn");
    assert_eq!(outcome.stop_reason, "end_turn");
    assert_eq!(count, 0, "no session/update frames were sent");
}

/// A `session/new` reply without a `sessionId` fails the turn with the protocol
/// cause and a message naming the missing field.
#[tokio::test]
async fn acp_session_new_without_session_id_fails() {
    let agent = acp_agent("nosession");
    let cancel = AtomicBool::new(false);
    let mut on_update = |_u: AcpUpdate| {};
    let err = relay_turn(&agent, Path::new("."), "hi", &cancel, &mut on_update)
        .await
        .expect_err("a reply with no sessionId must fail");
    assert_eq!(err.kind(), AcpErrorKind::Protocol);
    assert!(
        err.message().contains("sessionId"),
        "got: {}",
        err.message()
    );
}

/// A cancelled turn is reported as a protocol failure and the subprocess is
/// killed promptly, without waiting for the turn timeout.
#[tokio::test]
async fn acp_cancel_flag_aborts_a_stalled_turn() {
    let mut agent = acp_agent("stall");
    agent.turn_timeout_secs = 30;
    let cancel = Arc::new(AtomicBool::new(false));
    let flipper = Arc::clone(&cancel);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(400)).await;
        flipper.store(true, Ordering::SeqCst);
    });

    let mut on_update = |_u: AcpUpdate| {};
    let started = std::time::Instant::now();
    let err = relay_turn(
        &agent,
        Path::new("."),
        "hi",
        cancel.as_ref(),
        &mut on_update,
    )
    .await
    .expect_err("a cancelled turn must fail");
    assert_eq!(err.kind(), AcpErrorKind::Protocol);
    assert!(
        err.message().contains("cancelled"),
        "got: {}",
        err.message()
    );
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "cancellation must not wait for the 30s turn timeout"
    );
}
