//! Inline tests for `layout.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::messages_to_lines;
use ragent_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use serde_json::json;
use std::collections::HashMap;

#[test]
fn test_messages_to_lines_renders_agent_notice_bright_yellow_one_line_per_item() {
    let message = Message::new(
        "s1",
        Role::Assistant,
        vec![MessagePart::Text {
            text: "[notice] Agent Notice\nFirst item\nSecond item".to_string(),
        }],
    );

    let lines = messages_to_lines(&[message], &HashMap::new(), &HashMap::new(), "/project");
    let rendered: Vec<String> = lines.iter().map(ToString::to_string).collect();

    // Header plus each list item gets its own line.
    assert!(rendered.iter().any(|line| line.contains("Agent Notice")));
    assert!(rendered.iter().any(|line| line.contains("First item")));
    assert!(rendered.iter().any(|line| line.contains("Second item")));

    // All notice lines are styled bright yellow + bold.
    for line in &lines {
        for span in line.spans.iter() {
            assert_eq!(span.style.fg, Some(ratatui::style::Color::Yellow));
            assert!(
                span.style
                    .add_modifier
                    .contains(ratatui::style::Modifier::BOLD)
            );
        }
    }

    // A trailing blank line separates the notice bubble from following content.
    assert!(lines.last().is_some_and(
        |line| line.spans.is_empty() || line.spans.iter().all(|span| span.content.is_empty())
    ));
}

#[test]
fn test_messages_to_lines_renders_full_thinktool_output_multiline() {
    let message = Message::new(
        "s1",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "think".to_string(),
            call_id: "call-1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({"thought": "First line.\nSecond line."}),
                output: Some(json!({"thought": "First line.\nSecond line."})),
                error: None,
                duration_ms: Some(42),
            }),
        }],
    );

    let lines = messages_to_lines(&[message], &HashMap::new(), &HashMap::new(), "/project");
    let rendered: Vec<String> = lines.iter().map(ToString::to_string).collect();

    assert!(
        rendered.iter().any(|line| line.contains("Think")),
        "Expected tool header line in rendered output: {rendered:?}"
    );
    assert!(
        rendered
            .iter()
            .filter(|line| line.contains("Think"))
            .all(|line| !line.contains("First line.")),
        "Expected think header to omit inline thought summary: {rendered:?}"
    );
    assert!(
        rendered.iter().any(|line| line == "  First line."),
        "Expected first thought line in rendered output: {rendered:?}"
    );
    assert!(
        rendered.iter().any(|line| line == "  Second line."),
        "Expected second thought line in rendered output: {rendered:?}"
    );
}

#[test]
fn test_messages_to_lines_renders_full_agent_complete_output_multiline() {
    let message = Message::new(
        "s1",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "agent_complete".to_string(),
            call_id: "call-1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({"summary": "First line.\nSecond line."}),
                output: Some(json!({
                    "agent_complete": true,
                    "summary": "First line.\nSecond line."
                })),
                error: None,
                duration_ms: Some(42),
            }),
        }],
    );

    let lines = messages_to_lines(&[message], &HashMap::new(), &HashMap::new(), "/project");
    let rendered: Vec<String> = lines.iter().map(ToString::to_string).collect();

    // Each summary line must be rendered as its own ratatui Line. A single
    // Line containing the whole summary would lose the '\n' graphemes
    // (ratatui filters them out of Spans), collapsing the summary into one
    // visual paragraph.
    assert!(
        rendered
            .iter()
            .any(|line| line.contains("  └ [ok] First line.")),
        "Expected first summary line on its own rendered line: {rendered:?}"
    );
    assert!(
        rendered
            .iter()
            .any(|line| line.contains("  └ [ok] Second line.")),
        "Expected second summary line on its own rendered line: {rendered:?}"
    );
    assert!(
        !rendered
            .iter()
            .any(|line| { line.contains("First line.") && line.contains("Second line.") }),
        "Summary lines must not be joined into a single Line: {rendered:?}"
    );
}
