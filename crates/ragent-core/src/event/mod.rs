//! Event streaming infrastructure for ragent sessions.
//!
//! The [`EventBus`] broadcasts [`Event`] values to any number of subscribers
//! using a Tokio broadcast channel. Events cover the full lifecycle of a
//! session: creation, message streaming, tool calls, permission gates,
//! agent switches, errors, and token usage.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, RwLock};
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
    /// The user cancelled the agent loop (e.g. pressed ESC).
    Cancelled,
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
    /// A tool has requested switching to a different agent.
    AgentSwitchRequested {
        /// Session in which the switch was requested.
        session_id: String,
        /// Name of the target agent.
        to: String,
        /// Task description for the target agent.
        task: String,
        /// Optional additional context.
        context: String,
    },
    /// A tool has requested restoring the previous agent from the stack.
    AgentRestoreRequested {
        /// Session in which the restore was requested.
        session_id: String,
        /// Summary/output from the sub-agent to pass back.
        summary: String,
    },
    /// The agent signalled that its current autonomous task is complete.
    TaskCompleted {
        /// Session in which task completion was signalled.
        session_id: String,
        /// Human-readable summary of what was accomplished.
        summary: String,
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
        /// Wall-clock time from request sent to stream complete, in milliseconds.
        elapsed_ms: u64,
        /// Number of tokens in the prompt/input for this response.
        input_tokens: u64,
        /// Number of tokens in the completion/output for this response.
        output_tokens: u64,
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
        /// The result content (or error text), possibly truncated for display.
        content: String,
        /// Total number of lines in the full (untruncated) result content.
        content_line_count: usize,
        /// Optional structured metadata from the tool (e.g. file counts, edit counts).
        metadata: Option<serde_json::Value>,
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
    /// Rate-limit / quota usage from a provider response.
    QuotaUpdate {
        /// Session this update belongs to.
        session_id: String,
        /// Quota consumed as a percentage (0.0–100.0).
        /// Derived from rate-limit response headers where available.
        percent: f32,
    },
    /// A session was aborted by the user or the server.
    SessionAborted {
        /// Identifier of the aborted session.
        session_id: String,
        /// Human-readable reason for the abort (e.g. `"user_requested"`).
        reason: String,
    },

    // ── Sub-agent lifecycle events (F13/F14) ────────────────────
    /// A sub-agent task has been spawned.
    SubagentStart {
        /// Parent session that spawned the task.
        session_id: String,
        /// Unique identifier for this task.
        task_id: String,
        /// Session created for the sub-agent.
        child_session_id: String,
        /// Name of the agent running the task (e.g. `"explore"`).
        agent: String,
        /// The task prompt sent to the sub-agent.
        task: String,
        /// Whether the task runs in the background (`true`) or blocks (`false`).
        background: bool,
    },
    /// A background sub-agent task has completed.
    SubagentComplete {
        /// Parent session that spawned the task.
        session_id: String,
        /// Unique identifier for this task.
        task_id: String,
        /// Session used by the sub-agent.
        child_session_id: String,
        /// Brief summary of the sub-agent's result.
        summary: String,
        /// Whether the sub-agent succeeded.
        success: bool,
        /// Wall-clock duration in milliseconds.
        duration_ms: u64,
    },
    /// A sub-agent task was cancelled.
    SubagentCancelled {
        /// Parent session that spawned the task.
        session_id: String,
        /// Unique identifier for the cancelled task.
        task_id: String,
    },

    // ── Team lifecycle events ────────────────────────────────────────────
    /// A new teammate session was spawned into a team.
    TeammateSpawned {
        /// Lead session ID.
        session_id: String,
        /// Name of the team.
        team_name: String,
        /// Human-friendly name of the new teammate.
        teammate_name: String,
        /// Agent ID assigned to this teammate (e.g. `"tm-001"`).
        agent_id: String,
    },
    /// A teammate sent a message that was delivered to the lead session.
    TeammateMessage {
        /// Lead session ID.
        session_id: String,
        /// Name of the team.
        team_name: String,
        /// Sender's agent ID or `"lead"`.
        from: String,
        /// Recipient's agent ID or `"lead"`.
        to: String,
        /// First 200 chars of message content (preview).
        preview: String,
    },
    /// A teammate reported idle state.
    TeammateIdle {
        /// Lead session ID.
        session_id: String,
        /// Name of the team.
        team_name: String,
        /// Agent ID of the idle teammate.
        agent_id: String,
    },
    /// A teammate failed after exhausting all retries.
    TeammateFailed {
        /// Lead session ID.
        session_id: String,
        /// Name of the team.
        team_name: String,
        /// Agent ID of the failed teammate.
        agent_id: String,
        /// Error description.
        error: String,
    },
    /// A teammate claimed a task from the shared task list.
    TeamTaskClaimed {
        /// Lead session ID.
        session_id: String,
        /// Name of the team.
        team_name: String,
        /// Agent ID that claimed the task.
        agent_id: String,
        /// ID of the claimed task.
        task_id: String,
    },
    /// A teammate completed a task.
    TeamTaskCompleted {
        /// Lead session ID.
        session_id: String,
        /// Name of the team.
        team_name: String,
        /// Agent ID that completed the task.
        agent_id: String,
        /// ID of the completed task.
        task_id: String,
    },
    /// A team was cleaned up (all resources removed).
    TeamCleanedUp {
        /// Lead session ID.
        session_id: String,
        /// Name of the team that was cleaned up.
        team_name: String,
    },
    /// A teammate sent a direct message to another teammate (peer-to-peer).
    ///
    /// Published instead of `TeammateMessage` when neither the sender nor the
    /// recipient is `"lead"`, so the lead and TUI are aware of cross-team
    /// communication without being in the loop.
    TeammateP2PMessage {
        /// Lead session ID.
        session_id: String,
        /// Name of the team.
        team_name: String,
        /// Sender's agent ID.
        from: String,
        /// Recipient's agent ID.
        to: String,
        /// First 200 chars of message content (preview).
        preview: String,
    },

    // ── LSP lifecycle events ─────────────────────────────────────────────
    /// An LSP server's connection status changed.
    LspStatusChanged {
        /// The server id as declared in `ragent.json` (e.g. `"rust"`).
        server_id: String,
        /// The new status.
        status: crate::lsp::LspStatus,
    },

    // ── Shell state events ───────────────────────────────────────────────
    /// The shell working directory changed after a bash command.
    ShellCwdChanged {
        /// Session this event belongs to.
        session_id: String,
        /// The new working directory path.
        cwd: String,
    },

    // ── User input events ────────────────────────────────────────────────
    /// The user submitted a free-text response to a `question` tool call.
    UserInput {
        /// Session this response belongs to.
        session_id: String,
        /// The request ID originally generated by the `question` tool call.
        request_id: String,
        /// The text the user typed.
        response: String,
    },
}

/// Broadcast-based event bus for distributing [`Event`] values to subscribers.
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Event>,
    /// Per-session step counters.
    ///
    /// Keyed by session ID. The value is the current loop step for that agent
    /// run. Using a shared `RwLock<HashMap>` means each clone of the bus sees
    /// the same counters — important because the processor and TUI hold
    /// different clones of the same bus.
    steps: Arc<RwLock<HashMap<String, u64>>>,
}

impl Event {
    /// Returns the variant name for use in log messages.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::SessionCreated { .. } => "SessionCreated",
            Self::SessionUpdated { .. } => "SessionUpdated",
            Self::MessageStart { .. } => "MessageStart",
            Self::TextDelta { .. } => "TextDelta",
            Self::ReasoningDelta { .. } => "ReasoningDelta",
            Self::ToolCallStart { .. } => "ToolCallStart",
            Self::ToolCallEnd { .. } => "ToolCallEnd",
            Self::MessageEnd { .. } => "MessageEnd",
            Self::PermissionRequested { .. } => "PermissionRequested",
            Self::PermissionReplied { .. } => "PermissionReplied",
            Self::AgentSwitched { .. } => "AgentSwitched",
            Self::AgentSwitchRequested { .. } => "AgentSwitchRequested",
            Self::AgentRestoreRequested { .. } => "AgentRestoreRequested",
            Self::TaskCompleted { .. } => "TaskCompleted",
            Self::AgentError { .. } => "AgentError",
            Self::McpStatusChanged { .. } => "McpStatusChanged",
            Self::TokenUsage { .. } => "TokenUsage",
            Self::ToolsSent { .. } => "ToolsSent",
            Self::ModelResponse { .. } => "ModelResponse",
            Self::ToolCallArgs { .. } => "ToolCallArgs",
            Self::ToolResult { .. } => "ToolResult",
            Self::CopilotDeviceFlowComplete { .. } => "CopilotDeviceFlowComplete",
            Self::SessionAborted { .. } => "SessionAborted",
            Self::QuotaUpdate { .. } => "QuotaUpdate",
            Self::SubagentStart { .. } => "SubagentStart",
            Self::SubagentComplete { .. } => "SubagentComplete",
            Self::SubagentCancelled { .. } => "SubagentCancelled",
            Self::TeammateSpawned { .. } => "TeammateSpawned",
            Self::TeammateMessage { .. } => "TeammateMessage",
            Self::TeammateIdle { .. } => "TeammateIdle",
            Self::TeammateFailed { .. } => "TeammateFailed",
            Self::TeamTaskClaimed { .. } => "TeamTaskClaimed",
            Self::TeamTaskCompleted { .. } => "TeamTaskCompleted",
            Self::TeamCleanedUp { .. } => "TeamCleanedUp",
            Self::TeammateP2PMessage { .. } => "TeammateP2PMessage",
            Self::LspStatusChanged { .. } => "LspStatusChanged",
            Self::ShellCwdChanged { .. } => "ShellCwdChanged",
            Self::UserInput { .. } => "UserInput",
        }
    }

    /// Returns the session ID carried by this event, if any.
    ///
    /// Infrastructure events (`McpStatusChanged`, `LspStatusChanged`,
    /// `CopilotDeviceFlowComplete`) are not scoped to a session and return
    /// `None`.
    #[must_use]
    pub const fn session_id(&self) -> Option<&str> {
        match self {
            Self::SessionCreated { session_id, .. }
            | Self::SessionUpdated { session_id, .. }
            | Self::MessageStart { session_id, .. }
            | Self::TextDelta { session_id, .. }
            | Self::ReasoningDelta { session_id, .. }
            | Self::ToolCallStart { session_id, .. }
            | Self::ToolCallEnd { session_id, .. }
            | Self::MessageEnd { session_id, .. }
            | Self::PermissionRequested { session_id, .. }
            | Self::PermissionReplied { session_id, .. }
            | Self::AgentSwitched { session_id, .. }
            | Self::AgentSwitchRequested { session_id, .. }
            | Self::AgentRestoreRequested { session_id, .. }
            | Self::TaskCompleted { session_id, .. }
            | Self::AgentError { session_id, .. }
            | Self::TokenUsage { session_id, .. }
            | Self::ToolsSent { session_id, .. }
            | Self::ModelResponse { session_id, .. }
            | Self::ToolCallArgs { session_id, .. }
            | Self::ToolResult { session_id, .. }
            | Self::SessionAborted { session_id, .. }
            | Self::QuotaUpdate { session_id, .. }
            | Self::SubagentStart { session_id, .. }
            | Self::SubagentComplete { session_id, .. }
            | Self::SubagentCancelled { session_id, .. }
            | Self::TeammateSpawned { session_id, .. }
            | Self::TeammateMessage { session_id, .. }
            | Self::TeammateIdle { session_id, .. }
            | Self::TeammateFailed { session_id, .. }
            | Self::TeamTaskClaimed { session_id, .. }
            | Self::TeamTaskCompleted { session_id, .. }
            | Self::TeamCleanedUp { session_id, .. }
            | Self::TeammateP2PMessage { session_id, .. } => Some(session_id.as_str()),
            Self::McpStatusChanged { .. }
            | Self::CopilotDeviceFlowComplete { .. }
            | Self::LspStatusChanged { .. } => None,
            Self::ShellCwdChanged { session_id, .. } | Self::UserInput { session_id, .. } => {
                Some(session_id.as_str())
            }
        }
    }
}

impl EventBus {
    /// Creates a new event bus with the given channel capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::event::EventBus;
    ///
    /// let bus = EventBus::new(128);
    /// ```
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            steps: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set the current step number for a specific agent session.
    ///
    /// Called by the session processor at the start of each loop iteration.
    /// Pass `0` to clear (reset) the counter for that session.
    pub fn set_step(&self, session_id: &str, step: u64) {
        let mut map = self.steps.write().expect("step map poisoned");
        if step == 0 {
            map.remove(session_id);
        } else {
            map.insert(session_id.to_string(), step);
        }
    }

    /// Returns the current step number for a specific agent session.
    ///
    /// Returns `0` if no step has been set for this session.
    #[must_use]
    pub fn current_step(&self, session_id: &str) -> u64 {
        self.steps
            .read()
            .expect("step map poisoned")
            .get(session_id)
            .copied()
            .unwrap_or(0)
    }

    /// Returns a new receiver that will observe all future events.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::event::EventBus;
    ///
    /// let bus = EventBus::new(64);
    /// let mut rx = bus.subscribe();
    /// // rx.recv().await will yield future events published to the bus.
    /// ```
    #[must_use]
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
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::event::{Event, EventBus};
    ///
    /// let bus = EventBus::new(64);
    /// let mut rx = bus.subscribe();
    ///
    /// bus.publish(Event::SessionCreated {
    ///     session_id: "sess-001".to_string(),
    /// });
    /// ```
    pub fn publish(&self, event: Event) {
        if self.sender.send(event.clone()).is_err() {
            // Build a "[agent_id:step]" tag when we have a session with an
            // active step counter; fall back to just the event type name.
            let tag = event.session_id().and_then(|sid| {
                let step = self.current_step(sid);
                if step > 0 {
                    // Use the last 8 chars of the session id as a short label.
                    let short_id = &sid[sid.len().saturating_sub(8)..];
                    Some(format!("[{short_id}:{step}]"))
                } else {
                    None
                }
            });
            if let Some(tag) = tag {
                tracing::warn!(
                    "Event dropped (no active subscribers) {}: {}",
                    tag,
                    event.type_name()
                );
            } else {
                tracing::warn!(
                    "Event dropped (no active subscribers): {}",
                    event.type_name()
                );
            }
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
            Self::Stop => write!(f, "stop"),
            Self::ToolUse => write!(f, "tool_use"),
            Self::Length => write!(f, "length"),
            Self::ContentFilter => write!(f, "content_filter"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}
