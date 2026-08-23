//! External tests for `session::processor` helpers, migrated from the inline
//! `#[cfg(test)] mod tests` block that previously lived at the bottom of
//! `processor.rs` (REMPLAN.md M6 / T6.6).
//!
//! These tests exercise the free-standing helper functions that were extracted
//! into `session::{history, permissions, prompt_builders}` and re-exported via
//! `session::processor::*`.

use std::sync::Arc;

use ragent_agent::event::EventBus;
use ragent_agent::llm::{ChatContent, ChatMessage, ChatRequest};
use ragent_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use ragent_agent::permission::{PermissionAction, PermissionChecker};
use ragent_agent::session::processor::{
    build_detailed_tool_reference_section, chat_request_payload_bytes,
    check_permission_with_prompt, history_to_chat_messages, is_permanent_llm_api_error,
    is_token_overflow_error_message, should_retry_stream_error,
    stream_has_meaningful_partial_output, tool_result_content_for_llm,
};
use ragent_agent::tool::create_default_registry;
use serde_json::{Value, json};

#[tokio::test]
async fn test_hardwired_team_tool_is_auto_approved() {
    let checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(Vec::new())));
    let event_bus = Arc::new(EventBus::new(16));

    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-1",
        "tool:execute",
        "tool:team_status",
        "team_status",
        false,
        None,
    )
    .await
    .expect("permission check should succeed");

    assert_eq!(action, PermissionAction::Allow);
}

#[tokio::test]
async fn test_hardwired_task_suffix_tool_is_auto_approved() {
    let checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(Vec::new())));
    let event_bus = Arc::new(EventBus::new(16));

    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-1",
        "tool:execute",
        "tool:new_agent",
        "new_agent",
        false,
        None,
    )
    .await
    .expect("permission check should succeed");

    assert_eq!(action, PermissionAction::Allow);
}

#[tokio::test]
async fn test_hardwired_ask_user_tool_is_auto_approved() {
    let checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(Vec::new())));
    let event_bus = Arc::new(EventBus::new(16));

    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-1",
        "ask_user",
        "Which provider?",
        "ask_user",
        false,
        None,
    )
    .await
    .expect("permission check should succeed");

    assert_eq!(action, PermissionAction::Allow);
}

#[tokio::test]
async fn test_hardwired_wait_agents_is_auto_approved() {
    let checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(Vec::new())));
    let event_bus = Arc::new(EventBus::new(16));

    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-1",
        "wait_agents",
        "waiting",
        "wait_agents",
        false,
        None,
    )
    .await
    .expect("permission check should succeed");

    assert_eq!(action, PermissionAction::Allow);
}

#[tokio::test]
async fn test_hardwired_agent_complete_is_auto_approved() {
    // The `agent_complete` tool is a terminal signal that ends the
    // autonomous loop.  It must be auto-approved so the agent can
    // always finish a task without a permission prompt.
    let checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(Vec::new())));
    let event_bus = Arc::new(EventBus::new(16));

    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-1",
        "agent_complete",
        "summary",
        "agent_complete",
        false,
        None,
    )
    .await
    .expect("permission check should succeed");

    assert_eq!(action, PermissionAction::Allow);
}

#[tokio::test]
async fn test_hardwired_list_agents_is_auto_approved() {
    // `list_agents` is a read-only inspection tool.  It must be
    // auto-approved so the agent can always check on background
    // sub-agent tasks without a permission prompt.
    let checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(Vec::new())));
    let event_bus = Arc::new(EventBus::new(16));

    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-1",
        "list_agents",
        "list",
        "list_agents",
        false,
        None,
    )
    .await
    .expect("permission check should succeed");

    assert_eq!(action, PermissionAction::Allow);
}

#[tokio::test]
async fn test_hardwired_task_create_is_auto_approved() {
    // todo2tasks T-011: `task_create` shares the "task" permission
    // category and must be auto-approved.
    let checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(Vec::new())));
    let event_bus = Arc::new(EventBus::new(16));

    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-1",
        "task",
        "create task",
        "task_create",
        false,
        None,
    )
    .await
    .expect("permission check should succeed");

    assert_eq!(action, PermissionAction::Allow);
}

#[tokio::test]
async fn test_hardwired_task_update_is_auto_approved() {
    // todo2tasks T-011: `task_update` shares the "task" permission
    // category and must be auto-approved.
    let checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(Vec::new())));
    let event_bus = Arc::new(EventBus::new(16));

    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-1",
        "task",
        "update task",
        "task_update",
        false,
        None,
    )
    .await
    .expect("permission check should succeed");

    assert_eq!(action, PermissionAction::Allow);
}

#[tokio::test]
async fn test_hardwired_task_get_is_auto_approved() {
    // todo2tasks T-011: `task_get` shares the "task" permission
    // category and must be auto-approved.
    let checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(Vec::new())));
    let event_bus = Arc::new(EventBus::new(16));

    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-1",
        "task",
        "get task",
        "task_get",
        false,
        None,
    )
    .await
    .expect("permission check should succeed");

    assert_eq!(action, PermissionAction::Allow);
}

#[tokio::test]
async fn test_hardwired_task_list_is_auto_approved() {
    // todo2tasks T-011: `task_list` shares the "task" permission
    // category and must be auto-approved.
    let checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(Vec::new())));
    let event_bus = Arc::new(EventBus::new(16));

    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-1",
        "task",
        "list tasks",
        "task_list",
        false,
        None,
    )
    .await
    .expect("permission check should succeed");

    assert_eq!(action, PermissionAction::Allow);
}

#[test]
fn test_tool_result_content_for_llm_truncates_large_payloads() {
    let content = format!("{}{}", "a".repeat(9_000), "b".repeat(9_000));
    let truncated =
        tool_result_content_for_llm("read", &content, Some(&json!({"total_lines": 600})));

    assert!(truncated.contains("tool=read"));
    assert!(truncated.contains("600 lines"));
    assert!(truncated.contains("[... "));
    assert!(truncated.contains(&"a".repeat(200)));
    assert!(truncated.contains(&"b".repeat(200)));
    assert!(truncated.len() < content.len());
}

#[tokio::test]
async fn test_history_to_chat_messages_uses_tool_output_content_field() {
    let message = Message::new(
        "session-1",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "read".to_string(),
            call_id: "call-1".to_string(),
            state: ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({"path": "src/lib.rs"}),
                output: Some(json!({
                    "content": "fn main() {}\n",
                    "line_count": 1
                })),
                error: None,
                duration_ms: Some(3),
            },
        }],
    );

    let chat = history_to_chat_messages(&[message]).await;
    assert_eq!(chat.len(), 2);

    let ChatContent::Parts(parts) = &chat[1].content else {
        panic!("expected tool result parts");
    };
    let ragent_agent::llm::ContentPart::ToolResult { content, .. } = &parts[0] else {
        panic!("expected tool result content");
    };
    assert_eq!(content.as_ref(), "fn main() {}\n");
}

#[test]
fn test_chat_request_payload_bytes_counts_serialized_request() {
    let request = ChatRequest {
        model: "demo".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hello".to_string()),
        }]),
        tools: Arc::new(Vec::new()),
        temperature: Some(0.2),
        top_p: None,
        max_tokens: Some(64),
        system: Some(std::sync::Arc::from("system")),
        options: std::collections::HashMap::new(),
        session_id: Some("session-1".to_string()),
        request_id: Some("request-1".to_string()),
        stream_timeout_secs: None,
        thinking: None,
    };

    assert!(chat_request_payload_bytes(&request) >= 40);
}

#[test]
fn test_stream_has_meaningful_partial_output_detects_visible_text() {
    assert!(stream_has_meaningful_partial_output(
        "partial response",
        "",
        false
    ));
    assert!(stream_has_meaningful_partial_output(
        "",
        "partial reasoning",
        false
    ));
    assert!(stream_has_meaningful_partial_output("", "", true));
    assert!(!stream_has_meaningful_partial_output("   ", "\n", false));
}

#[test]
fn test_should_retry_stream_error_only_before_meaningful_output() {
    let stall = "Ollama Cloud: stream stalled — no data received for 120s";

    assert!(should_retry_stream_error(stall, 0, 4, false));
    assert!(!should_retry_stream_error(stall, 0, 4, true));
    assert!(!should_retry_stream_error(stall, 4, 4, false));
    assert!(!should_retry_stream_error(
        "provider rejected request",
        0,
        4,
        false
    ));

    // A raw "error decoding response body" before any output is treated as a
    // malformed/empty stream (e.g. local model not loaded) and should not be
    // retried. Once partial output exists it is kept, not retried.
    let decode_error = "error decoding response body";
    assert!(!should_retry_stream_error(decode_error, 0, 4, false));
    assert!(!should_retry_stream_error(decode_error, 0, 4, true));
}

#[test]
fn test_is_permanent_llm_api_error_detects_model_not_supported() {
    let error = "HuggingFace API error (400 Bad Request): {\"error\":{\"message\":\"The requested model `nvidia/Llama-3.1-Nemotron-70B-Instruct-HF` is not supported by any provider you have enabled.\",\"type\":\"invalid_request_error\",\"param\":\"model\",\"code\":\"model_not_supported\"}}";

    assert!(is_permanent_llm_api_error(error));
}

#[test]
fn test_is_permanent_llm_api_error_ignores_retryable_statuses() {
    assert!(!is_permanent_llm_api_error(
        "OpenAI API error (429 Too Many Requests): rate limited"
    ));
    assert!(!is_permanent_llm_api_error(
        "HTTP 408 Request Timeout: upstream timed out"
    ));
}

#[test]
fn test_is_permanent_llm_api_error_ignores_token_overflow() {
    assert!(!is_permanent_llm_api_error(
        "OpenAI API error (400 Bad Request): context_length_exceeded"
    ));
}

#[test]
fn test_hidden_tool_families_are_excluded_from_prompt_and_request_tools() {
    let registry = create_default_registry();
    let hidden = ragent_config::tool_family_names("github")
        .expect("github family should exist")
        .iter()
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();
    registry.set_hidden(&hidden);

    let defs = registry.definitions();
    assert!(!defs.iter().any(|tool| tool.name == "github_list_issues"));
    assert!(!defs.iter().any(|tool| tool.name == "github_review_pr"));
}

#[test]
fn test_detailed_tool_reference_includes_schemas_and_required_flags() {
    let registry = create_default_registry();
    let section = build_detailed_tool_reference_section(&registry);

    assert!(section.starts_with("## Available Tools"));
    assert!(section.contains("### `new_agent`"));
    assert!(section.contains("- `agent` (`string` (required)"));
    assert!(section.contains("- `task` (`string` (required)"));
    assert!(section.contains("- `background` (`boolean`)"));

    // The `read` tool should expose its range parameters.
    assert!(section.contains("### `read`"));
    assert!(section.contains("- `path` (`string` (required)"));
    assert!(section.contains("- `start_line` ("));
    assert!(section.contains("- `num_lines` ("));
}

#[test]
fn test_detailed_tool_reference_omits_hidden_tools() {
    let registry = create_default_registry();
    let hidden = ragent_config::tool_family_names("github")
        .expect("github family should exist")
        .iter()
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();
    registry.set_hidden(&hidden);

    let section = build_detailed_tool_reference_section(&registry);
    assert!(!section.contains("### `github_list_issues`"));
    assert!(!section.contains("### `github_review_pr`"));
    assert!(section.contains("### `read`"));
}

#[test]
fn test_detailed_tool_reference_handles_empty_registry() {
    use ragent_agent::tool::ToolRegistry;
    let registry = ToolRegistry::new();
    let section = build_detailed_tool_reference_section(&registry);
    assert!(section.is_empty());
}

#[test]
fn test_is_token_overflow_error_message_detects_common_patterns() {
    assert!(is_token_overflow_error_message(
        "prompt token count exceeds maximum context length"
    ));
    assert!(is_token_overflow_error_message("context_length_exceeded"));
    assert!(is_token_overflow_error_message(
        "maximum context length exceeded"
    ));
    assert!(is_token_overflow_error_message("prompt is too long"));
    assert!(is_token_overflow_error_message("input too large"));
    assert!(!is_token_overflow_error_message("rate limit exceeded"));
}

#[test]
fn test_token_overflow_is_not_permanent_error() {
    assert!(!is_permanent_llm_api_error(
        "prompt token count exceeds maximum context length"
    ));
}

// Anchor for `serde_json::Value` import used only under feature gates in this
// test file; referencing it once silences dead-code warnings.
#[allow(dead_code)]
const fn _unused_value_marker() -> Value {
    Value::Null
}
