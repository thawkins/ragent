//! Integration tests for the ACP **server** endpoint on stdio (spec `openhands`
//! T-008; FR-022, FR-028).
//!
//! The whole file is compiled only when the `acp-server` Cargo feature is on:
//! the endpoint is feature-gated and, per FR-028, off by default. Run these with
//! `cargo test -p ragent-agent --features acp-server`.
//!
//! The tests drive the real JSON-RPC transport over an in-memory duplex pipe
//! (an "editor" on one end, the ACP server on the other) and use a mock LLM
//! provider so a `session/prompt` round-trip streams real agent-loop events back
//! as ACP `session/update` notifications without touching a network provider.

#![cfg(feature = "acp-server")]

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures::stream;
use ragent_agent::acp::server::serve;
use ragent_agent::agent::{AgentInfo, ModelRef};
use ragent_agent::event::EventBus;
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::{SessionManager, processor::SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::{AcpAgentConfig, AcpConfig, Capabilities, Config, Cost};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};

// -- Mock provider --------------------------------------------------------

/// A provider whose single model returns a fixed streamed reply.
struct ServerMockProvider {
    reply: String,
}

struct ServerMockClient {
    reply: String,
}

#[async_trait::async_trait]
impl LlmClient for ServerMockClient {
    async fn chat(
        &self,
        _request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        Ok(Box::pin(stream::iter(vec![
            StreamEvent::TextDelta {
                text: self.reply.clone(),
            },
            StreamEvent::Finish {
                reason: LlmFinishReason::Stop,
            },
        ])))
    }
}

#[async_trait::async_trait]
impl Provider for ServerMockProvider {
    fn id(&self) -> &'static str {
        "ollama"
    }

    fn name(&self) -> &'static str {
        "ACP Server Mock Ollama"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "qwen3:latest".to_string(),
            provider_id: "ollama".to_string(),
            name: "Qwen3".to_string(),
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
            context_window: 10_000,
            max_output: Some(8_192),
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
        _options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        Ok(Box::new(ServerMockClient {
            reply: self.reply.clone(),
        }))
    }
}

// -- Harness --------------------------------------------------------------

/// Build a `SessionProcessor` over an in-memory store with the mock provider.
fn test_processor(mock_reply: &str) -> Arc<SessionProcessor> {
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(ServerMockProvider {
        reply: mock_reply.to_string(),
    }));

    let event_bus = Arc::new(EventBus::new(64));
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    Arc::new(SessionProcessor {
        session_manager,
        provider_registry: Arc::new(provider_registry),
        tool_registry: Arc::new(tool::create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![]))),
        event_bus,
        agent_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        connector_session: tokio::sync::RwLock::new(None),
        connector_statuses: tokio::sync::RwLock::new(None),
        code_index: std::sync::OnceLock::new(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
        cached_config: parking_lot::Mutex::new(Some(
            ragent_agent::session::processor::CachedConfig {
                config: Arc::new(Config::default()),
                file_mtimes: Vec::new(),
                env_overrides_present: false,
            },
        )),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
            std::collections::HashMap::new(),
        )),
        tool_repeat_guard: std::sync::Arc::new(parking_lot::Mutex::new(
            std::collections::HashMap::new(),
        )),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::StreamConfig::default(),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        telemetry: std::sync::Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        bg_service: std::sync::OnceLock::new(),
        activity_log: std::sync::OnceLock::new(),
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(std::collections::HashMap::new()),
    })
}

/// Spawn the ACP server over an in-memory duplex pipe and return the editor-side
/// read lines and write half, plus the server join handle.
type EditorRead = Lines<BufReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>>;
type EditorWrite = tokio::io::WriteHalf<tokio::io::DuplexStream>;

fn spawn_server(
    processor: Arc<SessionProcessor>,
) -> (EditorRead, EditorWrite, tokio::task::JoinHandle<Result<()>>) {
    let (server_io, editor_io) = tokio::io::duplex(64 * 1024);
    let (srv_rd, srv_wr) = tokio::io::split(server_io);
    let (cli_rd, cli_wr) = tokio::io::split(editor_io);

    // The server must observe the *same* bus the processor publishes on, so
    // share the processor's bus rather than constructing a fresh one.
    let bus = Arc::clone(&processor.event_bus);
    let mut agent = AgentInfo::new("general", "General");
    // The ACP server drives a full local turn, so the agent needs a model that
    // the mock provider (registered as `ollama`) supplies.
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    });
    let agent = Arc::new(agent);
    let dir = std::env::temp_dir();
    let handle = tokio::spawn(serve(processor, bus, agent, dir, srv_rd, srv_wr));

    (BufReader::new(cli_rd).lines(), cli_wr, handle)
}

/// Write one JSON-RPC frame followed by a newline.
async fn send(writer: &mut EditorWrite, frame: Value) {
    writer
        .write_all(frame.to_string().as_bytes())
        .await
        .expect("write frame");
    writer.write_all(b"\n").await.expect("write newline");
    writer.flush().await.expect("flush");
}

/// Shut the editor-side write half down so the server observes end-of-input
/// and its read loop can exit, then drop it.
async fn close(writer: &mut EditorWrite) {
    let _ = writer.shutdown().await;
}

/// Read one JSON-RPC frame, failing the test on timeout or invalid JSON.
async fn recv(lines: &mut EditorRead) -> Value {
    let line = tokio::time::timeout(Duration::from_secs(15), lines.next_line())
        .await
        .expect("timed out waiting for an ACP frame")
        .expect("read line")
        .expect("stream ended before a frame arrived");
    serde_json::from_str(&line).expect("frame should be valid JSON")
}

// -- Config-level tests ---------------------------------------------------

#[test]
fn acp_server_is_disabled_by_default() {
    // FR-028: the endpoint is off unless explicitly enabled.
    let config = Config::default();
    assert!(!config.acp_server_enabled());
    assert_eq!(config.acp_server_agent(), "general");

    let mut config = Config::default();
    config.acp = Some(AcpConfig::default());
    assert!(!config.acp_server_enabled());
}

#[test]
fn acp_server_enable_flag_and_agent_precedence() {
    // FR-022, FR-028: the flag opts in, and the driven agent resolves
    // server_agent -> default_agent -> top-level default.
    let mut config = Config::default();
    config.default_agent = "coder".to_string();
    config.acp = Some(AcpConfig {
        default_agent: Some("claude".to_string()),
        agents: HashMap::<String, AcpAgentConfig>::new(),
        server_enabled: true,
        server_agent: None,
    });
    assert!(config.acp_server_enabled());
    assert_eq!(config.acp_server_agent(), "claude");

    config.acp.as_mut().unwrap().server_agent = Some("codex".to_string());
    assert_eq!(config.acp_server_agent(), "codex");
}

#[test]
fn acp_server_config_round_trips_and_overlay_merges() {
    // The new keys survive serialization and a project overlay can enable the
    // endpoint without clobbering base agents.
    let base: Config = serde_json::from_str(
        r#"{ "acp": { "agents": { "alpha": { "id": "alpha", "command": "a" } } } }"#,
    )
    .expect("parse base");
    assert!(!base.acp_server_enabled());

    let overlay: Config =
        serde_json::from_str(r#"{ "acp": { "server_enabled": true, "server_agent": "beta" } }"#)
            .expect("parse overlay");
    let merged = Config::merge(base, overlay);
    assert!(merged.acp_server_enabled());
    assert_eq!(merged.acp_server_agent(), "beta");
    assert!(merged.acp.as_ref().unwrap().agents.contains_key("alpha"));
}

// -- Transport tests ------------------------------------------------------

#[tokio::test]
async fn initialize_negotiates_protocol_version() {
    let processor = test_processor("hi");
    let (mut rx, mut tx, handle) = spawn_server(processor);

    send(
        &mut tx,
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize"}),
    )
    .await;
    let reply = recv(&mut rx).await;
    assert_eq!(reply["id"].as_u64(), Some(1));
    assert_eq!(reply["result"]["protocolVersion"].as_u64(), Some(1));

    close(&mut tx).await;
    handle.await.expect("server task").expect("server ok");
}

#[tokio::test]
async fn session_new_returns_a_session_id() {
    let processor = test_processor("hi");
    let (mut rx, mut tx, handle) = spawn_server(processor);

    send(
        &mut tx,
        json!({"jsonrpc": "2.0", "id": 1, "method": "session/new",
               "params": {"cwd": "/tmp", "mcpServers": []}}),
    )
    .await;
    let reply = recv(&mut rx).await;
    let session_id = reply["result"]["sessionId"].as_str().expect("sessionId");
    assert_ne!(session_id, "");

    close(&mut tx).await;
    handle.await.expect("server task").expect("server ok");
}

#[tokio::test]
async fn unknown_method_returns_method_not_found() {
    let processor = test_processor("hi");
    let (mut rx, mut tx, handle) = spawn_server(processor);

    send(
        &mut tx,
        json!({"jsonrpc": "2.0", "id": 7, "method": "bogus/method"}),
    )
    .await;
    let reply = recv(&mut rx).await;
    assert_eq!(reply["id"].as_u64(), Some(7));
    assert_eq!(reply["error"]["code"].as_i64(), Some(-32601));

    close(&mut tx).await;
    handle.await.expect("server task").expect("server ok");
}

#[tokio::test]
async fn malformed_frame_does_not_tear_down_the_endpoint() {
    // FR-036 mirrored on the server side: one bad frame is skipped, the next
    // valid request is still served.
    let processor = test_processor("hi");
    let (mut rx, mut tx, handle) = spawn_server(processor);

    tx.write_all(b"this is not json\n")
        .await
        .expect("write junk");
    tx.flush().await.expect("flush");
    send(
        &mut tx,
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize"}),
    )
    .await;

    let reply = recv(&mut rx).await;
    assert_eq!(reply["id"].as_u64(), Some(1));
    assert_eq!(reply["result"]["protocolVersion"].as_u64(), Some(1));

    close(&mut tx).await;
    handle.await.expect("server task").expect("server ok");
}

#[tokio::test]
async fn prompt_streams_updates_and_reports_a_stop_reason() {
    // FR-022: a `session/prompt` runs a real ragent turn and each streamed
    // event is rendered as an ACP `session/update`; the reply carries the
    // mapped `stopReason`.
    let processor = test_processor("Hello from the ACP server");
    let (mut rx, mut tx, handle) = spawn_server(processor);

    send(
        &mut tx,
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize"}),
    )
    .await;
    let _ = recv(&mut rx).await;

    send(
        &mut tx,
        json!({"jsonrpc": "2.0", "id": 2, "method": "session/new",
               "params": {"cwd": "/tmp", "mcpServers": []}}),
    )
    .await;
    let new_reply = recv(&mut rx).await;
    let session_id = new_reply["result"]["sessionId"]
        .as_str()
        .expect("sessionId")
        .to_string();

    send(
        &mut tx,
        json!({"jsonrpc": "2.0", "id": 3, "method": "session/prompt",
               "params": {"sessionId": session_id,
                          "prompt": [{"type": "text", "text": "say hello"}]}}),
    )
    .await;

    // Collect streamed updates until the prompt reply (the frame carrying id 3).
    let mut streamed_text = String::new();
    let mut saw_update = false;
    let mut reply = None;
    for _ in 0..64 {
        let frame = recv(&mut rx).await;
        if frame.get("id").and_then(Value::as_u64) == Some(3) {
            reply = Some(frame);
            break;
        }
        if frame.get("method").and_then(Value::as_str) == Some("session/update") {
            saw_update = true;
            if frame["params"]["update"]["sessionUpdate"] == "agent_message_chunk"
                && let Some(text) = frame["params"]["update"]["content"]["text"].as_str()
            {
                streamed_text.push_str(text);
            }
        }
    }

    assert!(
        saw_update,
        "expected at least one session/update notification"
    );
    assert!(
        streamed_text.contains("Hello from the ACP server"),
        "streamed text should carry the mock reply, got: {streamed_text:?}"
    );
    let reply = reply.expect("prompt reply");
    assert_eq!(reply["result"]["stopReason"], "end_turn");

    close(&mut tx).await;
    handle.await.expect("server task").expect("server ok");
}

#[tokio::test]
async fn prompt_for_unknown_session_returns_an_error() {
    let processor = test_processor("hi");
    let (mut rx, mut tx, handle) = spawn_server(processor);

    send(
        &mut tx,
        json!({"jsonrpc": "2.0", "id": 1, "method": "session/prompt",
               "params": {"sessionId": "does-not-exist",
                          "prompt": [{"type": "text", "text": "hi"}]}}),
    )
    .await;
    let reply = recv(&mut rx).await;
    assert_eq!(reply["id"].as_u64(), Some(1));
    assert_eq!(reply["error"]["code"].as_i64(), Some(-32602));

    close(&mut tx).await;
    handle.await.expect("server task").expect("server ok");
}
