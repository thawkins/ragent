//! SSE event serialization.
//!
//! Converts [`ragent_core::event::Event`] variants into Axum SSE events with
//! typed event names and JSON payloads for streaming to HTTP clients.

use axum::response::sse::Event as SseEvent;
use ragent_core::event::Event;

/// Convert a ragent_core [`Event`] into an Axum [`SseEvent`].
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
        Event::ModelResponse { session_id, text } => (
            "model_response",
            serde_json::json!({
                "session_id": session_id,
                "text": text,
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
            success,
        } => (
            "tool_result",
            serde_json::json!({
                "session_id": session_id,
                "call_id": call_id,
                "tool": tool,
                "content": content,
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
        Event::SessionAborted {
            session_id,
            reason,
        } => (
            "session_aborted",
            serde_json::json!({
                "session_id": session_id,
                "reason": reason,
            }),
        ),
    };

    SseEvent::default().event(event_type).data(data.to_string())
}
