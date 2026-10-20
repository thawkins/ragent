//! Tests for the narrated tool-call scrubbing on think tool outputs and
//! Reasoning parts (`scrub_narrated_toolcalls`).
//!
//! Some models narrate their NEXT tool call as markup (tagged dialects or a
//! bare JSON object) inside a think tool's thought or the reasoning channel.
//! The agent loop recovers those calls, so the transcript must not also print
//! the raw narration.

use ragent_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use ragent_tui::widgets::message_widget::MessageWidget;
use serde_json::json;

fn assistant_with_parts(parts: Vec<MessagePart>) -> Message {
    Message::new("session-1", Role::Assistant, parts)
}

fn render_parts(parts: Vec<MessagePart>) -> Vec<String> {
    let tool_step_map: std::collections::HashMap<String, (String, u32, u32)> =
        std::collections::HashMap::new();
    let msg = assistant_with_parts(parts);
    let widget = MessageWidget::new(&msg, "/tmp", &tool_step_map);
    widget
        .to_lines()
        .into_iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.to_string())
                .collect::<String>()
        })
        .collect()
}

fn think_call(thought: &str) -> MessagePart {
    MessagePart::ToolCall {
        tool: "think".to_string(),
        call_id: "call-think-1".to_string(),
        state: Box::new(ToolCallState {
            status: ToolCallStatus::Completed,
            input: json!({"thought": thought}),
            output: Some(json!({
                "thinking": true,
                "thought": thought
            })),
            error: None,
            duration_ms: Some(0),
        }),
    }
}

#[test]
fn test_thought_with_narrated_underscore_dialect_does_not_render_markup() {
    let thought = format!(
        "Two options. Better is one. Call:\n<tool_call>\n{{\"name\": \"read\", \"arguments\": {{}}}}\n<tool_call>"
    );
    let lines = render_parts(vec![think_call(&thought)]);
    let joined = lines.join("\n");
    assert!(joined.contains("Two options."));
    assert!(!joined.contains("tool_call"), "got: {joined}");
    assert!(!joined.contains("\"name\": \"read\""), "got: {joined}");
}

#[test]
fn test_thought_with_narrated_bare_json_does_not_render_object() {
    let thought = "Let me check the file.\n{\"name\": \"bash\", \"arguments\": {\"command\": \"ls\"}}\nThen act.";
    let lines = render_parts(vec![think_call(thought)]);
    let joined = lines.join("\n");
    assert!(joined.contains("Let me check the file."));
    assert!(joined.contains("Then act."));
    assert!(!joined.contains("\"command\": \"ls\""), "got: {joined}");
}

#[test]
fn test_thought_with_narrated_plain_ascii_dialect_is_scrubbed() {
    let thought = "read the plan first\n<toolcall>Read</toolcall>\nthen continue";
    let lines = render_parts(vec![think_call(thought)]);
    let joined = lines.join("\n");
    assert!(joined.contains("read the plan first"));
    assert!(joined.contains("then continue"));
    assert!(!joined.contains("<toolcall>"), "got: {joined}");
}

#[test]
fn test_thought_ordinary_code_is_preserved() {
    let thought = "Use this shape:\nlet status = self.members.iter().find(|m| m.id == id);\nDone.";
    let lines = render_parts(vec![think_call(thought)]);
    let joined = lines.join("\n");
    assert!(
        joined.contains("let status = self.members.iter().find"),
        "got: {joined}"
    );
    assert!(joined.contains("Done."));
}

#[test]
fn test_thought_json_without_toolcall_shape_is_preserved() {
    // A JSON object the model quotes without a tool-call shape must survive -
    // ordinary prose and data never match.
    let thought = "Shape: {\"answer\": 42}";
    let lines = render_parts(vec![think_call(thought)]);
    let joined = lines.join("\n");
    assert!(joined.contains("\"answer\": 42"), "got: {joined}");
}

#[test]
fn test_reasoning_part_with_narrated_markup_is_scrubbed() {
    let text = format!(
        "Reasoning before acting.\n<tool_call>\n{{\"name\": \"grep\", \"arguments\": {{}}}}\n<tool_call>"
    );
    let lines = render_parts(vec![
        MessagePart::Reasoning { text },
        MessagePart::Text {
            text: "visible answer".to_string(),
        },
    ]);
    let joined = lines.join("\n");
    assert!(joined.contains("Reasoning before acting."));
    assert!(joined.contains("visible answer"));
    assert!(!joined.contains("tool_call"), "got: {joined}");
    assert!(!joined.contains("\"name\": \"grep\""), "got: {joined}");
}

#[test]
fn test_reasoning_part_ordinary_text_is_preserved() {
    let lines = render_parts(vec![MessagePart::Reasoning {
        text: "step 1: plan\nstep 2: execute".to_string(),
    }]);
    let joined = lines.join("\n");
    assert!(joined.contains("step 1: plan"), "got: {joined}");
    assert!(joined.contains("step 2: execute"), "got: {joined}");
}

#[test]
fn test_think_call_incomplete_block_scrubs_to_end_of_thought() {
    // An unterminated narration block consumes the remaining lines - the
    // recovery loop may have executed the call already, so the leftover block
    // must not print.
    let thought = "start\n<tool_call>\n{\"name\": \"read\", \"arguments\": {}}\nand never closed";
    let lines = render_parts(vec![think_call(thought)]);
    let joined = lines.join("\n");
    assert!(joined.contains("start"), "got: {joined}");
    assert!(!joined.contains("and never closed"), "got: {joined}");
}
