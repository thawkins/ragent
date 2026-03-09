//! Conversation message types.
//!
//! Contains [`Message`], the primary unit of conversation history, along with
//! [`MessagePart`] variants (text, tool calls, reasoning) and supporting types
//! like [`Role`], [`ToolCallState`], and [`ToolCallStatus`].

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// The role of a participant in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
        }
    }
}

/// Lifecycle status of a tool call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ToolCallStatus {
    Pending,
    Running,
    Completed,
    Error,
}

/// Runtime state of a single tool invocation, including its input,
/// output, and timing information.
impl fmt::Display for ToolCallStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolCallStatus::Pending => write!(f, "pending"),
            ToolCallStatus::Running => write!(f, "running"),
            ToolCallStatus::Completed => write!(f, "completed"),
            ToolCallStatus::Error => write!(f, "error"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallState {
    pub status: ToolCallStatus,
    // TODO: Replace `Value` with a typed struct for tool call input/output once
    // tool schemas are well-defined.
    pub input: Value,
    pub output: Option<Value>,
    pub error: Option<String>,
    pub duration_ms: Option<u64>,
}

/// A discrete content block within a [`Message`].
///
/// Messages are composed of one or more parts, allowing text, tool calls,
/// and model reasoning to coexist in a single message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessagePart {
    Text {
        text: String,
    },
    ToolCall {
        tool: String,
        call_id: String,
        state: ToolCallState,
    },
    Reasoning {
        text: String,
    },
}

/// A single message in a conversation session.
///
/// Each message has a unique ID, belongs to a session, carries a [`Role`],
/// and contains one or more [`MessagePart`]s.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: Role,
    pub parts: Vec<MessagePart>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Message {
    /// Creates a new message with a generated UUID and the current timestamp.
    pub fn new(session_id: impl Into<String>, role: Role, parts: Vec<MessagePart>) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            role,
            parts,
            created_at: now,
            updated_at: now,
        }
    }

    /// Convenience constructor for a simple user text message.
    pub fn user_text(session_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(
            session_id,
            Role::User,
            vec![MessagePart::Text { text: text.into() }],
        )
    }

    /// Concatenates all [`MessagePart::Text`] parts into a single string.
    pub fn text_content(&self) -> String {
        self.parts
            .iter()
            .filter_map(|p| match p {
                MessagePart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = self.text_content();
        let preview = if text.len() > 80 {
            format!("{}…", &text[..80])
        } else {
            text
        };
        let tool_calls = self
            .parts
            .iter()
            .filter(|p| matches!(p, MessagePart::ToolCall { .. }))
            .count();
        if tool_calls > 0 {
            write!(
                f,
                "[{}] {} ({} tool call{})",
                self.role,
                preview,
                tool_calls,
                if tool_calls == 1 { "" } else { "s" }
            )
        } else {
            write!(f, "[{}] {}", self.role, preview)
        }
    }
}
