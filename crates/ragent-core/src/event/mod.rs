//! Event streaming infrastructure for ragent sessions.
//!
//! The [`EventBus`] broadcasts [`Event`] values to any number of subscribers
//! using a Tokio broadcast channel. Events cover the full lifecycle of a
//! session: creation, message streaming, tool calls, permission gates,
//! agent switches, errors, and token usage.

use serde::{Deserialize, Serialize};
use std::fmt;
use tokio::sync::broadcast;

/// Reason an LLM stopped generating a response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    /// Model chose to stop (natural end of response).
    Stop,
    /// Model is requesting one or more tool calls.
    ToolUse,
    /// Response was truncated because the token limit was reached.
    Length,
    /// Response was blocked by the provider's content filter.
    ContentFilter,
}

/// A discrete occurrence in the lifecycle of a session.
///
/// TODO: Consider using `Cow<'static, str>` for string fields that are
/// often static (e.g., `tool`, `permission`, `status`) to avoid
/// unnecessary allocations when the value is a known constant.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// A new session has been created.
    SessionCreated {
        /// Unique identifier of the session.
        session_id: String,
    },
    /// An existing session's metadata was updated.
    SessionUpdated {
        /// Unique identifier of the session.
        session_id: String,
    },
    /// The model has started generating a new assistant message.
    MessageStart {
        /// Session this message belongs to.
        session_id: String,
        /// Unique identifier for the message.
        message_id: String,
    },
    /// An incremental chunk of assistant text.
    TextDelta {
        /// Session this delta belongs to.
        session_id: String,
        /// The text fragment.
        text: String,
    },
    /// An incremental chunk of chain-of-thought reasoning text.
    ReasoningDelta {
        /// Session this delta belongs to.
        session_id: String,
        /// The reasoning text fragment.
        text: String,
    },
    /// A tool call has started executing.
    ToolCallStart {
        /// Session this tool call belongs to.
        session_id: String,
        /// Provider-assigned call identifier.
        call_id: String,
        /// Name of the tool being invoked.
        tool: String,
    },
    /// A tool call has finished executing.
    ToolCallEnd {
        /// Session this tool call belongs to.
        session_id: String,
        /// Provider-assigned call identifier.
        call_id: String,
        /// Name of the tool that was invoked.
        tool: String,
        /// Error message if the tool call failed, or `None` on success.
        error: Option<String>,
        /// Wall-clock execution time in milliseconds.
        duration_ms: u64,
    },
    /// The model has finished generating an assistant message.
    MessageEnd {
        /// Session this message belongs to.
        session_id: String,
        /// Identifier of the completed message.
        message_id: String,
        /// Why the model stopped generating.
        reason: FinishReason,
    },
    /// A tool is requesting user permission before proceeding.
    PermissionRequested {
        /// Session making the request.
        session_id: String,
        /// Unique id for this permission request (used in the reply).
        request_id: String,
        /// Permission kind being requested (e.g. `"file:write"`).
        permission: String,
        /// Human-readable description of what is being requested.
        description: String,
    },
    /// The user has replied to a permission request.
    PermissionReplied {
        /// Session the reply belongs to.
        session_id: String,
        /// The request id that was answered.
        request_id: String,
        /// Whether the user granted permission.
        allowed: bool,
    },
    /// The active agent was switched during a session.
    AgentSwitched {
        /// Session in which the switch occurred.
        session_id: String,
        /// Name of the previous agent.
        from: String,
        /// Name of the newly active agent.
        to: String,
    },
    /// An unrecoverable error occurred in the agentic loop.
    AgentError {
        /// Session in which the error occurred.
        session_id: String,
        /// Human-readable error description.
        error: String,
    },
    /// An MCP server's connection status changed.
    McpStatusChanged {
        /// Identifier of the MCP server.
        server_id: String,
        /// New status string (e.g. `"connected"`, `"disconnected"`).
        status: String,
    },
    /// Token usage report for a single LLM request.
    TokenUsage {
        /// Session the usage belongs to.
        session_id: String,
        /// Number of input (prompt) tokens consumed.
        input_tokens: u64,
        /// Number of output (completion) tokens consumed.
        output_tokens: u64,
    },
    /// The set of tool definitions sent with an LLM request.
    ToolsSent {
        /// Session this request belongs to.
        session_id: String,
        /// Names of the tools included in the request.
        tools: Vec<String>,
    },
    /// The model returned text content (complete, not a delta).
    ModelResponse {
        /// Session this response belongs to.
        session_id: String,
        /// The full or truncated text returned by the model.
        text: String,
    },
    /// A tool call has been fully assembled with its arguments.
    ToolCallArgs {
        /// Session this tool call belongs to.
        session_id: String,
        /// Provider-assigned call identifier.
        call_id: String,
        /// Name of the tool being invoked.
        tool: String,
        /// JSON-encoded arguments.
        args: String,
    },
    /// The result of executing a tool.
    ToolResult {
        /// Session this tool result belongs to.
        session_id: String,
        /// Provider-assigned call identifier.
        call_id: String,
        /// Name of the tool that was invoked.
        tool: String,
        /// The result content (or error text).
        content: String,
        /// Whether the tool succeeded.
        success: bool,
    },
    /// The Copilot device flow completed successfully.
    CopilotDeviceFlowComplete {
        /// The GitHub OAuth token obtained from the device flow.
        token: String,
        /// The plan-specific API base URL discovered during setup.
        api_base: String,
    },
    /// A session was aborted by the user or the server.
    SessionAborted {
        /// Identifier of the aborted session.
        session_id: String,
        /// Human-readable reason for the abort (e.g. `"user_requested"`).
        reason: String,
    },
}

/// Broadcast-based event bus for distributing [`Event`] values to subscribers.
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Event>,
}

impl EventBus {
    /// Creates a new event bus with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Returns a new receiver that will observe all future events.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }

    /// Broadcasts an event to all current subscribers.
    ///
    /// Silently drops the event if there are no active subscribers.
    /// Publishes an event to all active subscribers.
    ///
    /// The underlying broadcast channel has a fixed-size buffer (256 by default).
    /// When the buffer is full, the oldest events are dropped and slow subscribers
    /// will receive a `Lagged` error on their next `recv()`.
    pub fn publish(&self, event: Event) {
        if self.sender.send(event).is_err() {
            tracing::warn!("Event dropped: no active subscribers");
        }
    }
}

impl Default for EventBus {
    /// Creates an `EventBus` with a default capacity of 256 events.
    fn default() -> Self {
        Self::new(256)
    }
}

impl fmt::Display for FinishReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FinishReason::Stop => write!(f, "stop"),
            FinishReason::ToolUse => write!(f, "tool_use"),
            FinishReason::Length => write!(f, "length"),
            FinishReason::ContentFilter => write!(f, "content_filter"),
        }
    }
}
