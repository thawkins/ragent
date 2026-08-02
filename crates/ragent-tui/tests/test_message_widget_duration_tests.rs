//! External tests for `duration_tests` from `crates/ragent-tui/src/widgets/message_widget.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use ragent_tui::widgets::message_widget::MessageWidget;
use serde_json::json;

/// Test that duration_ms is displayed for completed tool calls.
/// This is a regression test - this feature has been accidentally removed 3 times.
#[test]
fn test_toolcall_duration_displayed_when_completed() {
    let msg = Message::new(
        "session-1",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "read".to_string(),
            call_id: "call-001".to_string(),
            state: ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({"path": "/tmp/test.txt"}),
                output: Some(json!({"content": "Hello"})),
                error: None,
                duration_ms: Some(1234),
            },
        }],
    );

    let tool_step_map: std::collections::HashMap<String, (String, u32, u32)> =
        std::collections::HashMap::new();
    let widget = MessageWidget::new(&msg, "/tmp", &tool_step_map);
    let lines = widget.to_lines();

    // Find the tool call line and check for duration (tool name is capitalized to "Read")
    let tool_line = lines
        .iter()
        .find(|line| line.spans.iter().any(|span| span.content.contains("Read")))
        .expect("Should find a line with 'Read' tool");

    let line_text: String = tool_line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        line_text.contains("1.2s") || line_text.contains("1234ms"),
        "Duration should be displayed in tool call line. Got: {}",
        line_text
    );
}

/// Test that duration is NOT displayed when status is Running
#[test]
fn test_toolcall_duration_not_displayed_when_running() {
    let msg = Message::new(
        "session-1",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "read".to_string(),
            call_id: "call-002".to_string(),
            state: ToolCallState {
                status: ToolCallStatus::Running,
                input: json!({"path": "/tmp/test.txt"}),
                output: None,
                error: None,
                duration_ms: None,
            },
        }],
    );

    let tool_step_map: std::collections::HashMap<String, (String, u32, u32)> =
        std::collections::HashMap::new();
    let widget = MessageWidget::new(&msg, "/tmp", &tool_step_map);
    let lines = widget.to_lines();

    // Find the tool call line (tool name is capitalized to "Read")
    let tool_line = lines
        .iter()
        .find(|line| line.spans.iter().any(|span| span.content.contains("Read")))
        .expect("Should find a line with 'Read' tool");

    let line_text: String = tool_line.spans.iter().map(|s| s.content.as_ref()).collect();
    // Check that duration pattern is NOT present - look for " (XXXms)" or " (X.Xs)"
    // The "test.txt" path contains 's' so we need to be specific
    assert!(
        !line_text.contains("ms)") && !line_text.contains("s)"),
        "Duration should NOT be displayed when tool is running. Got: {}",
        line_text
    );
}
/// Test that duration is NOT displayed when duration_ms is None
#[test]
fn test_toolcall_duration_not_displayed_when_none() {
    let msg = Message::new(
        "session-1",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "read".to_string(),
            call_id: "call-003".to_string(),
            state: ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({"path": "/tmp/test.txt"}),
                output: Some(json!({"content": "Hello"})),
                error: None,
                duration_ms: None,
            },
        }],
    );

    let tool_step_map: std::collections::HashMap<String, (String, u32, u32)> =
        std::collections::HashMap::new();
    let widget = MessageWidget::new(&msg, "/tmp", &tool_step_map);
    let lines = widget.to_lines();

    // Find the tool call line (tool name is capitalized to "Read")
    let tool_line = lines
        .iter()
        .find(|line| line.spans.iter().any(|span| span.content.contains("Read")))
        .expect("Should find a line with 'Read' tool");

    let line_text: String = tool_line.spans.iter().map(|s| s.content.as_ref()).collect();
    // Duration format is " (XXXms)" or " (X.Xs)" - check for that specific pattern
    // The result summary line contains "lines read" which is fine
    // We just need to ensure there's no duration parenthetical
    assert!(
        !(line_text.contains('(') && line_text.contains("ms") || line_text.contains("s)")),
        "Duration should NOT be displayed when duration_ms is None. Got: {}",
        line_text
    );
}

/// Test millisecond format for durations under 1 second
#[test]
fn test_toolcall_duration_shows_ms_for_short_duration() {
    let msg = Message::new(
        "session-1",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "read".to_string(),
            call_id: "call-004".to_string(),
            state: ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({"path": "/tmp/test.txt"}),
                output: Some(json!({"content": "Hello"})),
                error: None,
                duration_ms: Some(456),
            },
        }],
    );

    let tool_step_map: std::collections::HashMap<String, (String, u32, u32)> =
        std::collections::HashMap::new();
    let widget = MessageWidget::new(&msg, "/tmp", &tool_step_map);
    let lines = widget.to_lines();

    // Find the tool call line (tool name is capitalized to "Read")
    let tool_line = lines
        .iter()
        .find(|line| line.spans.iter().any(|span| span.content.contains("Read")))
        .expect("Should find a line with 'Read' tool");

    let line_text: String = tool_line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        line_text.contains("456ms"),
        "Short durations should show as ms. Got: {}",
        line_text
    );
}
/// Test that duration is displayed correctly even with inline diff
#[test]
fn test_toolcall_duration_with_inline_diff() {
    let msg = Message::new(
        "session-1",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "edit".to_string(),
            call_id: "call-005".to_string(),
            state: ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({"path": "/tmp/test.txt", "old": "old", "new": "new"}),
                output: Some(json!({
                    "old_lines": 10,
                    "new_lines": 15,
                    "path": "/tmp/test.txt"
                })),
                error: None,
                duration_ms: Some(1500),
            },
        }],
    );

    let tool_step_map: std::collections::HashMap<String, (String, u32, u32)> =
        std::collections::HashMap::new();
    let widget = MessageWidget::new(&msg, "/tmp", &tool_step_map);
    let lines = widget.to_lines();

    let tool_line = lines
        .iter()
        .find(|line| line.spans.iter().any(|span| span.content.contains("Edit")))
        .expect("Should find a line with 'Edit' tool");

    let line_text: String = tool_line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        line_text.contains("1.5s") && line_text.contains("+15") && line_text.contains("-10"),
        "Duration should appear alongside inline diff. Got: {}",
        line_text
    );
}
