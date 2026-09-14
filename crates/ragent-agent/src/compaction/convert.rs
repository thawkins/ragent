//! Bidirectional conversion between provider-facing `ChatMessage`s and the
//! internal [`Message`] representation.
//!
//! These are pure data transforms used by the compaction runner and the agent
//! loop's pre-send / emergency-overflow compaction paths.

use chrono::Utc;
use std::collections::HashMap;

use crate::llm::{ChatContent, ChatMessage as LlmChatMessage, ContentPart};
use crate::message::{ImageData, Message, MessagePart, Role, ToolCallState, ToolCallStatus};

/// A location of an assistant [`MessagePart::ToolCall`] awaiting its result.
type PendingToolUse = (usize, usize);

/// Convert provider-facing [`ChatMessage`]s into the internal [`Message`]
/// representation used by the compaction runner.
///
/// Tool-use / tool-result content parts are paired back into assistant
/// [`MessagePart::ToolCall`] parts so the serialiser can render them in the
/// summarisation prompt.
///
/// PERF-037: pairing is done with a single `call_id -> (message, part)` index
/// built as messages are appended, so the conversion is O(messages x parts)
/// rather than O(messages^2) — the previous implementation scanned the whole
/// prior message list backwards for every `ToolResult`.
pub(crate) fn chat_messages_to_messages(chat_messages: &[LlmChatMessage]) -> Vec<Message> {
    let mut messages: Vec<Message> = Vec::new();
    // Index of assistant `ToolCall` parts that have not yet received a result.
    let mut pending: HashMap<&str, PendingToolUse> = HashMap::new();
    let now = Utc::now();
    for msg in chat_messages {
        let role = match msg.role.as_str() {
            "assistant" => Role::Assistant,
            // System and tool-role messages have no place in the internal
            // compaction history; skip them rather than silently re-roling.
            "system" | "tool" => continue,
            _ => Role::User,
        };
        let msg_idx = messages.len();
        let mut parts: Vec<MessagePart> = Vec::new();
        match &msg.content {
            ChatContent::Text(text) => {
                parts.push(MessagePart::Text { text: text.clone() });
            }
            ChatContent::Parts(content_parts) => {
                for part in content_parts {
                    match part {
                        ContentPart::Text { text } => {
                            parts.push(MessagePart::Text { text: text.clone() });
                        }
                        ContentPart::ToolUse { id, name, input } => {
                            if role == Role::Assistant {
                                pending.insert(id.as_str(), (msg_idx, parts.len()));
                            }
                            parts.push(MessagePart::ToolCall {
                                tool: name.clone(),
                                call_id: id.clone(),
                                state: Box::new(ToolCallState {
                                    status: ToolCallStatus::Completed,
                                    input: input.clone(),
                                    output: None,
                                    error: None,
                                    duration_ms: None,
                                }),
                            });
                        }
                        ContentPart::ToolResult {
                            tool_use_id,
                            content,
                        } => {
                            // Pair with the recorded assistant ToolUse for this
                            // id (single hash lookup; PERF-037). A part that
                            // already received a result is not re-paired, so a
                            // duplicate result still surfaces as a text part.
                            let paired =
                                pending
                                    .get(tool_use_id.as_str())
                                    .copied()
                                    .and_then(|(mi, pi)| {
                                        let target = messages.get_mut(mi)?;
                                        if let Some(MessagePart::ToolCall { state, .. }) =
                                            target.parts.get_mut(pi)
                                            && state.output.is_none()
                                        {
                                            state.output = Some(serde_json::Value::String(
                                                content.to_string(),
                                            ));
                                            return Some(());
                                        }
                                        None
                                    });
                            if paired.is_none() {
                                parts.push(MessagePart::Text {
                                    text: format!("[tool result {tool_use_id}]: {content}"),
                                });
                            }
                        }
                        ContentPart::ImageUrl { url } => {
                            // Parse the mime type from a `data:` URI; plain
                            // https URLs fall back to a generic image mime type.
                            let mime_type = url
                                .strip_prefix("data:")
                                .and_then(|rest| rest.split_once(';'))
                                .map(|(mime, _)| mime.to_string())
                                .unwrap_or_else(|| "image/png".to_string());
                            let path = std::path::PathBuf::from(url);
                            parts.push(MessagePart::Image(Box::new(ImageData { mime_type, path })));
                        }
                    }
                }
            }
        }
        messages.push(Message {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: "compaction".to_string(),
            role,
            parts,
            created_at: now,
            updated_at: now,
            edit_seq: 0,
        });
    }
    messages
}

/// Convert internal [`Message`]s back to provider-facing [`ChatMessage`]s.
///
/// Each assistant [`MessagePart::ToolCall`] produces an assistant `ToolUse`
/// part and a following user `ToolResult` part so the LLM API sees the
/// required pairs. [`Role::Compaction`] messages are emitted as `assistant`
/// turns so providers that do not understand a custom role still receive the
/// summary.
pub(crate) fn messages_to_chat_messages(messages: &[Message]) -> Vec<LlmChatMessage> {
    let mut chat_messages: Vec<LlmChatMessage> = Vec::new();
    for msg in messages {
        let role = match msg.role {
            Role::User => "user".to_string(),
            Role::Assistant | Role::Compaction => "assistant".to_string(),
        };
        let mut parts: Vec<ContentPart> = Vec::new();
        let mut tool_results: Vec<ContentPart> = Vec::new();
        for part in &msg.parts {
            match part {
                MessagePart::Text { text } => {
                    parts.push(ContentPart::Text { text: text.clone() });
                }
                MessagePart::Reasoning { text } => {
                    parts.push(ContentPart::Text {
                        text: format!("[reasoning]: {text}"),
                    });
                }
                MessagePart::Image(img) => {
                    parts.push(ContentPart::ImageUrl {
                        url: img.path.to_string_lossy().to_string(),
                    });
                }
                MessagePart::ToolCall {
                    tool,
                    call_id,
                    state,
                } => {
                    parts.push(ContentPart::ToolUse {
                        id: call_id.clone(),
                        name: tool.clone(),
                        input: state.input.clone(),
                    });
                    let result_text = state
                        .output
                        .as_ref()
                        .and_then(|v| v.as_str().map(std::string::ToString::to_string))
                        .unwrap_or_default();
                    let content = if result_text.is_empty() {
                        state.error.clone().unwrap_or_default()
                    } else {
                        result_text
                    };
                    tool_results.push(ContentPart::ToolResult {
                        tool_use_id: call_id.clone(),
                        content: content.into(),
                    });
                }
            }
        }
        let content = match parts.as_slice() {
            [ContentPart::Text { text }] => ChatContent::Text(text.clone()),
            _ => ChatContent::Parts(parts),
        };
        chat_messages.push(LlmChatMessage { role, content });
        if !tool_results.is_empty() && msg.role == Role::Assistant {
            chat_messages.push(LlmChatMessage {
                role: "user".to_string(),
                content: ChatContent::Parts(tool_results),
            });
        }
    }
    chat_messages
}

#[cfg(test)]
#[path = "../../tests/inline/test_compaction_convert.rs"]
mod tests;
