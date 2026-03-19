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
    /// A human user.
    User,
    /// An AI assistant.
    Assistant,
}

impl fmt::Display for Role {
    /// Format the role as a lowercase string.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the formatter fails.
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
    /// The tool call has been created but not yet started.
    Pending,
    /// The tool call is currently executing.
    Running,
    /// The tool call finished successfully.
    Completed,
    /// The tool call failed with an error.
    Error,
}

/// Runtime state of a single tool invocation, including its input,
/// output, and timing information.
impl fmt::Display for ToolCallStatus {
    /// Format the tool call status as a lowercase string.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to the formatter fails.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolCallStatus::Pending => write!(f, "pending"),
            ToolCallStatus::Running => write!(f, "running"),
            ToolCallStatus::Completed => write!(f, "completed"),
            ToolCallStatus::Error => write!(f, "error"),
        }
    }
}

/// Tracks the execution state of a single tool call, including its input
/// arguments, output result, error information, and timing.
///
/// # Examples
///
/// ```rust
/// use ragent_core::message::{ToolCallState, ToolCallStatus};
/// use serde_json::json;
///
/// let state = ToolCallState {
///     status: ToolCallStatus::Completed,
///     input: json!({"path": "/tmp/file.txt"}),
///     output: Some(json!({"ok": true})),
///     error: None,
///     duration_ms: Some(42),
/// };
/// assert_eq!(state.status, ToolCallStatus::Completed);
/// assert!(state.error.is_none());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallState {
    /// Current lifecycle status of the tool call.
    pub status: ToolCallStatus,
    // TODO: Replace `Value` with a typed struct for tool call input/output once
    // tool schemas are well-defined.
    /// JSON arguments passed to the tool.
    pub input: Value,
    /// JSON result returned by the tool, if available.
    pub output: Option<Value>,
    /// Error message if the tool call failed.
    pub error: Option<String>,
    /// Wall-clock execution time in milliseconds.
    pub duration_ms: Option<u64>,
}

/// A discrete content block within a [`Message`].
///
/// Messages are composed of one or more parts, allowing text, tool calls,
/// and model reasoning to coexist in a single message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessagePart {
    /// Plain text content.
    Text {
        /// The text body.
        text: String,
    },
    /// A tool invocation with its execution state.
    ToolCall {
        /// Name of the tool being called.
        tool: String,
        /// Unique identifier for this tool call.
        call_id: String,
        /// Execution state of the tool call.
        state: ToolCallState,
    },
    /// Internal model reasoning or chain-of-thought.
    Reasoning {
        /// The reasoning text.
        text: String,
    },
    /// An image attachment (e.g. pasted from clipboard via Alt+V).
    ///
    /// The image data is read from `path` at send time and base64-encoded for
    /// the LLM API. Storing the path rather than raw bytes keeps the session
    /// database small.
    Image {
        /// MIME type, e.g. `"image/png"`.
        mime_type: String,
        /// Absolute path to the image file on disk.
        path: std::path::PathBuf,
    },
}

/// A single message in a conversation session.
///
/// Each message has a unique ID, belongs to a session, carries a [`Role`],
/// and contains one or more [`MessagePart`]s.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier for this message (UUID v4).
    pub id: String,
    /// Identifier of the session this message belongs to.
    pub session_id: String,
    /// Who sent the message.
    pub role: Role,
    /// Ordered content blocks that make up the message.
    pub parts: Vec<MessagePart>,
    /// Timestamp when the message was created (UTC).
    pub created_at: DateTime<Utc>,
    /// Timestamp when the message was last modified (UTC).
    pub updated_at: DateTime<Utc>,
}

impl Message {
    /// Creates a new message with a generated UUID and the current timestamp.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ragent_core::message::{Message, MessagePart, Role};
    ///
    /// let msg = Message::new(
    ///     "session-1",
    ///     Role::Assistant,
    ///     vec![MessagePart::Text { text: "Hello!".into() }],
    /// );
    /// assert_eq!(msg.role, Role::Assistant);
    /// assert_eq!(msg.session_id, "session-1");
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ragent_core::message::{Message, Role};
    ///
    /// let msg = Message::user_text("session-1", "Fix the bug");
    /// assert_eq!(msg.role, Role::User);
    /// assert_eq!(msg.text_content(), "Fix the bug");
    /// ```
    pub fn user_text(session_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(
            session_id,
            Role::User,
            vec![MessagePart::Text { text: text.into() }],
        )
    }

    /// Concatenates all [`MessagePart::Text`] parts into a single string.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ragent_core::message::{Message, MessagePart, Role};
    ///
    /// let msg = Message::new(
    ///     "s1",
    ///     Role::User,
    ///     vec![
    ///         MessagePart::Text { text: "Hello ".into() },
    ///         MessagePart::Text { text: "world".into() },
    ///     ],
    /// );
    /// assert_eq!(msg.text_content(), "Hello world");
    /// ```
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
            let mut end = 80;
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}…", &text[..end])
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
