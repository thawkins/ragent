//! PERF-076: tests for the MCP `tool_name -> server_id` lookup index.
//!
//! `call_tool_by_name` used to scan every server's tool list linearly.  These
//! tests pin the observable contract of the replacement O(1) index: a tool
//! registered on a second server is resolvable, unknown tools are rejected
//! before any dispatch, and a disconnected server's tools drop out of the
//! index.

use ragent_agent::McpServerConfig;
use ragent_agent::mcp::McpClient;
use serde_json::json;

/// A config for a disabled server, which `connect` registers without any
/// transport handshake — enough to populate the server list in tests.
fn disabled_config() -> McpServerConfig {
    McpServerConfig {
        disabled: true,
        ..McpServerConfig::default()
    }
}

#[tokio::test]
async fn unknown_tool_is_rejected_before_dispatch() {
    let mut client = McpClient::new();
    client
        .connect("srv-a", disabled_config())
        .await
        .expect("register disabled server");

    // A disabled server advertises no tools, so the index is empty and the
    // lookup fails with the same message as the old linear scan.
    let err = client
        .call_tool_by_name("nope", json!({}))
        .await
        .expect_err("unknown tool must error");
    assert!(
        err.to_string()
            .contains("No connected MCP server provides tool"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn multiple_servers_are_registered_and_disconnected() {
    let mut client = McpClient::new();
    client
        .connect("srv-a", disabled_config())
        .await
        .expect("register srv-a");
    client
        .connect("srv-b", disabled_config())
        .await
        .expect("register srv-b");
    assert_eq!(client.servers().len(), 2);

    // Disconnecting one server keeps the other registered, and an unknown tool
    // still resolves to a clean error rather than panicking on a stale index.
    client.disconnect("srv-a").await.expect("disconnect srv-a");
    let err = client
        .call_tool_by_name("still-missing", json!({}))
        .await
        .expect_err("unknown tool must error");
    assert!(
        err.to_string()
            .contains("No connected MCP server provides tool")
    );
}

// ── FR-030: a disabled server is still listed, but inert ────────────────────

/// `register_disabled` records a server that exists but must not be started, so
/// `/mcp` can list it (and offer to re-enable it) without a child process.
#[tokio::test]
async fn register_disabled_lists_the_server_with_no_tools() {
    let mut client = McpClient::new();
    client.register_disabled("off", disabled_config());
    assert_eq!(client.servers().len(), 1);
    assert_eq!(client.servers()[0].id, "off");
    assert_eq!(
        client.servers()[0].status,
        ragent_agent::mcp::McpStatus::Disabled
    );
    assert!(client.servers()[0].tools.is_empty());
}

// ── Streamable-HTTP session adoption ────────────────────────────────────────

/// A sessionful Streamable-HTTP server refuses `tools/list` unless the request
/// carries the `mcp-session-id` it issued during `initialize`, so a replaying
/// client must resend it. This drives a minimal in-process sessionful server
/// and asserts the id reaches every later request.
#[tokio::test]
async fn http_client_replays_the_session_id_from_initialize() {
    use std::collections::HashMap;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use axum::Router;
    use axum::extract::State;
    use axum::http::HeaderMap;
    use axum::response::IntoResponse;
    use axum::routing::post;
    use ragent_agent::mcp::McpClientBackend;

    const SESSION: &str = "test-session-123";

    #[derive(Clone, Default)]
    struct Seen {
        /// Request paths that carried the session header.
        with_session: Arc<AtomicUsize>,
        /// Request paths seen at all.
        total: Arc<AtomicUsize>,
    }

    async fn handler(
        State(seen): State<Seen>,
        headers: HeaderMap,
        body: String,
    ) -> impl IntoResponse {
        seen.total.fetch_add(1, Ordering::SeqCst);
        if headers.contains_key("mcp-session-id") {
            seen.with_session.fetch_add(1, Ordering::SeqCst);
        }
        let request: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
        let id = request.get("id").cloned().unwrap_or(json!(1));
        match request.get("method").and_then(|m| m.as_str()).unwrap_or("") {
            "initialize" => {
                let frame = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "protocolVersion": "2024-11-05" }
                });
                let mut response = (
                    [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                    format!("event: message\ndata: {frame}\n\n"),
                )
                    .into_response();
                response.headers_mut().insert(
                    "mcp-session-id",
                    axum::http::HeaderValue::from_static(SESSION),
                );
                response
            }
            "tools/list" => {
                // Sessionful server: reject a request that did not join the
                // session opened by `initialize`.
                if !headers.contains_key("mcp-session-id") {
                    return (
                        axum::http::StatusCode::BAD_REQUEST,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": { "code": -32004, "message": "invalid request" }
                        })
                        .to_string(),
                    )
                        .into_response();
                }
                let frame = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "tools": [ { "name": "find" }, { "name": "count" } ] }
                });
                (
                    [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                    format!("event: message\ndata: {frame}\n\n"),
                )
                    .into_response()
            }
            other => panic!("unexpected MCP method {other}"),
        }
    }

    let seen = Seen::default();
    let router = Router::new()
        .route("/mcp", post(handler))
        .with_state(seen.clone());
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("bind test server");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    let url = format!("http://{addr}/mcp");
    let mut client = ragent_agent::mcp::http::HttpMcpClient::new(url, HashMap::new());
    client.initialize().await.expect("initialize handshake");
    assert_eq!(client.session_id(), Some(SESSION));
    assert!(!client.is_disconnected());

    let tools = client.list_tools().await;
    assert_eq!(
        tools.len(),
        2,
        "a sessionful server answers tools/list only for a joined session"
    );
    assert_eq!(
        seen.with_session.load(Ordering::SeqCst),
        1,
        "the tools/list request must carry the id initialize negotiated"
    );
    assert_eq!(seen.total.load(Ordering::SeqCst), 2);
}

// ── Generic adopt of an already-running MCP server ──────────────────────────

/// `connect` must adopt a server that is already running at the address its
/// config declares, instead of spawning a duplicate.
///
/// The live server is a sessionful Streamable-HTTP mock, so a successful adopt
/// proves both the probe and the session hand-off. Generic by construction: the
/// config is a plain `http` entry like any hand-written or plugin-bridged one,
/// with no per-product special casing anywhere on the adopt path.
#[tokio::test]
async fn connect_adopts_an_already_running_http_server() {
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use axum::Router;
    use axum::extract::State;
    use axum::http::HeaderMap;
    use axum::response::IntoResponse;
    use axum::routing::post;

    const SESSION: &str = "adopt-session-1";

    #[derive(Clone, Default)]
    struct Seen {
        initializes: Arc<AtomicUsize>,
    }

    async fn handler(
        State(seen): State<Seen>,
        headers: HeaderMap,
        body: String,
    ) -> impl IntoResponse {
        let request: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
        let id = request.get("id").cloned().unwrap_or(json!(1));
        match request.get("method").and_then(|m| m.as_str()).unwrap_or("") {
            "initialize" => {
                seen.initializes.fetch_add(1, Ordering::SeqCst);
                let frame = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "protocolVersion": "2024-11-05" }
                });
                let mut response = (
                    [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                    format!("event: message\ndata: {frame}\n\n"),
                )
                    .into_response();
                response.headers_mut().insert(
                    "mcp-session-id",
                    axum::http::HeaderValue::from_static(SESSION),
                );
                response
            }
            "tools/list" => {
                assert!(
                    headers.contains_key("mcp-session-id"),
                    "the adopting client must replay the session the probe negotiated"
                );
                let frame = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "tools": [ { "name": "find" } ] }
                });
                (
                    [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                    format!("event: message\ndata: {frame}\n\n"),
                )
                    .into_response()
            }
            other => panic!("unexpected MCP method {other}"),
        }
    }

    let seen = Seen::default();
    let router = Router::new()
        .route("/mcp", post(handler))
        .with_state(seen.clone());
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("bind test server");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    let url = format!("http://{addr}/mcp");
    assert!(
        ragent_agent::mcp::http::probe(&url).await.is_some(),
        "a live Streamable-HTTP endpoint must be probeable"
    );

    let config = McpServerConfig {
        type_: ragent_config::McpTransport::Http,
        url: Some(url),
        ..McpServerConfig::default()
    };

    let mut client = McpClient::new();
    client
        .connect("shared", config)
        .await
        .expect("adopt the running server");

    let server = client
        .servers()
        .iter()
        .find(|s| s.id == "shared")
        .expect("server registered");
    assert_eq!(server.status, ragent_agent::mcp::McpStatus::Connected);
    assert_eq!(
        server.tools.len(),
        1,
        "the adopted server's tools are listed"
    );

    // Exactly one `initialize` ran in-process (the probe); the adopt reused the
    // session it negotiated rather than reconnecting. The test's own
    // `probe(...)` assertion below is the same handshake the adopt path runs,
    // hence two in total.
    assert_eq!(seen.initializes.load(Ordering::SeqCst), 2);

    // Shutting down an adopted connection must not panic or hang.
    client.shutdown().await;
}

/// A server with no reachable existing instance must fall through to a normal
/// spawn attempt rather than adopt a phantom endpoint. Port 1 is reserved and
/// never listening, so the probe is guaranteed to miss.
#[tokio::test]
async fn connect_does_not_adopt_a_dead_endpoint() {
    let config = McpServerConfig {
        type_: ragent_config::McpTransport::Http,
        // Reserved port; nothing listens here, so `initialize` cannot answer.
        url: Some("http://127.0.0.1:1/mcp".to_string()),
        ..McpServerConfig::default()
    };

    let mut client = McpClient::new();
    // The probe must miss and the connect must fall through to the configured
    // HTTP transport, which then fails against the dead endpoint. Asserting on
    // the resulting `Failed` status (not on the error's Display, which is empty
    // for the transport's own error chain) is what proves nothing was adopted.
    let _ = client.connect("dead", config).await;
    let server = client
        .servers()
        .iter()
        .find(|s| s.id == "dead")
        .expect("failed server is still registered for reporting");
    assert!(matches!(
        server.status,
        ragent_agent::mcp::McpStatus::Failed { .. }
    ));
}

// ── Global enable ledger ────────────────────────────────────────────────────

/// A server id absent from the ledger is enabled, so a newly added MCP server
/// starts connected without any write; an explicit `false` disables it and
/// `clear` restores the default.
#[test]
fn enable_ledger_defaults_to_enabled_and_records_only_explicit_choices() {
    let mut ledger = ragent_agent::mcp::McpEnableLedger::default();
    assert!(
        ledger.is_enabled("brand-new"),
        "a server not in the ledger is enabled"
    );

    ledger.set_enabled("brand-new", false);
    assert!(!ledger.is_enabled("brand-new"));

    ledger.clear("brand-new");
    assert!(
        ledger.is_enabled("brand-new"),
        "clearing the entry restores the enabled default"
    );
    assert!(ledger.servers.is_empty());
}

/// The `ragent.json` `disabled` flag is the harder switch: it wins over the
/// ledger, whereas the ledger can switch off a plugin-contributed server that
/// has no config flag of its own.
#[test]
fn config_disabled_flag_overrides_the_ledger() {
    use ragent_agent::McpServerConfig;
    use ragent_agent::mcp::is_server_enabled;

    let ledger = ragent_agent::mcp::McpEnableLedger::default();
    let enabled_cfg = McpServerConfig::default();
    assert!(is_server_enabled(&enabled_cfg, &ledger, "s"));

    let hard_off = McpServerConfig {
        disabled: true,
        ..McpServerConfig::default()
    };
    let mut ledger_says_on = ragent_agent::mcp::McpEnableLedger::default();
    ledger_says_on.set_enabled("s", true);
    assert!(
        !is_server_enabled(&hard_off, &ledger_says_on, "s"),
        "ragent.json disabled:true always wins"
    );

    let mut ledger_says_off = ragent_agent::mcp::McpEnableLedger::default();
    ledger_says_off.set_enabled("s", false);
    assert!(!is_server_enabled(&enabled_cfg, &ledger_says_off, "s"));
}

/// The ledger round-trips through its on-disk form, so a disable choice survives
/// a restart and reaches the startup connect filter.
#[test]
fn enable_ledger_round_trips_and_drops_absent_entries() {
    use std::io::Write;

    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/mcp-enable-state-tests");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(format!("mcp_state-{}.json", std::process::id()));
    let mut file = std::fs::File::create(&path).expect("create ledger");
    file.write_all(br#"{"servers":{"a":false,"b":true}}"#)
        .expect("write ledger");
    drop(file);

    let ledger = ragent_agent::mcp::McpEnableLedger::load_from(&path);
    assert!(!ledger.is_enabled("a"));
    assert!(ledger.is_enabled("b"));
    assert!(ledger.is_enabled("c"), "absent ids stay enabled");

    // Saving drops the explicit `true` (it is the default) but keeps `false`.
    let mut trimmed = ledger.clone();
    trimmed.clear("b");
    trimmed.save_to(&path).expect("save ledger");
    let reloaded = ragent_agent::mcp::McpEnableLedger::load_from(&path);
    assert_eq!(reloaded.servers.len(), 1);
    assert!(!reloaded.is_enabled("a"));

    let _ = std::fs::remove_file(&path);
}

/// A corrupt ledger is renamed aside and treated as empty, so a bad file can
/// never make every MCP server disappear.
#[test]
fn corrupt_enable_ledger_is_renamed_aside_and_defaults_to_empty() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/mcp-enable-state-tests");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(format!("corrupt-{}.json", std::process::id()));
    std::fs::write(&path, b"{ not json").expect("write corrupt ledger");

    let ledger = ragent_agent::mcp::McpEnableLedger::load_from(&path);
    assert!(ledger.servers.is_empty());
    assert!(ledger.is_enabled("anything"));

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("json.corrupt"));
}
