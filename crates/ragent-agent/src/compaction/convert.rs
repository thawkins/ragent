//! Bidirectional conversion between provider-facing `ChatMessage`s and the
//! internal [`Message`] representation.
//!
//! These are pure data transforms used by the compaction runner and the agent
//! loop's pre-send / emergency-overflow compaction paths.

use chrono::Utc;

use crate::llm::{ChatContent, ChatMessage as LlmChatMessage, ContentPart};
use crate::message::{ImageData, Message, MessagePart, Role, ToolCallState, ToolCallStatus};

/// Convert provider-facing [`ChatMessage`]s into the internal [`Message`]
/// representation used by the compaction runner.
///
/// Tool-use / tool-result content parts are paired back into assistant
/// [`MessagePart::ToolCall`] parts so the serialiser can render them in the
/// summarisation prompt.
pub(crate) fn chat_messages_to_messages(chat_messages: &[LlmChatMessage]) -> Vec<Message> {
    let mut messages: Vec<Message> = Vec::new();
    let now = Utc::now();
    for msg in chat_messages {
        let role = if msg.role == "assistant" {
            Role::Assistant
        } else {
            Role::User
        };
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
                            parts.push(MessagePart::ToolCall {
                                tool: name.clone(),
                                call_id: id.clone(),
                                state: ToolCallState {
                                    status: ToolCallStatus::Completed,
                                    input: input.clone(),
                                    output: None,
                                    error: None,
                                    duration_ms: None,
                                },
                            });
                        }
                        ContentPart::ToolResult {
                            tool_use_id,
                            content,
                        } => {
                            // Pair with the most recent assistant ToolUse that
                            // has not yet received a result.
                            let mut paired = false;
                            for prev in messages.iter_mut().rev() {
                                if prev.role != Role::Assistant {
                                    break;
                                }
                                for p in &mut prev.parts {
                                    if let MessagePart::ToolCall { call_id, state, .. } = p {
                                        if call_id == tool_use_id && state.output.is_none() {
                                            state.output = Some(serde_json::Value::String(
                                                content.to_string(),
                                            ));
                                            paired = true;
                                            break;
                                        }
                                    }
                                }
                                if paired {
                                    break;
                                }
                            }
                            if !paired {
                                parts.push(MessagePart::Text {
                                    text: format!("[tool result {tool_use_id}]: {content}"),
                                });
                            }
                        }
                        ContentPart::ImageUrl { url } => {
                            parts.push(MessagePart::Image(Box::new(ImageData {
                                mime_type: "image/png".to_string(),
                                path: std::path::PathBuf::from(url),
                            })));
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
        let content = if parts.len() == 1 {
            if let Some(ContentPart::Text { text }) = parts.first() {
                ChatContent::Text(text.clone())
            } else {
                ChatContent::Parts(parts)
            }
        } else {
            ChatContent::Parts(parts)
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
