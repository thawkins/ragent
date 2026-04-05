//! SSE event serialization.
//!
//! Converts [`ragent_core::event::Event`] variants into Axum SSE events with
//! typed event names and JSON payloads for streaming to HTTP clients.
//!
//! Payloads are serialized directly from borrowed typed structs via
//! [`serde_json::to_string`], avoiding intermediate [`serde_json::Value`]
//! allocations that the `json!` macro would create.

use axum::response::sse::Event as SseEvent;
use ragent_core::event::{Event, FinishReason};
use ragent_core::sanitize::redact_secrets;
use serde::Serialize;

// ── Payload structs ──────────────────────────────────────────────────────

#[derive(Serialize)]
struct SessionOnly<'a> {
    session_id: &'a str,
}

#[derive(Serialize)]
struct SessionMsg<'a> {
    session_id: &'a str,
    message_id: &'a str,
}

#[derive(Serialize)]
struct SessionText<'a> {
    session_id: &'a str,
    text: &'a str,
}

#[derive(Serialize)]
struct ToolCallStartP<'a> {
    session_id: &'a str,
    call_id: &'a str,
    tool: &'a str,
}

#[derive(Serialize)]
struct ToolCallEndP<'a> {
    session_id: &'a str,
    call_id: &'a str,
    tool: &'a str,
    error: Option<&'a str>,
    duration_ms: u64,
}

#[derive(Serialize)]
struct MessageEndP<'a> {
    session_id: &'a str,
    message_id: &'a str,
    reason: &'a FinishReason,
}

#[derive(Serialize)]
struct PermReqP<'a> {
    session_id: &'a str,
    request_id: &'a str,
    permission: &'a str,
    description: &'a str,
}

#[derive(Serialize)]
struct PermRepP<'a> {
    session_id: &'a str,
    request_id: &'a str,
    allowed: bool,
}

#[derive(Serialize)]
struct AgentSwitchedP<'a> {
    session_id: &'a str,
    from: &'a str,
    to: &'a str,
}

#[derive(Serialize)]
struct AgentSwitchReqP<'a> {
    session_id: &'a str,
    to: &'a str,
    task: &'a str,
    context: &'a str,
}

#[derive(Serialize)]
struct SessionSummary<'a> {
    session_id: &'a str,
    summary: &'a str,
}

#[derive(Serialize)]
struct SessionError<'a> {
    session_id: &'a str,
    error: &'a str,
}

#[derive(Serialize)]
struct ServerStatus<'a> {
    server_id: &'a str,
    status: &'a str,
}

#[derive(Serialize)]
struct TokenUsageP<'a> {
    session_id: &'a str,
    input_tokens: u64,
    output_tokens: u64,
}

#[derive(Serialize)]
struct ToolsSentP<'a> {
    session_id: &'a str,
    tools: &'a [String],
}

#[derive(Serialize)]
struct ModelResponseP<'a> {
    session_id: &'a str,
    text: std::borrow::Cow<'a, str>,
    elapsed_ms: u64,
    input_tokens: u64,
    output_tokens: u64,
}

#[derive(Serialize)]
struct ToolCallArgsP<'a> {
    session_id: &'a str,
    call_id: &'a str,
    tool: &'a str,
    args: &'a str,
}

#[derive(Serialize)]
struct ToolResultP<'a> {
    session_id: &'a str,
    call_id: &'a str,
    tool: &'a str,
    content: std::borrow::Cow<'a, str>,
    content_line_count: usize,
    metadata: &'a Option<serde_json::Value>,
    success: bool,
}

#[derive(Serialize)]
struct CopilotFlowP<'a> {
    token_present: bool,
    api_base: &'a str,
}

#[derive(Serialize)]
struct SessionReasonP<'a> {
    session_id: &'a str,
    reason: &'a str,
}

#[derive(Serialize)]
struct QuotaP<'a> {
    session_id: &'a str,
    percent: f32,
}

#[derive(Serialize)]
struct SubagentStartP<'a> {
    session_id: &'a str,
    task_id: &'a str,
    child_session_id: &'a str,
    agent: &'a str,
    task: &'a str,
    background: bool,
}

#[derive(Serialize)]
struct SubagentCompleteP<'a> {
    session_id: &'a str,
    task_id: &'a str,
    child_session_id: &'a str,
    summary: &'a str,
    success: bool,
    duration_ms: u64,
}

#[derive(Serialize)]
struct SessionTaskP<'a> {
    session_id: &'a str,
    task_id: &'a str,
}

#[derive(Serialize)]
struct TeammateSpawnedP<'a> {
    session_id: &'a str,
    team_name: &'a str,
    teammate_name: &'a str,
    agent_id: &'a str,
}

#[derive(Serialize)]
struct TeammateMessageP<'a> {
    session_id: &'a str,
    team_name: &'a str,
    from: &'a str,
    to: &'a str,
    preview: &'a str,
}

#[derive(Serialize)]
struct TeamAgentP<'a> {
    session_id: &'a str,
    team_name: &'a str,
    agent_id: &'a str,
}

#[derive(Serialize)]
struct TeamAgentErrorP<'a> {
    session_id: &'a str,
    team_name: &'a str,
    agent_id: &'a str,
    error: &'a str,
}

#[derive(Serialize)]
struct TeamTaskP<'a> {
    session_id: &'a str,
    team_name: &'a str,
    agent_id: &'a str,
    task_id: &'a str,
}

#[derive(Serialize)]
struct TeamNameP<'a> {
    session_id: &'a str,
    team_name: &'a str,
}

// ── Helpers ──────────────────────────────────────────────────────────────

/// Serialize a payload directly to a JSON string, bypassing `serde_json::Value`.
fn to_data<T: Serialize>(payload: &T) -> String {
    serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string())
}

/// Return the SSE event type name for an [`Event`] variant.
const fn event_type_name(event: &Event) -> &'static str {
    match event {
        Event::SessionCreated { .. } => "session_created",
        Event::SessionUpdated { .. } => "session_updated",
        Event::MessageStart { .. } => "message_start",
        Event::TextDelta { .. } => "text_delta",
        Event::ReasoningDelta { .. } => "reasoning_delta",
        Event::ToolCallStart { .. } => "tool_call_start",
        Event::ToolCallEnd { .. } => "tool_call_end",
        Event::MessageEnd { .. } => "message_end",
        Event::PermissionRequested { .. } => "permission_requested",
        Event::PermissionReplied { .. } => "permission_replied",
        Event::AgentSwitched { .. } => "agent_switched",
        Event::AgentSwitchRequested { .. } => "agent_switch_requested",
        Event::AgentRestoreRequested { .. } => "agent_restore_requested",
        Event::AgentError { .. } => "agent_error",
        Event::McpStatusChanged { .. } => "mcp_status_changed",
        Event::TokenUsage { .. } => "token_usage",
        Event::ToolsSent { .. } => "tools_sent",
        Event::ModelResponse { .. } => "model_response",
        Event::ToolCallArgs { .. } => "tool_call_args",
        Event::ToolResult { .. } => "tool_result",
        Event::CopilotDeviceFlowComplete { .. } => "copilot_device_flow_complete",
        Event::SessionAborted { .. } => "session_aborted",
        Event::QuotaUpdate { .. } => "quota_update",
        Event::SubagentStart { .. } => "subagent_start",
        Event::SubagentComplete { .. } => "subagent_complete",
        Event::SubagentCancelled { .. } => "subagent_cancelled",
        Event::TeammateSpawned { .. } => "teammate_spawned",
        Event::TeammateMessage { .. } => "teammate_message",
        Event::TeammateIdle { .. } => "teammate_idle",
        Event::TeammateFailed { .. } => "teammate_failed",
        Event::TeamTaskClaimed { .. } => "team_task_claimed",
        Event::TeamTaskCompleted { .. } => "team_task_completed",
        Event::TeamCleanedUp { .. } => "team_cleaned_up",
        Event::TeammateP2PMessage { .. } => "teammate_p2p_message",
        Event::LspStatusChanged { .. } => "lsp_status_changed",
        Event::TaskCompleted { .. } => "task_completed",
        Event::ShellCwdChanged { .. } => "shell_cwd_changed",
    }
}

// ── Public API ───────────────────────────────────────────────────────────

/// Return the SSE event-type name and serialized JSON payload for an [`Event`].
///
/// This is the testable core of [`event_to_sse`]. The returned tuple is
/// `(event_name, json_data_string)`.
#[must_use]
pub fn event_to_parts(event: &Event) -> (&'static str, String) {
    let name = event_type_name(event);

    let data = match event {
        Event::SessionCreated { session_id } | Event::SessionUpdated { session_id } => {
            to_data(&SessionOnly { session_id })
        }

        Event::MessageStart {
            session_id,
            message_id,
        } => to_data(&SessionMsg {
            session_id,
            message_id,
        }),

        Event::TextDelta { session_id, text } | Event::ReasoningDelta { session_id, text } => {
            to_data(&SessionText { session_id, text })
        }

        Event::ToolCallStart {
            session_id,
            call_id,
            tool,
        } => to_data(&ToolCallStartP {
            session_id,
            call_id,
            tool,
        }),

        Event::ToolCallEnd {
            session_id,
            call_id,
            tool,
            error,
            duration_ms,
        } => to_data(&ToolCallEndP {
            session_id,
            call_id,
            tool,
            error: error.as_deref(),
            duration_ms: *duration_ms,
        }),

        Event::MessageEnd {
            session_id,
            message_id,
            reason,
        } => to_data(&MessageEndP {
            session_id,
            message_id,
            reason,
        }),

        Event::PermissionRequested {
            session_id,
            request_id,
            permission,
            description,
        } => to_data(&PermReqP {
            session_id,
            request_id,
            permission,
            description,
        }),

        Event::PermissionReplied {
            session_id,
            request_id,
            allowed,
        } => to_data(&PermRepP {
            session_id,
            request_id,
            allowed: *allowed,
        }),

        Event::AgentSwitched {
            session_id,
            from,
            to,
        } => to_data(&AgentSwitchedP {
            session_id,
            from,
            to,
        }),

        Event::AgentSwitchRequested {
            session_id,
            to,
            task,
            context,
        } => to_data(&AgentSwitchReqP {
            session_id,
            to,
            task,
            context,
        }),

        Event::AgentRestoreRequested {
            session_id,
            summary,
        } => to_data(&SessionSummary {
            session_id,
            summary,
        }),

        Event::AgentError { session_id, error } => to_data(&SessionError { session_id, error }),

        Event::McpStatusChanged { server_id, status } => {
            to_data(&ServerStatus { server_id, status })
        }

        Event::TokenUsage {
            session_id,
            input_tokens,
            output_tokens,
        } => to_data(&TokenUsageP {
            session_id,
            input_tokens: *input_tokens,
            output_tokens: *output_tokens,
        }),

        Event::ToolsSent { session_id, tools } => to_data(&ToolsSentP { session_id, tools }),

        Event::ModelResponse {
            session_id,
            text,
            elapsed_ms,
            input_tokens,
            output_tokens,
        } => {
            let redacted = redact_secrets(text);
            to_data(&ModelResponseP {
                session_id,
                text: if redacted == *text {
                    std::borrow::Cow::Borrowed(text)
                } else {
                    std::borrow::Cow::Owned(redacted)
                },
                elapsed_ms: *elapsed_ms,
                input_tokens: *input_tokens,
                output_tokens: *output_tokens,
            })
        }

        Event::ToolCallArgs {
            session_id,
            call_id,
            tool,
            args,
        } => to_data(&ToolCallArgsP {
            session_id,
            call_id,
            tool,
            args,
        }),

        Event::ToolResult {
            session_id,
            call_id,
            tool,
            content,
            content_line_count,
            metadata,
            success,
        } => {
            let redacted = redact_secrets(content);
            to_data(&ToolResultP {
                session_id,
                call_id,
                tool,
                content: if redacted == *content {
                    std::borrow::Cow::Borrowed(content)
                } else {
                    std::borrow::Cow::Owned(redacted)
                },
                content_line_count: *content_line_count,
                metadata,
                success: *success,
            })
        }

        Event::CopilotDeviceFlowComplete { token, api_base } => to_data(&CopilotFlowP {
            token_present: !token.is_empty(),
            api_base,
        }),

        Event::SessionAborted { session_id, reason } => {
            to_data(&SessionReasonP { session_id, reason })
        }

        Event::QuotaUpdate {
            session_id,
            percent,
        } => to_data(&QuotaP {
            session_id,
            percent: *percent,
        }),

        Event::SubagentStart {
            session_id,
            task_id,
            child_session_id,
            agent,
            task,
            background,
        } => to_data(&SubagentStartP {
            session_id,
            task_id,
            child_session_id,
            agent,
            task,
            background: *background,
        }),

        Event::SubagentComplete {
            session_id,
            task_id,
            child_session_id,
            summary,
            success,
            duration_ms,
        } => to_data(&SubagentCompleteP {
            session_id,
            task_id,
            child_session_id,
            summary,
            success: *success,
            duration_ms: *duration_ms,
        }),

        Event::SubagentCancelled {
            session_id,
            task_id,
        } => to_data(&SessionTaskP {
            session_id,
            task_id,
        }),

        Event::TeammateSpawned {
            session_id,
            team_name,
            teammate_name,
            agent_id,
        } => to_data(&TeammateSpawnedP {
            session_id,
            team_name,
            teammate_name,
            agent_id,
        }),

        Event::TeammateMessage {
            session_id,
            team_name,
            from,
            to,
            preview,
        } => to_data(&TeammateMessageP {
            session_id,
            team_name,
            from,
            to,
            preview,
        }),

        Event::TeammateIdle {
            session_id,
            team_name,
            agent_id,
        } => to_data(&TeamAgentP {
            session_id,
            team_name,
            agent_id,
        }),

        Event::TeammateFailed {
            session_id,
            team_name,
            agent_id,
            error,
        } => to_data(&TeamAgentErrorP {
            session_id,
            team_name,
            agent_id,
            error,
        }),

        Event::TeamTaskClaimed {
            session_id,
            team_name,
            agent_id,
            task_id,
        }
        | Event::TeamTaskCompleted {
            session_id,
            team_name,
            agent_id,
            task_id,
        } => to_data(&TeamTaskP {
            session_id,
            team_name,
            agent_id,
            task_id,
        }),

        Event::TeamCleanedUp {
            session_id,
            team_name,
        } => to_data(&TeamNameP {
            session_id,
            team_name,
        }),

        Event::TeammateP2PMessage {
            session_id,
            team_name,
            from,
            to,
            preview,
        } => to_data(&TeammateMessageP {
            session_id,
            team_name,
            from,
            to,
            preview,
        }),

        Event::LspStatusChanged { server_id, status } => {
            let status_str = format!("{status:?}");
            to_data(&ServerStatus {
                server_id,
                status: &status_str,
            })
        }

        Event::TaskCompleted {
            session_id,
            summary,
        } => to_data(&serde_json::json!({
            "session_id": session_id,
            "summary": summary,
        })),

        Event::ShellCwdChanged { session_id, cwd } => to_data(&serde_json::json!({
            "session_id": session_id,
            "cwd": cwd,
        })),
    };

    (name, data)
}

/// Convert a `ragent_core` [`Event`] into an Axum [`SseEvent`].
///
/// Payloads are serialized directly from typed structs — no intermediate
/// `serde_json::Value` is allocated.
///
/// # Examples
///
/// ```rust
/// use ragent_core::event::Event;
/// use ragent_server::sse::event_to_sse;
///
/// let event = Event::SessionCreated {
///     session_id: "abc-123".to_string(),
/// };
/// let sse_event = event_to_sse(&event);
/// ```
pub fn event_to_sse(event: &Event) -> SseEvent {
    let (name, data) = event_to_parts(event);
    SseEvent::default().event(name).data(data)
}
