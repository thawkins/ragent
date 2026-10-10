//! Integration tests for the ACP client (spec `openhands` T-007; FR-015,
//! FR-022, FR-036).
//!
//! These tests drive a real subprocess over the ACP JSON-RPC stdio transport.
//! The agent is a small **Python** script (Python is present on the CI image),
//! so no external ACP product is required. They cover:
//!
//! - FR-015: a turn is relayed over JSON-RPC on the subprocess's stdin/stdout,
//!   and the streamed `session/update` notifications are rendered in order.
//! - FR-022: an `AgentInfo` bound to a registered ACP agent resolves to that
//!   agent (and a plain agent does not), so the processor would not route the
//!   turn to a local provider.
//! - FR-036: a non-zero subprocess exit fails the turn with the exit code, a
//!   malformed stdout frame fails the turn without hanging, and a silent
//!   subprocess is failed by the turn timeout.

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use ragent_agent::acp::{AcpError, AcpErrorKind, AcpUpdate, parse_update};
use ragent_agent::agent::AgentInfo;
use ragent_agent::session::acp_dispatch::acp_agent_for;
use ragent_config::{AcpAgentConfig, AcpConfig, Config};
use serde_json::json;

/// A config for a single ACP agent running `script` with `args`.
fn acp_config(id: &str, script: &str, args: &[&str]) -> AcpAgentConfig {
    AcpAgentConfig {
        id: id.to_string(),
        name: None,
        enabled: true,
        command: "python3".to_string(),
        args: std::iter::once(script.to_string())
            .chain(args.iter().map(|s| (*s).to_string()))
            .collect(),
        env: std::collections::HashMap::new(),
        cwd: None,
        turn_timeout_secs: 10,
        system_prompt: None,
    }
}

/// A config whose `command` is `command` (used to exercise spawn failure).
fn acp_config_command(id: &str, command: &str) -> AcpAgentConfig {
    AcpAgentConfig {
        id: id.to_string(),
        command: command.to_string(),
        ..AcpAgentConfig::default()
    }
}

/// Write `body` to a scratch Python file and return its path.
fn write_script(name: &str, body: &str) -> std::path::PathBuf {
    let dir = Path::new("target/temp/acp-tests");
    std::fs::create_dir_all(dir).expect("create scratch dir");
    let path = dir.join(name);
    std::fs::write(&path, body).expect("write script");
    path
}

const ECHO_AGENT: &str = r#"
import sys, json

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

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
        send({"jsonrpc": "2.0", "method": "session/update", "params": {
            "sessionId": "s-1",
            "update": {"sessionUpdate": "agent_message_chunk",
                       "content": {"type": "text", "text": "Hello from ACP"}}}})
        send({"jsonrpc": "2.0", "id": rid, "result": {"stopReason": "end_turn"}})
"#;

#[tokio::test]
async fn relays_turn_and_renders_streamed_updates() {
    // FR-015, FR-022: the turn is relayed over JSON-RPC on stdio and each
    // session/update is delivered to the render callback.
    let script = write_script("echo_agent.py", ECHO_AGENT);
    let agent = acp_config("echo", script.to_str().unwrap(), &[]);
    let cancel = AtomicBool::new(false);
    let mut rendered: Vec<AcpUpdate> = Vec::new();
    let mut on_update = |update: AcpUpdate| rendered.push(update);

    let outcome =
        ragent_agent::acp::relay_turn(&agent, Path::new("."), "hi", &cancel, &mut on_update)
            .await
            .expect("the echo agent must complete the turn");

    assert_eq!(outcome.stop_reason, "end_turn");
    assert_eq!(outcome.text, "Hello from ACP");
    assert_eq!(rendered.len(), 1);
    assert_eq!(
        rendered[0],
        AcpUpdate::AgentMessageChunk {
            text: "Hello from ACP".to_string()
        }
    );
}

#[tokio::test]
async fn non_zero_exit_fails_the_turn_with_the_exit_code() {
    // FR-036: a non-zero subprocess exit fails the turn and reports the code;
    // it does not hang waiting for further output.
    let script = write_script(
        "exit_agent.py",
        "import sys\nsys.stderr.write('boom\\n')\nsys.stderr.flush()\nsys.exit(7)\n",
    );
    let agent = acp_config("exit", script.to_str().unwrap(), &[]);
    let cancel = AtomicBool::new(false);
    let mut on_update = |_update: AcpUpdate| {};

    let err = ragent_agent::acp::relay_turn(&agent, Path::new("."), "hi", &cancel, &mut on_update)
        .await
        .expect_err("a non-zero exit must fail the turn");

    let acp_err = err;
    assert_eq!(acp_err.kind(), AcpErrorKind::Exited);
    assert_eq!(acp_err.exit_code(), Some(7));
    assert_eq!(acp_err.agent(), "exit");
}

#[tokio::test]
async fn malformed_frame_fails_the_turn_without_hanging() {
    // FR-036: a malformed JSON-RPC frame fails the turn rather than blocking.
    let script = write_script(
        "malformed_agent.py",
        "import sys, json\nfor line in sys.stdin:\n    if not line.strip():\n        continue\n    req = json.loads(line)\n    rid = req.get('id')\n    sys.stdout.write('this is not json\\n')\n    sys.stdout.flush()\n",
    );
    let agent = acp_config("malformed", script.to_str().unwrap(), &[]);
    let cancel = AtomicBool::new(false);
    let mut on_update = |_update: AcpUpdate| {};

    let err = ragent_agent::acp::relay_turn(&agent, Path::new("."), "hi", &cancel, &mut on_update)
        .await
        .expect_err("a malformed frame must fail the turn");

    assert_eq!(err.kind(), AcpErrorKind::MalformedFrame);
    assert!(err.message().contains("malformed"));
}

#[tokio::test]
async fn missing_command_fails_with_spawn_error() {
    // FR-036: a command that cannot be spawned fails fast with a clear error.
    let agent = acp_config_command("missing", "/nonexistent/acp-agent-does-not-exist");
    let cancel = AtomicBool::new(false);
    let mut on_update = |_update: AcpUpdate| {};

    let err =
        ragent_agent::acp::relay_turn(&agent, Path::new("."), "hello", &cancel, &mut on_update)
            .await
            .expect_err("a missing command must fail the turn");

    assert_eq!(err.kind(), AcpErrorKind::Spawn);
}

#[tokio::test]
async fn silent_subprocess_is_failed_by_the_turn_timeout() {
    // FR-036: a subprocess that reads input and never replies must be failed by
    // the turn timeout rather than blocking indefinitely.
    let script = write_script("silent_agent.py", "import sys\nsys.stdin.read()\n");
    let mut agent = acp_config("silent", script.to_str().unwrap(), &[]);
    // Keep the test quick while exercising the real timeout path.
    agent.turn_timeout_secs = 1;
    let cancel = AtomicBool::new(false);
    let mut on_update = |_update: AcpUpdate| {};

    // Bound the whole call so a regression that hangs cannot stall the suite.
    let started = std::time::Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(30),
        ragent_agent::acp::relay_turn(&agent, Path::new("."), "hi", &cancel, &mut on_update),
    )
    .await
    .expect("the relay must return within the outer bound, never hang");

    let err = result.expect_err("a silent subprocess must be failed by the timeout");
    assert_eq!(err.kind(), AcpErrorKind::Timeout);
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "the turn timeout must fire promptly"
    );
}

#[tokio::test]
async fn prompt_without_reply_is_failed_by_the_turn_timeout() {
    // FR-036: an agent that completes the handshake but never answers
    // session/prompt must be failed by the per-turn timeout, not awaited
    // forever.
    let script = write_script(
        "stall_prompt_agent.py",
        r#"
import sys, json
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    method = req.get("method")
    rid = req.get("id")
    if method == "initialize":
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": {"protocolVersion": 1}}) + "\n")
    elif method == "session/new":
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": {"sessionId": "s-1"}}) + "\n")
    # session/prompt is deliberately ignored: no reply, no exit.
    sys.stdout.flush()
"#,
    );
    let mut agent = acp_config("stall", script.to_str().unwrap(), &[]);
    agent.turn_timeout_secs = 1;
    let cancel = AtomicBool::new(false);
    let mut on_update = |_update: AcpUpdate| {};

    let started = std::time::Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(30),
        ragent_agent::acp::relay_turn(&agent, Path::new("."), "hi", &cancel, &mut on_update),
    )
    .await
    .expect("the relay must return within the outer bound, never hang");

    let err = result.expect_err("a stalled prompt must be failed by the turn timeout");
    assert_eq!(err.kind(), AcpErrorKind::Timeout);
    assert!(started.elapsed() < Duration::from_secs(30));
}

#[tokio::test]
async fn session_prompt_error_reply_fails_the_turn() {
    // FR-036: a JSON-RPC error reply to session/prompt fails the turn with the
    // protocol cause and the agent's message.
    let script = write_script(
        "error_agent.py",
        r#"
import sys, json
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    method = req.get("method")
    rid = req.get("id")
    if method == "initialize":
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": {"protocolVersion": 1}}) + "\n")
    elif method == "session/new":
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": {"sessionId": "s-1"}}) + "\n")
    elif method == "session/prompt":
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": rid,
            "error": {"code": -32000, "message": "out of tokens"}}) + "\n")
    sys.stdout.flush()
"#,
    );
    let agent = acp_config("error", script.to_str().unwrap(), &[]);
    let cancel = AtomicBool::new(false);
    let mut on_update = |_update: AcpUpdate| {};

    let err = ragent_agent::acp::relay_turn(&agent, Path::new("."), "hi", &cancel, &mut on_update)
        .await
        .expect_err("a JSON-RPC error must fail the turn");

    assert_eq!(err.kind(), AcpErrorKind::Protocol);
    assert!(err.message().contains("out of tokens"));
}

#[test]
fn agent_binding_requires_explicit_membership() {
    // FR-022: an agent bound by `options.acp` (or by sharing a registered ACP
    // id) resolves to that agent; a plain agent resolves to none, so it is never
    // silently rerouted off the local provider path.
    let mut config = Config::default();
    config.acp = Some(AcpConfig {
        default_agent: None,
        server_enabled: false,
        server_agent: None,
        agents: std::collections::HashMap::from([(
            "claude".to_string(),
            AcpAgentConfig {
                id: "claude".to_string(),
                command: "claude-code-acp".to_string(),
                ..AcpAgentConfig::default()
            },
        )]),
    });

    let mut bound = AgentInfo::new("wrapper", "bound via options");
    bound.options = std::sync::Arc::new(std::collections::HashMap::from([(
        "acp".to_string(),
        json!("claude"),
    )]));
    assert_eq!(
        acp_agent_for(&config, &bound).map(|a| a.id.as_str()),
        Some("claude")
    );

    let by_name = AgentInfo::new("claude", "same id as the ACP agent");
    assert_eq!(
        acp_agent_for(&config, &by_name).map(|a| a.id.as_str()),
        Some("claude")
    );

    let plain = AgentInfo::new("coder", "ordinary local agent");
    assert!(acp_agent_for(&config, &plain).is_none());
}

#[test]
fn update_frames_decode_into_typed_updates() {
    // FR-015: the notification payloads an ACP agent streams decode into the
    // typed update model the TUI renders.
    let message = json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {
            "sessionId": "s-1",
            "update": {
                "sessionUpdate": "agent_message_chunk",
                "content": {"type": "text", "text": "chunk"}
            }
        }
    });
    assert_eq!(
        parse_update(&message),
        AcpUpdate::AgentMessageChunk {
            text: "chunk".to_string()
        }
    );

    let tool = json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {
            "sessionId": "s-1",
            "update": {
                "sessionUpdate": "tool_call",
                "toolCallId": "call-1",
                "title": "Read file",
                "status": "in_progress"
            }
        }
    });
    assert_eq!(
        parse_update(&tool),
        AcpUpdate::ToolCall {
            tool_call_id: "call-1".to_string(),
            title: "Read file".to_string(),
            status: "in_progress".to_string(),
        }
    );

    let unknown = json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": { "update": { "sessionUpdate": "future_kind" } }
    });
    assert_eq!(
        parse_update(&unknown),
        AcpUpdate::Other {
            kind: "future_kind".to_string()
        }
    );
}

#[test]
fn acp_error_is_a_std_error() {
    // The structured error must travel inside anyhow and stay downcastable.
    let error = AcpError::new("agent", AcpErrorKind::Protocol, "boom");
    let anyhow_error: anyhow::Error = error.into();
    let recovered = anyhow_error
        .downcast_ref::<AcpError>()
        .expect("AcpError must downcast");
    assert_eq!(recovered.kind(), AcpErrorKind::Protocol);
    assert_eq!(recovered.agent(), "agent");
}
