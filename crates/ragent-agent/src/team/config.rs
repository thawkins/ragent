//! Team configuration types: `TeamConfig`, `TeamMember`, `TeamStatus`, `MemberStatus`.
//!
//! These types are serialised to/from `config.json` inside the team directory.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::agent::ModelRef;

/// Persistent memory scope for an agent or teammate.
///
/// When set, the agent receives a dedicated memory directory where it can
/// persist notes, findings, and context across sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MemoryScope {
    /// No persistent memory (default).
    #[default]
    None,
    /// User-global: `~/.ragent/agent-memory/<agent-name>/`.
    User,
    /// Project-local: `<project>/.ragent/agent-memory/<agent-name>/`.
    Project,
}

impl MemoryScope {
    /// Return the snake_case string used for serialization.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::User => "user",
            Self::Project => "project",
        }
    }
}

/// Resolve the memory directory for a given agent name and scope.
///
/// Returns `None` when `scope` is [`MemoryScope::None`].
/// The directory is **not** created - callers should create it on first write.
#[must_use]
pub fn resolve_memory_dir(
    scope: MemoryScope,
    agent_name: &str,
    working_dir: &Path,
) -> Option<PathBuf> {
    // SEC-ragent-team-007 (SECTASKS T-066): `agent_name` reaches this function
    // from team/agent configuration, and `Path::join` would honour an absolute
    // component or a `..` escape, writing the memory file outside the intended
    // directory. Accept only a single safe path component.
    if !is_safe_memory_component(agent_name) {
        tracing::warn!(
            agent_name,
            "refusing to resolve a memory directory for an unsafe agent name"
        );
        return Option::None;
    }
    match scope {
        MemoryScope::None => Option::None,
        MemoryScope::User => Some(
            ragent_config::user_dirs::global_agent_memory_dir()
                .map(|d| d.join(agent_name))
                .unwrap_or_else(|| PathBuf::from(".").join("agent-memory").join(agent_name)),
        ),
        MemoryScope::Project => Some(
            working_dir
                .join(".ragent")
                .join("agent-memory")
                .join(agent_name),
        ),
    }
}

/// Whether `name` is a single safe path component (`[A-Za-z0-9._-]`, not `.`/`..`).
fn is_safe_memory_component(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Overall lifecycle state of a team.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TeamStatus {
    /// Team has been created and is ready to accept teammates.
    #[default]
    Active,
    /// All work is complete; team is being cleaned up.
    Completed,
    /// Team was explicitly disbanded; cleanup has finished.
    Disbanded,
}

impl TeamStatus {
    /// Return the canonical snake_case string (M5-T1).
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Disbanded => "disbanded",
        }
    }
}

/// Lifecycle state of an individual teammate session.
///
/// Serialises to snake_case via `#[serde(rename_all = "lowercase")]` and via
/// [`MemberStatus::as_str`] (M5-T1). All output paths produce the same
/// snake_case string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MemberStatus {
    /// Session created but spawn not yet confirmed.
    #[default]
    Spawning,
    /// Actively processing a task.
    Working,
    /// Waiting for a task or message (no current task).
    Idle,
    /// Submitted a plan; awaiting lead approval.
    PlanPending,
    /// Waiting for dependency tasks to complete before spawning.
    Blocked,
    /// Paused by the lead; can be resumed later.
    Suspended,
    /// Graceful shutdown in progress (awaiting `team_shutdown_ack`).
    ShuttingDown,
    /// Session has terminated.
    Stopped,
    /// Spawn or startup failed; see `last_spawn_error` for details.
    Failed,
}

impl MemberStatus {
    /// Every canonical wire token, in lifecycle order (spec `teamssetup`
    /// FR-032, FR-033).
    ///
    /// A refusal that rejects a token names the tokens that would have worked,
    /// and it takes that list from here rather than re-typing it, so adding a
    /// status cannot leave the refusal advertising a stale set.
    pub const ALL_TOKENS: &'static [&'static str] = &[
        "spawning",
        "working",
        "idle",
        "plan_pending",
        "blocked",
        "suspended",
        "shutting_down",
        "stopped",
        "failed",
    ];

    /// Return the canonical snake_case string used for serialization, tool
    /// output, and SSE (M5-T1).
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Spawning => "spawning",
            Self::Working => "working",
            Self::Idle => "idle",
            Self::PlanPending => "plan_pending",
            Self::Blocked => "blocked",
            Self::Suspended => "suspended",
            Self::ShuttingDown => "shutting_down",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
        }
    }

    /// Parse a canonical snake_case status token back into a [`MemberStatus`].
    ///
    /// The inverse of [`Self::as_str`], for the callers that can only exchange
    /// tokens: the type-erased `TeamManagerInterface` a team tool holds is
    /// defined in a module that must not depend on this one, so a tool names the
    /// destination status as its canonical token and the manager parses it here
    /// (spec `teamssetup` FR-008, FR-026).
    ///
    /// # Returns
    ///
    /// The status, or `None` when `token` is not one of the canonical tokens.
    #[must_use]
    pub fn from_wire(token: &str) -> Option<Self> {
        match token {
            "spawning" => Some(Self::Spawning),
            "working" => Some(Self::Working),
            "idle" => Some(Self::Idle),
            "plan_pending" => Some(Self::PlanPending),
            "blocked" => Some(Self::Blocked),
            "suspended" => Some(Self::Suspended),
            "shutting_down" => Some(Self::ShuttingDown),
            "stopped" => Some(Self::Stopped),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }

    /// Return the compact status word the UI panels paint for this state
    /// (spec `agentui` FR-023, NFR-009).
    ///
    /// This is the single canonical status-word mapping: every panel takes its
    /// wording from here, so no surface can disagree with another about what a
    /// teammate is doing. The words match [`MemberStatus::as_str`] except where
    /// the snake_case spelling is too wide for a table column
    /// (`plan_pending` -> `planning`, `shutting_down` -> `stopping`).
    #[must_use]
    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Spawning => "spawning",
            Self::Working => "working",
            Self::Idle => "idle",
            Self::PlanPending => "planning",
            Self::Blocked => "blocked",
            Self::Suspended => "suspended",
            Self::ShuttingDown => "stopping",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
        }
    }

    /// Whether this status is a terminal resting state with no session behind
    /// it (spec `teamssetup` FR-009, D2).
    ///
    /// A terminal member is finished: it holds no session, will run no further
    /// work, and only a respawn can revive it. `ShuttingDown` is deliberately
    /// **not** terminal - the session still exists and is waiting on
    /// `team_shutdown_ack`, so a member in that state must still be waited on
    /// and must not be treated as gone.
    ///
    /// Guards that ask "is this member finished?" take this predicate rather
    /// than spelling out `Stopped | Failed`.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Stopped | Self::Failed)
    }

    /// Whether this status is a parked state: the session is alive but is not
    /// yet attached to work (spec `teamssetup` FR-009, D2).
    ///
    /// A parked member is stopped short of progress by something external: it is
    /// waiting for a task or message (`Idle`), a plan approval (`PlanPending`),
    /// an unsatisfied dependency (`Blocked`), or the lead (`Suspended`). It is
    /// therefore neither progressing nor finished, and time spent parked is not
    /// evidence of a wedge: FR-012 requires the watchdog to skip it, and FR-031
    /// requires `team_wait` to report it distinctly from a still-running member.
    ///
    /// `Idle` belongs here rather than with the live states because a teammate
    /// with no work cannot be wedged; the watchdog advances it with an
    /// assignment or lets `team_wait` return, and neither action needs a
    /// timeout to rescue it.
    #[must_use]
    pub fn is_parked(&self) -> bool {
        matches!(
            self,
            Self::Idle | Self::PlanPending | Self::Blocked | Self::Suspended
        )
    }

    /// Whether this status is a live state in which the member is expected to
    /// make progress on work it already holds (spec `teamssetup` FR-009, D2).
    ///
    /// `Spawning`, `Working`, and `ShuttingDown` are live: each has a session
    /// with an outstanding action it must complete on its own (finish starting,
    /// finish the task, or acknowledge shutdown). This is the only predicate the
    /// watchdog may use to decide a member is eligible for a failure verdict
    /// (FR-012).
    ///
    /// Every status is exactly one of `is_terminal`, `is_parked`, or `is_live`.
    #[must_use]
    pub fn is_live(&self) -> bool {
        matches!(self, Self::Spawning | Self::Working | Self::ShuttingDown)
    }

    /// Whether a member may move from this status to `to` (spec `teamssetup`
    /// FR-003, FR-008, FR-009).
    ///
    /// This is the single legal-transition table for [`MemberStatus`]; every
    /// status write validates against it before mutating a member, so a
    /// reachable-but-unleaveable state cannot be introduced by accident.
    ///
    /// The legal transitions are:
    ///
    /// | From | May become |
    /// | --- | --- |
    /// | `Spawning` | `Working`, `Failed` |
    /// | `Working` | `Idle`, `PlanPending`, `Blocked`, `Suspended`, `ShuttingDown`, `Failed` |
    /// | `Idle` | `Working`, `Blocked`, `Suspended`, `ShuttingDown`, `Failed` |
    /// | `PlanPending` | `Working`, `Idle`, `Failed` |
    /// | `Blocked` | `Idle`, `Suspended`, `ShuttingDown`, `Failed` |
    /// | `Suspended` | `Idle`, `ShuttingDown`, `Failed` |
    /// | `ShuttingDown` | `Stopped`, `Failed` |
    /// | `Stopped` | `Spawning` (respawn only) |
    /// | `Failed` | `Spawning` (respawn only) |
    ///
    /// A transition to the same status is *not* legal: re-asserting a status is
    /// not a state change, and allowing it would let a writer claim it published
    /// an event when nothing moved. A self-transition must be filtered by the
    /// caller before it reaches the writer.
    ///
    /// # Arguments
    ///
    /// * `to` - the status the caller intends to write.
    ///
    /// # Returns
    ///
    /// `true` when the transition is allowed, `false` otherwise.
    #[must_use]
    pub fn can_transition_to(&self, to: Self) -> bool {
        match self {
            Self::Spawning => matches!(to, Self::Working | Self::Failed),
            Self::Working => matches!(
                to,
                Self::Idle
                    | Self::PlanPending
                    | Self::Blocked
                    | Self::Suspended
                    | Self::ShuttingDown
                    | Self::Failed
            ),
            Self::Idle => matches!(
                to,
                Self::Working | Self::Blocked | Self::Suspended | Self::ShuttingDown | Self::Failed
            ),
            Self::PlanPending => matches!(to, Self::Working | Self::Idle | Self::Failed),
            Self::Blocked => matches!(
                to,
                Self::Idle | Self::Suspended | Self::ShuttingDown | Self::Failed
            ),
            Self::Suspended => matches!(to, Self::Idle | Self::ShuttingDown | Self::Failed),
            Self::ShuttingDown => matches!(to, Self::Stopped | Self::Failed),
            // A finished member is only revived by a respawn, which re-enters
            // the lifecycle at `Spawning`.
            Self::Stopped | Self::Failed => matches!(to, Self::Spawning),
        }
    }

    /// The [`TransitionCause`] implied by moving from this status to `to` (spec
    /// `teamssetup` FR-035).
    ///
    /// [`TransitionCause`] is passed *in* by the writer, because only the caller
    /// knows why it is moving a member (`set_member_status(agent_id, to, cause)`).
    /// This helper exists for the reverse direction: a site that discovers a
    /// status changed without being told the cause (a legacy write, a
    /// deserialised history row) can still name a plausible one, and tests can
    /// assert the vocabulary covers the transition table. It is deliberately
    /// *lossy* - several causes share an edge (a member can reach `Idle` by
    /// finishing a task, by a plan being approved, or by going idle) - so a
    /// caller that knows its cause must pass it rather than derive it.
    ///
    /// # Arguments
    ///
    /// * `to` - the destination status.
    ///
    /// # Returns
    ///
    /// The best-matching cause for the edge.
    #[must_use]
    pub fn transition_cause(&self, to: Self) -> TransitionCause {
        match (self, to) {
            // Entry into the lifecycle only happens on spawn or respawn.
            (_, Self::Spawning) => TransitionCause::Spawned,
            // A failure verdict is only ever the watchdog's or a spawn error's.
            (Self::Spawning, Self::Failed) => TransitionCause::SpawnFailed,
            (_, Self::Failed) => TransitionCause::WatchdogTimeout,
            // Work assignment: the only way into `Working` is claiming a task or
            // resuming from a plan approval.
            (Self::PlanPending, Self::Working) => TransitionCause::PlanApproved,
            (_, Self::Working) => TransitionCause::TaskClaimed,
            (_, Self::PlanPending) => TransitionCause::PlanSubmitted,
            (_, Self::Blocked) => TransitionCause::DependencyBlocked,
            (Self::Suspended, _) => TransitionCause::Resumed,
            (_, Self::Suspended) => TransitionCause::Suspended,
            (_, Self::ShuttingDown) => TransitionCause::ShutdownRequested,
            (_, Self::Stopped) => TransitionCause::ShutdownAcked,
            // Everything landing on `Idle` that is not a resume is a
            // task-finished or explicit idle report; `TaskCompleted` is the
            // common case and the caller overrides it when it knows better.
            (_, Self::Idle) => TransitionCause::TaskCompleted,
        }
    }
}

/// Why a [`MemberStatus`] changed (spec `teamssetup` FR-035).
///
/// Every member-status write records one of these, so the
/// `Event::TeammateStatusChanged` stream alone can reconstruct a member's status
/// history: "why did this teammate stop?" is answerable from the cause rather
/// than inferred from the ordering of coarse events (`TeammateIdle`,
/// `TeammateFailed`, `TeammateResumed`).
///
/// The vocabulary is deliberately small and event-shaped: one variant per
/// distinct *reason a writer moves a member*, not one per edge in the transition
/// table. Several causes share an edge, and a cause is never inferred from the
/// status pair alone - the writer passes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionCause {
    /// A new session was created for the member (`Spawning`), including the
    /// respawn of a `Stopped`/`Failed` member.
    Spawned,
    /// The spawn attempt failed; see `last_spawn_error`.
    SpawnFailed,
    /// The member claimed a task from the shared list and started work.
    TaskClaimed,
    /// The member's claimed task reached a terminal status.
    TaskCompleted,
    /// The member reported idle with no work to do (`team_idle`).
    ReportedIdle,
    /// The member submitted a plan and is awaiting the lead's decision.
    PlanSubmitted,
    /// The lead approved the submitted plan.
    PlanApproved,
    /// The lead rejected the submitted plan; the member returns to `Idle`.
    PlanRejected,
    /// An unsatisfied task dependency parked the member.
    DependencyBlocked,
    /// The last unmet dependency completed, so the member was unblocked.
    DependencySatisfied,
    /// The lead suspended the member.
    Suspended,
    /// The lead resumed a suspended or blocked member.
    Resumed,
    /// The lead asked the member to shut down; it must acknowledge.
    ShutdownRequested,
    /// The member acknowledged the shutdown request.
    ShutdownAcked,
    /// The watchdog found the member live but making no progress and failed it.
    WatchdogTimeout,
    /// The member's task was cancelled, releasing it to `Idle`.
    TaskCancelled,
    /// The member was torn down with the team (`team_cleanup`).
    TeamCleanedUp,
}

impl TransitionCause {
    /// Every canonical wire token (spec `teamssetup` FR-032).
    ///
    /// A refusal that rejects a cause token names the accepted ones from here,
    /// so the list cannot drift from [`Self::from_wire`].
    pub const ALL_TOKENS: &'static [&'static str] = &[
        "spawned",
        "spawn_failed",
        "task_claimed",
        "task_completed",
        "reported_idle",
        "plan_submitted",
        "plan_approved",
        "plan_rejected",
        "dependency_blocked",
        "dependency_satisfied",
        "suspended",
        "resumed",
        "shutdown_requested",
        "shutdown_acked",
        "watchdog_timeout",
        "task_cancelled",
        "team_cleaned_up",
    ];

    /// Return the canonical snake_case string used for serialization, event
    /// payloads, and the `team_status` history (FR-035, FR-036).
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Spawned => "spawned",
            Self::SpawnFailed => "spawn_failed",
            Self::TaskClaimed => "task_claimed",
            Self::TaskCompleted => "task_completed",
            Self::ReportedIdle => "reported_idle",
            Self::PlanSubmitted => "plan_submitted",
            Self::PlanApproved => "plan_approved",
            Self::PlanRejected => "plan_rejected",
            Self::DependencyBlocked => "dependency_blocked",
            Self::DependencySatisfied => "dependency_satisfied",
            Self::Suspended => "suspended",
            Self::Resumed => "resumed",
            Self::ShutdownRequested => "shutdown_requested",
            Self::ShutdownAcked => "shutdown_acked",
            Self::WatchdogTimeout => "watchdog_timeout",
            Self::TaskCancelled => "task_cancelled",
            Self::TeamCleanedUp => "team_cleaned_up",
        }
    }

    /// Parse a canonical snake_case cause token back into a [`TransitionCause`].
    ///
    /// The inverse of [`Self::as_str`], for the same reason as
    /// [`MemberStatus::from_wire`]: the type-erased manager a team tool holds
    /// exchanges tokens, and the manager parses them here (spec `teamssetup`
    /// FR-008, FR-026).
    ///
    /// # Returns
    ///
    /// The cause, or `None` when `token` is not one of the canonical tokens.
    #[must_use]
    pub fn from_wire(token: &str) -> Option<Self> {
        match token {
            "spawned" => Some(Self::Spawned),
            "spawn_failed" => Some(Self::SpawnFailed),
            "task_claimed" => Some(Self::TaskClaimed),
            "task_completed" => Some(Self::TaskCompleted),
            "reported_idle" => Some(Self::ReportedIdle),
            "plan_submitted" => Some(Self::PlanSubmitted),
            "plan_approved" => Some(Self::PlanApproved),
            "plan_rejected" => Some(Self::PlanRejected),
            "dependency_blocked" => Some(Self::DependencyBlocked),
            "dependency_satisfied" => Some(Self::DependencySatisfied),
            "suspended" => Some(Self::Suspended),
            "resumed" => Some(Self::Resumed),
            "shutdown_requested" => Some(Self::ShutdownRequested),
            "shutdown_acked" => Some(Self::ShutdownAcked),
            "watchdog_timeout" => Some(Self::WatchdogTimeout),
            "task_cancelled" => Some(Self::TaskCancelled),
            "team_cleaned_up" => Some(Self::TeamCleanedUp),
            _ => None,
        }
    }
}

/// Plan approval state for a teammate that has submitted a plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PlanStatus {
    /// No pending plan.
    #[default]
    None,
    /// Plan submitted; lead has not yet reviewed it.
    Pending,
    /// Lead approved the plan.
    Approved,
    /// Lead rejected the plan.
    Rejected,
}

impl PlanStatus {
    /// Return the canonical snake_case string (M5-T1).
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }
}

/// Lifecycle event that can trigger a quality-gate hook.
///
/// Every variant fires from the team runtime's own mutation path, not from the
/// `team_task_*` tools (spec `teamssetup` FR-017): see
/// [`fire_task_hook`](crate::team::task::fire_task_hook), which derives the
/// event from the mutation it is reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HookEvent {
    /// Fires when a teammate reports idle (no more tasks).
    TeammateIdle,
    /// Fires when a new task is added to the shared task list.
    TaskCreated,
    /// Fires when a teammate claims a task (`Pending -> InProgress`).
    TaskClaimed,
    /// Fires when a task is marked as completed.
    TaskCompleted,
}

/// A single quality-gate hook: an event trigger paired with a shell command.
///
/// When the matching `event` fires, the `command` is executed as a shell
/// command.  Exit codes follow the quality-gate protocol:
///
/// - **Exit 0** -> allow the action.
/// - **Exit 2** -> reject / send feedback (stdout is returned to the agent).
/// - **Other** -> log a warning, allow the action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEntry {
    /// The lifecycle event that triggers this hook.
    pub event: HookEvent,
    /// Shell command to run when the event fires.
    pub command: String,
}

/// Describes one teammate within a team.
///
/// `plan_request_id` and `shutdown_request_id` carry the correlation id of the
/// most recent `PlanRequest` / `ShutdownRequest` so the corresponding reply
/// (`PlanApproved`/`PlanRejected`/`ShutdownAck`) can copy it (M5-T4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    /// Human-friendly name for this teammate (e.g. `"security-reviewer"`).
    pub name: String,
    /// Unique agent ID assigned at spawn time (e.g. `"tm-001"`).
    pub agent_id: String,
    /// Session ID of the underlying ragent session, if spawned.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Agent type / definition name used when spawning this session.
    ///
    /// For swarm-created teammates this value is derived from the subtask's
    /// `agent_type` field and resolved through the classification fallback chain.
    pub agent_type: String,
    /// Current lifecycle state.
    pub status: MemberStatus,
    /// ID of the task currently being worked on, if any.
    #[serde(default)]
    pub current_task_id: Option<String>,
    /// Plan approval state.
    pub plan_status: PlanStatus,
    /// When this member was added to the team.
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    /// Last error message from a failed spawn attempt, if any.
    pub last_spawn_error: Option<String>,
    /// Initial prompt sent to this teammate when spawned.  Stored so that the
    /// reconcile loop can replay it if the manager was unavailable at blueprint
    /// seeding time.
    #[serde(default)]
    pub spawn_prompt: Option<String>,
    /// Optional per-teammate model override. When set, the teammate uses this
    /// model instead of inheriting the lead's active model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_override: Option<ModelRef>,
    /// Persistent memory scope for this teammate.  When not `None`, a memory
    /// directory is created and `MEMORY.md` is injected into the system prompt.
    #[serde(default)]
    pub memory_scope: MemoryScope,
    /// Correlation id of the most recent `PlanRequest` sent by this teammate
    /// (M5-T4). Copied into the `PlanApproved`/`PlanRejected` reply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_request_id: Option<String>,
    /// Correlation id of the most recent `ShutdownRequest` sent to this
    /// teammate (M5-T4). Copied into the `ShutdownAck` reply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shutdown_request_id: Option<String>,
}

impl TeamMember {
    /// Create a new member record in `Spawning` state.
    pub fn new(
        name: impl Into<String>,
        agent_id: impl Into<String>,
        agent_type: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            agent_id: agent_id.into(),
            session_id: None,
            agent_type: agent_type.into(),
            status: MemberStatus::Spawning,
            current_task_id: None,
            plan_status: PlanStatus::None,
            created_at: Utc::now(),
            last_spawn_error: None,
            spawn_prompt: None,
            model_override: None,
            memory_scope: MemoryScope::None,
            plan_request_id: None,
            shutdown_request_id: None,
        }
    }
}

/// The default no-progress window before the watchdog fails a live teammate.
///
/// Five minutes: long enough that a legitimate long build is not mistaken for a
/// wedge, short enough that a genuinely hung teammate is reported within the
/// same working session. An operator can widen or tighten it per team with
/// [`TeamSettings::watchdog_timeout_secs`] (FR-013).
pub const DEFAULT_WATCHDOG_TIMEOUT_SECS: u64 = 300;

/// Team-wide settings stored inside `config.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamSettings {
    /// Maximum number of concurrent teammates allowed.
    pub max_teammates: usize,
    /// If `true`, teammates must submit a plan before starting implementation.
    pub require_plan_approval: bool,
    /// If `true`, teammates automatically claim the next available task when idle.
    pub auto_claim_tasks: bool,
    /// Seconds a **live** teammate (`Spawning`, `Working`, `ShuttingDown`) may
    /// go without progress before the watchdog fails it (FR-013).
    ///
    /// This is the operator's remedy for F-09: a teammate legitimately running
    /// one long build can be given a wider window instead of being failed with
    /// no recourse. Parked members (`Idle`, `PlanPending`, `Blocked`,
    /// `Suspended`) and terminal ones are never eligible whatever the value.
    ///
    /// `0` means "use [`DEFAULT_WATCHDOG_TIMEOUT_SECS`]", so a config written
    /// without the field (or with it zeroed) keeps the documented default
    /// rather than failing every live teammate on the first tick.
    #[serde(default = "default_watchdog_timeout_secs")]
    pub watchdog_timeout_secs: u64,
    /// Quality-gate hooks that fire at team lifecycle points.
    #[serde(default)]
    pub hooks: Vec<HookEntry>,
}

impl Default for TeamSettings {
    fn default() -> Self {
        Self {
            max_teammates: 8,
            require_plan_approval: false,
            auto_claim_tasks: true,
            watchdog_timeout_secs: DEFAULT_WATCHDOG_TIMEOUT_SECS,
            hooks: Vec::new(),
        }
    }
}

/// The `serde` default for [`TeamSettings::watchdog_timeout_secs`].
fn default_watchdog_timeout_secs() -> u64 {
    DEFAULT_WATCHDOG_TIMEOUT_SECS
}

impl TeamSettings {
    /// The no-progress window the watchdog should use, in seconds.
    ///
    /// Substitutes [`DEFAULT_WATCHDOG_TIMEOUT_SECS`] for a configured `0`, so a
    /// zeroed or absent field can never arm a zero-length window.
    #[must_use]
    pub fn effective_watchdog_timeout_secs(&self) -> u64 {
        if self.watchdog_timeout_secs == 0 {
            DEFAULT_WATCHDOG_TIMEOUT_SECS
        } else {
            self.watchdog_timeout_secs
        }
    }
}

/// Root configuration object for a team, stored as `config.json`.
///
/// Carries a `schema_version` (M5-T2) and `updated_at` (M5-T5) so future
/// schema changes can be migrated and concurrent races can be debugged.
/// `#[serde(deny_unknown_fields)]` rejects unknown fields on manual edits
/// (M5-T3).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TeamConfig {
    /// Schema version of the on-disk format (M5-T2).
    #[serde(default)]
    pub schema_version: u32,
    /// Unique name for this team (also the directory name).
    pub name: String,
    /// Session ID of the lead session that created this team.
    pub lead_session_id: String,
    /// When the team was created.
    pub created_at: DateTime<Utc>,
    /// When this config was last written (M5-T5).
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
    /// Current overall status.
    pub status: TeamStatus,
    /// All registered teammates (active and stopped).
    pub members: Vec<TeamMember>,
    /// Team-wide settings.
    pub settings: TeamSettings,
}

/// Current on-disk schema version for [`TeamConfig`] (M5-T2).
pub const TEAM_CONFIG_SCHEMA_VERSION: u32 = 1;

impl TeamConfig {
    /// Create a new team config with no members.
    pub fn new(name: impl Into<String>, lead_session_id: impl Into<String>) -> Self {
        Self {
            schema_version: TEAM_CONFIG_SCHEMA_VERSION,
            name: name.into(),
            lead_session_id: lead_session_id.into(),
            created_at: Utc::now(),
            updated_at: Some(Utc::now()),
            status: TeamStatus::Active,
            members: Vec::new(),
            settings: TeamSettings::default(),
        }
    }

    /// Validate the config's invariants (M5-T3).
    ///
    /// Returns `Ok(())` if the config is well-formed, or an error describing
    /// the first violation. Checks:
    /// - `name` and `lead_session_id` are non-empty.
    /// - member `agent_id`s are unique and non-empty.
    pub fn validate(&self) -> anyhow::Result<()> {
        use anyhow::anyhow;
        if self.name.is_empty() {
            return Err(anyhow!("team config name is empty"));
        }
        if self.lead_session_id.is_empty() {
            return Err(anyhow!("team config lead_session_id is empty"));
        }
        let mut seen = std::collections::HashSet::new();
        for m in &self.members {
            if m.agent_id.is_empty() {
                return Err(anyhow!("member '{}' has an empty agent_id", m.name));
            }
            if !seen.insert(&m.agent_id) {
                return Err(anyhow!("duplicate agent_id '{}'", m.agent_id));
            }
        }
        Ok(())
    }

    /// Migrate this config to the current schema version (M5-T2).
    ///
    /// Currently a no-op: the only schema change so far is the addition of
    /// `schema_version` and `updated_at`, both of which are
    /// `#[serde(default)]`. Future breaking changes should bump
    /// [`TEAM_CONFIG_SCHEMA_VERSION`] and perform the field transforms here
    /// before the config is used.
    pub fn migrate(&mut self) {
        if self.schema_version == 0 {
            self.schema_version = TEAM_CONFIG_SCHEMA_VERSION;
        }
        if self.updated_at.is_none() {
            self.updated_at = Some(chrono::Utc::now());
        }
    }

    /// Return the member with the given `agent_id`, if found.    #[must_use]
    pub fn member_by_id(&self, agent_id: &str) -> Option<&TeamMember> {
        self.members.iter().find(|m| m.agent_id == agent_id)
    }

    /// Return a mutable reference to the member with the given `agent_id`, if found.
    pub fn member_by_id_mut(&mut self, agent_id: &str) -> Option<&mut TeamMember> {
        self.members.iter_mut().find(|m| m.agent_id == agent_id)
    }

    /// Return the member with the given `name`, if found.
    #[must_use]
    pub fn member_by_name(&self, name: &str) -> Option<&TeamMember> {
        self.members.iter().find(|m| m.name == name)
    }

    /// Return an iterator over members that are currently active (not `Stopped`).
    pub fn active_members(&self) -> impl Iterator<Item = &TeamMember> {
        self.members
            .iter()
            .filter(|m| m.status != MemberStatus::Stopped)
    }
}
