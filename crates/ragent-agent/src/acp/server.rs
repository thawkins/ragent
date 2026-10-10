//! Agent Client Protocol (ACP) **server** endpoint on stdio (spec `openhands`
//! FR-022, FR-028).
//!
//! This is the mirror image of the [`AcpClient`](super::AcpClient): instead of
//! ragent driving an external agent, ragent **serves** an ACP-capable editor
//! (Zed, VS Code, JetBrains). The editor speaks JSON-RPC 2.0 over ragent's
//! standard input and output; ragent maps each editor session onto a local
//! ragent session and streams that session's agent-loop events back as ACP
//! `session/update` notifications (FR-022).
//!
//! The endpoint is **opt-in**: it is compiled only behind the `acp-server` Cargo
//! feature and, even then, only runs when `acp.server_enabled` is `true` in the
//! configuration. A plain terminal launch therefore never exposes an
//! editor-facing JSON-RPC surface (FR-028).
//!
//! Handled requests (all line-delimited JSON-RPC frames):
//!
//! - `initialize` - negotiates [`ACP_PROTOCOL_VERSION`](super::ACP_PROTOCOL_VERSION).
//! - `session/new` - opens a local ragent session rooted at the editor's `cwd`.
//! - `session/prompt` - runs one ragent turn and streams updates; the reply
//!   carries the mapped `stopReason`.
//! - `session/cancel` - raises the turn's cancellation flag.
//!
//! A malformed request frame is logged and skipped rather than killing the
//! endpoint, so one bad frame cannot take the editor connection down. An
//! unknown request method is answered with a JSON-RPC "method not found".

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::agent::AgentInfo;
use crate::event::{Event, EventBus, FinishReason};
use crate::session::processor::SessionProcessor;

/// A local ragent session bound to one editor ACP session.
struct SessionEntry {
    /// The local ragent session id driven by this editor session.
    local_session_id: String,
    /// Cancellation flag watched by the in-flight turn.
    cancel: Arc<AtomicBool>,
}

/// Serve the ACP endpoint on ragent's real stdin/stdout.
///
/// # Errors
///
/// Returns an error only when the stdio transport itself fails (a read error on
/// stdin or a write error on stdout). A protocol-level failure for one request
/// is reported to the editor in a JSON-RPC error frame and does not tear the
/// endpoint down.
pub async fn serve_stdio(
    processor: Arc<SessionProcessor>,
    event_bus: Arc<EventBus>,
    agent: Arc<AgentInfo>,
    working_dir: PathBuf,
) -> Result<()> {
    serve(
        processor,
        event_bus,
        agent,
        working_dir,
        tokio::io::stdin(),
        tokio::io::stdout(),
    )
    .await
}

/// Serve the ACP endpoint over an arbitrary reader/writer pair.
///
/// Split out from [`serve_stdio`] so the transport can be driven by in-memory
/// pipes in tests. The reader is read line-by-line (one JSON-RPC frame per
/// line); each emitted frame is written and flushed as a single line.
///
/// # Errors
///
/// Returns an error when reading the request stream fails. Individual
/// requests that cannot be serviced are answered with a JSON-RPC error frame
/// instead.
pub async fn serve<R, W>(
    processor: Arc<SessionProcessor>,
    event_bus: Arc<EventBus>,
    agent: Arc<AgentInfo>,
    working_dir: PathBuf,
    reader: R,
    writer: W,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
    W: tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    // Serialise all outgoing frames through one writer task so a streamed
    // update and a request reply can never interleave mid-line.
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let writer_task = tokio::spawn(async move {
        let mut out = writer;
        while let Some(line) = rx.recv().await {
            if out.write_all(line.as_bytes()).await.is_err() {
                break;
            }
            if out.write_all(b"\n").await.is_err() {
                break;
            }
            if out.flush().await.is_err() {
                break;
            }
        }
        let _ = out.flush().await; // INTENTIONAL: best-effort final flush; the peer may already be gone
    });

    let mut sessions: HashMap<String, SessionEntry> = HashMap::new();
    let mut active_turn: Option<tokio::task::JoinHandle<()>> = None;
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(frame) = serde_json::from_str::<Value>(&line) else {
            // One malformed frame must not take the editor connection down.
            warn!("ignoring malformed ACP request frame on stdin");
            continue;
        };

        let method = frame
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let id = frame.get("id").cloned();

        match method.as_str() {
            "initialize" => {
                if let Some(id) = id {
                    let _ = tx.send(initialize_reply(&id).to_string());
                }
            }
            "session/new" => {
                let cwd = frame
                    .pointer("/params/cwd")
                    .and_then(Value::as_str)
                    .map(PathBuf::from);
                let dir = cwd
                    .filter(|p| p.is_absolute() && p.is_dir())
                    .unwrap_or_else(|| working_dir.clone());
                match processor.session_manager.create_session(dir) {
                    Ok(session) => {
                        let acp_id = uuid::Uuid::new_v4().to_string();
                        sessions.insert(
                            acp_id.clone(),
                            SessionEntry {
                                local_session_id: session.id.clone(),
                                cancel: Arc::new(AtomicBool::new(false)),
                            },
                        );
                        if let Some(id) = id {
                            let _ = tx.send(reply(&id, json!({ "sessionId": acp_id })).to_string());
                        }
                    }
                    Err(e) => {
                        if let Some(id) = id {
                            let _ = tx.send(
                                error_reply(&id, -32603, &format!("session creation failed: {e}"))
                                    .to_string(),
                            );
                        }
                    }
                }
            }
            "session/prompt" => {
                let acp_id = frame
                    .pointer("/params/sessionId")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let Some(entry) = sessions.get(&acp_id) else {
                    if let Some(id) = id {
                        let _ =
                            tx.send(error_reply(&id, -32602, "unknown ACP session id").to_string());
                    }
                    continue;
                };

                // Serialise turns: reject a second prompt while one is running
                // rather than interleaving two agent loops on one server.
                if let Some(handle) = active_turn.take() {
                    if handle.is_finished() {
                        let _ = handle.await;
                    } else {
                        active_turn = Some(handle);
                        if let Some(id) = id {
                            let _ = tx.send(
                                error_reply(&id, -32000, "a turn is already in progress")
                                    .to_string(),
                            );
                        }
                        continue;
                    }
                }

                let prompt = extract_prompt(&frame);
                if prompt.is_empty() {
                    if let Some(id) = id {
                        let _ = tx.send(
                            error_reply(&id, -32602, "prompt carried no text content").to_string(),
                        );
                    }
                    continue;
                }

                let local_session_id = entry.local_session_id.clone();
                let cancel = Arc::clone(&entry.cancel);
                cancel.store(false, Ordering::SeqCst);

                // Subscribe *before* the turn starts so the first streamed
                // update is not missed.
                let events = event_bus.subscribe();
                active_turn = Some(tokio::spawn(run_prompt(
                    Arc::clone(&processor),
                    Arc::clone(&agent),
                    local_session_id,
                    acp_id,
                    id,
                    prompt,
                    cancel,
                    events,
                    tx.clone(),
                )));
            }
            "session/cancel" => {
                if let Some(acp_id) = frame.pointer("/params/sessionId").and_then(Value::as_str)
                    && let Some(entry) = sessions.get(acp_id)
                {
                    entry.cancel.store(true, Ordering::SeqCst);
                }
                if let Some(id) = id {
                    let _ = tx.send(reply(&id, json!({})).to_string());
                }
            }
            other => {
                debug!(method = other, "unhandled ACP server request");
                if let Some(id) = id {
                    let _ = tx
                        .send(error_reply(&id, -32601, "server method not supported").to_string());
                }
            }
        }
    }

    // stdin closed: let an in-flight turn finish so its updates and reply are
    // flushed before the endpoint returns.
    if let Some(handle) = active_turn {
        let _ = handle.await;
    }
    drop(tx);
    let _ = writer_task.await;
    Ok(())
}

/// Run one ragent turn and stream its events to the editor as ACP updates.
///
/// The bus subscription is created by the caller before the turn starts so no
/// early update is lost. When the turn completes, the editor receives a
/// `session/prompt` reply carrying the mapped `stopReason`; a turn failure
/// becomes a JSON-RPC error reply.
#[allow(clippy::too_many_arguments)]
async fn run_prompt(
    processor: Arc<SessionProcessor>,
    agent: Arc<AgentInfo>,
    local_session_id: String,
    acp_session_id: String,
    request_id: Option<Value>,
    prompt: String,
    cancel: Arc<AtomicBool>,
    mut events: tokio::sync::broadcast::Receiver<Event>,
    tx: mpsc::UnboundedSender<String>,
) {
    let turn = {
        let processor = Arc::clone(&processor);
        let agent = Arc::clone(&agent);
        let local = local_session_id.clone();
        tokio::spawn(async move {
            processor
                .process_message(&local, &prompt, &agent, cancel)
                .await
        })
    };
    let mut turn = turn;
    let mut outcome = None;

    loop {
        tokio::select! {
            biased;
            joined = &mut turn => {
                outcome = Some(match joined {
                    Ok(result) => result,
                    Err(e) => Err(anyhow::anyhow!("turn task failed: {e}")),
                });
                break;
            }
            event = events.recv() => {
                match event {
                    Ok(event) => {
                        if event.session_id() == Some(local_session_id.as_str())
                            && let Some(update) = event_to_update(&event)
                        {
                            let frame = update_frame(&acp_session_id, update);
                            if tx.send(frame.to_string()).is_err() {
                                break;
                            }
                        }
                    }
                    Err(RecvError::Lagged(_)) => {
                        // A slow reader dropped some updates; keep streaming the
                        // rest rather than failing the turn.
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        }
    }

    // Drain updates that were queued after the turn's final event resolved the
    // join, so the trailing assistant chunk is not dropped.
    while let Ok(event) = events.try_recv() {
        if event.session_id() == Some(local_session_id.as_str())
            && let Some(update) = event_to_update(&event)
        {
            let _ = tx.send(update_frame(&acp_session_id, update).to_string());
        }
    }

    let Some(request_id) = request_id else {
        return;
    };
    match outcome {
        Some(Ok(_)) => {
            let reason = processor
                .last_message_end_reason(&local_session_id)
                .unwrap_or(FinishReason::Stop);
            let _ = tx.send(
                reply(&request_id, json!({ "stopReason": stop_reason(&reason) })).to_string(),
            );
        }
        Some(Err(e)) => {
            let _ = tx.send(error_reply(&request_id, -32603, &e.to_string()).to_string());
        }
        None => {}
    }
}

/// Build an `initialize` reply advertising the negotiated protocol version.
fn initialize_reply(id: &Value) -> Value {
    reply(
        id,
        json!({
            "protocolVersion": super::ACP_PROTOCOL_VERSION,
            "agentCapabilities": {
                "loadSession": false,
                "promptCapabilities": {
                    "image": false,
                    "audio": false,
                    "embeddedContext": true,
                },
            },
        }),
    )
}

/// Build a JSON-RPC success reply for `id`.
fn reply(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// Build a JSON-RPC error reply for `id`.
fn error_reply(id: &Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

/// Build a `session/update` notification carrying one ACP update.
fn update_frame(session_id: &str, update: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": { "sessionId": session_id, "update": update },
    })
}

/// Concatenate the text of every `{ "type": "text", "text": ... }` block in an
/// ACP `session/prompt` request.
fn extract_prompt(frame: &Value) -> String {
    frame
        .pointer("/params/prompt")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

/// Map a ragent [`FinishReason`] onto an ACP `stopReason` string.
fn stop_reason(reason: &FinishReason) -> &'static str {
    match reason {
        FinishReason::Stop | FinishReason::ToolUse => "end_turn",
        FinishReason::Length | FinishReason::Truncation => "max_tokens",
        FinishReason::ContentFilter => "refusal",
        FinishReason::Cancelled => "cancelled",
    }
}

/// Translate one ragent session event into an ACP `session/update` payload.
///
/// Returns `None` for events that have no ACP representation.
fn event_to_update(event: &Event) -> Option<Value> {
    match event {
        Event::TextDelta { text, .. } => Some(json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "text", "text": text },
        })),
        Event::ReasoningDelta { text, .. } => Some(json!({
            "sessionUpdate": "agent_thought_chunk",
            "content": { "type": "text", "text": text },
        })),
        Event::ToolCallStart { call_id, tool, .. } => Some(json!({
            "sessionUpdate": "tool_call",
            "toolCallId": call_id,
            "title": tool,
            "status": "in_progress",
        })),
        Event::ToolCallEnd {
            call_id,
            tool,
            error,
            ..
        } => Some(json!({
            "sessionUpdate": "tool_call_update",
            "toolCallId": call_id,
            "title": tool,
            "status": if error.is_some() { "failed" } else { "completed" },
        })),
        Event::AgentError { error, .. } => Some(json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "text", "text": format!("error: {error}") },
        })),
        _ => None,
    }
}
