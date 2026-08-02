//! Conversation serialiser for OpenCode-derived compaction prompts.
//!
//! This module converts ragent's internal [`Message`] representation into a flat
//! text transcript that is fed into the LLM summarisation prompt. It is a
//! Rust port of the `SessionCompaction.serialize` function in
//! `~/Projects/opencode/packages/core/src/session/compaction.ts`.
//!
//! # Output format
//!
//! - `user` → `[User]: <text>` plus one line per file attachment.
//! - `assistant` → one or more of:
//!   - `[Assistant]: <text>`
//!   - `[Assistant reasoning]: <text>`
//!   - `[Assistant tool call]: name(input)`
//!   - `[Tool result]: <truncated text>`
//!   - `[Tool error]: <message>`
//! - `system` → `[System update]: <text>`.
//!
//! Tool outputs are truncated to [`TOOL_OUTPUT_MAX_CHARS`] characters so the
//! compaction prompt stays bounded. Images and other non-text parts are
//! represented by placeholders rather than dropped silently (FR-014).

use ragent_types::message::{ImageData, Message, MessagePart, Role, ToolCallState, ToolCallStatus};

/// Default maximum characters for tool output text included in a compaction
/// prompt. Matches OpenCode's `TOOL_OUTPUT_MAX_CHARS`.
pub const TOOL_OUTPUT_MAX_CHARS: usize = 2_000;

/// Serialise a slice of messages into the OpenCode compaction transcript format.
///
/// Returns `None` when there is no serialisable content (e.g. every message is
/// empty). The returned string is intended to be passed to
/// [`super::prompt::build_prompt`] as the `context`.
///
/// # Arguments
///
/// * `messages` — conversation history in ragent internal format.
/// * `tool_output_max_chars` — cap for individual tool output lines.
#[must_use]
pub fn serialize_messages(messages: &[Message], tool_output_max_chars: usize) -> Option<String> {
    let lines: Vec<String> = messages
        .iter()
        .map(|message| serialize_message(message, tool_output_max_chars))
        .filter(|s| !s.is_empty())
        .collect();
    if lines.is_empty() {
        return None;
    }
    Some(lines.join("\n\n"))
}

/// Serialise a single message into the compaction transcript format.
#[must_use]
pub fn serialize_message(message: &Message, tool_output_max_chars: usize) -> String {
    match message.role {
        Role::User => serialize_user(message, tool_output_max_chars),
        Role::Assistant | Role::Compaction => serialize_assistant(message, tool_output_max_chars),
    }
}

fn serialize_user(message: &Message, _tool_output_max_chars: usize) -> String {
    let mut lines = Vec::new();
    let mut text_parts = Vec::new();
    for part in &message.parts {
        match part {
            MessagePart::Text { text } => text_parts.push(text.as_str()),
            MessagePart::Image(image) => lines.push(serialize_image_attachment(image)),
            MessagePart::ToolCall { .. } | MessagePart::Reasoning { .. } => {
                // User messages should not contain tool calls or reasoning,
                // but if they do, represent them explicitly rather than drop.
                lines.push(format!("[User part]: {}", part.text_content()));
            }
        }
    }
    if !text_parts.is_empty() {
        lines.insert(0, format!("[User]: {}", text_parts.join("\n")));
    }
    lines.join("\n")
}

fn serialize_assistant(message: &Message, tool_output_max_chars: usize) -> String {
    let mut lines = Vec::new();
    for part in &message.parts {
        match part {
            MessagePart::Text { text } => {
                if !text.is_empty() {
                    lines.push(format!("[Assistant]: {text}"));
                }
            }
            MessagePart::Reasoning { text } => {
                if !text.is_empty() {
                    lines.push(format!("[Assistant reasoning]: {text}"));
                }
            }
            MessagePart::ToolCall { tool, state, .. } => {
                lines.extend(serialize_tool_call(tool, state, tool_output_max_chars));
            }
            MessagePart::Image(image) => {
                lines.push(serialize_image_attachment(image));
            }
        }
    }
    lines.join("\n")
}

fn serialize_tool_call(
    tool: &str,
    state: &ToolCallState,
    tool_output_max_chars: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let input = serialize_json_value(&state.input);
    lines.push(format!("[Assistant tool call]: {tool}({input})"));
    match state.status {
        ToolCallStatus::Pending | ToolCallStatus::Running => {
            // No result yet; only the call line is emitted.
        }
        ToolCallStatus::Completed => {
            if let Some(output) = &state.output {
                let output_text = serialize_tool_content(output, tool_output_max_chars);
                lines.push(format!("[Tool result]: {output_text}"));
            }
        }
        ToolCallStatus::Error => {
            if let Some(error) = &state.error {
                lines.push(format!("[Tool error]: {error}"));
            }
        }
    }
    lines
}

fn serialize_tool_content(content: &serde_json::Value, max_chars: usize) -> String {
    let text = match content {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    truncate(&text, max_chars)
}

fn serialize_json_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn serialize_image_attachment(image: &ImageData) -> String {
    let name = image
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("<attachment>");
    format!("[Attached {}: {name}]", image.mime_type)
}

/// Truncate a string to `max` *characters*, adding a `[truncated]` marker.
///
/// The truncation is Unicode-safe: it never splits a multi-byte code point or
/// a grapheme cluster. If `max` lands in the middle of a character, the cut is
/// moved to the preceding character boundary.
#[must_use]
pub fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        // Build the prefix by taking exactly `max` characters. This avoids
        // byte-index slicing which panics on multi-byte UTF-8 sequences.
        let prefix: String = value.chars().take(max).collect();
        format!("{prefix}\n[truncated]")
    }
}

/// Convenience trait for extracting the textual content of a message part.
trait PartTextContent {
    fn text_content(&self) -> String;
}

impl PartTextContent for MessagePart {
    fn text_content(&self) -> String {
        match self {
            Self::Text { text } => text.clone(),
            Self::Reasoning { text } => text.clone(),
            Self::ToolCall { tool, state, .. } => {
                let mut parts = vec![format!(
                    "tool={tool} input={}",
                    serialize_json_value(&state.input)
                )];
                if let Some(output) = &state.output {
                    parts.push(format!("output={}", serialize_json_value(output)));
                }
                if let Some(error) = &state.error {
                    parts.push(format!("error={error}"));
                }
                parts.join(" ")
            }
            Self::Image(image) => {
                format!("[image {}]", image.path.display())
            }
        }
    }
}

/// Serialise a synthetic system-update message as `[System update]: <text>`.
///
/// ragent's `Role` enum does not currently have a dedicated `System` variant,
/// so callers that need to represent system-context updates in the compaction
/// transcript can use this helper on an `Assistant` text message.
#[must_use]
pub fn serialize_system(message: &Message, _tool_output_max_chars: usize) -> String {
    let text = message
        .parts
        .iter()
        .map(|part| part.text_content())
        .collect::<Vec<_>>()
        .join(" ");
    if text.is_empty() {
        String::new()
    } else {
        format!("[System update]: {text}")
    }
}
