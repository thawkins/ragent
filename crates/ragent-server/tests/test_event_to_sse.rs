//! Unit tests for [`ragent_server::sse::event_to_parts`].
//!
//! Every [`Event`] variant is tested for:
//! 1. Correct SSE event-type name.
//! 2. Valid JSON payload with expected fields.
//! 3. No panics during serialization.

use ragent_agent as ragent_core;
use ragent_core::event::{Event, FinishReason};
use ragent_core::lsp::LspStatus;
use ragent_server::sse::event_to_parts;
use serde_json::Value;

/// Parse the JSON data string from `event_to_parts`.
fn parse_data(event: &Event) -> (String, Value) {
    let (name, json) = event_to_parts(event);
    let val: Value = serde_json::from_str(&json).expect("payload must be valid JSON");
    (name.to_string(), val)
}

// ── Session lifecycle ────────────────────────────────────────────────────

#[test]
fn test_session_created() {
    let (name, v) = parse_data(&Event::SessionCreated {
        session_id: "s1".into(),
    });
    assert_eq!(name, "session_created");
    assert_eq!(v["session_id"], "s1");
}

#[test]
fn test_session_updated() {
    let (name, v) = parse_data(&Event::SessionUpdated {
        session_id: "s2".into(),
    });
    assert_eq!(name, "session_updated");
    assert_eq!(v["session_id"], "s2");
}

// ── Message events ───────────────────────────────────────────────────────

#[test]
fn test_message_start() {
    let (name, v) = parse_data(&Event::MessageStart {
        session_id: "s1".into(),
        message_id: "m1".into(),
    });
    assert_eq!(name, "message_start");
    assert_eq!(v["session_id"], "s1");
    assert_eq!(v["message_id"], "m1");
}

#[test]
fn test_text_delta() {
    let (name, v) = parse_data(&Event::TextDelta {
        session_id: "s1".into(),
        text: "hello".into(),
    });
    assert_eq!(name, "text_delta");
    assert_eq!(v["session_id"], "s1");
    assert_eq!(v["text"], "hello");
}

#[test]
fn test_reasoning_delta() {
    let (name, v) = parse_data(&Event::ReasoningDelta {
        session_id: "s1".into(),
        text: "thinking...".into(),
    });
    assert_eq!(name, "reasoning_delta");
    assert_eq!(v["text"], "thinking...");
}

#[test]
fn test_message_end() {
    let (name, v) = parse_data(&Event::MessageEnd {
        session_id: "s1".into(),
        message_id: "m1".into(),
        reason: FinishReason::Stop,
    });
    assert_eq!(name, "message_end");
    assert_eq!(v["session_id"], "s1");
    assert_eq!(v["message_id"], "m1");
    assert_eq!(v["reason"], "stop");
}

#[test]
fn test_model_response() {
    let (name, v) = parse_data(&Event::ModelResponse {
        session_id: "s1".into(),
        text: "response text".into(),
        elapsed_ms: 500,
        input_tokens: 42,
        output_tokens: 24,
    });
    assert_eq!(name, "model_response");
    assert_eq!(v["elapsed_ms"], 500);
    assert_eq!(v["input_tokens"], 42);
    assert_eq!(v["output_tokens"], 24);
}

// ── Tool events ──────────────────────────────────────────────────────────

#[test]
fn test_tool_call_start() {
    let (name, v) = parse_data(&Event::ToolCallStart {
        session_id: "s1".into(),
        call_id: "c1".into(),
        tool: "read_file".into(),
    });
    assert_eq!(name, "tool_call_start");
    assert_eq!(v["tool"], "read_file");
    assert_eq!(v["call_id"], "c1");
}

#[test]
fn test_tool_call_end_success() {
    let (name, v) = parse_data(&Event::ToolCallEnd {
        session_id: "s1".into(),
        call_id: "c1".into(),
        tool: "read_file".into(),
        error: None,
        duration_ms: 42,
    });
    assert_eq!(name, "tool_call_end");
    assert!(v["error"].is_null());
    assert_eq!(v["duration_ms"], 42);
}

#[test]
fn test_tool_call_end_error() {
    let (name, v) = parse_data(&Event::ToolCallEnd {
        session_id: "s1".into(),
        call_id: "c1".into(),
        tool: "write_file".into(),
        error: Some("permission denied".into()),
        duration_ms: 10,
    });
    assert_eq!(name, "tool_call_end");
    assert_eq!(v["error"], "permission denied");
}

#[test]
fn test_tool_call_args() {
    let (name, v) = parse_data(&Event::ToolCallArgs {
        session_id: "s1".into(),
        call_id: "c1".into(),
        tool: "bash".into(),
        args: r#"{"cmd":"ls"}"#.into(),
    });
    assert_eq!(name, "tool_call_args");
    assert_eq!(v["args"], r#"{"cmd":"ls"}"#);
}

#[test]
fn test_tool_result() {
    let (name, v) = parse_data(&Event::ToolResult {
        session_id: "s1".into(),
        call_id: "c1".into(),
        tool: "read_file".into(),
        content: "file contents".into(),
        content_line_count: 5,
        metadata: Some(serde_json::json!({"lines_read": 5})),
        success: true,
    });
    assert_eq!(name, "tool_result");
    assert_eq!(v["content"], "file contents");
    assert_eq!(v["content_line_count"], 5);
    assert_eq!(v["success"], true);
    assert_eq!(v["metadata"]["lines_read"], 5);
}

#[test]
fn test_tool_result_no_metadata() {
    let (_, v) = parse_data(&Event::ToolResult {
        session_id: "s1".into(),
        call_id: "c1".into(),
        tool: "bash".into(),
        content: "ok".into(),
        content_line_count: 1,
        metadata: None,
        success: false,
    });
    assert!(v["metadata"].is_null());
    assert_eq!(v["success"], false);
}

#[test]
fn test_tools_sent() {
    let (name, v) = parse_data(&Event::ToolsSent {
        session_id: "s1".into(),
        tools: vec!["read_file".into(), "bash".into()],
    });
    assert_eq!(name, "tools_sent");
    let arr = v["tools"].as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0], "read_file");
}

// ── Permission events ────────────────────────────────────────────────────

#[test]
fn test_permission_requested() {
    let (name, v) = parse_data(&Event::PermissionRequested {
        session_id: "s1".into(),
        request_id: "r1".into(),
        permission: "file:write".into(),
        description: "Write to /tmp/out".into(),
        options: vec![],
    });
    assert_eq!(name, "permission_requested");
    assert_eq!(v["permission"], "file:write");
    assert_eq!(v["request_id"], "r1");
}

#[test]
fn test_permission_replied() {
    let (name, v) = parse_data(&Event::PermissionReplied {
        session_id: "s1".into(),
        request_id: "r1".into(),
        allowed: true,
        decision: ragent_core::permission::PermissionDecision::Once,
    });
    assert_eq!(name, "permission_replied");
    assert_eq!(v["allowed"], true);
}

#[test]
fn test_question_requested() {
    let (name, v) = parse_data(&Event::QuestionRequested {
        session_id: "s1".into(),
        request_id: "q1".into(),
        question: "Pick one".into(),
        options: vec!["a".into(), "b".into()],
    });
    assert_eq!(name, "question_requested");
    assert_eq!(v["question"], "Pick one");
    assert_eq!(v["options"][0], "a");
}

#[test]
fn test_question_answered() {
    let (name, v) = parse_data(&Event::QuestionAnswered {
        session_id: "s1".into(),
        request_id: "q1".into(),
        response: "a".into(),
    });
    assert_eq!(name, "question_answered");
    assert_eq!(v["request_id"], "q1");
    assert_eq!(v["response"], "a");
}

// ── Agent events ─────────────────────────────────────────────────────────

#[test]
fn test_agent_switched() {
    let (name, v) = parse_data(&Event::AgentSwitched {
        session_id: "s1".into(),
        from: "general".into(),
        to: "code-review".into(),
    });
    assert_eq!(name, "agent_switched");
    assert_eq!(v["from"], "general");
    assert_eq!(v["to"], "code-review");
}

#[test]
fn test_agent_switch_requested() {
    let (name, v) = parse_data(&Event::AgentSwitchRequested {
        session_id: "s1".into(),
        to: "explore".into(),
        task: "find files".into(),
        context: "extra".into(),
    });
    assert_eq!(name, "agent_switch_requested");
    assert_eq!(v["to"], "explore");
    assert_eq!(v["task"], "find files");
}

#[test]
fn test_agent_restore_requested() {
    let (name, v) = parse_data(&Event::AgentRestoreRequested {
        session_id: "s1".into(),
        summary: "done reviewing".into(),
    });
    assert_eq!(name, "agent_restore_requested");
    assert_eq!(v["summary"], "done reviewing");
}

#[test]
fn test_agent_error() {
    let (name, v) = parse_data(&Event::AgentError {
        session_id: "s1".into(),
        error: "model timeout".into(),
    });
    assert_eq!(name, "agent_error");
    assert_eq!(v["error"], "model timeout");
}

// ── Infrastructure events (no session_id) ────────────────────────────────

#[test]
fn test_mcp_status_changed() {
    let (name, v) = parse_data(&Event::McpStatusChanged {
        server_id: "mcp-1".into(),
        status: "connected".into(),
    });
    assert_eq!(name, "mcp_status_changed");
    assert_eq!(v["server_id"], "mcp-1");
    assert_eq!(v["status"], "connected");
    assert!(v.get("session_id").is_none());
}

#[test]
fn test_copilot_device_flow_complete_with_token() {
    let (name, v) = parse_data(&Event::CopilotDeviceFlowComplete {
        token: "ghp_secret123".into(),
        api_base: "https://api.github.com".into(),
    });
    assert_eq!(name, "copilot_device_flow_complete");
    // Token must be redacted to a boolean
    assert_eq!(v["token_present"], true);
    assert!(
        v.get("token").is_none(),
        "raw token must not appear in SSE payload"
    );
    assert_eq!(v["api_base"], "https://api.github.com");
}

#[test]
fn test_copilot_device_flow_complete_empty_token() {
    let (_, v) = parse_data(&Event::CopilotDeviceFlowComplete {
        token: String::new(),
        api_base: "https://api.github.com".into(),
    });
    assert_eq!(v["token_present"], false);
}

#[test]
fn test_lsp_status_changed() {
    let (name, v) = parse_data(&Event::LspStatusChanged {
        server_id: "rust".into(),
        status: LspStatus::Connected,
    });
    assert_eq!(name, "lsp_status_changed");
    assert_eq!(v["server_id"], "rust");
    // Status uses Debug format
    assert_eq!(v["status"], "Connected");
}

#[test]
fn test_lsp_status_failed() {
    let (_, v) = parse_data(&Event::LspStatusChanged {
        server_id: "ts".into(),
        status: LspStatus::Failed {
            error: "crash".into(),
        },
    });
    // Debug format includes variant + fields
    let status = v["status"].as_str().unwrap();
    assert!(
        status.contains("Failed"),
        "expected Debug format, got: {status}"
    );
}

// ── Quota / usage events ─────────────────────────────────────────────────

#[test]
fn test_token_usage() {
    let (name, v) = parse_data(&Event::TokenUsage {
        session_id: "s1".into(),
        input_tokens: 100,
        output_tokens: 200,
    });
    assert_eq!(name, "token_usage");
    assert_eq!(v["input_tokens"], 100);
    assert_eq!(v["output_tokens"], 200);
}

#[test]
fn test_quota_update() {
    let (name, v) = parse_data(&Event::QuotaUpdate {
        session_id: "s1".into(),
        percent: 42.5,
    });
    assert_eq!(name, "quota_update");
    let pct = v["percent"].as_f64().unwrap();
    assert!((pct - 42.5).abs() < 0.01);
}

#[test]
fn test_session_aborted() {
    let (name, v) = parse_data(&Event::SessionAborted {
        session_id: "s1".into(),
        reason: "user_requested".into(),
    });
    assert_eq!(name, "session_aborted");
    assert_eq!(v["reason"], "user_requested");
}

// ── Sub-agent events ─────────────────────────────────────────────────────

#[test]
fn test_subagent_start() {
    let (name, v) = parse_data(&Event::SubagentStart {
        session_id: "s1".into(),
        task_id: "t1".into(),
        child_session_id: "child-s1".into(),
        agent: "explore".into(),
        task: "find usages".into(),
        background: true,
    });
    assert_eq!(name, "subagent_start");
    assert_eq!(v["task_id"], "t1");
    assert_eq!(v["child_session_id"], "child-s1");
    assert_eq!(v["background"], true);
}

#[test]
fn test_subagent_complete() {
    let (name, v) = parse_data(&Event::SubagentComplete {
        session_id: "s1".into(),
        task_id: "t1".into(),
        child_session_id: "child-s1".into(),
        summary: "Found 3 usages".into(),
        success: true,
        duration_ms: 1500,
    });
    assert_eq!(name, "subagent_complete");
    assert_eq!(v["success"], true);
    assert_eq!(v["duration_ms"], 1500);
}

#[test]
fn test_subagent_cancelled() {
    let (name, v) = parse_data(&Event::SubagentCancelled {
        session_id: "s1".into(),
        task_id: "t1".into(),
    });
    assert_eq!(name, "subagent_cancelled");
    assert_eq!(v["task_id"], "t1");
}

// ── Team events ──────────────────────────────────────────────────────────

#[test]
fn test_teammate_spawned() {
    let (name, v) = parse_data(&Event::TeammateSpawned {
        session_id: "s1".into(),
        team_name: "code-review".into(),
        teammate_name: "reviewer".into(),
        agent_id: "tm-001".into(),
    });
    assert_eq!(name, "teammate_spawned");
    assert_eq!(v["team_name"], "code-review");
    assert_eq!(v["teammate_name"], "reviewer");
    assert_eq!(v["agent_id"], "tm-001");
}

#[test]
fn test_teammate_message() {
    let (name, v) = parse_data(&Event::TeammateMessage {
        session_id: "s1".into(),
        team_name: "code-review".into(),
        from: "tm-001".into(),
        to: "lead".into(),
        preview: "Looks good to me".into(),
    });
    assert_eq!(name, "teammate_message");
    assert_eq!(v["from"], "tm-001");
    assert_eq!(v["to"], "lead");
    assert_eq!(v["preview"], "Looks good to me");
}

#[test]
fn test_teammate_idle() {
    let (name, v) = parse_data(&Event::TeammateIdle {
        session_id: "s1".into(),
        team_name: "code-review".into(),
        agent_id: "tm-001".into(),
    });
    assert_eq!(name, "teammate_idle");
    assert_eq!(v["agent_id"], "tm-001");
}

#[test]
fn test_team_task_claimed() {
    let (name, v) = parse_data(&Event::TeamTaskClaimed {
        session_id: "s1".into(),
        team_name: "code-review".into(),
        agent_id: "tm-001".into(),
        task_id: "task-001".into(),
    });
    assert_eq!(name, "team_task_claimed");
    assert_eq!(v["task_id"], "task-001");
}

#[test]
fn test_team_task_completed() {
    let (name, v) = parse_data(&Event::TeamTaskCompleted {
        session_id: "s1".into(),
        team_name: "code-review".into(),
        agent_id: "tm-001".into(),
        task_id: "task-001".into(),
    });
    assert_eq!(name, "team_task_completed");
    assert_eq!(v["task_id"], "task-001");
}

#[test]
fn test_team_cleaned_up() {
    let (name, v) = parse_data(&Event::TeamCleanedUp {
        session_id: "s1".into(),
        team_name: "code-review".into(),
    });
    assert_eq!(name, "team_cleaned_up");
    assert_eq!(v["team_name"], "code-review");
}

// ── Secret redaction tests ───────────────────────────────────────────────

#[test]
fn test_tool_result_redacts_api_key() {
    let secret = "sk-abcdefghijklmnopqrstuvwxyz1234567890";
    let (_, v) = parse_data(&Event::ToolResult {
        session_id: "s1".into(),
        call_id: "c1".into(),
        tool: "bash".into(),
        content: format!("Result: {secret}"),
        content_line_count: 1,
        metadata: None,
        success: true,
    });
    let content = v["content"].as_str().unwrap();
    assert!(
        !content.contains(secret),
        "API key must be redacted: {content}"
    );
    assert!(content.contains("[REDACTED]"));
}

#[test]
fn test_tool_result_redacts_bearer_token() {
    let token = "Bearer abcdefghijklmnopqrstuvwxyz1234567890";
    let (_, v) = parse_data(&Event::ToolResult {
        session_id: "s1".into(),
        call_id: "c1".into(),
        tool: "bash".into(),
        content: format!("Auth: {token}"),
        content_line_count: 1,
        metadata: None,
        success: true,
    });
    let content = v["content"].as_str().unwrap();
    assert!(
        !content.contains("abcdefghijklmnopqrstuvwxyz"),
        "Bearer token must be redacted: {content}"
    );
    assert!(content.contains("[REDACTED]"));
}

#[test]
fn test_tool_result_redacts_key_prefix() {
    let secret = "key-abcdefghijklmnopqrstuvwxyz1234567890";
    let (_, v) = parse_data(&Event::ToolResult {
        session_id: "s1".into(),
        call_id: "c1".into(),
        tool: "bash".into(),
        content: format!("My key: {secret}"),
        content_line_count: 1,
        metadata: None,
        success: true,
    });
    let content = v["content"].as_str().unwrap();
    assert!(
        !content.contains(secret),
        "key- prefix must be redacted: {content}"
    );
    assert!(content.contains("[REDACTED]"));
}

#[test]
fn test_tool_result_no_secrets_passes_through() {
    let (_, v) = parse_data(&Event::ToolResult {
        session_id: "s1".into(),
        call_id: "c1".into(),
        tool: "bash".into(),
        content: "Hello world, no secrets here".into(),
        content_line_count: 1,
        metadata: None,
        success: true,
    });
    assert_eq!(v["content"], "Hello world, no secrets here");
}

#[test]
fn test_model_response_redacts_api_key() {
    let secret = "sk-abcdefghijklmnopqrstuvwxyz1234567890";
    let (_, v) = parse_data(&Event::ModelResponse {
        session_id: "s1".into(),
        text: format!("Your key is {secret}"),
        elapsed_ms: 100,
        input_tokens: 12,
        output_tokens: 34,
    });
    let text = v["text"].as_str().unwrap();
    assert!(
        !text.contains(secret),
        "API key must be redacted in model response: {text}"
    );
    assert!(text.contains("[REDACTED]"));
}

#[test]
fn test_model_response_no_secrets_passes_through() {
    let (_, v) = parse_data(&Event::ModelResponse {
        session_id: "s1".into(),
        text: "Normal response text".into(),
        elapsed_ms: 50,
        input_tokens: 4,
        output_tokens: 6,
    });
    assert_eq!(v["text"], "Normal response text");
}
