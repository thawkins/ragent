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
        "tool:new_task",
        "new_task",
        false,
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
    )
    .await
    .expect("permission check should succeed");

    assert_eq!(action, PermissionAction::Allow);
}

#[tokio::test]
async fn test_hardwired_todo_read_is_auto_approved() {
    let checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(Vec::new())));
    let event_bus = Arc::new(EventBus::new(16));

    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-1",
        "todo_read",
        "list all",
        "todo_read",
        false,
    )
    .await
    .expect("permission check should succeed");

    assert_eq!(action, PermissionAction::Allow);
}

#[tokio::test]
async fn test_hardwired_todo_write_is_auto_approved() {
    let checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(Vec::new())));
    let event_bus = Arc::new(EventBus::new(16));

    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-1",
        "todo_write",
        "add item",
        "todo_write",
        false,
    )
    .await
    .expect("permission check should succeed");

    assert_eq!(action, PermissionAction::Allow);
}

#[tokio::test]
async fn test_hardwired_wait_tasks_is_auto_approved() {
    let checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(Vec::new())));
    let event_bus = Arc::new(EventBus::new(16));

    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-1",
        "wait_tasks",
        "waiting",
        "wait_tasks",
        false,
    )
    .await
    .expect("permission check should succeed");

    assert_eq!(action, PermissionAction::Allow);
}

#[tokio::test]
async fn test_hardwired_task_complete_is_auto_approved() {
    // The `task_complete` tool is a terminal signal that ends the
    // autonomous loop.  It must be auto-approved so the agent can
    // always finish a task without a permission prompt.
    let checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(Vec::new())));
    let event_bus = Arc::new(EventBus::new(16));

    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-1",
        "task_complete",
        "summary",
        "task_complete",
        false,
    )
    .await
    .expect("permission check should succeed");

    assert_eq!(action, PermissionAction::Allow);
}

#[tokio::test]
async fn test_hardwired_list_tasks_is_auto_approved() {
    // `list_tasks` is a read-only inspection tool.  It must be
    // auto-approved so the agent can always check on background
    // sub-agent tasks without a permission prompt.
    let checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(Vec::new())));
    let event_bus = Arc::new(EventBus::new(16));

    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-1",
        "list_tasks",
        "list",
        "list_tasks",
        false,
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
    assert!(section.contains("### `new_task`"));
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

// Silence unused-import warnings for items only used under feature gates.
#[allow(dead_code)]
fn _unused_value_marker() -> Value {
    Value::Null
}
