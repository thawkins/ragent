//! SSE event serialization.
//!
//! Converts [`ragent_core::event::Event`] variants into Axum SSE events with
//! typed event names and JSON payloads for streaming to HTTP clients.

use axum::response::sse::Event as SseEvent;
use ragent_core::event::Event;

/// Convert a `ragent_core` [`Event`] into an Axum [`SseEvent`].
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
    let (event_type, data) = match event {
        Event::SessionCreated { session_id } => (
            "session_created",
            serde_json::json!({ "session_id": session_id }),
        ),
        Event::SessionUpdated { session_id } => (
            "session_updated",
            serde_json::json!({ "session_id": session_id }),
        ),
        Event::MessageStart {
            session_id,
            message_id,
        } => (
            "message_start",
            serde_json::json!({ "session_id": session_id, "message_id": message_id }),
        ),
        Event::TextDelta { session_id, text } => (
            "text_delta",
            serde_json::json!({ "session_id": session_id, "text": text }),
        ),
        Event::ReasoningDelta { session_id, text } => (
            "reasoning_delta",
            serde_json::json!({ "session_id": session_id, "text": text }),
        ),
        Event::ToolCallStart {
            session_id,
            call_id,
            tool,
        } => (
            "tool_call_start",
            serde_json::json!({
                "session_id": session_id,
                "call_id": call_id,
                "tool": tool,
            }),
        ),
        Event::ToolCallEnd {
            session_id,
            call_id,
            tool,
            error,
            duration_ms,
        } => (
            "tool_call_end",
            serde_json::json!({
                "session_id": session_id,
                "call_id": call_id,
                "tool": tool,
                "error": error,
                "duration_ms": duration_ms,
            }),
        ),
        Event::MessageEnd {
            session_id,
            message_id,
            reason,
        } => (
            "message_end",
            serde_json::json!({
                "session_id": session_id,
                "message_id": message_id,
                "reason": reason,
            }),
        ),
        Event::PermissionRequested {
            session_id,
            request_id,
            permission,
            description,
        } => (
            "permission_requested",
            serde_json::json!({
                "session_id": session_id,
                "request_id": request_id,
                "permission": permission,
                "description": description,
            }),
        ),
        Event::PermissionReplied {
            session_id,
            request_id,
            allowed,
        } => (
            "permission_replied",
            serde_json::json!({
                "session_id": session_id,
                "request_id": request_id,
                "allowed": allowed,
            }),
        ),
        Event::AgentSwitched {
            session_id,
            from,
            to,
        } => (
            "agent_switched",
            serde_json::json!({
                "session_id": session_id,
                "from": from,
                "to": to,
            }),
        ),
        Event::AgentSwitchRequested {
            session_id,
            to,
            task,
            context,
        } => (
            "agent_switch_requested",
            serde_json::json!({
                "session_id": session_id,
                "to": to,
                "task": task,
                "context": context,
            }),
        ),
        Event::AgentRestoreRequested {
            session_id,
            summary,
        } => (
            "agent_restore_requested",
            serde_json::json!({
                "session_id": session_id,
                "summary": summary,
            }),
        ),
        Event::AgentError { session_id, error } => (
            "agent_error",
            serde_json::json!({ "session_id": session_id, "error": error }),
        ),
        Event::McpStatusChanged { server_id, status } => (
            "mcp_status_changed",
            serde_json::json!({ "server_id": server_id, "status": status }),
        ),
        Event::TokenUsage {
            session_id,
            input_tokens,
            output_tokens,
        } => (
            "token_usage",
            serde_json::json!({
                "session_id": session_id,
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
            }),
        ),
        Event::ToolsSent { session_id, tools } => (
            "tools_sent",
            serde_json::json!({
                "session_id": session_id,
                "tools": tools,
            }),
        ),
        Event::ModelResponse { session_id, text, elapsed_ms } => (
            "model_response",
            serde_json::json!({
                "session_id": session_id,
                "text": text,
                "elapsed_ms": elapsed_ms,
            }),
        ),
        Event::ToolCallArgs {
            session_id,
            call_id,
            tool,
            args,
        } => (
            "tool_call_args",
            serde_json::json!({
                "session_id": session_id,
                "call_id": call_id,
                "tool": tool,
                "args": args,
            }),
        ),
        Event::ToolResult {
            session_id,
            call_id,
            tool,
            content,
            content_line_count,
            metadata,
            success,
        } => (
            "tool_result",
            serde_json::json!({
                "session_id": session_id,
                "call_id": call_id,
                "tool": tool,
                "content": content,
                "content_line_count": content_line_count,
                "metadata": metadata,
                "success": success,
            }),
        ),
        Event::CopilotDeviceFlowComplete { token, api_base } => (
            "copilot_device_flow_complete",
            serde_json::json!({
                "token_present": !token.is_empty(),
                "api_base": api_base,
            }),
        ),
        Event::SessionAborted { session_id, reason } => (
            "session_aborted",
            serde_json::json!({
                "session_id": session_id,
                "reason": reason,
            }),
        ),
        Event::QuotaUpdate { session_id, percent } => (
            "quota_update",
            serde_json::json!({
                "session_id": session_id,
                "percent": percent,
            }),
        ),
        Event::SubagentStart {
            session_id,
            task_id,
            child_session_id,
            agent,
            task,
            background,
        } => (
            "subagent_start",
            serde_json::json!({
                "session_id": session_id,
                "task_id": task_id,
                "child_session_id": child_session_id,
                "agent": agent,
                "task": task,
                "background": background,
            }),
        ),
        Event::SubagentComplete {
            session_id,
            task_id,
            child_session_id,
            summary,
            success,
            duration_ms,
        } => (
            "subagent_complete",
            serde_json::json!({
                "session_id": session_id,
                "task_id": task_id,
                "child_session_id": child_session_id,
                "summary": summary,
                "success": success,
                "duration_ms": duration_ms,
            }),
        ),
        Event::SubagentCancelled {
            session_id,
            task_id,
        } => (
            "subagent_cancelled",
            serde_json::json!({
                "session_id": session_id,
                "task_id": task_id,
            }),
        ),
        Event::LspStatusChanged { server_id, status } => (
            "lsp_status_changed",
            serde_json::json!({
                "server_id": server_id,
                "status": format!("{:?}", status),
            }),
        ),
        Event::TeammateSpawned { session_id, team_name, teammate_name, agent_id } => (
            "teammate_spawned",
            serde_json::json!({
                "session_id": session_id,
                "team_name": team_name,
                "teammate_name": teammate_name,
                "agent_id": agent_id,
            }),
        ),
        Event::TeammateMessage { session_id, team_name, from, to, preview } => (
            "teammate_message",
            serde_json::json!({
                "session_id": session_id,
                "team_name": team_name,
                "from": from,
                "to": to,
                "preview": preview,
            }),
        ),
        Event::TeammateIdle { session_id, team_name, agent_id } => (
            "teammate_idle",
            serde_json::json!({
                "session_id": session_id,
                "team_name": team_name,
                "agent_id": agent_id,
            }),
        ),
        Event::TeamTaskClaimed { session_id, team_name, agent_id, task_id } => (
            "team_task_claimed",
            serde_json::json!({
                "session_id": session_id,
                "team_name": team_name,
                "agent_id": agent_id,
                "task_id": task_id,
            }),
        ),
        Event::TeamTaskCompleted { session_id, team_name, agent_id, task_id } => (
            "team_task_completed",
            serde_json::json!({
                "session_id": session_id,
                "team_name": team_name,
                "agent_id": agent_id,
                "task_id": task_id,
            }),
        ),
        Event::TeamCleanedUp { session_id, team_name } => (
            "team_cleaned_up",
            serde_json::json!({
                "session_id": session_id,
                "team_name": team_name,
            }),
        ),
    };

    SseEvent::default().event(event_type).data(data.to_string())
}
