//! Per-session TUI state (spec MULTISESSION, milestone M2).
//!
//! [`SessionView`] holds the state that belongs to one session rather than to
//! the whole TUI. Today [`App`](super::App) owns exactly one view, the focused
//! session, in `App::session`; M3 keeps background sessions in a registry and
//! swaps them into that field on switch. Keeping the focused view as a plain
//! field (rather than behind an accessor) preserves disjoint field borrows
//! such as `self.session.messages` alongside other `App` fields.
//!
//! Fields move here group by group (see `docs/plans/MULTISESSION-M1.md`,
//! T1.1). Groups moved so far: S1 (transcript, scroll position and the
//! per-message render cache) and S2 (input line, draft, history cursor,
//! attachments and the input queue) and S3a (agent run lifecycle: session
//! id, active agent, processing/cancel/compaction flags and run timing) and
//! S3b (continuations that resume the session's run: plan approval and
//! delegation, rollback offer, reverse-spec chaining, `/spec impl`, role mode
//! and autopilot) and S4 (attention: queued permission and question
//! requests awaiting the user) and S5 (token usage, request stats, router
//! tier, the context-partition snapshot and the session's TODO cache) and S6
//! (the session's model, thinking level, provider health and execution
//! backend) and S7 (the team this session leads, its swarm, and the
//! selected agent and teammate).

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8};

use ragent_agent::agent::AgentInfo;
use ragent_agent::message::Message;
use ragent_agent::permission::PermissionRequest;
use ragent_agent::team::{SwarmState, TeamConfig, TeamMember};

use super::state::{
    ContextPartitionSnapshot, InputRenderCache, LlmRequestStat, MessageLineGroup,
    PendingForceCleanup, PlanApprovalState, QuestionRequest, QueuedInput, RoleMode,
    RollbackOfferState, SpecImplState,
};

/// State owned by one open session.
pub struct SessionView {
    /// Chat message history.
    pub messages: Vec<Message>,
    /// Scroll offset for the message view (lines from bottom).
    pub scroll_offset: u16,
    /// When true, the next assistant text delta starts a new message instead
    /// of appending to the current one. Set by `MessageEnd` events to
    /// separate init-exchange output from the main response.
    pub force_new_message: bool,
    /// Per-message cache of rendered `Line<'static>` groups, so `render_messages`
    /// can avoid re-rendering the entire timeline every frame.  Each entry
    /// holds the lines for one message; the cache is rebuilt incrementally
    /// when only the last message changes (streaming).
    pub message_line_cache: Vec<MessageLineGroup>,
    /// Terminal inner width when the `content_lines` and `wrapped_count`
    /// fields of [`message_line_cache`] were last computed.  When the width
    /// changes, all groups need re-wrapping (but not re-rendering).
    pub message_cache_width: u16,
    /// PERF-043: lowest message index whose cache group may be stale.
    ///
    /// Every site that mutates a message in place (the `Message::touch()`
    /// call sites) lowers this watermark to the mutated index, so the render
    /// path only has to scan `[message_cache_dirty_from, len)` for staleness
    /// instead of the whole transcript.  An idle frame performs zero
    /// iterations here.
    pub message_cache_dirty_from: usize,
    /// PERF-042: instant of the most recent throttled streaming re-render, or
    /// `None` when no re-render has been throttled yet.  While a message grows
    /// token-by-token its cache group is re-rendered at most once per
    /// `message_stream_min_interval`, bounding streaming CPU instead of
    /// re-parsing the whole reply on every token.
    pub message_stream_throttle_at: Option<std::time::Instant>,
    /// Plain-text lines from the last message pane render (for copy).
    pub message_content_lines: Vec<String>,
    /// PERF-041: `true` when `message_content_lines` no longer reflects the
    /// per-message wrapped rows and must be rebuilt before a copy reads it.
    /// Idle frames leave this `false`; a deferred streaming group, a width
    /// re-wrap, or a cache reconciliation sets it, and
    /// [`App::ensure_copy_content_lines`] rebuilds on demand.
    pub message_content_lines_dirty: bool,
    /// Maximum scroll value for the messages pane (set during render).
    pub message_max_scroll: u16,
    // ---- S2: input and draft ----
    /// Current text input buffer.
    pub input: String,
    /// Cursor position (character index) within the input line.
    pub input_cursor: usize,
    /// Keyboard selection anchor (character index). When `Some(n)`, the region
    /// between `n` and `input_cursor` forms the active keyboard selection.
    pub kb_select_anchor: Option<usize>,
    /// Current position when navigating history (`None` = new input).
    pub history_index: Option<usize>,
    /// Saved in-progress input while browsing history.
    pub history_draft: String,
    /// Image files staged to be sent with the next message (populated by Alt+V).
    pub pending_attachments: Vec<std::path::PathBuf>,
    /// PERF-046: last rendered chat-input rows, keyed on
    /// `(input text, cursor, keyboard selection, inner width)`.
    pub input_render_cache: InputRenderCache,
    /// The most recent prompt submitted by the user (chat message, slash
    /// command, bang command, or loop goal). Shown truncated in the status
    /// bar's top line.
    pub last_prompt: String,
    /// FIFO queue of messages submitted while the primary agent is executing
    /// (spec `inputqueue` FR-001): oldest entry first, bounded to
    /// [`MAX_INPUT_QUEUE`] entries (FR-004). Cleared whenever the session is
    /// reset and never persisted to disk (NFR-005).
    pub input_queue: VecDeque<QueuedInput>,
    /// Whether a queue-control `Next` selection is waiting to dispatch the oldest
    /// queued entry at the next turn boundary (spec `inputqueue` FR-024).
    ///
    /// A live turn cannot be dispatched over (FR-016), so selecting `Next` while
    /// the agent is executing cancels the turn and defers the dispatch here. The
    /// first turn boundary that can act consumes the flag and runs the oldest
    /// entry, which is why it fires exactly once. Cleared on session reset
    /// (NFR-005).
    pub queue_next_pending: bool,
    /// User message queued while auto-compaction runs: `(text, image_paths)`.
    pub pending_send_after_compact: Option<(String, Vec<std::path::PathBuf>)>,
    // ---- S3a: agent run lifecycle ----
    /// Current session identifier.
    pub session_id: Option<String>,
    /// Name of the active agent.
    pub agent_name: String,
    /// Resolved agent configuration.
    pub agent_info: AgentInfo,
    /// Index into `cycleable_agents` for the currently active agent.
    pub current_agent_index: usize,
    /// Saved agent stack for returning from sub-agents (e.g. plan -> general).
    pub agent_stack: Vec<AgentInfo>,
    /// Whether the agent is currently processing a message.
    pub is_processing: bool,
    /// Cancellation flag shared with the processor task; set to `true` on ESC.
    pub cancel_flag: Option<Arc<AtomicBool>>,
    /// Whether the last agent run was halted by the user (ESC).
    pub agent_halted: bool,
    /// True while an automatic pre-send compaction run is active.
    pub auto_compact_in_progress: bool,
    /// True while any compaction run (manual or auto) is active.
    /// Used to trigger message-history replacement when the LLM finishes.
    pub compact_in_progress: bool,
    /// True while a `/compress` compression pipeline is running.
    pub compress_in_progress: bool,
    /// Set when an auto-compaction run returns an error.
    pub auto_compact_failed: bool,
    /// Pending result from an async compaction call: the replacement history
    /// (`[compaction, ...recent]`) on success, or the error message.
    pub compact_result: Arc<std::sync::Mutex<Option<Result<Vec<Message>, String>>>>,
    /// Wall-clock instant when the current prompt was sent (for total elapsed time).
    pub prompt_start_time: Option<std::time::Instant>,
    /// Cumulative time spent in tool calls during this processing cycle.
    pub tool_time_ms: u64,
    /// Cumulative time spent waiting for LLM responses during this processing cycle.
    pub llm_time_ms: u64,
    /// Timestamp of the last `TaskCompleted` event received for this session.
    /// Used to suppress the autopilot auto-continue when the agent already
    /// signalled completion during the current turn.
    pub last_task_completed_at: Option<std::time::Instant>,
    /// Transient one-line run-complete banner shown after an agent run ends.
    ///
    /// Populated from `Event::RunCostSummary` and dismissed on the next
    /// keypress or after `RUN_COST_BANNER_EXPIRY_SECS` seconds. The full
    /// summary is always logged to the log panel regardless of this banner's
    /// visibility.
    pub run_cost_banner: Option<String>,
    /// When the run-cost banner was first shown, for auto-dismissal.
    pub run_cost_banner_at: Option<std::time::Instant>,
    /// Pending Alt+X stop-agent confirmation. When `true`, Enter confirms the
    /// halt and Esc keeps the agent running (nothing is stopped until Yes).
    pub pending_stop_confirm: bool,
    /// Cached `created_at` timestamp for the lead session, so the teams
    /// panel does not read `storage.get_session()` every frame (FR-009).
    pub lead_session_created_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Shell working directory as reported after each bash command.
    ///
    /// Updated by `ShellCwdChanged` events. `None` until the first bash
    /// command is executed in this session.
    pub shell_cwd: Option<String>,
    // ---- S3b: plan, rollback, spec and autopilot continuations ----
    /// Currently active role mode. None = normal (general-purpose) mode.
    pub role_mode: Option<RoleMode>,
    /// the agent to restore on approval.
    pub plan_approval_pending: Option<PlanApprovalState>,
    /// Pending plan delegation: `(task, context)` set by `AgentSwitchRequested`,
    /// consumed by `MessageEnd` to auto-send the task to the plan agent.
    pub pending_plan_task: Option<(String, String)>,
    /// Pending agent restore: summary from `AgentRestoreRequested`,
    /// consumed by `MessageEnd` to pop the agent stack and inject the summary.
    pub pending_plan_restore: Option<String>,
    /// Pending rollback offer after a terminated loop (FR-020, T-013).
    pub pending_rollback: Option<RollbackOfferState>,
    /// Pending result from the async loop-rollback task (FR-020/T-013):
    /// `Ok(true)` restored the pre-loop snapshot, `Ok(false)` there was no
    /// pending capture, `Err(reason)` the restore failed (capture kept for
    /// retry). Drained by `poll_rollback_result` on the UI thread; the
    /// spawned task only logs, so this slot is what keeps the UI status from
    /// being stuck at "rolling back..." forever.
    pub rollback_result: Arc<std::sync::Mutex<Option<Result<bool, String>>>>,
    /// Pending `/spec reverse --create <name>` chaining: the spec name is stored
    /// here before the LLM generation task spawns, and consumed by `MessageEnd`
    /// to invoke `/spec create <name> <generated-prompt>` once the LLM
    /// finishes (FR-012).
    pub pending_reverse_create: Option<String>,
    /// Pending `/spec reverse --folder <path> --create <name>` chaining: the
    /// scaffolded project folder, when one was scaffolded. Consumed by
    /// `MessageEnd` alongside [`Self::pending_reverse_create`] so the chained
    /// `/spec create` writes `specs/<name>/` inside the scaffolded project
    /// rather than the invoking directory (FR-027).
    pub pending_reverse_create_folder: Option<String>,
    /// Active `/spec impl` run, if any. Drives tasks one at a time: after each
    /// agent turn ends, the TUI checks the just-run task's status and, if
    /// completed, dispatches the next task's prompt.
    pub spec_impl_state: Option<SpecImplState>,
    /// Currently active spec ID for context injection.
    pub active_spec: Option<String>,
    /// True when autopilot mode is active. Agent continues autonomously until
    /// agent_complete is called, limits are hit, or the user runs /autopilot off.
    pub autopilot_enabled: bool,
    /// Maximum number of tokens to consume before stopping autopilot.
    pub autopilot_token_budget: Option<u64>,
    /// Maximum wall-clock seconds to run before stopping autopilot.
    pub autopilot_time_limit_secs: Option<u64>,
    /// Wall-clock instant when autopilot was started (for time-limit enforcement).
    pub autopilot_started_at: Option<std::time::Instant>,
    /// Pending autopilot continuation: when Some, the next render tick will
    /// auto-send this text to the agent to continue processing.
    pub autopilot_pending_continue: Option<String>,
    /// Set to `true` inside a single event-loop wake once an autopilot
    /// continuation has been dispatched, so multiple events arriving in the
    /// same drain cannot re-enter `dispatch_user_message`.
    pub autopilot_continued_this_wake: bool,
    // ---- S4: attention (pending permission and question requests) ----
    /// Queue of pending permission requests awaiting user resolution.
    /// The front of the queue is the currently displayed dialog; subsequent
    /// requests are shown one-at-a-time as earlier ones are resolved.
    pub permission_queue: VecDeque<PermissionRequest>,
    /// Queue of pending direct question requests awaiting user answers.
    pub question_queue: VecDeque<QuestionRequest>,
    /// Text typed by the user in response to the active question dialog.
    pub pending_question_input: String,
    /// Selected index for multiple-choice question dialogs.
    pub question_selected_index: usize,
    // ---- S5: usage, context snapshot and TODO cache ----
    /// Cumulative (input, output) token counts.
    pub token_usage: (u64, u64),
    /// Completed LLM request samples used to compute `/llmstats`.
    pub llm_request_stats: Vec<LlmRequestStat>,
    /// Input token count from the most recent LLM request (used for context-window % display).
    pub last_input_tokens: u64,
    /// Bytes received from the current LLM streaming response (reset per request).
    pub stream_in_bytes: u64,
    /// Bytes sent in the current LLM request payload (reset per request).
    pub stream_out_bytes: u64,
    /// Number of LLM security-analyzer verdicts published this session
    /// (spec `openhands` T-015; FR-006, FR-017). Displayed by `/security status`.
    pub security_verdicts: u64,
    /// The last tier selected by the router for the most recent request.
    /// `None` when no request has been routed yet or the router is not active.
    pub router_current_tier: Option<String>,
    /// The last downstream model selected by the router (format "provider:model"),
    /// for the most recent request.
    /// `None` when no request has been routed yet or the router is not active.
    pub router_current_model: Option<String>,
    /// Cached partition snapshot consumed by the Context panel render
    /// (T-013/FR-015: disk/SQLite-backed partitions are computed off the UI
    /// thread and adopted here, so per-frame renders never block).
    pub context_snapshot_cache: Option<ContextPartitionSnapshot>,
    /// Deposit box for background context-snapshot computations
    /// (T-013/FR-015). The blocking task writes its result here; the UI
    /// thread adopts it in `poll_context_snapshot_refresh`.
    pub context_snapshot_result: Arc<std::sync::Mutex<Option<ContextPartitionSnapshot>>>,
    /// Whether a context snapshot refresh is currently running off the UI
    /// thread (guards against stacking blocking tasks).
    pub context_refresh_inflight: bool,
    /// Cached task rows for the Tasks side panel so the render path does
    /// not query SQLite every frame.  Refreshed only when
    /// `tasks_cache_dirty` is `true` (set on task mutations and cleared
    /// after the panel reads the cache).  Stores the rows and the derived
    /// DAG so both expensive computations are amortised.
    pub tasks_cache_rows: Vec<ragent_storage::TaskRow>,
    /// `true` when `tasks_cache_rows` is stale and needs a SQLite refresh
    /// on the next `render_tasks_panel` call.
    pub tasks_cache_dirty: bool,
    // ---- S6: model selection ----
    /// Currently selected model in `"provider/model"` format, if any.
    pub selected_model: Option<String>,
    /// C-006: cached status-bar model label. Recomputed only when
    /// `selected_model` / `configured_provider` / thinking level changes
    /// (invalidated in `refresh_provider`, `finalize_model_selection`,
    /// `try_restore_provider_model`, `select_router_as_active`, and the
    /// thinking-level setter). `None` means "not yet computed".
    pub cached_provider_model_label: Option<String>,
    /// Context window size (tokens) for the currently selected model.
    /// Set during model selection; used when `resolve_model()` cannot find the model
    /// (e.g. dynamically discovered ollama/ollama_cloud models).
    pub selected_model_ctx_window: Option<usize>,
    /// Persisted thinking level for the currently selected model.
    pub selected_thinking_level: Option<ragent_types::ThinkingLevel>,
    /// Whether the configured provider/model is reachable.
    /// `0` = not yet checked, `1` = available, `2` = unavailable.
    pub provider_health: Arc<AtomicU8>,
    /// The active execution backend kind for this session (spec `openhands`
    /// FR-008, FR-019). Seeded from the loaded config at startup and updated by
    /// a `/backend` switch. The actual dispatch still resolves the backend from
    /// the per-turn config, so this is the surfaced label the TUI shows.
    pub active_backend: String,
    // ---- S7: team and swarm ----
    /// The currently active team config, if the lead is managing a team.
    pub active_team: Option<TeamConfig>,
    /// Current members of the active team (updated from events).
    pub team_members: Vec<TeamMember>,
    /// Per-teammate message counters: `agent_id -> (sent, received)`.
    pub team_message_counts: HashMap<String, (u32, u32)>,
    // -- Teams panel caches (FR-003, FR-009) ---------------------------------
    /// Cached per-agent task counts `(claimed, completed)` for the teams
    /// panel.  Updated by `TeamTaskClaimed` / `TeamTaskCompleted` events
    /// so the render path never reads the task store from disk (FR-009).
    pub team_task_counts: HashMap<String, (usize, usize)>,
    /// `true` when `team_task_counts` needs a one-shot disk refresh (set
    /// when a team is first opened).  The render path checks this flag,
    /// loads from disk once, then clears it so subsequent frames use the
    /// in-memory cache only.
    pub team_task_counts_dirty: bool,
    /// Currently focused teammate (agent_id). When set, the status
    /// bar shows a focus indicator and the input box routes messages
    /// to this teammate's mailbox instead of the lead session.
    pub focused_teammate: Option<String>,
    /// Active swarm state (if a /swarm is running).
    pub swarm_state: Option<SwarmState>,
    /// Pending result from an async `/swarm` LLM decomposition call.
    pub swarm_result: Arc<std::sync::Mutex<Option<Result<String, String>>>>,
    /// When swarm unblock was last polled (debounces filesystem I/O).
    pub swarm_unblock_last_poll: std::time::Instant,
    /// When swarm completion was last polled (debounces filesystem I/O).
    pub swarm_completion_last_poll: std::time::Instant,
    /// Session ID of the currently selected agent in the agents panel.
    /// When set, messages and logs are filtered to show only from this session.
    /// When `None`, shows primary session messages/logs.
    pub selected_agent_session_id: Option<String>,
    /// Pending confirmation for destructive force-cleanup modal.
    pub pending_forcecleanup: Option<PendingForceCleanup>,
    // ---- M3: background attention ----
    /// `true` when a run finished while this session was in the background
    /// and the user has not focused it since (spec MULTISESSION FR-04).
    pub unread: bool,
    /// `true` when a run failed while this session was in the background
    /// and the user has not focused it since (FR-04).
    pub errored: bool,
    // ---- M6: per-session working directory ----
    /// The directory this session works in (`sessions.directory`). Empty
    /// until the session is created or loaded; focusing the session makes it
    /// the TUI's `cwd_path` (spec MULTISESSION M6).
    pub directory: std::path::PathBuf,
}

/// What a session needs from the user, derived from its state
/// (spec MULTISESSION FR-04). Ordered from most to least urgent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attention {
    /// A permission request or question is waiting for the user.
    Waiting,
    /// The agent is running a turn.
    Running,
    /// A run failed while the session was in the background.
    Error,
    /// A run finished while the session was in the background.
    Unread,
    /// Nothing pending.
    Idle,
}

impl SessionView {
    /// Creates the view for a session that has not been created or resumed
    /// yet: empty transcript and input, no session id, idle run state.
    ///
    /// # Arguments
    ///
    /// * `agent_name` - display name of the active agent.
    /// * `agent_info` - resolved configuration of the active agent.
    /// * `current_agent_index` - index of that agent in the cycleable list.
    ///
    /// # Returns
    ///
    /// A view in its initial state for the given agent. No model is selected
    /// and `active_backend` is empty; the caller sets the session's model and
    /// backend (see `App::new`).
    pub fn new(agent_name: String, agent_info: AgentInfo, current_agent_index: usize) -> Self {
        Self {
            messages: Vec::new(),
            scroll_offset: 0,
            force_new_message: false,
            message_line_cache: Vec::new(),
            message_cache_width: 0,
            message_cache_dirty_from: 0,
            message_stream_throttle_at: None,
            message_content_lines: Vec::new(),
            message_content_lines_dirty: false,
            message_max_scroll: 0,
            input: String::new(),
            input_cursor: 0,
            kb_select_anchor: None,
            history_index: None,
            history_draft: String::new(),
            pending_attachments: Vec::new(),
            input_render_cache: InputRenderCache::default(),
            last_prompt: String::new(),
            input_queue: VecDeque::new(),
            queue_next_pending: false,
            pending_send_after_compact: None,
            session_id: None,
            agent_name,
            agent_info,
            current_agent_index,
            agent_stack: Vec::new(),
            is_processing: false,
            cancel_flag: None,
            agent_halted: false,
            auto_compact_in_progress: false,
            compact_in_progress: false,
            compress_in_progress: false,
            auto_compact_failed: false,
            compact_result: Arc::new(std::sync::Mutex::new(None)),
            prompt_start_time: None,
            tool_time_ms: 0,
            llm_time_ms: 0,
            last_task_completed_at: None,
            run_cost_banner: None,
            run_cost_banner_at: None,
            pending_stop_confirm: false,
            lead_session_created_at: None,
            shell_cwd: None,
            role_mode: None,
            plan_approval_pending: None,
            pending_plan_task: None,
            pending_plan_restore: None,
            pending_rollback: None,
            rollback_result: Arc::new(std::sync::Mutex::new(None)),
            pending_reverse_create: None,
            pending_reverse_create_folder: None,
            spec_impl_state: None,
            active_spec: None,
            autopilot_enabled: false,
            autopilot_token_budget: None,
            autopilot_time_limit_secs: None,
            autopilot_started_at: None,
            autopilot_pending_continue: None,
            autopilot_continued_this_wake: false,
            permission_queue: VecDeque::new(),
            question_queue: VecDeque::new(),
            pending_question_input: String::new(),
            question_selected_index: 0,
            token_usage: (0, 0),
            llm_request_stats: Vec::new(),
            last_input_tokens: 0,
            stream_in_bytes: 0,
            stream_out_bytes: 0,
            security_verdicts: 0,
            router_current_tier: None,
            router_current_model: None,
            context_snapshot_cache: None,
            context_snapshot_result: Arc::new(std::sync::Mutex::new(None)),
            context_refresh_inflight: false,
            tasks_cache_rows: Vec::new(),
            // Starts stale so the first Tasks-panel render loads from SQLite.
            tasks_cache_dirty: true,
            selected_model: None,
            cached_provider_model_label: None,
            selected_model_ctx_window: None,
            selected_thinking_level: None,
            provider_health: Arc::new(AtomicU8::new(0)),
            active_backend: String::new(),
            active_team: None,
            team_members: Vec::new(),
            team_message_counts: HashMap::new(),
            team_task_counts: HashMap::new(),
            team_task_counts_dirty: false,
            focused_teammate: None,
            swarm_state: None,
            swarm_result: Arc::new(std::sync::Mutex::new(None)),
            swarm_unblock_last_poll: std::time::Instant::now(),
            swarm_completion_last_poll: std::time::Instant::now(),
            selected_agent_session_id: None,
            pending_forcecleanup: None,
            unread: false,
            errored: false,
            directory: std::path::PathBuf::new(),
        }
    }

    /// Returns what this session needs from the user right now.
    ///
    /// # Returns
    ///
    /// [`Attention::Waiting`] while a permission request or question is
    /// queued, otherwise [`Attention::Running`] while a turn runs, otherwise
    /// [`Attention::Error`] when a background run failed unseen, otherwise
    /// [`Attention::Unread`] when a background run finished unseen, otherwise
    /// [`Attention::Idle`].
    pub fn attention(&self) -> Attention {
        if !self.permission_queue.is_empty() || !self.question_queue.is_empty() {
            Attention::Waiting
        } else if self.is_processing {
            Attention::Running
        } else if self.errored {
            Attention::Error
        } else if self.unread {
            Attention::Unread
        } else {
            Attention::Idle
        }
    }
}

impl std::fmt::Debug for SessionView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionView")
            .field("messages", &self.messages.len())
            .field("scroll_offset", &self.scroll_offset)
            .field("force_new_message", &self.force_new_message)
            .field("message_line_cache", &self.message_line_cache.len())
            .field("message_cache_width", &self.message_cache_width)
            .field("message_cache_dirty_from", &self.message_cache_dirty_from)
            .field("message_max_scroll", &self.message_max_scroll)
            .field("input_len", &self.input.len())
            .field("input_cursor", &self.input_cursor)
            .field("input_queue", &self.input_queue.len())
            .field("pending_attachments", &self.pending_attachments.len())
            .field("session_id", &self.session_id)
            .field("agent_name", &self.agent_name)
            .field("is_processing", &self.is_processing)
            .field("agent_halted", &self.agent_halted)
            .field("active_spec", &self.active_spec)
            .field("autopilot_enabled", &self.autopilot_enabled)
            .field("permission_queue", &self.permission_queue.len())
            .field("question_queue", &self.question_queue.len())
            .field("token_usage", &self.token_usage)
            .field("last_input_tokens", &self.last_input_tokens)
            .field("selected_model", &self.selected_model)
            .field("active_backend", &self.active_backend)
            .field("team_members", &self.team_members.len())
            .field("swarm_active", &self.swarm_state.is_some())
            .field("unread", &self.unread)
            .field("errored", &self.errored)
            .field("directory", &self.directory)
            .finish_non_exhaustive()
    }
}
