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

/// P-15: one tool call's lifecycle summary inside a [`Event::ToolCallBatch`].
///
/// Bundles the `ToolCallStart` + `ToolCallEnd` + `ToolResult` data for a
/// single tool call so consumers can render a whole step's tool calls
/// atomically without sorting racing per-call events by `call_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallBatchEntry {
    /// Provider-assigned call identifier.
    pub call_id: String,
    /// Name of the tool that was invoked.
    pub tool: String,
    /// Error message if the tool call failed, or `None` on success.
    pub error: Option<String>,
    /// Wall-clock execution time in milliseconds.
    pub duration_ms: u64,
    /// The result content (or error text), possibly truncated for display.
    pub content: String,
    /// Total number of lines in the full (untruncated) result content.
    pub content_line_count: usize,
    /// Optional structured metadata from the tool (e.g. file counts, edit counts).
    pub metadata: Option<serde_json::Value>,
    /// Whether the tool succeeded.
    pub success: bool,
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
        /// Optional multiple-choice options for the user.
        /// When present, the TUI renders a pick-list instead of a free-text
        /// input field.  The user selects one option and the chosen string is
        /// returned as the response.
        #[serde(default)]
        options: Vec<String>,
    },
    /// The user has replied to a permission request.
    PermissionReplied {
        /// Session the reply belongs to.
        session_id: String,
        /// The request id that was answered.
        request_id: String,
        /// Whether the user granted permission.
        allowed: bool,
        /// The decision type (Once, Always, or Deny).
        decision: crate::permission::PermissionDecision,
    },
    /// A tool is asking the user a direct question.
    QuestionRequested {
        /// Session making the request.
        session_id: String,
        /// Unique id for this question request (used in the reply).
        request_id: String,
        /// Human-readable question prompt.
        question: String,
        /// Optional multiple-choice options for the user.
        #[serde(default)]
        options: Vec<String>,
    },
    /// The user has answered a direct question request.
    QuestionAnswered {
        /// Session the answer belongs to.
        session_id: String,
        /// The request id that was answered.
        request_id: String,
        /// The selected or typed response.
        response: String,
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
    /// A recoverable notice occurred in the agent loop.
    AgentNotice {
        /// Session in which the notice occurred.
        session_id: String,
        /// Human-readable notice description.
        message: String,
    },
    /// An unrecoverable error occurred in the agentic loop.
    AgentError {
        /// Session in which the error occurred.
        session_id: String,
        /// Human-readable error description.
        error: String,
    },
    /// A local service (e.g. a keyless local provider's runtime) failed to
    /// start within the configured timeout.
    ///
    /// Carries structured diagnostics so the TUI can show a detailed error
    /// dialog with the command path and captured output.
    ServiceStartError {
        /// Session in which the error occurred.
        session_id: String,
        /// Name of the service (e.g. a local provider's runtime).
        service: String,
        /// Full path of the command that was run.
        command_path: String,
        /// Captured standard output from the command.
        stdout: String,
        /// Captured standard error from the command.
        stderr: String,
        /// Human-readable summary of the failure.
        error: String,
    },
    /// A context compression pipeline has started for a session.
    CompressionStarted {
        /// Session being compressed.
        session_id: String,
        /// Reason the compaction was initiated.
        #[serde(default)]
        reason: String,
    },
    /// A context compression pipeline has finished for a session.
    CompressionFinished {
        /// Session that was compressed.
        session_id: String,
        /// Number of tokens before compression.
        original_tokens: usize,
        /// Number of tokens after compression.
        compressed_tokens: usize,
        /// Compression ratio (original / compressed). 1.0 = no reduction.
        compression_ratio: f64,
        /// True when compression actually reduced token count.
        did_compress: bool,
        /// Reason the compaction was initiated.
        #[serde(default)]
        reason: String,
    },

    // ── Provider model-list loading (TUI spinner) ────────────────────────
    /// The TUI has started loading the model list for a provider.
    ProviderLoadingStarted {
        /// Provider identifier (e.g. `"ollama"`).
        provider_id: String,
        /// Human-readable provider name.
        provider_name: String,
    },
    /// The TUI finished loading the model list for a provider.
    ProviderLoadingFinished {
        /// Provider identifier.
        provider_id: String,
        /// Human-readable provider name.
        provider_name: String,
        /// Models discovered, serialized as JSON values.
        /// Empty when discovery failed or returned no models.
        models: Vec<serde_json::Value>,
        /// Error message if model discovery failed.
        error: Option<String>,
    },

    // ── Model Router classification logging ────────────────────────────
    /// The Model Router has classified a prompt and chosen a downstream
    /// model. Published so the TUI can log the bucket, model, prompt, and
    /// dimension scores regardless of the tracing filter level.
    RouterClassification {
        /// Session that triggered the classification.
        session_id: String,
        /// Selected routing tier (bucket) actually used to choose the model.
        /// When tier fallback is active this may differ from `requested_tier`.
        tier: String,
        /// The tier originally requested by the classifier (before fallback).
        /// `None` when the event is created by older codepaths that do not
        /// distinguish the requested and selected tiers.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        requested_tier: Option<String>,
        /// Selected downstream model as "provider:model".
        model: String,
        /// Composite weighted complexity score (0.0–1.0).
        composite_score: f64,
        /// Prompt text that was classified (after modifier stripping).
        prompt: String,
        /// Per-dimension scores above the reporting threshold (0.05).
        /// Zeroed or negligible dimensions are excluded to keep the log
        /// panel concise and consistent with the tracing-based summary.
        dimensions: Vec<(String, f64)>,
    },

    // ── Model download progress (e.g. local providers) ─────────────────
    /// A local provider started downloading a model.
    ModelDownloadStarted {
        /// Provider identifier.
        provider_id: String,
        /// Model identifier being downloaded.
        model_id: String,
        /// Session that triggered the download.
        session_id: String,
    },
    /// Progress update while a local provider downloads a model.
    ModelDownloadProgress {
        /// Provider identifier.
        provider_id: String,
        /// Model identifier being downloaded.
        model_id: String,
        /// Session that triggered the download.
        session_id: String,
        /// Download progress from 0.0 to 100.0.
        percent: f32,
    },
    /// A local provider finished downloading a model.
    ModelDownloadFinished {
        /// Provider identifier.
        provider_id: String,
        /// Model identifier that was downloaded.
        model_id: String,
        /// Session that triggered the download.
        session_id: String,
        /// Error message if the download failed.
        error: Option<String>,
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
    /// Estimated cost summary for a completed agent run.
    ///
    /// Published once at the end of a `process_user_message` turn so the TUI
    /// and HTTP consumers can surface per-run spend without having to
    /// accumulate per-request usage events themselves.
    RunCostSummary {
        /// Session the summary belongs to.
        session_id: String,
        /// Model identifier that produced the usage.
        model_id: String,
        /// Total input (prompt) tokens across all LLM requests in the run.
        input_tokens: u64,
        /// Total output (completion) tokens across all LLM requests in the run.
        output_tokens: u64,
        /// Estimated total cost in USD.
        total_cost_usd: f64,
        /// Wall-clock duration of the run in milliseconds.
        duration_ms: u64,
    },
    /// A new LLM request has been sent, including its serialized outbound size.
    RequestStarted {
        /// Session this request belongs to.
        session_id: String,
        /// Approximate serialized request payload size in bytes.
        outbound_bytes: u64,
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
    /// P-15: an atomic batch of per-tool-call lifecycle events for one loop
    /// step.
    ///
    /// In `parallel_tool_calls` mode the per-call `ToolCallStart` /
    /// `ToolCallEnd` / `ToolResult` events race and the TUI must sort them
    /// by `call_id`. This variant delivers the complete set of tool calls
    /// for a step in a single publication so consumers can render them
    /// atomically. The per-call events are still published as a fallback
    /// (per PERFPLAN Milestone D risk note) until the TUI/HTTP server are
    /// proven on the batch variant.
    ToolCallBatch {
        /// Session this batch belongs to.
        session_id: String,
        /// Step number within the turn (1-based).
        step: u64,
        /// One entry per tool call in this step, in dispatch order.
        calls: Vec<ToolCallBatchEntry>,
    },
    /// The Copilot device flow completed successfully.
    CopilotDeviceFlowComplete {
        /// The GitHub OAuth token obtained from the device flow.
        token: String,
        /// The plan-specific API base URL discovered during setup.
        api_base: String,
    },
    /// Result of an async GitLab token validation started by `/gitlab setup`.
    GitLabSetupComplete {
        /// Whether validation and credential save succeeded.
        success: bool,
        /// Error message, if any.
        error: Option<String>,
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
    /// A sub-agent task was suspended by the user.
    SubagentSuspended {
        /// Parent session that spawned the task.
        session_id: String,
        /// Unique identifier for the suspended task.
        task_id: String,
        /// Session used by the sub-agent.
        child_session_id: String,
    },
    /// A sub-agent task was resumed by the user.
    SubagentResumed {
        /// Parent session that spawned the task.
        session_id: String,
        /// Unique identifier for the resumed task.
        task_id: String,
        /// Session used by the sub-agent.
        child_session_id: String,
    },
    /// A sub-agent task was killed by the user.
    SubagentKilled {
        /// Parent session that spawned the task.
        session_id: String,
        /// Unique identifier for the killed task.
        task_id: String,
        /// Session used by the sub-agent.
        child_session_id: String,
        /// Whether the kill was a force-kill after timeout.
        force: bool,
    },
    /// A sub-agent task was cancelled.
    SubagentCancelled {
        /// Parent session that spawned the task.
        session_id: String,
        /// Unique identifier for the cancelled task.
        task_id: String,
    },

    // ── Background shell task events (M3) ────────────────────────────────
    /// A background shell task was spawned.
    BackgroundTaskSpawned {
        /// Session this task belongs to.
        session_id: String,
        /// Unique identifier for this background task.
        task_id: String,
        /// The shell command being executed.
        command: String,
    },
    /// A background shell task emitted progress, output, or a status change.
    BackgroundTaskUpdated {
        /// Session this task belongs to.
        session_id: String,
        /// Unique identifier for this background task.
        task_id: String,
        /// Current status: `running`, `completed`, `failed`, `cancelled`.
        status: String,
        /// Optional parsed `JCODE_PROGRESS` payload.
        #[serde(default)]
        progress: Option<serde_json::Value>,
    },
    /// A background shell task finished.
    BackgroundTaskCompleted {
        /// Session this task belongs to.
        session_id: String,
        /// Unique identifier for this background task.
        task_id: String,
        /// Final status: `completed`, `failed`, or `cancelled`.
        status: String,
        /// Exit code, if the process exited.
        #[serde(default)]
        exit_code: Option<i32>,
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
    ///
    /// M5-T6: `message_type` carries the `snake_case` `MessageType` so event
    /// consumers can distinguish a plan approval from a broadcast without
    /// parsing the preview.
    TeammateMessage {
        /// Lead session ID.
        session_id: String,
        /// Name of the team.
        team_name: String,
        /// Sender's agent ID or `"lead"`.
        from: String,
        /// Recipient's agent ID or `"lead"`.
        to: String,
        /// `Snake_case` message type (e.g. `"message"`, `"plan_approved"`,
        /// `"broadcast"`).
        message_type: String,
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
    /// A teammate was suspended (paused) by the lead.
    TeammateSuspended {
        /// Lead session ID.
        session_id: String,
        /// Name of the team.
        team_name: String,
        /// Agent ID of the suspended teammate.
        agent_id: String,
    },
    /// A previously suspended teammate was resumed by the lead.
    TeammateResumed {
        /// Lead session ID.
        session_id: String,
        /// Name of the team.
        team_name: String,
        /// Agent ID of the resumed teammate.
        agent_id: String,
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
        /// `Snake_case` message type (M5-T6).
        message_type: String,
        /// First 200 chars of message content (preview).
        preview: String,
    },

    // ── Shell state events ───────────────────────────────────────────────
    /// The shell working directory changed after a bash command.
    ShellCwdChanged {
        /// Session this event belongs to.
        session_id: String,
        /// The new working directory path.
        cwd: String,
    },

    /// Open the `/research open` markdown viewer in the TUI.
    OpenResearchView {
        /// Research item name displayed in the title.
        name: String,
        /// Absolute path to the RESEARCH.md file.
        path: std::path::PathBuf,
        /// Markdown body (frontmatter already stripped).
        markdown: String,
    },

    // ── User input events ────────────────────────────────────────────────
    /// The user submitted generic free-text input to the running session.
    UserInput {
        /// Session this response belongs to.
        session_id: String,
        /// The request ID originally generated by the `question` tool call.
        request_id: String,
        /// The text the user typed.
        response: String,
    },

    /// A structured memory was stored.
    MemoryStored {
        /// Session that stored the memory.
        session_id: String,
        /// Row ID of the new memory.
        id: i64,
        /// Category of the memory.
        category: String,
    },
    /// A structured memory search was performed.
    MemoryRecalled {
        /// Session that performed the search.
        session_id: String,
        /// The search query used.
        query: String,
        /// Number of results returned.
        result_count: usize,
    },
    /// Structured memories were deleted.
    MemoryForgotten {
        /// Session that triggered the deletion.
        session_id: String,
        /// Number of memories deleted.
        count: usize,
    },
    /// A semantic or keyword memory search was performed.
    MemorySearched {
        /// Session that performed the search.
        session_id: String,
        /// The search query used.
        query: String,
        /// Number of results returned.
        result_count: usize,
        /// Search mode used: "semantic" or "fts".
        mode: String,
    },
    /// A current-session conversation search was performed.
    ConversationSearched {
        /// Session that performed the search.
        session_id: String,
        /// The search query or mode descriptor.
        query: String,
        /// Number of results returned.
        result_count: usize,
        /// Search mode used: "keyword", "turn_range", or "stats".
        mode: String,
    },
    /// A cross-session search was performed.
    SessionSearched {
        /// Session that performed the search.
        session_id: String,
        /// The search query used.
        query: String,
        /// Number of results returned.
        result_count: usize,
        /// Search mode used: "keyword" or "semantic".
        mode: String,
    },
    /// A `PreToolUse` or `PostToolUse` hook exited with code 1, signalling a
    /// warning but not a block.
    HookWarning {
        /// Session that triggered the hook.
        session_id: String,
        /// Hook command that produced the warning.
        hook_command: String,
        /// Name of the tool being checked.
        tool: String,
        /// Trimmed stderr from the hook (capped at 500 characters).
        stderr: String,
    },
    /// A `PostToolUse` hook exited with code 2, flagging the tool result as
    /// policy-violated.
    ToolResultFlagged {
        /// Session that triggered the flag.
        session_id: String,
        /// Name of the tool whose result was flagged.
        tool: String,
        /// Hook command that produced the flag.
        hook_command: String,
        /// Trimmed stderr from the hook (capped at 500 characters).
        reason: String,
    },
    /// A memory candidate was extracted automatically (requires confirmation).
    MemoryCandidateExtracted {
        /// Session that triggered the extraction.
        session_id: String,
        /// Proposed memory content.
        content: String,
        /// Category of the candidate.
        category: String,
        /// Tags for the candidate.
        tags: Vec<String>,
        /// Confidence score.
        confidence: f64,
        /// Source of the extraction.
        source: String,
        /// Why this was extracted.
        reason: String,
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
            Self::QuestionRequested { .. } => "QuestionRequested",
            Self::QuestionAnswered { .. } => "QuestionAnswered",
            Self::AgentSwitched { .. } => "AgentSwitched",
            Self::AgentSwitchRequested { .. } => "AgentSwitchRequested",
            Self::AgentRestoreRequested { .. } => "AgentRestoreRequested",
            Self::TaskCompleted { .. } => "TaskCompleted",
            Self::AgentNotice { .. } => "AgentNotice",
            Self::AgentError { .. } => "AgentError",
            Self::ServiceStartError { .. } => "ServiceStartError",
            Self::McpStatusChanged { .. } => "McpStatusChanged",
            Self::TokenUsage { .. } => "TokenUsage",
            Self::RunCostSummary { .. } => "RunCostSummary",
            Self::RequestStarted { .. } => "RequestStarted",
            Self::ToolsSent { .. } => "ToolsSent",
            Self::ModelResponse { .. } => "ModelResponse",
            Self::ToolCallArgs { .. } => "ToolCallArgs",
            Self::ToolResult { .. } => "ToolResult",
            Self::ToolCallBatch { .. } => "ToolCallBatch",
            Self::CopilotDeviceFlowComplete { .. } => "CopilotDeviceFlowComplete",
            Self::GitLabSetupComplete { .. } => "GitLabSetupComplete",
            Self::SessionAborted { .. } => "SessionAborted",
            Self::QuotaUpdate { .. } => "QuotaUpdate",
            Self::SubagentStart { .. } => "SubagentStart",
            Self::SubagentComplete { .. } => "SubagentComplete",
            Self::SubagentSuspended { .. } => "SubagentSuspended",
            Self::SubagentResumed { .. } => "SubagentResumed",
            Self::SubagentKilled { .. } => "SubagentKilled",
            Self::SubagentCancelled { .. } => "SubagentCancelled",
            Self::BackgroundTaskSpawned { .. } => "BackgroundTaskSpawned",
            Self::BackgroundTaskUpdated { .. } => "BackgroundTaskUpdated",
            Self::BackgroundTaskCompleted { .. } => "BackgroundTaskCompleted",
            Self::TeammateSpawned { .. } => "TeammateSpawned",
            Self::TeammateMessage { .. } => "TeammateMessage",
            Self::TeammateIdle { .. } => "TeammateIdle",
            Self::TeammateFailed { .. } => "TeammateFailed",
            Self::TeammateSuspended { .. } => "TeammateSuspended",
            Self::TeammateResumed { .. } => "TeammateResumed",
            Self::TeamTaskClaimed { .. } => "TeamTaskClaimed",
            Self::TeamTaskCompleted { .. } => "TeamTaskCompleted",
            Self::TeamCleanedUp { .. } => "TeamCleanedUp",
            Self::TeammateP2PMessage { .. } => "TeammateP2PMessage",
            Self::ShellCwdChanged { .. } => "ShellCwdChanged",
            Self::OpenResearchView { .. } => "OpenResearchView",
            Self::UserInput { .. } => "UserInput",
            Self::MemoryStored { .. } => "MemoryStored",
            Self::MemoryRecalled { .. } => "MemoryRecalled",
            Self::MemoryForgotten { .. } => "MemoryForgotten",
            Self::MemorySearched { .. } => "MemorySearched",
            Self::ConversationSearched { .. } => "ConversationSearched",
            Self::SessionSearched { .. } => "SessionSearched",
            Self::MemoryCandidateExtracted { .. } => "MemoryCandidateExtracted",
            Self::HookWarning { .. } => "HookWarning",
            Self::ToolResultFlagged { .. } => "ToolResultFlagged",
            Self::CompressionStarted { .. } => "CompressionStarted",
            Self::CompressionFinished { .. } => "CompressionFinished",
            Self::ProviderLoadingStarted { .. } => "ProviderLoadingStarted",
            Self::ProviderLoadingFinished { .. } => "ProviderLoadingFinished",
            Self::RouterClassification { .. } => "RouterClassification",
            Self::ModelDownloadStarted { .. } => "ModelDownloadStarted",
            Self::ModelDownloadProgress { .. } => "ModelDownloadProgress",
            Self::ModelDownloadFinished { .. } => "ModelDownloadFinished",
        }
    }

    /// Returns the session ID carried by this event, if any.
    ///
    /// Infrastructure events (`McpStatusChanged`, `CopilotDeviceFlowComplete`,
    /// `GitLabSetupComplete`) are not scoped to a session and return `None`.
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
            | Self::QuestionRequested { session_id, .. }
            | Self::QuestionAnswered { session_id, .. }
            | Self::AgentSwitched { session_id, .. }
            | Self::AgentSwitchRequested { session_id, .. }
            | Self::AgentRestoreRequested { session_id, .. }
            | Self::TaskCompleted { session_id, .. }
            | Self::AgentNotice { session_id, .. }
            | Self::AgentError { session_id, .. }
            | Self::ServiceStartError { session_id, .. }
            | Self::TokenUsage { session_id, .. }
            | Self::RunCostSummary { session_id, .. }
            | Self::RequestStarted { session_id, .. }
            | Self::ToolsSent { session_id, .. }
            | Self::ModelResponse { session_id, .. }
            | Self::ToolCallArgs { session_id, .. }
            | Self::ToolResult { session_id, .. }
            | Self::ToolCallBatch { session_id, .. }
            | Self::SessionAborted { session_id, .. }
            | Self::QuotaUpdate { session_id, .. }
            | Self::SubagentStart { session_id, .. }
            | Self::SubagentComplete { session_id, .. }
            | Self::SubagentSuspended { session_id, .. }
            | Self::SubagentResumed { session_id, .. }
            | Self::SubagentKilled { session_id, .. }
            | Self::SubagentCancelled { session_id, .. }
            | Self::BackgroundTaskSpawned { session_id, .. }
            | Self::BackgroundTaskUpdated { session_id, .. }
            | Self::BackgroundTaskCompleted { session_id, .. }
            | Self::TeammateSpawned { session_id, .. }
            | Self::TeammateMessage { session_id, .. }
            | Self::TeammateIdle { session_id, .. }
            | Self::TeammateFailed { session_id, .. }
            | Self::TeammateSuspended { session_id, .. }
            | Self::TeammateResumed { session_id, .. }
            | Self::TeamTaskClaimed { session_id, .. }
            | Self::TeamTaskCompleted { session_id, .. }
            | Self::TeamCleanedUp { session_id, .. }
            | Self::TeammateP2PMessage { session_id, .. } => Some(session_id.as_str()),
            Self::McpStatusChanged { .. }
            | Self::CopilotDeviceFlowComplete { .. }
            | Self::GitLabSetupComplete { .. } => None,
            Self::ShellCwdChanged { session_id, .. } | Self::UserInput { session_id, .. } => {
                Some(session_id.as_str())
            }
            Self::OpenResearchView { .. } => None,
            Self::MemoryStored { session_id, .. }
            | Self::MemoryRecalled { session_id, .. }
            | Self::MemoryForgotten { session_id, .. }
            | Self::MemorySearched { session_id, .. }
            | Self::ConversationSearched { session_id, .. }
            | Self::SessionSearched { session_id, .. }
            | Self::MemoryCandidateExtracted { session_id, .. }
            | Self::HookWarning { session_id, .. }
            | Self::ToolResultFlagged { session_id, .. }
            | Self::ModelDownloadStarted { session_id, .. }
            | Self::ModelDownloadProgress { session_id, .. }
            | Self::ModelDownloadFinished { session_id, .. } => Some(session_id.as_str()),
            Self::CompressionStarted { session_id, .. }
            | Self::CompressionFinished { session_id, .. } => Some(session_id.as_str()),
            Self::ProviderLoadingStarted { .. } | Self::ProviderLoadingFinished { .. } => None,
            Self::RouterClassification { session_id, .. } => Some(session_id.as_str()),
        }
    }
}

impl EventBus {
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_types::event::EventBus;
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
        let mut map = self
            .steps
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let map = self
            .steps
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.get(session_id).copied().unwrap_or(0)
    }

    /// Returns a new receiver that will observe all future events.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_types::event::EventBus;
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
    /// The underlying broadcast channel has a fixed-size buffer (1024 by default).
    /// When the buffer is full, the oldest events are dropped and slow subscribers
    /// will receive a `Lagged` error on their next `recv()`.  A warning is emitted
    /// when events are dropped due to a full buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_types::event::{Event, EventBus};
    ///
    /// let bus = EventBus::new(64);
    /// let mut rx = bus.subscribe();
    ///
    /// bus.publish(Event::SessionCreated {
    ///     session_id: "sess-001".to_string(),
    /// });
    /// ```
    pub fn publish(&self, event: Event) {
        match self.sender.send(event.clone()) {
            Ok(n) => {
                // n = number of receivers that got the event
                if n == 0 {
                    let tag = event.session_id().and_then(|sid| {
                        let step = self.current_step(sid);
                        if step > 0 {
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
            Err(broadcast::error::SendError(ev)) => {
                // Buffer overflow — some receivers are lagging
                tracing::warn!("Event dropped (broadcast channel full): {}", ev.type_name());
            }
        }
    }
}

impl Default for EventBus {
    /// Creates an `EventBus` with a default capacity of 1024 events.
    fn default() -> Self {
        Self::new(1024)
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
