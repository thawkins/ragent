//! `TeamManager` - runtime for spawning and coordinating teammate sessions.
//!
//! Implements [`crate::tool::TeamManagerInterface`] so the `team_spawn` tool
//! can call it once M3 is wired into the session processor.
//!
//! # Architecture
//!
//! ```text
//! TeamManager (Arc-shared)
//!   |- spawn_teammate()   -> creates child session, injects team system prompt,
//!   |                       starts mailbox polling loop
//!   |- mailbox_poll_loop  -> tokio::spawn per teammate; drains unread messages,
//!   |                       publishes Event::TeammateMessage etc.
//!   |- run_hook()         -> exec shell hook, interpret exit code
//!   \- shutdown_teammate()-> writes shutdown_request mailbox message, marks Stopped
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Result, anyhow};
use tokio::sync::{Mutex, Notify};
use tracing::{debug, warn};

use crate::Config;
use crate::agent::{AgentInfo, AgentMode, resolve_agent_with_customs};
use crate::event::{Event, EventBus};
use crate::session::processor::SessionProcessor;
use crate::team::config::{
    DEFAULT_WATCHDOG_TIMEOUT_SECS, HookEvent, MemberStatus, PlanStatus, TeamConfig, TransitionCause,
};
use crate::team::hook_ledger::{HookFailure, HookFailureReason, record_hook_failure};
use crate::team::mailbox::{
    Mailbox, MailboxMessage, MessageType, deregister_notifier, register_notifier,
};
use crate::team::store::TeamStore;
use crate::team::task::{
    Task, TaskCancellation, TaskClaim, TaskList, TaskStatus, TaskTransition, transition_task,
};
use crate::team::{DEFAULT_AGENT_TYPE, TeamMember};
use crate::tool::TeamManagerInterface;

/// How often the watchdog re-examines each member, derived from `timeout` (FR-013).
///
/// The tick must be short relative to the window it polices, or a stalled member
/// is only noticed up to two ticks late. Half the timeout gives at most one
/// tick's delay past the deadline, and the one-minute cap keeps a wide operator
/// window from also widening the detection latency: a team configured with a
/// one-hour window still checks its members every thirty seconds.
#[must_use]
pub fn watchdog_check_interval(timeout: Duration) -> Duration {
    timeout.min(Duration::from_mins(1)) / 2
}

/// PERF-023: read the `mtime` of a path, returning `None` when the file does
/// not exist or its mtime cannot be determined. Used by the in-memory
/// `TaskList` cache for mtime-based invalidation.
fn fs_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

/// Check if an error message indicates a context-window / token-count overflow.
///
/// These errors come from Anthropic, `OpenAI`, and GitHub Copilot when the prompt
/// is too long for the model's context window. They are *not* permanent failures -
/// the session processor's compression pipeline will reduce context on retry.
///
/// [`crate::session::history::is_token_overflow_error_message`] so the team
/// manager stays in sync with the same patterns used by the session loop.
fn is_token_overflow_error(error_msg: &str) -> bool {
    crate::session::history::is_token_overflow_error_message(error_msg)
}

/// Check if an error message indicates a permanent (non-retryable) API error.
///
/// Matches HTTP 4xx errors, excluding 429 (Too Many Requests), 408 (Timeout),
/// and token-overflow errors (handled by the compression pipeline on retry).
///
/// This is a thin wrapper around the canonical implementation in
/// [`crate::session::history::is_permanent_llm_api_error`] so the team
/// manager stays in sync with the same patterns used by the session loop.
fn is_permanent_api_error(error_msg: &str) -> bool {
    crate::session::history::is_permanent_llm_api_error(error_msg)
}

/// Compute the retry backoff for a teammate agent loop that has just failed.
///
/// Uses **exponential backoff with jitter** so that multiple teammates that
/// fail in lockstep (e.g. because the shared LLM provider is rate-limited
/// or cold-starting) do not synchronously retry and re-trigger the same
/// upstream pressure.  The schedule is:
///
/// | attempt | base    | range (jitter) |
/// |---------|---------|----------------|
/// | 0       |  0 ms   | 0 ms           |
/// | 1       |  1 s    | 1.0 s - 1.5 s  |
/// | 2       |  2 s    | 2.0 s - 2.5 s  |
/// | 3       |  4 s    | 4.0 s - 4.5 s  |
/// | 4       |  8 s    | 8.0 s - 8.5 s  |
///
/// `attempt` is the 1-based retry index: `1` = first retry, `2` = second,
/// and so on.  `attempt == 0` returns [`Duration::ZERO`] so callers can
/// unconditionally `sleep(backoff)` without an `if attempt > 0` guard.
///
/// The cap (`MAX_TEAMMATE_BACKOFF_MS`) prevents an unbounded wait when the
/// caller passes a large `attempt` value by mistake.
///
/// Jitter is derived from the current monotonic clock - cheap, allocation-
/// free, and sufficient to spread sibling retries across a sub-second
/// window.  True cryptographic randomness is not required here.
#[must_use]
pub fn teammate_retry_backoff(attempt: u32) -> std::time::Duration {
    /// Base cap on any single teammate retry backoff (30 s).
    const MAX_TEAMMATE_BACKOFF_MS: u64 = 30_000;
    /// Maximum jitter added to each attempt (500 ms).
    const MAX_JITTER_MS: u64 = 500;

    if attempt == 0 {
        return std::time::Duration::ZERO;
    }

    // 2^(attempt-1) * 1 s, saturating at MAX_TEAMMATE_BACKOFF_MS.
    let shift = attempt.saturating_sub(1).min(20); // 2^20 s already dwarfs the cap
    let base_ms = (1u64 << shift).saturating_mul(1_000);

    // Cheap pseudo-jitter from the system clock nanosecond component.
    // `Instant::now().elapsed()` always returns ~0; use `SystemTime` so
    // concurrent retries from the same provider actually desync.
    let jitter_ms = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0))
        % MAX_JITTER_MS;

    std::time::Duration::from_millis(
        base_ms
            .saturating_add(jitter_ms)
            .min(MAX_TEAMMATE_BACKOFF_MS),
    )
}

// -- System prompt addition for teammate sessions ------------------------------

/// Build the team-context section injected into every teammate's system prompt.
///
/// Template variables:
/// - `{{TEAM_NAME}}` - name of the team
/// - `{{TEAMMATE_NAME}}` - this teammate's friendly name
/// - `{{AGENT_ID}}` - this teammate's agent ID (e.g. `"tm-001"`)
/// - `{{TEAMMATE_ROSTER}}` - list of other teammates with names and agent IDs
///
/// `auto_claim_tasks` is the team's effective setting, so the claim rule the
/// prompt states is the rule the runtime enforces (FR-027). With the setting
/// on (the default) the teammate claims the next task itself after completing
/// one; with it off the runtime never claims on the teammate's behalf, so the
/// prompt tells the teammate not to claim without an explicit lead
/// instruction. A prompt that said "claim the next task" either way would
/// describe behaviour the runtime does not perform.
#[must_use]
pub fn build_team_prompt_addition(
    team_name: &str,
    teammate_name: &str,
    agent_id: &str,
    teammate_roster: &[(String, String)],
    auto_claim_tasks: bool,
) -> String {
    let others = if teammate_roster.is_empty() {
        "none yet".to_string()
    } else {
        teammate_roster
            .iter()
            .map(|(name, id)| format!("{name} ({id})"))
            .collect::<Vec<_>>()
            .join(", ")
    };

    // FR-027: state the claim rule the runtime actually enforces. The setting
    // decides whether completing a task means "pick up the next one" or
    // "stop and wait", and the prompt must not claim the other behaviour.
    let claim_rule = if auto_claim_tasks {
        "When you finish a task, call `team_task_complete`: it parks you idle (or hands you the\n\
         next task when this team's runtime claims one for you). Call `team_task_claim` to pick\n\
         up further work. If no tasks remain, you are already idle - call `team_idle` once more\n\
         only to notify the lead with a summary."
    } else {
        "When you finish a task, call `team_task_complete`: it parks you idle and notifies the\n\
         lead. Calling `team_idle` afterwards is harmless but unnecessary. This team has\n\
         `auto_claim_tasks` turned off, so idling does NOT hand you the next task. Do not call\n\
         `team_task_claim` without an explicit instruction from the lead: claim only a task the\n\
         lead names, and wait for the lead otherwise."
    };

    format!(
        r#"
## Team Context

You are a teammate in team "{team_name}". Your name is "{teammate_name}" (agent ID: {agent_id}).
The team lead is "lead". Other teammates: {others}.

### Team tool usage - CRITICAL

**Your very first action in every response MUST be a tool call.** Do NOT write planning text.
Call `team_read_messages` (team_name: "{team_name}") immediately at the start of each turn
to check for new instructions, plan approval results, or shutdown requests from the lead.

{claim_rule}

If you receive a `shutdown_request` message, finish your current step cleanly and call
`team_shutdown_ack` to confirm termination.

If `require_plan_approval` is enabled, call `team_submit_plan` before starting
implementation and wait for a `plan_approved` mailbox reply. Until the lead
approves, every write-class and bash-class tool call is refused, so a refused
call is not a bug in the tool - submit the plan.

### Peer collaboration

You can message other teammates directly using `team_message` when you have findings to
share, need to coordinate on overlapping work, or want to challenge each other's
assumptions. Use the agent ID from the roster above as the `to` parameter. For example:
`team_message(team_name: "{team_name}", to: "<agent-id>", content: "...")`.

Prefer peer messaging when:
- You discover something that affects another teammate's task.
- You need input or a second opinion before proceeding.
- You want to share intermediate results to avoid duplicated effort.
"#,
    )
}

/// Resolve which model a teammate should run with.
///
/// Priority order: an explicit per-teammate model override, then the lead's
/// active model, and finally whatever model is already set on the agent
/// definition (which is left untouched when both override inputs are `None`).
pub fn apply_teammate_model_override(
    agent: &mut AgentInfo,
    teammate_model: Option<&crate::agent::ModelRef>,
    lead_model: Option<&crate::agent::ModelRef>,
) {
    // Priority: per-teammate model > lead's active model > agent definition default.
    if let Some(m) = teammate_model {
        agent.model = Some(m.clone());
    } else if let Some(m) = lead_model {
        agent.model = Some(m.clone());
    }
}

// -- Persistent memory injection ------------------------------------------------

/// Maximum number of memories to inject from the team's structured memory store.
const TEAM_MEMORY_LIMIT: usize = 25;

/// Load the persistent-memory prompt block for a teammate from the structured
/// SQLite store.
///
/// Queries memories whose `project` field matches the team name and formats
/// them as a labelled section. Returns an empty string when memory is
/// unavailable or when the teammate's memory scope is `None`.
fn load_team_memory_block(
    storage: &crate::storage::Storage,
    team_name: &str,
    teammate_name: &str,
) -> String {
    let memories = match storage.list_memories(team_name, TEAM_MEMORY_LIMIT) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(team = %team_name, error = %e, "Failed to load team memories");
            return String::new();
        }
    };

    if memories.is_empty() {
        return format!(
            "\n\n## Persistent Memory\n\
             \n\
             Team memory is enabled. No prior memories have been recorded for team \"{team_name}\". \
\
             Use `team_memory_write` to store notes, decisions, and context so you can recall them in future sessions.\n"
        );
    }

    let mut lines = vec![
        String::new(),
        String::from("## Persistent Memory"),
        String::new(),
        format!(
            "The following structured memories are shared with team \"{team_name}\" (teammate \"{teammate_name}\"):"
        ),
        String::new(),
    ];

    // M-002: fetch all tags for these memories in one batched query instead of
    // one `get_memory_tags` SQLite round-trip per memory.
    let ids: Vec<i64> = memories.iter().map(|m| m.id).collect();
    let tags_map = storage.get_memory_tags_batched(&ids).unwrap_or_default();

    for mem in &memories {
        let tags: Vec<String> = tags_map.get(&mem.id).cloned().unwrap_or_default();
        let tag_str = if tags.is_empty() {
            String::new()
        } else {
            format!(" tags: {}", tags.join(", "))
        };
        lines.push(format!(
            "- [{}] {} (confidence: {:.2}){}",
            mem.category, mem.content, mem.confidence, tag_str
        ));
    }

    lines.push(String::new());
    lines.push(String::from(
        "Use `team_memory_read` to recall and `team_memory_write` to add to these memories.",
    ));
    lines.push(String::new());

    lines.join("\n")
}

// -- Hook runner ---------------------------------------------------------------

/// Exit-code protocol for quality-gate hooks.
#[derive(Debug, PartialEq, Eq)]
pub enum HookOutcome {
    /// Allow the action (exit 0 or unrecognised code).
    Allow,
    /// Block the action; stdout is returned as feedback to the agent (exit 2).
    Feedback(String),
}

/// Execute a hook command and interpret its exit code.
///
/// - Exit 0 -> `HookOutcome::Allow`
/// - Exit 2 -> `HookOutcome::Feedback(stdout)`
/// - Other -> log warning, allow
///
/// If `stdin_data` is `Some`, it is piped to the child process on stdin.
///
/// SEC-ragent-team-005 (SECTASKS T-047): the command runs under a hard
/// [`HOOK_TIMEOUT`] and its process group is killed on expiry, so a hook that
/// never exits (or that spawns a long-lived child) cannot wedge the tool call
/// or leak processes. Feedback returned to the model is capped at
/// [`MAX_HOOK_FEEDBACK_BYTES`]. The command itself is validated by
/// [`validate_hook_command`] before it is spawned.
///
/// FR-019: the three "the hook never ran" outcomes - validation refusal, spawn
/// failure, and timeout - still return `HookOutcome::Allow` (a hook is a gate,
/// not a task, and refusing to proceed on an inert gate would be worse), but
/// each also appends one record to `team_dir`'s `hook-failures.jsonl` via
/// [`record_hook_failure`], so an inert hook stops looking like a passing one.
/// `event` names the hook's trigger in that record.
pub async fn run_hook(
    team_dir: &Path,
    event: HookEvent,
    command: &str,
    args: &[String],
    stdin_data: Option<&str>,
) -> HookOutcome {
    if let Err(reason) = validate_hook_command(command) {
        warn!(command, %reason, "Refusing to execute untrusted hook command");
        record_hook_failure(
            team_dir,
            &HookFailure::new(event, command, HookFailureReason::Invalid, reason),
        );
        return HookOutcome::Allow;
    }
    let mut child_cmd = tokio::process::Command::new(command);
    child_cmd.args(args);

    if stdin_data.is_some() {
        child_cmd.stdin(std::process::Stdio::piped());
    }
    child_cmd.stdout(std::process::Stdio::piped());
    child_cmd.stderr(std::process::Stdio::piped());

    let child = child_cmd.spawn();

    match child {
        Err(e) => {
            warn!(command, error = %e, "Hook failed to execute");
            record_hook_failure(
                team_dir,
                &HookFailure::new(
                    event,
                    command,
                    HookFailureReason::SpawnFailed,
                    &e.to_string(),
                ),
            );
            HookOutcome::Allow
        }
        Ok(mut child_proc) => {
            // SEC-ragent-team-005 (SECTASKS T-047): record the pid so the
            // timeout path can terminate the whole process group; the hook
            // runs unsandboxed, so leaving its children behind is a leak.
            let pid = child_proc.id();
            // Write stdin data if provided.
            if let Some(data) = stdin_data
                && let Some(mut stdin) = child_proc.stdin.take()
            {
                use tokio::io::AsyncWriteExt;
                // INTENTIONAL: best-effort teammate stdin write
                let _ = stdin.write_all(data.as_bytes()).await;
                drop(stdin);
            }

            let waited =
                match tokio::time::timeout(HOOK_TIMEOUT, child_proc.wait_with_output()).await {
                    Ok(result) => result,
                    Err(_) => {
                        warn!(
                            command,
                            timeout_secs = HOOK_TIMEOUT.as_secs(),
                            "Hook timed out; killing it"
                        );
                        if let Some(pid) = pid {
                            kill_hook_process_group(pid);
                        }
                        record_hook_failure(
                            team_dir,
                            &HookFailure::new(
                                event,
                                command,
                                HookFailureReason::Timeout,
                                &format!("hook exceeded the {}s timeout", HOOK_TIMEOUT.as_secs()),
                            ),
                        );
                        return HookOutcome::Allow;
                    }
                };

            match waited {
                Err(e) => {
                    warn!(command, error = %e, "Hook failed to complete");
                    HookOutcome::Allow
                }
                Ok(out) => match out.status.code() {
                    Some(0) => HookOutcome::Allow,
                    Some(2) => {
                        let feedback = String::from_utf8_lossy(&out.stdout).into_owned();
                        HookOutcome::Feedback(cap_hook_feedback(feedback))
                    }
                    Some(code) => {
                        warn!(
                            command,
                            code, "Hook returned unexpected exit code; allowing"
                        );
                        HookOutcome::Allow
                    }
                    None => {
                        warn!(command, "Hook terminated by signal; allowing");
                        HookOutcome::Allow
                    }
                },
            }
        }
    }
}

/// Hard timeout applied to a team quality-gate hook.
pub const HOOK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Maximum hook feedback bytes injected back into the model context.
pub const MAX_HOOK_FEEDBACK_BYTES: usize = 8192;

/// Truncate hook feedback to [`MAX_HOOK_FEEDBACK_BYTES`] on a char boundary.
fn cap_hook_feedback(mut feedback: String) -> String {
    if feedback.len() <= MAX_HOOK_FEEDBACK_BYTES {
        return feedback;
    }
    let mut end = MAX_HOOK_FEEDBACK_BYTES;
    while end > 0 && !feedback.is_char_boundary(end) {
        end -= 1;
    }
    feedback.truncate(end);
    feedback.push_str("\n[hook feedback truncated]");
    feedback
}

/// Validate a hook command before execution.
///
/// SEC-ragent-team-005 (SECTASKS T-047): a team `config.json` hook is ordinary
/// on-disk state that an attacker with write access to the team directory (or a
/// seeded team dir) can modify, and it is executed as the user with no
/// validation. Refuse commands carrying shell metacharacters or control bytes,
/// which are never part of a legitimate quality-gate command.
pub fn validate_hook_command(command: &str) -> std::result::Result<(), &'static str> {
    if command.trim().is_empty() {
        return Err("hook command is empty");
    }
    if command.len() > 4096 {
        return Err("hook command is longer than 4096 bytes");
    }
    for ch in command.chars() {
        if ch == '\n' || ch == '\r' || ch.is_control() {
            return Err("hook command contains a control character");
        }
    }
    for meta in [";", "|", "&", "`", "$(", ">", "<", "*", "?", "~", "\\"] {
        if command.contains(meta) {
            return Err("hook command contains a shell metacharacter");
        }
    }
    Ok(())
}

/// Kill a hook's process group so its children cannot be orphaned.
///
/// Mirrors the MCP shutdown teardown (`mcp::kill_stdio_child`): the command is
/// spawned without `process_group(0)`, so the group id equals the child pid and
/// `killpg` reaches whatever the hook spawned. A pid that is not a group leader
/// returns `ESRCH` and the plain `kill` fallback covers it.
#[cfg(unix)]
#[allow(unsafe_code)]
fn kill_hook_process_group(pid: u32) {
    let pid = pid as i32;
    if pid <= 0 {
        return;
    }
    // SAFETY: `killpg`/`kill` are async-signal-safe libc calls that take only
    // integers; there is no safe std equivalent for signalling a process group.
    unsafe {
        let _ = libc::killpg(pid, libc::SIGKILL);
        let _ = libc::kill(pid, libc::SIGKILL);
    }
}

/// Windows equivalent: tokio cannot signal a Windows pid without a handle, and
/// the hook child does not survive its own `KillOnDrop` teardown there, so
/// nothing further is required when the 30 s hook timeout fires.
#[cfg(windows)]
fn kill_hook_process_group(_pid: u32) {}

/// Look up and run a quality-gate hook for the given event in a team's settings.
///
/// Returns `HookOutcome::Allow` if no hook is configured for this event.
/// The `stdin_json` parameter is piped to the hook process as stdin (useful for
/// passing task metadata to `TaskCreated` / `TaskCompleted` hooks).
///
/// FR-019: a hook that cannot run is recorded in the team's
/// `hook-failures.jsonl` by [`run_hook`], which is what makes an inert hook
/// visible in `/team show` and `team_status`.
pub async fn run_team_hook(
    team_dir: &Path,
    event: HookEvent,
    stdin_json: Option<&str>,
) -> HookOutcome {
    let store = match TeamStore::load(team_dir) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "Cannot load team store for hook lookup");
            return HookOutcome::Allow;
        }
    };

    let hook = store
        .config
        .settings
        .hooks
        .iter()
        .find(|h| h.event == event);
    let Some(hook) = hook else {
        return HookOutcome::Allow;
    };

    run_hook(team_dir, event, &hook.command, &[], stdin_json).await
}

// -- TeamManager ---------------------------------------------------------------

/// Tracks the runtime state of one teammate.
struct TeammateHandle {
    /// Friendly name (e.g. `"security-reviewer"`).
    _name: String,
    /// Agent ID (e.g. `"tm-001"`).
    _agent_id: String,
    /// Child session ID used by the teammate's agent loop.
    _child_session_id: String,
    /// Cancel flag; set to `true` to terminate the teammate's agent loop.
    cancel: Arc<AtomicBool>,
    /// Cancel flag for the mailbox polling task.
    poll_cancel: Arc<AtomicBool>,
    /// Notify handle for push-based mailbox wakeup.
    notify: Arc<Notify>,
    /// M6-T1: last time progress was observed for this teammate (idle / failure /
    /// task completion). Used by the watchdog to detect hung agents.
    last_progress: Arc<std::sync::Mutex<std::time::Instant>>,
    /// R-19: Handle to the teammate agent-loop task so it can be force-aborted
    /// on `shutdown_teammate` / `Drop`.
    agent_handle: Option<tokio::task::JoinHandle<()>>,
    /// R-18: Handle to the mailbox poll-loop task so it can be force-aborted
    /// on `shutdown_teammate` / `Drop`.
    poll_handle: Option<tokio::task::JoinHandle<()>>,
}

/// Manages the runtime lifecycle of all teammates in one team.
///
/// Created by the lead's session processor and shared as
/// `Arc<TeamManager>`.
pub struct TeamManager {
    /// Name of the managed team.
    pub team_name: String,
    /// Lead session ID (used for event routing).
    pub lead_session_id: String,
    /// Absolute path to the team directory on disk.
    pub team_dir: PathBuf,
    /// Active teammate handles, indexed by agent ID.
    ///
    /// PERF-025: a [`DashMap`] instead of `RwLock<HashMap>`. `handles` is
    /// accessed by nearly every manager method (and by the watchdog), and
    /// DashMap provides lock-free shard-based concurrent access - readers
    /// on one agent's handle never block readers on another's. `DashMap`
    /// is already a workspace dependency (used by the orchestrator in
    /// `ragent-agent`).
    handles: Arc<dashmap::DashMap<String, TeammateHandle>>,
    /// Underlying session processor (shared with the lead).
    ///
    /// R-6: Stored as `Weak` to break the `TeamManager` <-> `SessionProcessor`
    /// `Arc` cycle. Upgraded to `Arc` on access via `processor()`.
    processor: std::sync::Weak<SessionProcessor>,
    /// Event bus for publishing team lifecycle events.
    event_bus: Arc<EventBus>,
    /// Mailbox poll interval.
    ///
    /// Idle-CPU fix: raised from 500 ms to 5 s. The push path
    /// (`notify.notified()`, registered by [`Mailbox::push`]) is the normal
    /// wakeup, so the timer is only a missed-notify safety net; 5 s keeps
    /// external-writer delivery latency bounded while removing 10x of the
    /// per-teammate idle disk-I/O wakes (2 Hz -> 0.2 Hz).
    poll_interval: Duration,
    /// Serialises spawn operations to avoid concurrent config read/write races.
    spawn_lock: Arc<Mutex<()>>,
    /// The lead's active model - teammates inherit this when spawned via
    /// the reconcile loop (where no `ToolContext` model is available).
    pub active_model: Option<crate::agent::ModelRef>,
    /// M6-T1: watchdog timeout. If a `Working` / `Spawning` member records no
    /// progress within this duration, it is marked `Failed` and
    /// `Event::TeammateFailed` is published.
    ///
    /// Seeded from [`TeamSettings::effective_watchdog_timeout_secs`] at
    /// construction (FR-013) and left `pub` so a caller can tighten it after
    /// construction - the tests rely on that to exercise the watchdog without
    /// waiting minutes. A change here also changes the watchdog's tick interval,
    /// because [`Self::watchdog_check_interval`] derives both from it.
    pub watchdog_timeout: Duration,
    /// M6-T1: cancel flag for the watchdog task (set on shutdown_all).
    watchdog_cancel: Arc<AtomicBool>,
    /// Idle-CPU fix: handle to the watchdog task so `Drop` can abort it
    /// instead of waiting for the next 30 s tick to observe the flag.
    watchdog_handle: parking_lot::Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// PERF-027: short-TTL in-memory cache of per-agent `is_plan_pending`
    /// results so the session processor's per-tool-call plan-pending gate
    /// does not hit disk on every invocation. The cache is invalidated by
    /// `team_submit_plan` / `team_approve_plan` (which mutate plan status)
    /// and naturally expires after `plan_pending_ttl` so an external process
    /// editing `config.json` is eventually observed.
    plan_pending_cache: parking_lot::Mutex<HashMap<String, PlanPendingEntry>>,
    /// PERF-027: maximum age of a cached `is_plan_pending` entry before it
    /// is re-read from disk.
    plan_pending_ttl: Duration,
    /// `Arc` handle on this manager, installed at construction by
    /// [`Self::new_in_arc`].
    ///
    /// FR-008: the teammate agent loop spawned by
    /// [`Self::spawn_teammate_internal`] records its member's status through
    /// [`Self::set_member_status`], so it must keep the manager alive rather
    /// than reach for whatever disk layout the team had. A `Weak` would do for
    /// a live manager, but nothing guarantees a caller holds the `Arc` for the
    /// whole of a long teammate run, and an upgraded handle that fails mid-run
    /// would lose the transition: the loop clones a strong reference from this
    /// slot once, up front.
    self_handle: parking_lot::Mutex<Option<std::sync::Weak<Self>>>,
    /// PERF-023: in-memory `TaskList` cache with write-through persistence.
    ///
    /// The cache holds the most recently observed on-disk `tasks.json`
    /// contents plus the file's `mtime` at the time of the last load. The
    /// first call to [`Self::task_list`] (or any write-through mutator)
    /// loads the file once; subsequent calls return the cached list in O(1)
    /// unless the on-disk `mtime` has advanced (which signals an external
    /// write, e.g. by another `TeamManager` process or a `team_task_*`
    /// tool that bypassed the cache). On an mtime change the cache is
    /// reloaded transparently.
    ///
    /// Write-through mutators ([`Self::apply_to_task_list`] and friends)
    /// perform the mutation under the `TaskStore` flock and then store the
    /// freshly-written list back into the cache, so the cache and the disk
    /// never drift apart.
    task_cache: parking_lot::Mutex<TaskCacheEntry>,
    /// In-memory `TeamConfig` cache, used only by the watchdog.
    ///
    /// A watchdog tick needs each stale member's status to decide whether the
    /// member is eligible for a failure verdict ([`MemberStatus::is_live`],
    /// FR-012). Reading `config.json` once per stale member per tick would put
    /// the watchdog on the disk continuously for a long-lived team; this cache
    /// makes one read serve the whole tick, and the file's `mtime` (advanced by
    /// every [`TeamStore::save`] and by the lead's own writes) is what makes the
    /// answer fresh rather than stale.
    config_cache: parking_lot::Mutex<ConfigCacheEntry>,
}

impl Drop for TeamManager {
    /// R-5: On drop, set every `poll_cancel` and `cancel` flag in `handles`
    /// so orphaned mailbox poll loops and teammate agent loops stop doing
    /// filesystem I/O. Without this, abandoning a team without
    /// `team_cleanup` leaves poll loops running at 500 ms intervals
    /// forever.
    ///
    /// Idle-CPU fix: flags alone leave each loop parked in a
    /// `sleep(500 ms)` that fires one last disk-I/O cycle per teammate
    /// before noticing the flag, and the loops would stay alive until the
    /// process exits when the processor holds this Arc for the session's
    /// lifetime. Aborting the JoinHandles (and re-registering on each
    /// `Notify` so a parked `notified()` arm does not miss the bounce)
    /// removes every tick the moment the manager goes away.
    fn drop(&mut self) {
        self.watchdog_cancel.store(true, Ordering::Relaxed);
        for entry in self.handles.iter() {
            entry.cancel.store(true, Ordering::Relaxed);
            entry.poll_cancel.store(true, Ordering::Relaxed);
            if let Some(h) = entry.poll_handle.as_ref() {
                h.abort();
            }
            if let Some(h) = entry.agent_handle.as_ref() {
                h.abort();
            }
            entry.notify.notify_waiters();
        }
        let watchdog = self.watchdog_handle.lock().take();
        if let Some(h) = watchdog {
            h.abort();
        }
    }
}

/// PERF-023: cached `TaskList` with the `mtime` of the on-disk file at the
/// time it was loaded. `mtime` is stored as a `SystemTime` so comparisons
/// against a fresh `fs::metadata(...).modified()` are direct.
struct TaskCacheEntry {
    list: Option<TaskList>,
    mtime: Option<std::time::SystemTime>,
}

/// PERF-027: a cached `is_plan_pending` result with its observation time.
struct PlanPendingEntry {
    pending: bool,
    observed_at: Instant,
}

/// A cached `config.json` load keyed on the file's `mtime`, giving the watchdog
/// one read per tick instead of one read per stale member.
struct ConfigCacheEntry {
    config: Option<TeamConfig>,
    mtime: Option<std::time::SystemTime>,
}

/// One applied member-status transition, returned by
/// [`TeamManager::set_member_status`] (spec `teamssetup` FR-008, FR-035).
///
/// It is the same record the published `Event::TeammateStatusChanged` carries,
/// returned so a caller can log or assert on what moved without subscribing to
/// the bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusTransition {
    /// The status the member held before the write.
    pub from: MemberStatus,
    /// The status the member holds now.
    pub to: MemberStatus,
    /// Why the caller moved the member.
    pub cause: TransitionCause,
}

/// Whether a constructor may take over a team owned by another lead session
/// (spec `teamssetup` FR-023).
///
/// `Refuse` is the default every existing caller keeps: opening a team that
/// records a different `lead_session_id` is an error naming the owning session,
/// and the team's files are not touched. `Adopt` is the explicit consent an
/// operator gives to take the team over; it is the only way to reach
/// [`TeamManager::adopt_orphaned_tasks`] (FR-024).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AdoptTeam {
    /// Refuse to open a team owned by another lead session (FR-023 default).
    #[default]
    Refuse,
    /// Take the team over, resetting every orphaned `InProgress` task (FR-024).
    Adopt,
}

impl TeamManager {
    /// Create a new `TeamManager` for an existing team on disk.
    ///
    /// FR-023: opening a team that records a **different** `lead_session_id`
    /// from the one passed here does **not** mutate that team. A foreign team is
    /// refused with an error naming the owning lead session, so a second
    /// terminal cannot quietly take over a team another session is driving. Pass
    /// [`AdoptTeam::Adopt`] to take the team over on purpose.
    ///
    /// FR-024: on an explicit adoption every `InProgress` task is reset to
    /// `Pending` - regardless of whether the previous owner was the old lead or
    /// a departed teammate - and one [`Event::TeamAdopted`] listing each reset
    /// task ID is published. See [`TeamManager::adopt_orphaned_tasks`].
    ///
    /// Use [`TeamManager::try_new`] when the refusal has to be *reported* rather
    /// than panicked on; this constructor logs the refusal and returns a manager
    /// whose team was left untouched (it still names the caller's session, so a
    /// manager built this way never writes to the foreign team).
    pub fn new(
        team_name: impl Into<String>,
        lead_session_id: impl Into<String>,
        team_dir: PathBuf,
        processor: Arc<SessionProcessor>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self::build(
            team_name.into(),
            lead_session_id.into(),
            team_dir,
            processor,
            event_bus,
            AdoptTeam::Refuse,
        )
    }

    /// Create a `TeamManager`, reporting a refusal to open a foreign team.
    ///
    /// This is [`TeamManager::new`] for callers that surface the refusal to an
    /// operator instead of logging it: a foreign `lead_session_id` with
    /// [`AdoptTeam::Refuse`] returns the FR-023 error **without touching the
    /// team's files**, and the same call with [`AdoptTeam::Adopt`] proceeds.
    ///
    /// # Errors
    ///
    /// The FR-023 refusal when the team belongs to another lead session and
    /// `adopt` does not allow it.
    pub fn try_new(
        team_name: impl Into<String>,
        lead_session_id: impl Into<String>,
        team_dir: PathBuf,
        processor: Arc<SessionProcessor>,
        event_bus: Arc<EventBus>,
        adopt: AdoptTeam,
    ) -> Result<Self> {
        let name: String = team_name.into();
        let lead_sid: String = lead_session_id.into();
        // FR-023: check before any construction so a refused call leaves the
        // team's files exactly as it found them (no `tasks.json` write, no
        // `config.json` write).
        Self::refuse_foreign_lead(&name, &team_dir, &lead_sid, adopt)?;
        Ok(Self::build(
            name, lead_sid, team_dir, processor, event_bus, adopt,
        ))
    }

    /// FR-023: the refusal check, shared by every constructor.
    ///
    /// Returns `Ok(Some(previous_lead_session_id))` when the team is owned by a
    /// different session **and** `adopt` allows the takeover, `Ok(None)` when
    /// the team is the caller's own or absent (nothing to adopt), and an error
    /// naming the owning session when the team is foreign and `adopt` does not.
    fn refuse_foreign_lead(
        team_name: &str,
        team_dir: &Path,
        lead_sid: &str,
        adopt: AdoptTeam,
    ) -> Result<Option<String>> {
        let Ok(store) = TeamStore::load(team_dir) else {
            return Ok(None);
        };
        let previous = store.config.lead_session_id.clone();
        if previous == lead_sid {
            return Ok(None);
        }
        match adopt {
            AdoptTeam::Adopt => Ok(Some(previous)),
            AdoptTeam::Refuse => Err(anyhow!(
                "team '{team_name}' is owned by lead session '{previous}'; this session \
                 ('{lead_sid}') may not take it over without explicit adoption (pass \
                 adopt: true)"
            )),
        }
    }

    /// The shared body of [`Self::new`] and [`Self::try_new`].
    fn build(
        name: String,
        lead_sid: String,
        team_dir: PathBuf,
        processor: Arc<SessionProcessor>,
        event_bus: Arc<EventBus>,
        adopt: AdoptTeam,
    ) -> Self {
        // FR-023/FR-024: a foreign team is adopted only on explicit consent, and
        // the takeover resets every orphaned task - not just the old lead's.
        match Self::refuse_foreign_lead(&name, &team_dir, &lead_sid, adopt) {
            Ok(Some(previous_lead)) => {
                tracing::info!(
                    team = %name,
                    old_lead = %previous_lead,
                    new_lead = %lead_sid,
                    "adopting team from previous lead session"
                );
                let reset_task_ids = match Self::adopt_orphaned_tasks(&team_dir, &previous_lead) {
                    Ok(ids) => ids,
                    Err(e) => {
                        tracing::warn!(team = %name, error = %e,
                            "failed to reassign orphaned tasks");
                        Vec::new()
                    }
                };
                event_bus.publish(Event::TeamAdopted {
                    session_id: lead_sid.clone(),
                    team_name: name.clone(),
                    previous_lead_session_id: previous_lead,
                    reset_task_ids,
                });
                // Update the config to reflect the new lead.
                if let Ok(mut new_store) = TeamStore::load(&team_dir) {
                    new_store.config.lead_session_id = lead_sid.clone();
                    // Best-effort team-store persistence; log a failure so an
                    // unpersisted lead hand-off is visible.
                    if let Err(e) = new_store.save() {
                        tracing::warn!(team = %name, error = %e,
                            "failed to persist lead-session hand-off");
                    }
                }
            }
            Ok(None) => {}
            Err(e) => {
                // Only reachable through `Self::new`, which cannot return the
                // error: log it so the refusal is visible and leave the team
                // untouched. `Self::try_new` is the reporting constructor.
                tracing::warn!(team = %name, error = %e,
                    "refusing to open a team owned by another lead session");
            }
        }

        // FR-013: the no-progress window is the team's configuration, not a
        // constant. A config without the field (or with it zeroed) resolves to
        // `DEFAULT_WATCHDOG_TIMEOUT_SECS` through the accessor, so this is
        // always a usable window. Read before `team_dir` moves into the struct.
        let watchdog_timeout = Duration::from_secs(
            TeamStore::load(&team_dir)
                .map(|store| store.config.settings.effective_watchdog_timeout_secs())
                .unwrap_or(DEFAULT_WATCHDOG_TIMEOUT_SECS),
        );

        // R-6: Store a `Weak<SessionProcessor>` to break the
        // `TeamManager` <-> `SessionProcessor` `Arc` cycle.
        Self {
            team_name: name,
            lead_session_id: lead_sid,
            team_dir,
            handles: Arc::new(dashmap::DashMap::new()),
            processor: Arc::downgrade(&processor),
            event_bus,
            poll_interval: Duration::from_secs(5),
            spawn_lock: Arc::new(Mutex::new(())),
            active_model: None,
            watchdog_timeout,
            watchdog_cancel: Arc::new(AtomicBool::new(false)),
            watchdog_handle: parking_lot::Mutex::new(None),
            // PERF-027: cache `is_plan_pending` results for 2 seconds so the
            // per-tool-call plan-pending gate doesn't hit disk on every
            // teammate tool invocation. The cache is explicitly invalidated
            // by `team_submit_plan` / `team_approve_plan`.
            plan_pending_cache: parking_lot::Mutex::new(HashMap::new()),
            plan_pending_ttl: Duration::from_secs(2),
            // PERF-023: lazily-loaded TaskList cache; populated on first
            // `task_list()` call or write-through mutation.
            task_cache: parking_lot::Mutex::new(TaskCacheEntry {
                list: None,
                mtime: None,
            }),
            // FR-012: lazily-loaded TeamConfig cache for the watchdog; an empty
            // entry (with no mtime) always misses, so the first tick reads disk.
            config_cache: parking_lot::Mutex::new(ConfigCacheEntry {
                config: None,
                mtime: None,
            }),
            // FR-008: filled by `self_handle()` on the first spawn. A manager
            // that is never spawned through the async loop never needs it.
            self_handle: parking_lot::Mutex::new(None),
        }
    }

    /// Borrow this manager as an `Arc<Self>`.
    ///
    /// [`Self::spawn_teammate_internal`] starts a background agent loop that
    /// records the member's status through [`Self::set_member_status`]
    /// (FR-008), so the loop needs a strong handle on the manager. The handle
    /// is captured at construction by [`Self::self_handle`]'s caller
    /// ([`Self::install_self_handle`]) for exactly that reason.
    ///
    /// # Errors
    ///
    /// Fails when no handle has been installed, which happens only for a
    /// manager created without [`Self::new_in_arc`]; no teammate can then be
    /// spawned through it.
    fn self_handle(&self) -> Result<Arc<Self>> {
        self.self_handle
            .lock()
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "team '{}' has no self handle; construct the manager with \
                     TeamManager::new_in_arc before spawning teammates",
                    self.team_name
                )
            })
    }

    /// Capture the `Arc` this manager lives in so the spawn loop can hold it.
    ///
    /// [`Self::new`] cannot know the `Arc` it is about to be placed in, so a
    /// caller that needs to configure the manager before wrapping it does so
    /// and then installs the handle once:
    ///
    /// ```ignore
    /// let mut manager = TeamManager::new(name, lead, dir, processor, bus);
    /// manager.active_model = model;
    /// let manager = TeamManager::into_arc(manager);
    /// ```
    fn install_self_handle(&self, handle: &Arc<Self>) {
        *self.self_handle.lock() = Some(Arc::downgrade(handle));
    }

    /// Create a manager together with the self handle its spawn loop needs.
    ///
    /// This is [`Self::new`] plus [`Self::install_self_handle`], returning the
    /// `Arc` so no caller can hold a manager that would refuse to spawn.
    #[must_use]
    pub fn new_in_arc(
        team_name: impl Into<String>,
        lead_session_id: impl Into<String>,
        team_dir: PathBuf,
        processor: Arc<SessionProcessor>,
        event_bus: Arc<EventBus>,
    ) -> Arc<Self> {
        Self::into_arc(Self::new(
            team_name,
            lead_session_id,
            team_dir,
            processor,
            event_bus,
        ))
    }

    /// Wrap a fully configured manager in the `Arc` its spawn loop needs.
    ///
    /// Use this instead of a bare `Arc::new` for a manager that has been
    /// configured after [`Self::new`]; a manager wrapped any other way refuses
    /// to spawn a teammate, because the loop it starts has no handle to record a
    /// status with (FR-008).
    #[must_use]
    pub fn into_arc(self) -> Arc<Self> {
        let manager = Arc::new(self);
        manager.install_self_handle(&manager);
        manager
    }

    /// Load this team's config through the mtime-keyed cache.
    ///
    /// A repeated call on an unchanged `config.json` returns the cached parse;
    /// any [`TeamStore::save`] (by this manager, another `TeamManager`, or the
    /// TUI) advances the file's `mtime` and the next call re-reads. The cache
    /// exists so the watchdog ([`Self::start_watchdog`]) can ask for every stale
    /// member's status in one tick without reading the file once per member.
    ///
    /// # Errors
    ///
    /// Fails when `config.json` cannot be read or parsed.
    fn cached_config(&self) -> Result<TeamConfig> {
        let path = self.team_dir.join("config.json");
        let disk_mtime = fs_mtime(&path);
        self.read_config_cached(disk_mtime)
    }

    /// Shared body of [`Self::cached_config`]: take the cache when its `mtime`
    /// matches `disk_mtime`, otherwise read through and store the result.
    fn read_config_cached(&self, disk_mtime: Option<std::time::SystemTime>) -> Result<TeamConfig> {
        {
            let guard = self.config_cache.lock();
            if let Some(ref config) = guard.config
                && guard.mtime == disk_mtime
            {
                return Ok(config.clone());
            }
        }
        let config = TeamStore::load(&self.team_dir)?.config;
        let mut guard = self.config_cache.lock();
        guard.config = Some(config.clone());
        guard.mtime = disk_mtime;
        Ok(config)
    }

    /// The one place a member's `Idle` transition is turned into work
    /// (spec `teamssetup` FR-015).
    ///
    /// FR-015 makes `TeamSettings.auto_claim_tasks` decide what "idle" means:
    ///
    /// - **`true`** (the default): an idle member with no current task is
    ///   handed the next claimable task automatically, so a teammate that
    ///   finishes its assignment and goes idle starts the next one without the
    ///   lead having to re-issue `team_task_claim` on its behalf.
    /// - **`false`**: the member stays `Idle`. The plan records this as the
    ///   only defensible reading (see `specs/teamssetup/PLAN.md`, M4): the
    ///   competing "the runtime claims on the teammate's behalf" reading would
    ///   make the default a no-op, because the runtime claimed for nobody
    ///   before FR-015 existed.
    ///
    /// Both entry points into `Idle` call this - [`Self::set_member_status`],
    /// which every writer goes through (the `team_idle` tool, the teammate
    /// agent loop, `resume_teammate`, task cancellation, plan rejection), so
    /// "the member went idle" is answered once and the routes cannot disagree.
    ///
    /// The claim goes through [`Self::claim_next_task`], the same function
    /// `team_task_claim` uses, so the task-claim predicate, the `TaskClaimed`
    /// hook, the `TeamTaskClaimed` event and the `current_task_id`
    /// bookkeeping are identical to an explicit claim (FR-017, FR-025,
    /// FR-026). A member that already holds a task is left alone: the
    /// `team_idle` guard forbids idling mid-task, and any other route into
    /// `Idle` has already cleared `current_task_id`.
    ///
    /// # The `Working` this may leave behind is backed by a run
    ///
    /// The claim is only made for a member that has an agent loop **still
    /// running** to work the task ([`Self::has_live_agent_loop`]). A task
    /// recorded on a roster row whose loop has already returned is work nobody
    /// will ever do: the member reads `Working`, `team_idle` refuses to park it
    /// ("you still hold a task"), and no run exists to complete the task or to
    /// call [`Self::record_progress`], so the watchdog later fails a teammate
    /// that had in fact finished. The spawn loop is the one route into `Idle`
    /// this bites: it awaits `process_message` to completion and *then* parks
    /// the member, so at that instant its loop is over. A member whose loop is
    /// over stays `Idle` and can still claim for itself (the claim rule is in
    /// its prompt) or be assigned by the lead, which is exactly what the
    /// setting-off path leaves it to do.
    ///
    /// A claim that finds nothing claimable is not an error - the member stays
    /// idle exactly as it would with the setting off, which is what
    /// `team_task_claim` reports as "no more tasks available". A failed claim
    /// (an unreadable `tasks.json`, a hook that could not run) is logged and
    /// swallowed for the same reason: the move to `Idle` is already committed
    /// and must not be reported as a failed transition because a *later*
    /// convenience step could not run.
    pub async fn auto_claim_on_idle(&self, agent_id: &str) {
        let config = match self.cached_config() {
            Ok(config) => config,
            Err(e) => {
                // Without the setting the member stays idle: the conservative
                // direction, because claiming work for a member whose team
                // settings cannot be read would act on a setting nobody saw.
                tracing::warn!(
                    team = %self.team_name,
                    agent_id,
                    error = %e,
                    "cannot read team settings; leaving the idle member idle (FR-015)"
                );
                return;
            }
        };
        if !config.settings.auto_claim_tasks {
            tracing::debug!(
                team = %self.team_name,
                agent_id,
                "auto_claim_tasks is false; holding the idle member idle (FR-015)"
            );
            return;
        }
        // A member that already held a task is not looking for one. The `Idle`
        // side effect clears `current_task_id`, so this reads the roster after
        // the write and can only see a task another writer recorded meanwhile.
        if config
            .member_by_id(agent_id)
            .is_some_and(|m| m.current_task_id.is_some())
        {
            return;
        }
        // Nobody is left to work a task for a member whose loop has returned.
        if !self.has_live_agent_loop(agent_id) {
            tracing::debug!(
                team = %self.team_name,
                agent_id,
                "auto-claim: no agent loop is running for this member; leaving it idle so the \
                 task stays claimable (FR-015)"
            );
            return;
        }

        match self.claim_next_task(agent_id).await {
            Ok((Some(task), false)) => {
                // The bookkeeping `team_task_claim` performs on a committed
                // claim: record the task on the roster so `team_status`, the
                // TUI and the `team_idle` guard see it, then move the member to
                // `Working` through the same single writer.
                if let Err(e) = self.update_member(agent_id, |m| {
                    m.current_task_id = Some(task.id.clone());
                }) {
                    tracing::warn!(
                        team = %self.team_name,
                        agent_id,
                        task_id = %task.id,
                        error = %e,
                        "auto-claim: failed to record the claimed task on the roster (FR-015)"
                    );
                    return;
                }
                if let Err(e) = self.set_member_status(
                    agent_id,
                    MemberStatus::Working,
                    TransitionCause::TaskClaimed,
                ) {
                    tracing::warn!(
                        team = %self.team_name,
                        agent_id,
                        task_id = %task.id,
                        error = %e,
                        "auto-claim: claimed a task but could not move the member to working \
                         (FR-015)"
                    );
                    return;
                }
                tracing::info!(
                    team = %self.team_name,
                    agent_id,
                    task_id = %task.id,
                    "auto-claimed the next task for an idle member (FR-015)"
                );
            }
            // Nothing claimable: the member stays idle, which is also what the
            // setting off does.
            Ok((None, _)) => {
                tracing::debug!(
                    team = %self.team_name,
                    agent_id,
                    "auto-claim found no claimable task; member stays idle (FR-015)"
                );
            }
            // The member already owned a task; leave it where it is.
            Ok((Some(task), true)) => {
                tracing::debug!(
                    team = %self.team_name,
                    agent_id,
                    task_id = %task.id,
                    "auto-claim found the member already owns a task; member stays idle (FR-015)"
                );
            }
            Err(e) => {
                tracing::warn!(
                    team = %self.team_name,
                    agent_id,
                    error = %e,
                    "auto-claim failed; the member stays idle (FR-015)"
                );
            }
        }
    }

    /// Drop the cached `config.json` parse (spec `teamssetup` FR-012).
    ///
    /// Called at the start of a write that is about to change the file's
    /// contents. Without it, a write landing inside the filesystem's `mtime`
    /// granularity would leave the cache holding the pre-write config and the
    /// watchdog could act on a status the file no longer records.
    fn invalidate_config_cache(&self) {
        let mut guard = self.config_cache.lock();
        guard.config = None;
        guard.mtime = None;
    }

    /// PERF-027: invalidate the cached `is_plan_pending` entry for `agent_id`.
    ///
    /// Called by `team_submit_plan` and `team_approve_plan` so the next
    /// `is_plan_pending` query re-reads from disk immediately after a plan
    /// status transition, rather than waiting for the TTL to expire.
    pub fn invalidate_plan_pending_cache(&self, agent_id: &str) {
        let mut cache = self.plan_pending_cache.lock();
        cache.remove(agent_id);
    }

    // -- Single member-status writer (FR-008, FR-035) ----------------------

    /// Move one member to `to`, the only place a `TeamMember::status` may be
    /// written (spec `teamssetup` FR-008, FR-035).
    ///
    /// This function is the whole status machine's write side. It
    ///
    /// 1. **validates** the transition against
    ///    [`MemberStatus::can_transition_to`], refusing an illegal move with a
    ///    message that names the member, the current status, the requested
    ///    status, and the cause, so no code path can introduce a
    ///    reachable-but-unleaveable state;
    /// 2. **applies the side effects** the destination status implies
    ///    (clearing `current_task_id` on the way out of `Working`, clearing the
    ///    plan request id on the way out of `PlanPending`, clearing the shutdown
    ///    request id on the way out of `ShuttingDown`);
    /// 3. **persists** the roster with [`TeamStore::save`], which takes the
    ///    `config.json` `flock` and writes atomically; and
    /// 4. **publishes exactly one** `Event::TeammateStatusChanged` carrying the
    ///    agent ID, the previous status, the new status, and the cause.
    ///
    /// A write that does *not* change the status is not a transition: it
    /// applies no side effect, persists nothing, and publishes nothing, so a
    /// re-asserting caller (`team_idle` on an already-idle member, a respawn
    /// recording the status a member already holds) cannot make the event
    /// stream claim that something moved.
    ///
    /// # Arguments
    ///
    /// * `agent_id` - the member to move; an unknown ID is an error, not a
    ///   silent no-op.
    /// * `to` - the destination status.
    /// * `cause` - why the caller is moving the member. It is never inferred
    ///   from the status pair: several causes share an edge, and only the
    ///   caller knows which one applies.
    ///
    /// # Returns
    ///
    /// The transition that was applied (`from`, `to`, `cause`), or `Ok(None)`
    /// when `to` is the status the member already holds.
    ///
    /// # Errors
    ///
    /// Fails when the team's `config.json` cannot be read or written, when no
    /// member has `agent_id`, or when the transition is illegal under
    /// [`MemberStatus::can_transition_to`].
    pub fn set_member_status(
        &self,
        agent_id: &str,
        to: MemberStatus,
        cause: TransitionCause,
    ) -> Result<Option<StatusTransition>> {
        self.commit_member_status(agent_id, to, cause)
            .map(|(transition, _held_task)| transition)
    }

    /// [`Self::set_member_status`] plus the `Idle` interception of FR-015.
    ///
    /// Every writer that runs inside a `tokio` runtime calls this instead of
    /// [`Self::set_member_status`]: it commits the move through the same single
    /// writer, then - **only** when the move was to `Idle` and it actually
    /// happened - asks [`Self::auto_claim_on_idle`] whether the member should
    /// immediately be given the next claimable task.
    ///
    /// The synchronous [`Self::set_member_status`] cannot do that work itself
    /// (claiming a task awaits the claim hook), so it stays the plain commit and
    /// this wrapper is the FR-015-aware one.
    ///
    /// The claim is made only for a member that still has an agent loop to work
    /// the task; see [`Self::auto_claim_on_idle`] for why a loop that has
    /// already returned is left `Idle` instead of being handed work nobody will
    /// do.
    ///
    /// # Returns
    ///
    /// `(transition, interrupted)`:
    ///
    /// - `transition` is exactly what [`Self::set_member_status`] returns;
    /// - `interrupted` is `true` when the `Idle` move was applied and
    ///   `auto_claim_tasks` then claimed a task and took the member back to
    ///   `Working`. The idle transition itself is unchanged - persisted and
    ///   published like any other - so the flag only spares the caller a second
    ///   roster read to learn where the member really is.
    ///
    /// # Errors
    ///
    /// Exactly the errors of [`Self::set_member_status`]. A claim that fails
    /// afterwards is logged, not propagated: the `Idle` move is already
    /// committed and is not a failed transition because a later convenience
    /// step could not run.
    pub async fn set_member_status_auto_claiming(
        &self,
        agent_id: &str,
        to: MemberStatus,
        cause: TransitionCause,
    ) -> Result<(Option<StatusTransition>, bool)> {
        let (transition, _held_task) = self.commit_member_status(agent_id, to, cause)?;
        if transition.is_none() || to != MemberStatus::Idle {
            return Ok((transition, false));
        }
        self.auto_claim_on_idle(agent_id).await;
        let interrupted = self
            .member_status(agent_id)
            .is_some_and(|now| now != MemberStatus::Idle);
        Ok((transition, interrupted))
    }

    /// Whether `agent_id` has an agent-loop task that has not finished.
    ///
    /// [`Self::auto_claim_on_idle`] asks this before it hands an idle member a
    /// task: a claim recorded on a roster row whose loop has returned is work
    /// nobody will ever pick up (see the `auto_claim_on_idle` docs). A member
    /// with no registered handle, or whose handle has no loop (it never
    /// started, or the loop task is finished or cancelled), has nothing left to
    /// run a task.
    ///
    /// PERF-025: `DashMap` - `.get()` returns a short-lived shard guard, so this
    /// stays a plain synchronous read.
    #[must_use]
    pub fn has_live_agent_loop(&self, agent_id: &str) -> bool {
        self.handles.get(agent_id).is_some_and(|handle| {
            handle
                .agent_handle
                .as_ref()
                .is_some_and(|join| !join.is_finished())
        })
    }

    /// Record that `agent_id` has a live agent loop (or clear it).
    ///
    /// The spawn loop is the only writer of this handle in production. It is
    /// exposed so a test can put a member into the two shapes that decide the
    /// auto-claim - a loop still running, and a loop that has returned - on top
    /// of the shared fixture, which builds a manager without one.
    #[doc(hidden)]
    pub fn set_agent_loop_handle(
        &self,
        agent_id: &str,
        handle: Option<tokio::task::JoinHandle<()>>,
    ) {
        if let Some(mut entry) = self.handles.get_mut(agent_id) {
            entry.agent_handle = handle;
            return;
        }
        tracing::warn!(
            team = %self.team_name,
            agent_id,
            "no handle registered for this member; the agent-loop handle was not recorded"
        );
    }

    /// Register the runtime handle `agent_id` would get from a spawn.
    ///
    /// A spawned teammate gets a [`TeammateHandle`] from
    /// [`Self::spawn_teammate_internal`]; a manager built over a fixture roster
    /// ([`manager_support::team_manager`] in the tests) has none, so the
    /// handle-keyed state - the agent-loop handle the auto-claim now gates on,
    /// the watchdog's `last_progress` - has nowhere to live. Registering one here
    /// makes such a fixture indistinguishable from a spawned member to every
    /// reader of the handle.
    ///
    /// [`manager_support::team_manager`]: ../../tests/manager_support/fn.team_manager.html
    #[doc(hidden)]
    pub fn register_handle_for_test(&self, agent_id: &str) {
        self.handles.insert(
            agent_id.to_string(),
            TeammateHandle {
                _name: agent_id.to_string(),
                _agent_id: agent_id.to_string(),
                _child_session_id: String::new(),
                cancel: Arc::new(AtomicBool::new(false)),
                poll_cancel: Arc::new(AtomicBool::new(false)),
                notify: Arc::new(Notify::new()),
                last_progress: Arc::new(std::sync::Mutex::new(std::time::Instant::now())),
                agent_handle: None,
                poll_handle: None,
            },
        );
    }

    /// The commit half of [`Self::set_member_status`]: validate, apply the
    /// destination's side effects, persist, and publish exactly one
    /// `TeammateStatusChanged` (FR-008, FR-035).
    fn commit_member_status(
        &self,
        agent_id: &str,
        to: MemberStatus,
        cause: TransitionCause,
    ) -> Result<(Option<StatusTransition>, bool)> {
        // This write is about to change `config.json`, so the watchdog's cached
        // parse must not survive it (FR-012).
        self.invalidate_config_cache();
        let mut store = TeamStore::load(&self.team_dir)?;

        // Read the current status before mutating so the refusal below can name
        // it, and so the event carries the real previous status. FR-015 also
        // needs the pre-write answer to "does this member hold a task": the
        // `Idle` side effect below clears `current_task_id`, so an auto-claim
        // that re-read afterwards would see a parked member and hand it work it
        // was already doing.
        let (from, member_name, held_task) = store
            .config
            .member_by_id(agent_id)
            .map(|m| (m.status, m.name.clone(), m.current_task_id.is_some()))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "cannot move a member of team '{}' to {}: no teammate with id '{agent_id}' \
                     is in the roster. Read the roster with `team_status`, which lists every \
                     member and its id",
                    self.team_name,
                    crate::team::status_word(to)
                )
            })?;

        if from == to {
            // FR-008 / FR-035: a re-asserted status is not a *transition*, so it
            // writes no roster row, appends no history row and returns `Ok(None)`.
            // It is still *published*, because publication is the only evidence
            // a re-assertion carries. A member the watchdog declared dead stays
            // `Failed` on disk until the move back to `Working` is legal; the
            // re-assertion is how its own loop says "I am still in this turn",
            // and a lead (or a panel) that never sees an event has no way to
            // distinguish a recovered teammate from a wedged one. Suppressing
            // the event made `working -> working` look exactly like silence.
            tracing::debug!(
                team = %self.team_name,
                agent_id,
                status = to.as_str(),
                cause = cause.as_str(),
                "member status re-asserted; not a transition"
            );
            //
            // FR-036: it is appended to the history for the same reason. The
            // history is what an operator reads back after the event has gone
            // past, so a teammate whose only surviving signal is a re-assertion
            // has to be findable there - otherwise the watchdog's `Failed` row
            // is the last word on the member that was, in fact, still working.
            crate::team::status_history::record_status_transition(
                &self.team_dir,
                &crate::team::status_history::StatusHistoryEntry::new(
                    agent_id,
                    &member_name,
                    from,
                    to,
                    cause,
                ),
            );
            self.event_bus.publish(Event::TeammateStatusChanged {
                session_id: self.lead_session_id.clone(),
                team_name: self.team_name.clone(),
                agent_id: agent_id.to_string(),
                from: from.as_str().to_string(),
                to: to.as_str().to_string(),
                cause: cause.as_str().to_string(),
            });
            return Ok((None, held_task));
        }

        if !from.can_transition_to(to) {
            // FR-032/FR-033: the refusal names the member, the state it holds
            // and the state it was asked for, using the same status words every
            // panel paints, and closes with the next action.
            anyhow::bail!(
                "cannot move {} to {} in team '{}' (cause: {}). The member status transition \
                 table has no edge from {} to {} - check the member with `team_status` and \
                 choose an action that matches the state it is actually in",
                crate::team::member_status_clause(&member_name, agent_id, from),
                crate::team::status_word(to),
                self.team_name,
                cause.as_str(),
                from.display_label(),
                to.display_label()
            );
        }

        // -- Side effects, derived from the destination status -------------
        let member = store.config.member_by_id_mut(agent_id).ok_or_else(|| {
            anyhow::anyhow!("teammate '{agent_id}' disappeared while being moved")
        })?;
        // T-004: this is the **only** assignment to `TeamMember::status` in the
        // workspace's manager. Every write site (spawn, the teammate agent
        // loop's idle and failure verdicts, suspend, resume, shutdown, the
        // watchdog, plan approval) calls this function instead, so a status
        // cannot move without the validation above, the side effects below, the
        // `config.json` write, and the event at the end of this function.
        member.status = to;
        match to {
            // Work finished, was released, or the session ended: the member
            // holds no task.
            MemberStatus::Idle | MemberStatus::Stopped | MemberStatus::Failed => {
                member.current_task_id = None;
            }
            // No per-status side effect beyond the status itself.
            MemberStatus::Spawning
            | MemberStatus::Working
            | MemberStatus::PlanPending
            | MemberStatus::Blocked
            | MemberStatus::Suspended
            | MemberStatus::ShuttingDown => {}
        }
        // A plan verdict was reached, so the request id it correlated has been
        // answered and must not be copied again by a later reply.
        if from == MemberStatus::PlanPending {
            member.plan_request_id = None;
        }
        // The shutdown request was answered: the correlation id has served its
        // purpose.
        if to == MemberStatus::Stopped {
            member.shutdown_request_id = None;
        }

        store.save()?;

        // FR-036: the transition is persisted next to the roster, so the tool
        // surface can answer "why did this teammate stop?" after the
        // `TeammateStatusChanged` event has already gone past a listener that
        // was never subscribed. Writing here - in the one status writer - is
        // what keeps the log and the roster from disagreeing.
        crate::team::status_history::record_status_transition(
            &self.team_dir,
            &crate::team::status_history::StatusHistoryEntry::new(
                agent_id,
                &member_name,
                from,
                to,
                cause,
            ),
        );

        // PERF-027: a status move can change whether a plan is pending, so drop
        // the cached answer instead of waiting for its TTL.
        self.invalidate_plan_pending_cache(agent_id);

        // FR-035: exactly one event per transition, carrying the complete
        // record. The coarse `TeammateIdle` / `TeammateFailed` /
        // `TeammateSuspended` / `TeammateResumed` events stay the business of
        // the call sites that own them.
        self.event_bus.publish(Event::TeammateStatusChanged {
            session_id: self.lead_session_id.clone(),
            team_name: self.team_name.clone(),
            agent_id: agent_id.to_string(),
            from: from.as_str().to_string(),
            to: to.as_str().to_string(),
            cause: cause.as_str().to_string(),
        });

        // FR-017: the spawn/stop hook pair fires from the runtime's own
        // mutation path, so a spawn or a stop reached by any route (a tool
        // call, a blueprint seed, a forced shutdown, a team cleanup) reports
        // the same way instead of only at the one call site that used to
        // publish it. Both are *entered* here, not "left": a respawn re-enters
        // `Spawning` and a second stop would be a no-op transition, so
        // `from != to` is already guaranteed above.
        match to {
            MemberStatus::Spawning => {
                // The roster load above is the read the event needs; the name
                // is the human-facing one the TUI keys its row on.
                let teammate_name = TeamStore::load(&self.team_dir)
                    .ok()
                    .and_then(|store| store.config.member_by_id(agent_id).map(|m| m.name.clone()))
                    .unwrap_or_else(|| agent_id.to_string());
                self.event_bus.publish(Event::TeammateSpawned {
                    session_id: self.lead_session_id.clone(),
                    team_name: self.team_name.clone(),
                    teammate_name,
                    agent_id: agent_id.to_string(),
                });
            }
            MemberStatus::Stopped => {
                self.event_bus.publish(Event::TeammateStopped {
                    session_id: self.lead_session_id.clone(),
                    team_name: self.team_name.clone(),
                    agent_id: agent_id.to_string(),
                    cause: cause.as_str().to_string(),
                });
            }
            // Everything else has no coarse hook of its own: the
            // `TeammateStatusChanged` above is the complete record.
            MemberStatus::Working
            | MemberStatus::Idle
            | MemberStatus::PlanPending
            | MemberStatus::Blocked
            | MemberStatus::Suspended
            | MemberStatus::ShuttingDown
            | MemberStatus::Failed => {}
        }

        tracing::info!(
            team = %self.team_name,
            agent_id,
            from = from.as_str(),
            to = to.as_str(),
            cause = cause.as_str(),
            "member status changed"
        );

        // FR-015's `Idle` interception is *not* here: the claim a
        // `true` `auto_claim_tasks` implies awaits the claim hook, so it lives in
        // [`Self::set_member_status_auto_claiming`] and this function stays the
        // synchronous commit every writer shares. `held_task` is the pre-write
        // answer that wrapper needs, since the `Idle` side effect above has now
        // cleared `current_task_id`.
        Ok((Some(StatusTransition { from, to, cause }), held_task))
    }

    /// Read one member's current status from the mtime-keyed config cache, or
    /// `None` when the roster cannot be read or has no such member.
    ///
    /// Used by the status writer to tell whether the `Idle` interception of
    /// FR-015 actually moved the member on, so an idle that stayed idle is
    /// reported as idle. A cache read is enough here: the writer's own
    /// [`Self::invalidate_config_cache`] and every [`TeamStore::save`] advance
    /// the file's `mtime`, so the answer is the status that was just written.
    fn member_status(&self, agent_id: &str) -> Option<MemberStatus> {
        self.cached_config()
            .ok()
            .and_then(|config| config.member_by_id(agent_id).map(|m| m.status))
    }

    /// Apply `mutate` to one member's record and persist the result with a
    /// single `config.json` write (spec `teamssetup` FR-008).
    ///
    /// This is the escape hatch for the fields that accompany a status change
    /// and have to land in the *same* write as it: the session id recorded at
    /// spawn, `last_spawn_error` at failure, `shutdown_request_id` at the
    /// shutdown request. A caller that needs one of those must not set the
    /// status itself - [member status writes][`Self::set_member_status`] are
    /// not permitted - so it asks for the accompanying fields here and lets the
    /// caller's own `set_member_status` call perform the move.
    ///
    /// `mutate` is applied only when the member exists; an unknown `agent_id` is
    /// an error, matching [`Self::set_member_status`].
    ///
    /// # Arguments
    ///
    /// * `agent_id` - the member to update.
    /// * `mutate` - the field updates to apply to that member's record. It must
    ///   carry its own log line when it changes something worth reporting,
    ///   because this helper does not know what changed.
    ///
    /// # Errors
    ///
    /// Fails when the team's `config.json` cannot be read or written, or when no
    /// member has `agent_id`.
    pub fn update_member<F>(&self, agent_id: &str, mutate: F) -> Result<()>
    where
        F: FnOnce(&mut TeamMember),
    {
        // This write is about to change `config.json`, so the watchdog's cached
        // parse must not survive it (FR-012).
        self.invalidate_config_cache();
        let mut store = TeamStore::load(&self.team_dir)?;
        let member = store.config.member_by_id_mut(agent_id).ok_or_else(|| {
            anyhow::anyhow!("team '{}' has no teammate '{agent_id}'", self.team_name)
        })?;
        mutate(member);
        store.save()
    }

    /// Reconcile any members recorded on-disk with `Spawning` status by
    /// attempting to spawn them now that the `TeamManager` exists.
    ///
    /// This runs in a background tokio task and will call `spawn_teammate_internal`
    /// for each queued member. Prompts are not persisted by blueprints, so an
    /// empty prompt is used for reconciliation spawns.
    pub fn reconcile_spawning_members(self: Arc<Self>) {
        let manager = Arc::clone(&self);
        tokio::spawn(async move {
            tracing::info!(team = %manager.team_name, "Reconciling spawning members from config");
            // Retry loop: sometimes blueprint seeding races with TeamManager init.
            // Attempt reconciliation multiple times with short delays to catch
            // members that are written slightly after the manager appears.
            const ATTEMPTS: usize = 10;
            for attempt in 1..=ATTEMPTS {
                tracing::debug!(team = %manager.team_name, attempt, "Reconcile attempt");
                match TeamStore::load(&manager.team_dir) {
                    Ok(store) => {
                        // Collect candidates to spawn, then drop the lock before spawning.
                        let to_spawn: Vec<(
                            String,
                            String,
                            String,
                            Option<crate::agent::ModelRef>,
                        )> = {
                            // PERF-025: DashMap - no async read guard; we just
                            // check `contains_key` on each candidate's agent_id.
                            store
                                .config
                                .members
                                .iter()
                                .filter(|m| {
                                    m.status == MemberStatus::Spawning
                                        && m.session_id.is_none()
                                        && !manager.handles.contains_key(&m.agent_id)
                                })
                                .map(|m| {
                                    (
                                        m.name.clone(),
                                        m.agent_type.clone(),
                                        m.spawn_prompt.clone().unwrap_or_default(),
                                        m.model_override.clone(),
                                    )
                                })
                                .collect()
                        };
                        if to_spawn.is_empty() {
                            tracing::info!(team = %manager.team_name, attempt, "No queued spawning members found; reconciliation complete");
                            break;
                        }
                        for (name, agent_type, spawn_prompt, member_model) in to_spawn {
                            tracing::info!(team = %manager.team_name, teammate = %name, "Attempting to spawn queued teammate (attempt: {})", attempt);
                            // Use the lead session's working directory (project root),
                            // not team_dir, so teammates resolve relative paths correctly.
                            let lead_wd = manager
                                .processor
                                .upgrade()
                                .and_then(|p| {
                                    p.session_manager
                                        .get_session(&manager.lead_session_id)
                                        .ok()
                                        .flatten()
                                        .map_or_else(
                                            || std::env::current_dir().unwrap_or_default(),
                                            |s| s.directory,
                                        )
                                        .into()
                                })
                                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                            match manager
                                .spawn_teammate_internal(
                                    &name,
                                    &agent_type,
                                    &spawn_prompt,
                                    member_model.as_ref(),
                                    manager.active_model.as_ref(),
                                    &lead_wd,
                                )
                                .await
                            {
                                Ok(agent_id) => {
                                    tracing::info!(team = %manager.team_name, teammate = %name, agent_id = %agent_id, "Successfully reconciled queued teammate");
                                }
                                Err(e) => {
                                    tracing::warn!(team = %manager.team_name, teammate = %name, error = %e, "Failed to spawn queued teammate");
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(team = %manager.team_name, error = %e, "Cannot load team store to reconcile spawning members");
                    }
                }
                // Short backoff between attempts (~1s total for 10 attempts)
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            tracing::info!(team = %manager.team_name, "Reconciliation task finished after attempts");
        });
    }

    /// Move one member of the team on disk at `team_dir` to `to`, through this
    /// manager's status writer (spec `teamssetup` FR-008, FR-026).
    ///
    /// A team tool holds a [`TeamManagerInterface`] (`Arc<dyn ...>`), not the
    /// concrete manager, so it cannot call [`Self::set_member_status`] itself.
    /// It also cannot write `TeamMember::status` on disk: that would be the
    /// second status writer FR-008 forbids, and the move would skip the
    /// transition table, the destination's side effects, and the
    /// `TeammateStatusChanged` event.
    ///
    /// The tool resolves the team directory from its own `working_dir` (an
    /// `Arc<dyn ...>` cannot know whether the manager it was handed is the team
    /// it is operating on), so this refuses a `team_dir` that is not the roster
    /// this manager owns. That makes a tool called with a mismatched
    /// `team_name` fail loudly instead of moving a member of the wrong team.
    ///
    /// # Arguments
    ///
    /// * `team_dir` - the team directory the tool resolved for its
    ///   `team_name`; it must be this manager's.
    /// * `agent_id` - the member to move.
    /// * `to` - the destination status.
    /// * `cause` - why the caller is moving the member.
    ///
    /// # Returns
    ///
    /// The applied transition, or `Ok(None)` when the member already holds `to`.
    ///
    /// # Errors
    ///
    /// Fails when `team_dir` is not this manager's team, when no member has
    /// `agent_id`, when `config.json` cannot be read or written, or when the
    /// transition is illegal under [`MemberStatus::can_transition_to`].
    pub fn set_member_status_in(
        &self,
        team_dir: &Path,
        agent_id: &str,
        to: MemberStatus,
        cause: TransitionCause,
    ) -> Result<Option<StatusTransition>> {
        self.check_team_dir(team_dir, agent_id, to)?;
        self.set_member_status(agent_id, to, cause)
    }

    /// [`Self::set_member_status_in`] plus the `Idle` interception of FR-015.
    ///
    /// The team-tool path: a tool asking to move a member to `Idle` gets the
    /// `auto_claim_tasks` decision applied before this returns, so a member
    /// with claimable work is already `Working` when the tool builds its reply.
    ///
    /// # Errors
    ///
    /// Exactly the errors of [`Self::set_member_status_in`]. A claim that fails
    /// after the `Idle` commit is logged, not propagated - the move happened.
    pub async fn set_member_status_in_auto_claiming(
        &self,
        team_dir: &Path,
        agent_id: &str,
        to: MemberStatus,
        cause: TransitionCause,
    ) -> Result<Option<StatusTransition>> {
        self.check_team_dir(team_dir, agent_id, to)?;
        let (transition, _auto_claimed) = self
            .set_member_status_auto_claiming(agent_id, to, cause)
            .await?;
        Ok(transition)
    }

    /// Refuse a `team_dir` that is not the roster this manager owns.
    ///
    /// Both [`Self::set_member_status_in`] entry points share it, so the two
    /// cannot disagree about which teams a tool may reach.
    fn check_team_dir(&self, team_dir: &Path, agent_id: &str, to: MemberStatus) -> Result<()> {
        if team_dir != self.team_dir {
            anyhow::bail!(
                "cannot move teammate '{agent_id}' to {}: {} does not belong to this team \
                 manager, which owns '{}'. Address the team this manager was created for, or \
                 open a manager for that directory",
                crate::team::status_word(to),
                team_dir.display(),
                self.team_dir.display()
            );
        }
        Ok(())
    }

    // -- Spawn ------------------------------------------------------------

    /// Spawn a new teammate session.
    ///
    /// 1. Allocates an agent ID, updates `config.json`.
    /// 2. Creates a child session.
    /// 3. Resolves the agent type and augments its system prompt.
    /// 4. Starts the teammate's agent loop in a background `tokio` task.
    /// 5. Starts a mailbox polling loop for this teammate.
    /// 6. `Event::TeammateSpawned` is published by
    ///    [`Self::set_member_status`] when the member enters `Spawning`.
    ///
    /// PERF-024: the `spawn_lock` is now held **only** for the config
    /// read -> `next_agent_id()` -> `add_member()` -> `save()` cycle that
    /// allocates the agent ID and records the `Spawning` member. Everything
    /// afterwards (child session creation, system-prompt build, memory
    /// load, handle registration, the `tokio::spawn` of the agent loop) runs
    /// **outside** the lock, so multiple teammates can be spawned
    /// concurrently. The only state that must be serialised is the
    /// agent-ID allocation + on-disk config write, and that is captured in
    /// the short critical section below.
    pub async fn spawn_teammate_internal(
        self: &Arc<Self>,
        teammate_name: &str,
        agent_type: &str,
        prompt: &str,
        teammate_model: Option<&crate::agent::ModelRef>,
        lead_model: Option<&crate::agent::ModelRef>,
        working_dir: &Path,
    ) -> Result<String> {
        // Validate the requested agent type against available agents. If it cannot
        // be resolved, warn and fall back to the default agent type. This runs
        // outside the spawn_lock (validation is read-only and never touches the
        // shared config).
        let config = Config::default();
        let resolved_agent_type =
            if crate::agent::resolve_agent_with_customs(agent_type, &config, working_dir).is_ok() {
                agent_type.to_string()
            } else {
                tracing::warn!(
                    team = %self.team_name,
                    teammate = %teammate_name,
                    agent_type = %agent_type,
                    fallback = %DEFAULT_AGENT_TYPE,
                    "Unknown agent type for teammate; falling back to default"
                );
                DEFAULT_AGENT_TYPE.to_string()
            };

        // -- PERF-024: spawn_lock held only for the config + agent-ID write --
        //
        // This is the only section that must be serialised: it reads the
        // shared `config.json`, allocates a fresh `tm-NNN` agent ID, records
        // the new `Spawning` member, and saves. Holding the lock for just
        // this cycle lets concurrent `spawn_teammate_internal` calls proceed
        // in parallel once their agent IDs are allocated.
        let agent_id = {
            let _guard = self.spawn_lock.lock().await;
            let mut store = TeamStore::load(&self.team_dir)?;
            // SEC-ragent-team-004 (SECTASKS T-052): `max_teammates` was
            // declared in `TeamSettings` but never consulted, so a blueprint or
            // an LLM loop could spawn an unbounded number of concurrent
            // teammate sessions (each an agent loop with its own LLM spend).
            // Enforce the limit here, before a fresh id is allocated and while
            // the spawn lock is held.
            //
            // teamssetup FR-016 / F-12: the check must run on **both** paths.
            // It used to sit inside `if existing.is_none()`, so respawning an
            // existing name skipped the cap entirely and an LLM loop that
            // re-spawned one name repeatedly was unbounded. The count is now
            // taken *including* the member being respawned, which is the
            // roster the spawn would produce: a new name adds one to the
            // active count, a respawn replaces a slot it already occupies.
            let max_teammates = store.config.settings.max_teammates;
            let existing = store.config.member_by_name(teammate_name).cloned();
            // A respawn of a member that is still active reuses its slot, so it
            // only has to fit the cap as it already does. A respawn of a
            // terminal member (`Stopped`/`Failed`) or a brand-new name starts
            // from no slot and must fit one more.
            let respawn_reuses_slot = existing
                .as_ref()
                .is_some_and(|m| !matches!(m.status, MemberStatus::Stopped | MemberStatus::Failed));
            let active = store
                .config
                .members
                .iter()
                .filter(|m| !matches!(m.status, MemberStatus::Stopped | MemberStatus::Failed))
                .count();
            if max_teammates > 0 && !respawn_reuses_slot && active >= max_teammates {
                // FR-032: name the limit, the current count, and the one action
                // that frees a slot, so the refusal is actionable as written.
                anyhow::bail!(
                    "cannot spawn teammate '{teammate_name}' in team '{}': {active} teammate(s) \
                     are already live and the configured maximum is {max_teammates}. Shut one \
                     down with `team_shutdown_teammate`, or raise `max_teammates` in the team's \
                     settings",
                    self.team_name
                );
            }
            // FR-008: the record is inserted in `Spawning` (the status
            // `TeamMember::new` already sets) or, for a respawn, the existing
            // records are updated here. The move to `Spawning` itself is made
            // through `set_member_status` below, after the lock is released, so
            // a respawn of a terminal member validates `Failed | Stopped ->
            // Spawning` against the transition table instead of bypassing it.
            let id = if let Some(existing) = existing {
                tracing::debug!(team = %self.team_name, teammate = %teammate_name, agent_id = %existing.agent_id, "Reusing existing member record for spawn");
                existing.agent_id.clone()
            } else {
                let id = store.next_agent_id();
                tracing::info!(team = %self.team_name, teammate = %teammate_name, agent_id = %id, "Allocating new agent id and recording Spawning member");
                let mut member = TeamMember::new(teammate_name, &id, &resolved_agent_type);
                member.model_override = teammate_model.cloned();
                store.add_member(member)?;
                id
            };
            // Persist now so external tools (e.g., team_create) see the member.
            store.save()?;
            id
        };
        // spawn_lock released here - concurrent spawns may proceed.

        // FR-008: record `Spawning` through the single status writer. A fresh
        // record is already `Spawning`, so this is a no-op for it; a respawn of
        // a member that came to rest in a parked or terminal status is validated
        // against the table and published as one `TeammateStatusChanged`.
        match self.set_member_status(&agent_id, MemberStatus::Spawning, TransitionCause::Spawned) {
            // `Ok(None)` - a fresh member is already `Spawning`.
            Ok(_) => {}
            Err(e) => {
                // A member still live in another status (`Working`, `Idle`, …)
                // has no edge to `Spawning`: reusing its record would leave the
                // roster claiming a live session that this spawn does not have.
                // Fail the spawn instead of publishing a status the table
                // forbids.
                anyhow::bail!(
                    "cannot spawn teammate '{teammate_name}' in team '{}': {e}",
                    self.team_name
                );
            }
        }

        // Create isolated child session.
        tracing::info!(team = %self.team_name, agent_id = %agent_id, "Creating child session for teammate");
        let processor = self.processor.upgrade().ok_or_else(|| {
            anyhow::anyhow!("session processor unavailable (dropped); cannot spawn teammate")
        })?;
        let child_session = processor
            .session_manager
            .create_session(working_dir.to_path_buf())?;
        let child_sid = child_session.id.clone();

        // -- Single config reload: update session, build roster, read memory -
        //
        // PERF-024: this second config write does not need the spawn_lock:
        // it only updates the *this* agent's* own member record (looked up
        // by `agent_id`, which is now unique and owned by this caller), and
        // `TeamStore::save` already takes the config.json `flock` for atomic
        // write safety. The only state that needs serialisation across
        // concurrent spawns - the agent-ID allocation - already happened
        // under the lock above.
        let (teammate_roster, memory_scope, auto_claim_tasks) = {
            let mut store = TeamStore::load(&self.team_dir)?;
            if let Some(m) = store.config.member_by_id_mut(&agent_id) {
                m.session_id = Some(child_sid.clone());
                tracing::info!(team = %self.team_name, teammate = %m.name, agent_id = %agent_id, session_id = %child_sid, "Updated team config with session id");
            } else {
                tracing::warn!(team = %self.team_name, agent_id = %agent_id, "Could not find member by id when updating session info");
            }
            let roster: Vec<(String, String)> = store
                .config
                .members
                .iter()
                .filter(|m| m.agent_id != agent_id)
                .map(|m| (m.name.clone(), m.agent_id.clone()))
                .collect();
            let mem_scope = store
                .config
                .member_by_id(&agent_id)
                .map_or(super::config::MemoryScope::None, |m| m.memory_scope);
            // FR-027: read the same setting `auto_claim_on_idle` reads, from
            // the same store read, so the prompt cannot describe a claim rule
            // the runtime will not apply.
            let auto_claim = store.config.settings.auto_claim_tasks;
            store.save()?;
            tracing::debug!(team = %self.team_name, agent_id = %agent_id, "Team config saved after session assignment");
            (roster, mem_scope, auto_claim)
        };

        // FR-008: the member becomes `Working` through the single status writer,
        // which validates `Spawning -> Working`, persists, and publishes one
        // `TeammateStatusChanged`.
        if let Err(e) =
            self.set_member_status(&agent_id, MemberStatus::Working, TransitionCause::Spawned)
        {
            tracing::warn!(team = %self.team_name, agent_id = %agent_id, error = %e,
                "failed to record teammate as Working");
            return Err(e);
        }
        tracing::info!(team = %self.team_name, agent_id = %agent_id, session_id = %child_sid, "Teammate is Working");

        // Resolve agent and augment system prompt.
        //
        // The resolved configuration, not `Config::default()`: a teammate's
        // agent profile (its model, prompt and step bounds) comes from
        // `agent.<name>` in the user's config, so a defaulted config left every
        // teammate with no model unless the caller supplied an override and the
        // spawn failed with "Agent 'general' has no model configured".
        let config = Config::load().unwrap_or_default();
        let mut agent = resolve_agent_with_customs(agent_type, &config, working_dir)
            .unwrap_or_else(|_| Arc::new(AgentInfo::new(agent_type, "Teammate agent")));
        Arc::make_mut(&mut agent).mode = AgentMode::Subagent;
        apply_teammate_model_override(Arc::make_mut(&mut agent), teammate_model, lead_model);

        // Ensure the agent has a model configured. Some custom agent names may
        // not resolve to a configured model; fall back to the built-in "general"
        // agent's model to avoid immediate startup failures in the agent loop.
        if agent.model.is_none()
            && let Ok(default_agent) = crate::agent::resolve_agent("general", &config)
        {
            Arc::make_mut(&mut agent).model = default_agent.model.clone();
            tracing::info!(team = %self.team_name, teammate = %teammate_name, agent_type = %agent_type, "No model on agent; falling back to 'general' model");
        }

        let team_addition = build_team_prompt_addition(
            &self.team_name,
            teammate_name,
            &agent_id,
            &teammate_roster,
            auto_claim_tasks,
        );
        // Append the team context block to the agent's system prompt.
        let base = agent.prompt.as_deref().unwrap_or("");
        Arc::make_mut(&mut agent).prompt = Some(Arc::from(format!("{base}\n{team_addition}")));

        // -- Persistent memory injection ------------------------------------
        // Resolve memory scope: member-level config (from blueprint) takes
        // priority, then the agent profile's setting, then None.
        let effective_scope = if memory_scope == super::config::MemoryScope::None {
            match agent.memory.as_str() {
                "user" => super::config::MemoryScope::User,
                "project" => super::config::MemoryScope::Project,
                _ => super::config::MemoryScope::None,
            }
        } else {
            memory_scope
        };
        if effective_scope != super::config::MemoryScope::None {
            let processor = self.processor.upgrade().ok_or_else(|| {
                anyhow::anyhow!("session processor unavailable (dropped); cannot load team memory")
            })?;
            let storage = processor.session_manager.storage();
            let memory_block = load_team_memory_block(storage, &self.team_name, teammate_name);
            let current = agent.prompt.as_deref().unwrap_or("");
            Arc::make_mut(&mut agent).prompt = Some(Arc::from(format!("{current}{memory_block}")));
        }

        let cancel = Arc::new(AtomicBool::new(false));
        let poll_cancel = Arc::new(AtomicBool::new(false));
        let notify = Arc::new(Notify::new());

        // Register notifier so Mailbox::push() can wake this agent's poll loop.
        register_notifier(&self.team_dir, &agent_id, Arc::clone(&notify));

        // Register handle.
        // PERF-025: DashMap - direct insert, no async write guard.
        self.handles.insert(
            agent_id.clone(),
            TeammateHandle {
                _name: teammate_name.to_string(),
                _agent_id: agent_id.clone(),
                _child_session_id: child_sid.clone(),
                cancel: cancel.clone(),
                poll_cancel: poll_cancel.clone(),
                notify: Arc::clone(&notify),
                last_progress: Arc::new(std::sync::Mutex::new(std::time::Instant::now())),
                agent_handle: None,
                poll_handle: None,
            },
        );
        // Start agent loop in background. Capture agent_id and team_dir for error persistence.
        let proc = self.processor.upgrade().ok_or_else(|| {
            anyhow::anyhow!("session processor unavailable (dropped); cannot start agent loop")
        })?;
        let proc = Arc::clone(&proc);
        // FR-012: the watchdog must hear this member's work from the runtime, not
        // from whatever UI happens to be drawing the session - see
        // [`Self::watch_teammate_progress`]. The guard lives exactly as long as
        // the turn, so the subscription ends with it.
        let progress_guard = self.watch_teammate_progress(&agent_id, &child_sid);
        let child_sid_clone = child_sid.clone();
        let agent_clone = agent.clone();
        let prompt_owned = prompt.to_string();
        let cancel_clone = cancel.clone();
        let agent_id_clone = agent_id.clone();
        let team_name_clone = self.team_name.clone();
        let lead_sid_clone = self.lead_session_id.clone();
        let event_bus_clone = self.event_bus.clone();
        // The background loop writes status through the single writer, so it
        // captures a strong handle on the manager. The loop owns this handle
        // until it exits, so it cannot observe the manager being dropped.
        let manager = Arc::clone(self);
        let agent_handle = tokio::spawn(async move {
            // FR-012: the progress subscription ends with the turn it observes.
            // It lives inside the loop task rather than beside it so a loop that
            // finishes (or is aborted) always takes its listener with it, and a
            // respawn cannot accumulate one subscription per attempt.
            let _progress_guard = progress_guard;
            // Retry with exponential backoff + jitter for transient API errors
            // (e.g. rate limits, cloud-provider cold-start).  Linear backoff
            // was insufficient - multiple teammates that failed at the same
            // moment retried at the same time and re-triggered the upstream
            // pressure.  See `teammate_retry_backoff` for the schedule.
            const MAX_RETRIES: u32 = 3;
            let mut last_error = String::new();
            for attempt in 0..=MAX_RETRIES {
                if attempt > 0 {
                    let backoff = crate::team::manager::teammate_retry_backoff(attempt);
                    tracing::info!(
                        team = %team_name_clone,
                        agent_id = %agent_id_clone,
                        attempt,
                        backoff_ms = backoff.as_millis() as u64,
                        "Retrying teammate agent loop after transient failure"
                    );
                    tokio::time::sleep(backoff).await;
                }

                match proc
                    .process_message(
                        &child_sid_clone,
                        &prompt_owned,
                        &agent_clone,
                        cancel_clone.clone(),
                    )
                    .await
                {
                    Ok(_msg) => {
                        // Teammate finished its initial prompt - mark as Idle.
                        tracing::info!(
                            team = %team_name_clone,
                            agent_id = %agent_id_clone,
                            "Teammate finished initial prompt; marking idle"
                        );
                        // FR-008: the parked move goes through the single status
                        // writer, which validates `Working -> Idle`, clears
                        // `current_task_id`, persists, and publishes one
                        // `TeammateStatusChanged`. FR-015: the auto-claiming
                        // variant is the one that asks `auto_claim_tasks` what
                        // "idle" means, so a teammate with work waiting is given
                        // it instead of parking.
                        if let Err(e) = manager
                            .set_member_status_auto_claiming(
                                &agent_id_clone,
                                crate::team::config::MemberStatus::Idle,
                                TransitionCause::ReportedIdle,
                            )
                            .await
                        {
                            tracing::warn!(team = %team_name_clone, agent = %agent_id_clone,
                                error = %e, "failed to persist teammate-idle state");
                        }
                        event_bus_clone.publish(Event::TeammateIdle {
                            session_id: lead_sid_clone,
                            team_name: team_name_clone,
                            agent_id: agent_id_clone,
                        });
                        return; // success - exit the retry loop
                    }
                    Err(e) => {
                        last_error = format!("{e}");
                        warn!(
                            child_session = %child_sid_clone,
                            error = %last_error,
                            attempt,
                            max_retries = MAX_RETRIES,
                            "Teammate agent loop error"
                        );

                        // Token overflow is handled by the session processor's compression
                        // pipeline on the next turn, so no special recovery is needed here.
                        if is_token_overflow_error(&last_error) {
                            tracing::warn!(
                                team = %team_name_clone,
                                agent_id = %agent_id_clone,
                                "Token overflow - the session processor will compress history on retry"
                            );
                        }

                        // Don't retry permanent errors (4xx except 429 / 408 / token overflow).
                        if is_permanent_api_error(&last_error) {
                            tracing::error!(
                                team = %team_name_clone,
                                agent_id = %agent_id_clone,
                                "Permanent API error - skipping remaining retries"
                            );
                            break;
                        }
                    }
                }
            }
            // All retries exhausted or permanent error - persist failure.
            tracing::error!(
                team = %team_name_clone,
                agent_id = %agent_id_clone,
                "Teammate agent loop failed after {} retries",
                MAX_RETRIES
            );
            // FR-008: the failure verdict goes through the single status writer
            // (`Working | Spawning -> Failed` are the only legal edges here).
            // `last_spawn_error` accompanies the move and lands in an earlier
            // write, so a status the table forbids keeps the recorded error
            // rather than the verdict.
            let recorded = manager
                .update_member(&agent_id_clone, |m| {
                    m.last_spawn_error = Some(last_error.clone());
                })
                .map_err(|e| {
                    warn!(error = %e, "Failed to save team config after spawn error");
                });
            if recorded.is_ok()
                && let Err(e) = manager.set_member_status(
                    &agent_id_clone,
                    crate::team::config::MemberStatus::Failed,
                    TransitionCause::SpawnFailed,
                )
            {
                warn!(error = %e, "Failed to persist teammate-failed status");
            }
            event_bus_clone.publish(Event::TeammateFailed {
                session_id: lead_sid_clone,
                team_name: team_name_clone,
                agent_id: agent_id_clone,
                error: last_error,
            });
        });

        // Start mailbox polling loop.
        let poll_handle = self.start_poll_loop(agent_id.clone(), poll_cancel, notify);

        // R-18/R-19: Store the JoinHandles so they can be force-aborted on
        // shutdown_teammate / Drop.
        if let Some(mut entry) = self.handles.get_mut(&agent_id) {
            entry.agent_handle = Some(agent_handle);
            entry.poll_handle = Some(poll_handle);
        }

        // NOTE: `Event::TeammateSpawned` is published by `set_member_status`
        // when the member enters `Spawning` (spec `teamssetup` FR-017), so it
        // is not published here: doing both would announce one spawn twice, and
        // a spawn that fails after this point would already have announced a
        // teammate that never ran.

        Ok(agent_id)
    }

    // -- Mailbox polling ---------------------------------------------------

    /// Start a tokio background task that polls `agent_id`'s mailbox and
    /// publishes events when new messages arrive.
    ///
    /// Uses `tokio::select!` to wake on either:
    /// - `notify.notified()` - instant push from [`Mailbox::push`], or
    /// - `sleep(poll_interval)` - safety net for external writers.
    fn start_poll_loop(
        &self,
        agent_id: String,
        cancel: Arc<AtomicBool>,
        notify: Arc<Notify>,
    ) -> tokio::task::JoinHandle<()> {
        let team_dir = self.team_dir.clone();
        let team_name = self.team_name.clone();
        let lead_session_id = self.lead_session_id.clone();
        let event_bus = self.event_bus.clone();
        let poll_interval = self.poll_interval;

        tokio::spawn(async move {
            loop {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }

                // Wait for a push notification or the fallback interval.
                //
                // Idle-CPU fix: the fallback was 500 ms, i.e. 2 disk-I/O
                // wakes per second per teammate even when the team is
                // completely quiet. The push path (`notify.notified()`) is
                // the normal wakeup (registered whenever a message is
                // written), so the timer only exists as a missed-notify
                // safety net; 5 s bounds a missed-notify delay to one
                // mailbox read while cutting idle wakes 10x.
                tokio::select! {
                    () = notify.notified() => {}
                    () = tokio::time::sleep(poll_interval) => {}
                }

                if cancel.load(Ordering::Relaxed) {
                    break;
                }

                // Drain unread messages.
                let mailbox = match Mailbox::open(&team_dir, &agent_id) {
                    Ok(m) => m,
                    Err(e) => {
                        warn!(agent_id, error = %e, "Cannot open mailbox for polling");
                        continue;
                    }
                };
                let unread = match mailbox.drain_unread() {
                    Ok(msgs) => msgs,
                    Err(e) => {
                        warn!(agent_id, error = %e, "Cannot drain mailbox");
                        continue;
                    }
                };

                for msg in unread {
                    publish_message_event(
                        &event_bus,
                        &lead_session_id,
                        &team_name,
                        &agent_id,
                        &msg,
                    );
                }
            }
            debug!(agent_id, "Mailbox polling loop stopped");
        })
    }

    // -- Shutdown ----------------------------------------------------------

    /// Suspend (pause) a running teammate by agent ID.
    ///
    /// Marks the member as `Suspended` through the single status writer
    /// ([`Self::set_member_status`]) and pauses the mailbox poll loop so the
    /// teammate stops receiving new messages. The agent loop continues running
    /// but will not be woken for new mailbox messages.
    /// Use [`resume_teammate`] to restore the teammate to active status.
    ///
    /// Re-asserting `Suspended` on a member that is already suspended is not a
    /// transition: nothing is written and no event is published. A member that
    /// has no edge to `Suspended` (it is already `ShuttingDown`, or terminal) is
    /// refused by the writer with the transition table's explanation.
    pub async fn suspend_teammate(&self, agent_id: &str) -> Result<()> {
        // Pause the poll loop so no new messages wake the agent.
        // PERF-025: DashMap - `.get()` returns a short-lived shard guard.
        if let Some(handle) = self.handles.get(agent_id) {
            handle.poll_cancel.store(true, Ordering::Relaxed);
        }

        // FR-008: the parked move goes through the single status writer.
        let transition = self.set_member_status(
            agent_id,
            MemberStatus::Suspended,
            TransitionCause::Suspended,
        )?;

        if transition.is_some() {
            self.event_bus.publish(Event::TeammateSuspended {
                session_id: self.lead_session_id.clone(),
                team_name: self.team_name.clone(),
                agent_id: agent_id.to_string(),
            });
        }
        tracing::info!(agent_id, "Teammate suspended");
        Ok(())
    }

    /// Resume a parked teammate by agent ID.
    ///
    /// This is the operator's single resume verb, and it accepts both parked
    /// statuses that need one (spec `teamssetup` FR-010):
    ///
    /// - `Suspended` - the member was paused by [`Self::suspend_teammate`];
    /// - `Blocked` - the member was parked on an unsatisfied task dependency.
    ///   Resuming returns it to `Idle`, which is the status that carries no
    ///   block: `Blocked` has no reason field of its own, so the exit *is* the
    ///   clearing of the block (F-06, F-25).
    ///
    /// Either way the member is restored to `Idle` and mailbox polling restarts.
    /// The move goes through the single status writer
    /// ([`Self::set_member_status`]), so it is validated against the transition
    /// table, persisted, and published as one `TeammateStatusChanged` with
    /// cause `resumed`; a `TeammateResumed` event follows for the call sites
    /// that track the coarse lifecycle.
    ///
    /// # Errors
    ///
    /// Fails when the member is unknown, when its status is neither `Suspended`
    /// nor `Blocked` (a `Working`, `PlanPending`, `ShuttingDown` or terminal
    /// member has no business being resumed), or when the team's `config.json`
    /// cannot be read or written.
    pub async fn resume_teammate(&self, agent_id: &str) -> Result<()> {
        // Only the two parked statuses that resume exists for are accepted. The
        // check reads the current status so the refusal can name it, and so a
        // `Blocked` member is told it can be resumed rather than that it is
        // "not suspended".
        let store = TeamStore::load(&self.team_dir)?;
        let current = store
            .config
            .member_by_id(agent_id)
            .map(|m| m.status)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "team '{}' has no teammate '{agent_id}'; nothing to resume",
                    self.team_name
                )
            })?;
        if !matches!(current, MemberStatus::Suspended | MemberStatus::Blocked) {
            anyhow::bail!(
                "teammate '{agent_id}' in team '{}' is {} and nothing to resume; only \
                 suspended or blocked members can be resumed",
                self.team_name,
                current.as_str()
            );
        }
        drop(store);

        // Re-enable the poll loop.
        // PERF-025: DashMap - `.get()` returns a short-lived shard guard.
        if let Some(handle) = self.handles.get(agent_id) {
            handle.poll_cancel.store(false, Ordering::Relaxed);
            // Wake the poll loop so it starts processing again.
            handle.notify.notify_one();
        }

        // FR-008: the move back to `Idle` goes through the single status writer.
        // FR-015: a resumed member that `auto_claim_tasks` is set for is handed
        // the next claimable task instead of sitting idle, exactly as a teammate
        // that finished its own work would be.
        let (transition, _auto_claimed) = self
            .set_member_status_auto_claiming(agent_id, MemberStatus::Idle, TransitionCause::Resumed)
            .await?;

        if transition.is_some() {
            self.event_bus.publish(Event::TeammateResumed {
                session_id: self.lead_session_id.clone(),
                team_name: self.team_name.clone(),
                agent_id: agent_id.to_string(),
            });
        }
        tracing::info!(agent_id, "Teammate resumed");
        Ok(())
    }

    /// Request shutdown of a teammate by agent ID.
    ///
    /// This is the **single unified shutdown path** (M3-T6) used by both the
    /// `team_shutdown_teammate` tool (via [`TeamManagerInterface::shutdown_teammate`])
    /// and internal callers such as `shutdown_all`.
    ///
    /// # Graceful vs immediate
    ///
    /// When `graceful` is `true`:
    /// - The member is marked `ShuttingDown` on disk (so `team_wait` and the
    ///   TUI can show the transition).
    /// - A `ShutdownRequest` mailbox message is pushed so a teammate that is
    ///   currently in its agent loop receives it via `team_read_messages` and
    ///   can call `team_shutdown_ack` to terminate cleanly.
    /// - Cancel flags are **not** set; the teammate is expected to ack.
    ///
    /// When `graceful` is `false` (immediate):
    /// - The agent-loop `cancel` flag and the mailbox-poll `poll_cancel` flag
    ///   are both set so the agent loop and the poll loop terminate promptly.
    /// - The mailbox notifier is deregistered so no further push wakes occur.
    /// - A `ShutdownRequest` is still pushed (in case the agent loop is
    ///   mid-turn and checks its mailbox before observing the cancel flag).
    /// - The member is marked `ShuttingDown` and then `Stopped`.
    ///
    /// The immediate path's two moves are the transition table's only route to
    /// `Stopped`: `ShuttingDown` is the sole status with an edge to it, so a
    /// member cannot reach the terminal state without passing through the
    /// request it answers (FR-003, FR-008). Both moves go through
    /// [`Self::set_member_status`], so each publishes its own
    /// `TeammateStatusChanged`.
    ///
    /// In both cases the on-disk status is updated atomically via
    /// [`TeamStore::save`]. Errors from mailbox or store I/O are propagated.
    /// Missing in-memory handles (e.g. a teammate whose agent loop already
    /// exited) are tolerated: only the on-disk status is updated.
    pub async fn shutdown_teammate(&self, agent_id: &str, graceful: bool) -> Result<()> {
        // -- Handle-level cancellation (immediate path only) ---------------
        if !graceful {
            // PERF-025: DashMap - `.get()` returns a short-lived shard guard,
            // so there's no async read lock to hold/drop.
            if let Some(handle) = self.handles.get(agent_id) {
                handle.cancel.store(true, Ordering::Relaxed);
                handle.poll_cancel.store(true, Ordering::Relaxed);
                // Wake the poll loop so it sees the cancel flag immediately.
                handle.notify.notify_one();
            }

            // R-18/R-19: Force-abort the spawned tasks so they stop
            // immediately even if they don't check the cancel flags.
            if let Some((_key, handle)) = self.handles.remove(agent_id) {
                if let Some(h) = handle.agent_handle {
                    h.abort();
                }
                if let Some(h) = handle.poll_handle {
                    h.abort();
                }
            }

            // Deregister the notifier now that this agent is shutting down.
            deregister_notifier(&self.team_dir, agent_id);
        }

        // -- Mailbox: push ShutdownRequest (both paths) --------------------
        // For graceful, this is the primary signal. For immediate, it is a
        // fallback for an agent loop that checks its mailbox before observing
        // the cancel flag.
        // M5-T4: stamp a correlation id and record it on the member so the
        // teammate's `team_shutdown_ack` reply can copy it.
        let correlation_id = uuid::Uuid::new_v4().to_string();
        let mailbox = Mailbox::open(&self.team_dir, agent_id)?;
        // SEC-ragent-team-003: a lead-authored shutdown request must carry
        // the lead session id that the push validates, or the unified
        // shutdown path is rejected at the mailbox write boundary.
        mailbox.push(
            MailboxMessage::new_correlated(
                "lead".to_string(),
                agent_id.to_string(),
                MessageType::ShutdownRequest,
                if graceful {
                    "Graceful shutdown requested by TeamManager; call team_shutdown_ack to terminate."
                } else {
                    "Immediate shutdown requested by TeamManager; agent loop cancelled."
                },
                &correlation_id,
            )
            .with_sender_session(&self.lead_session_id),
        )?;

        // -- On-disk status update -----------------------------------------
        // FR-008: the move is made by the single status writer. The
        // correlation id the ack reply copies is recorded first so it lands
        // beside the status the ack will answer.
        if let Err(e) = self.update_member(agent_id, |member| {
            // M5-T4: record the correlation id for the ack reply. Overwriting a
            // previous id is correct: it is the request currently pending.
            member.shutdown_request_id = Some(correlation_id);
        }) {
            tracing::warn!(agent_id, error = %e, "failed to record shutdown correlation id");
        }
        // R-24: For graceful shutdown, remove the handle entry so cancel
        // flags and JoinHandles are dropped rather than lingering in the
        // handles map. The teammate's agent loop will observe the
        // ShutdownRequest mailbox message and exit on its own.
        if graceful {
            if let Some((_key, handle)) = self.handles.remove(agent_id) {
                // Abort the poll loop immediately; the agent loop will
                // exit via the mailbox message.
                if let Some(h) = handle.poll_handle {
                    h.abort();
                }
            }
        }

        // `ShuttingDown -> Stopped` is the table's only route to the terminal
        // state, so an immediate shutdown makes the same request move a graceful
        // one does and then acknowledges it. `Ok(None)` on the first move is a
        // member already `ShuttingDown` (a graceful request is outstanding).
        let shutting_down = self
            .set_member_status(
                agent_id,
                MemberStatus::ShuttingDown,
                TransitionCause::ShutdownRequested,
            )
            .map_err(|e| {
                tracing::warn!(agent_id, graceful, error = %e, "shutdown status write refused");
                e
            })?;
        if !graceful {
            self.set_member_status(
                agent_id,
                MemberStatus::Stopped,
                TransitionCause::ShutdownAcked,
            )
            .map_err(|e| {
                // The request move succeeded, so the member is
                // `ShuttingDown`; a refusal here would mean it moved on
                // between the two writes. Report it rather than claiming the
                // member stopped.
                tracing::warn!(agent_id, error = %e, "shutdown stop write refused");
                e
            })?;
        }
        tracing::debug!(
            agent_id,
            graceful,
            transitioned = shutting_down.is_some(),
            "shutdown status recorded"
        );

        tracing::info!(
            agent_id,
            graceful,
            "Teammate shutdown requested (unified path)"
        );
        Ok(())
    }

    /// Shut down all active teammates and clean up.
    ///
    /// Uses the immediate (`graceful = false`) shutdown path so that lead
    /// teardown terminates all teammates promptly without waiting for acks.
    pub async fn shutdown_all(&self) -> Result<()> {
        // M6-T1: stop the watchdog so it does not race with shutdown.
        self.watchdog_cancel.store(true, Ordering::Relaxed);
        // Idle-CPU fix: abort the watchdog task as well as flagging it, so
        // an explicit team teardown never leaves the 30 s ticker running.
        let watchdog = self.watchdog_handle.lock().take();
        if let Some(h) = watchdog {
            h.abort();
        }

        // PERF-025: DashMap - `.iter()` over the map yields owned keys (or
        // we can collect via `.keys()` on a `DashMap`). No async read guard.
        let agent_ids: Vec<String> = self
            .handles
            .iter()
            .map(|entry| entry.key().clone())
            .collect();
        for id in agent_ids {
            if let Err(e) = self.shutdown_teammate(&id, false).await {
                warn!(agent_id = %id, error = %e, "Error during teammate shutdown");
            }
        }
        Ok(())
    }

    // -- Watchdog (M6-T1) & leader recovery (M6-T2) ----------------------

    /// M6-T1: Start the teammate watchdog.
    ///
    /// Spawns a background task that periodically checks each member that
    /// [`MemberStatus::is_live`] - one with an outstanding action it must finish
    /// on its own (`Spawning`, `Working`, `ShuttingDown`). If no progress had
    /// been observed for that member within [`TeamManager::watchdog_timeout`],
    /// the member is marked `Failed`, its cancel flags are set, and
    /// `Event::TeammateFailed` is published so `team_wait` can return.
    ///
    /// A parked member (`Idle`, `PlanPending`, `Blocked`, `Suspended`) is never
    /// eligible, however long it has been quiet: it is waiting on something
    /// external and cannot be wedged (FR-012). Neither is a terminal member
    /// (`Stopped`, `Failed`).
    ///
    /// Each tick reads `config.json` once through the mtime-keyed
    /// [`Self::cached_config`] and classifies **every** status explicitly - live,
    /// parked, or terminal - so no status reaches the failure verdict by
    /// fall-through (FR-012, F-08). A member missing from the config, or a tick
    /// whose config cannot be read, is treated as not live.
    ///
    /// "Progress" is recorded via [`Self::record_progress`]. The runtime feeds it
    /// from the teammate's own session event stream for as long as its turn runs
    /// ([`Self::watch_teammate_progress`]); a UI may feed it from the coarse
    /// team events it observes (`TeammateIdle`, `TeamTaskClaimed`,
    /// `TeamTaskCompleted`, `TeammateFailed`), but a headless team must not
    /// depend on one existing.
    pub fn start_watchdog(self: Arc<Self>) {
        let manager = Arc::clone(&self);
        let cancel = Arc::clone(&self.watchdog_cancel);
        let check_interval = watchdog_check_interval(self.watchdog_timeout);
        let timeout = self.watchdog_timeout;
        // Only spawn if a tokio runtime is available; tests that call
        // TeamManager::new outside a runtime will silently skip the watchdog.
        let runtime = match tokio::runtime::Handle::try_current() {
            Ok(h) => h,
            Err(_) => {
                tracing::warn!("start_watchdog called outside a tokio runtime; skipping");
                return;
            }
        };
        let task_handle = runtime.spawn(async move {
            let mut ticker = tokio::time::interval(check_interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                if cancel.load(Ordering::Relaxed) {
                    tracing::debug!(team = %manager.team_name, "watchdog: cancelled; exiting");
                    break;
                }
                // Collect candidates whose last_progress is older than the timeout.
                //
                // PERF-025: DashMap - iterate directly via `.iter()`; the
                // returned iterator yields `(String, TeammateHandle)` owned
                // entries (key is `String`, value is deref'd). No async read
                // guard is held across the iteration boundary.
                let stale: Vec<(String, std::time::Instant)> = {
                    let now = std::time::Instant::now();
                    manager
                        .handles
                        .iter()
                        .filter_map(|entry| {
                            let id = entry.key().clone();
                            let h = entry.value();
                            let lp = *h
                                .last_progress
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            if now.duration_since(lp) > timeout {
                                Some((id, lp))
                            } else {
                                None
                            }
                        })
                        .collect()
                };

                // Nothing to police: no teammate handle is registered, so no
                // member can be stalled. Skip the tick before touching disk.
                //
                // The config read below opens the companion `config.json.lock`
                // with `create(true)`, i.e. it *creates* a file inside the team
                // directory on every tick. An idle or freshly created team has
                // an empty handle map, so that read was pure overhead - one
                // `open` per tick forever - and it also re-created a file in a
                // tree an operator was deleting, making `remove_dir_all` fail
                // with ENOTEMPTY (the CI failure in
                // `test_slash_arms_deep::team_forcecleanup_with_missing_directory_reports_failure`,
                // where the immediate first tick raced the test's removal).
                if stale.is_empty() {
                    continue;
                }

                // FR-012: read the config once per tick and let every stale
                // member's status be looked up in that one parse. One read
                // failing means the watchdog cannot confirm any member is live,
                // so the whole tick is a no-op ("never fail a member it cannot
                // confirm is live") and every handle keeps its stale marker for
                // the next tick to reconsider.
                let config = match manager.cached_config() {
                    Ok(config) => Some(config),
                    Err(e) => {
                        tracing::warn!(
                            team = %manager.team_name,
                            error = %e,
                            "watchdog: cannot read team config; skipping this tick"
                        );
                        None
                    }
                };

                for (agent_id, last_progress) in stale {
                    // FR-012: every status is classified explicitly - there is no
                    // fall-through and no `unwrap_or(false)` default. The verdict
                    // below is the only place a member can be failed, so each
                    // status has to name its own outcome here.
                    //
                    // A member missing from the config (or from a tick whose
                    // config could not be read) is treated as not live: the
                    // watchdog never fails a member it cannot confirm is live.
                    let status = config
                        .as_ref()
                        .and_then(|c| c.member_by_id(&agent_id).map(|m| m.status));
                    match status {
                        // Live: the member holds an outstanding action it must
                        // finish on its own, so a silent member is genuinely
                        // stalled and is eligible for a failure verdict.
                        Some(MemberStatus::Spawning)
                        | Some(MemberStatus::Working)
                        | Some(MemberStatus::ShuttingDown) => {}
                        // Parked: waiting on something external (a task, a plan
                        // verdict, a dependency, the lead). Time spent parked is
                        // not evidence of a wedge, so it can never be failed -
                        // however long ago it last made progress.
                        Some(MemberStatus::Idle)
                        | Some(MemberStatus::PlanPending)
                        | Some(MemberStatus::Blocked)
                        | Some(MemberStatus::Suspended) => {
                            // Refresh the marker so a parked member is not
                            // re-examined on every tick while it waits.
                            if let Some(h) = manager.handles.get(&agent_id) {
                                *h.last_progress
                                    .lock()
                                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                                    std::time::Instant::now();
                            }
                            continue;
                        }
                        // Terminal: no session is left to fail.
                        Some(MemberStatus::Stopped) | Some(MemberStatus::Failed) => {
                            if let Some(h) = manager.handles.get(&agent_id) {
                                *h.last_progress
                                    .lock()
                                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                                    std::time::Instant::now();
                            }
                            continue;
                        }
                        // Member absent from a config that was read, or no config
                        // at all: not confirmed live, so not failed.
                        None => {
                            tracing::debug!(
                                team = %manager.team_name,
                                agent_id = %agent_id,
                                "watchdog: member status unavailable; not eligible for failure"
                            );
                            continue;
                        }
                    }

                    tracing::warn!(
                        team = %manager.team_name,
                        agent_id = %agent_id,
                        ?last_progress,
                        "watchdog: no progress for teammate within timeout; marking Failed"
                    );

                    // Set cancel flags so any live agent loop terminates.
                    // PERF-025: DashMap - `.get()` returns a short-lived guard.
                    if let Some(h) = manager.handles.get(&agent_id) {
                        h.cancel.store(true, Ordering::Relaxed);
                        h.poll_cancel.store(true, Ordering::Relaxed);
                        h.notify.notify_one();
                    }
                    deregister_notifier(&manager.team_dir, &agent_id);

                    // Mark Failed on disk through the single status writer
                    // (FR-008), so the move is validated, publishes exactly one
                    // `TeammateStatusChanged`, and drops the caches whose answers
                    // it invalidates. `last_spawn_error` accompanies the move and
                    // lands in the same `config.json` write.
                    //
                    // The read failed before the write, so a member that vanished
                    // in between yields an error here rather than being failed;
                    // best-effort, logged so an unpersisted failure is visible.
                    let error_msg = format!(
                        "watchdog: no progress for {agent_id} within {}s",
                        timeout.as_secs()
                    );
                    if let Err(e) = manager.update_member(&agent_id, |m| {
                        m.last_spawn_error = Some(error_msg.clone());
                    }) {
                        tracing::warn!(team = %manager.team_name, agent = %agent_id,
                            error = %e, "failed to record watchdog error on teammate");
                    }
                    match manager.set_member_status(
                        &agent_id,
                        MemberStatus::Failed,
                        TransitionCause::WatchdogTimeout,
                    ) {
                        Ok(_) => {}
                        Err(e) => {
                            // A member that moved to a status from which `Failed`
                            // is illegal (or vanished) keeps its status: the
                            // verdict is lost for this tick, and the failure is
                            // logged rather than forced through the table.
                            tracing::warn!(team = %manager.team_name, agent = %agent_id,
                                error = %e, "failed to persist teammate-failed state");
                        }
                    }

                    manager.event_bus.publish(Event::TeammateFailed {
                        session_id: manager.lead_session_id.clone(),
                        team_name: manager.team_name.clone(),
                        agent_id: agent_id.clone(),
                        error: error_msg,
                    });
                }
            }
        });
        // Idle-CPU fix: remember the task so `Drop` can abort it instead of
        // leaving a 30 s-interval tick running for the rest of the session.
        *self.watchdog_handle.lock() = Some(task_handle);
    }

    /// M6-T2 / FR-024: reset **every** `InProgress` task in `team_dir` to
    /// `Pending` and return the IDs it reset.
    ///
    /// Called from [`TeamManager::new`] / [`TeamManager::try_new`] only on an
    /// explicit [`AdoptTeam::Adopt`], after the FR-023 consent check. FR-024
    /// makes the reset symmetric: a task left `InProgress` by a departed
    /// **teammate** is as orphaned as one left by the previous lead, so
    /// ownership is not consulted at all - only the status is.
    ///
    /// Each reset goes through [`transition_task`], the single writer of
    /// [`Task::status`] (FR-003), so the `InProgress -> Pending` edge is
    /// validated by the same table every other path uses. The write happens
    /// under the `TaskStore` lock and no hook is fired: an adoption is not a
    /// claim, completion or creation, so FR-017 has no event for it; the
    /// adoption itself is announced by [`Event::TeamAdopted`].
    ///
    /// # Errors
    ///
    /// When the team directory has no readable task store. An individual task
    /// that refuses the reset is logged and skipped rather than failing the
    /// whole adoption, so one malformed entry cannot strand the rest.
    ///
    /// [`transition_task`]: crate::team::task::transition_task
    pub fn adopt_orphaned_tasks(team_dir: &Path, old_lead_sid: &str) -> Result<Vec<String>> {
        let task_store = crate::team::task::TaskStore::open(team_dir)?;
        // Read first so a directory with no tasks at all does not create a
        // `tasks.json` just to write nothing back to it.
        let list = task_store.read()?;
        let orphaned: Vec<String> = list
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::InProgress)
            .map(|t| t.id.clone())
            .collect();
        if orphaned.is_empty() {
            return Ok(Vec::new());
        }

        let mut reset: Vec<String> = Vec::new();
        task_store.write_through_fallible(|list| {
            for task_id in &orphaned {
                // FR-003: the status moves through `transition_task`, never by
                // assignment, so the adoption obeys the same D3 table.
                match transition_task(list, task_id, TaskStatus::Pending, "adopt") {
                    Ok(Some(_)) => {
                        tracing::info!(
                            task_id = %task_id,
                            old_lead = old_lead_sid,
                            "resetting orphaned InProgress task to Pending on adoption"
                        );
                        if let Some(task) = list.get_mut(task_id) {
                            task.assigned_to = None;
                            task.claimed_at = None;
                        }
                        reset.push(task_id.clone());
                    }
                    Ok(None) => {}
                    Err(e) => {
                        tracing::warn!(team_dir = %team_dir.display(), task_id = %task_id,
                            error = %e, "failed to reassign orphaned task to Pending");
                    }
                }
            }
            Ok(())
        })?;
        Ok(reset)
    }

    // -- PERF-023: in-memory TaskList cache with write-through persistence --

    /// Return the current [`TaskList`] for this team, using the in-memory
    /// cache when the on-disk `tasks.json` `mtime` has not advanced.
    ///
    /// PERF-023: the first call loads `tasks.json` once and caches both the
    /// parsed list and the file's `mtime`. Subsequent calls return the
    /// cached list in O(1). If another process (or a `team_task_*` tool
    /// that bypassed the cache) wrote to `tasks.json`, the `mtime` will
    /// have advanced and the cache is transparently reloaded - so the
    /// cache is always consistent with disk without sacrificing the O(1)
    /// hit path.
    pub fn task_list(&self) -> Result<TaskList> {
        let path = self.team_dir.join("tasks.json");
        let disk_mtime = fs_mtime(&path);
        {
            let guard = self.task_cache.lock();
            if let Some(ref list) = guard.list {
                if guard.mtime == disk_mtime {
                    return Ok(list.clone());
                }
            }
        }
        // Cache miss / stale: reload from disk and store back.
        let store = crate::team::task::TaskStore::open(&self.team_dir)?;
        let list = store.read()?;
        let mut guard = self.task_cache.lock();
        guard.list = Some(list.clone());
        guard.mtime = disk_mtime;
        Ok(list)
    }

    /// PERF-023: invalidate the in-memory [`TaskList`] cache. The next
    /// [`Self::task_list`] call will reload from disk.
    ///
    /// Called after any operation that bypassed the write-through path
    /// (e.g. `team_task_*` tools that open their own `TaskStore`). The
    /// mtime-based invalidation in [`Self::task_list`] makes this strictly
    /// optional, but calling it after a known external mutation avoids a
    /// one-stale-read window.
    pub fn invalidate_task_cache(&self) {
        let mut guard = self.task_cache.lock();
        guard.list = None;
        guard.mtime = None;
    }

    /// PERF-023: apply `f` to the team's task list with write-through
    /// persistence.
    ///
    /// Acquires the `TaskStore` exclusive flock, re-reads the on-disk list
    /// (so the mutation is reconciled with any external writes that
    /// happened since the cache was loaded), applies `f`, writes the
    /// result back atomically, and refreshes the in-memory cache with the
    /// freshly-written list + its new `mtime`.
    ///
    /// This is the single safe entry point for mutating the shared task
    /// list through the `TeamManager` cache: it never lets the cache and
    /// the disk drift apart.
    pub fn apply_to_task_list<F>(&self, f: F) -> Result<TaskList>
    where
        F: FnOnce(&mut TaskList),
    {
        self.try_apply_to_task_list(|list| {
            f(list);
            Ok(())
        })
    }

    /// Fallible [`TeamManager::apply_to_task_list`]: `f` may refuse the mutation
    /// by returning an error, in which case nothing is written, the
    /// in-memory cache is dropped (so the next read re-loads from disk rather
    /// than serving a half-applied view), and `tasks.json` is left
    /// byte-identical.
    ///
    /// The cached counterpart of the structural guarantee [`TaskStore::complete`]
    /// gets for free by returning before it reaches the write lock: a refused
    /// operation must not reach `write_locked` even to bump `updated_at`, or
    /// FR-002's "shall not mutate" would hold for the task but not the file.
    ///
    /// [`TaskStore::complete`]: crate::team::task::TaskStore::complete
    pub fn try_apply_to_task_list<F>(&self, f: F) -> Result<TaskList>
    where
        F: FnOnce(&mut TaskList) -> Result<()>,
    {
        let store = crate::team::task::TaskStore::open(&self.team_dir)?;
        let written = match store.write_through_fallible(f) {
            Ok(written) => written,
            Err(e) => {
                // The closure may have mutated its copy before refusing; that
                // copy is discarded, but the cache must not keep serving a view
                // it can no longer prove matches the disk.
                let mut guard = self.task_cache.lock();
                guard.list = None;
                guard.mtime = None;
                return Err(e);
            }
        };
        // Refresh the cache from the write-through result + the new mtime.
        let path = self.team_dir.join("tasks.json");
        let disk_mtime = fs_mtime(&path);
        let mut guard = self.task_cache.lock();
        guard.list = Some(written.clone());
        guard.mtime = disk_mtime;
        Ok(written)
    }

    /// PERF-023: claim the next available task for `agent_id` via the
    /// write-through cache.
    ///
    /// The decision (one task at a time, which tasks are claimable, what the
    /// move writes, what a refusal says) is [`claim_task_in_list`]'s, the same
    /// function [`TaskStore::claim_next`] calls, so the cached and uncached
    /// paths cannot disagree (FR-025, FR-026). This method only supplies the
    /// caching shape the manager adds: the mutation runs inside
    /// [`Self::apply_to_task_list`], which holds the `TaskStore` exclusive flock
    /// for the whole read-modify-write cycle (NFR-003) and refreshes the
    /// in-memory cache from what was written.
    ///
    /// A committed claim is followed by the
    /// [`TaskClaimed`](crate::team::HookEvent::TaskClaimed) hook and the
    /// `TeamTaskClaimed` event, both fired from here rather than from the
    /// `team_task_claim` tool so an auto-claim and a lead-side claim fire them
    /// too (FR-017). A no-change outcome fires and publishes nothing.
    ///
    /// Returns `(Some(task), already_had)` where `already_had` is `true` if the
    /// agent already owned an in-progress task, or `(None, false)` when nothing
    /// is claimable.
    ///
    /// [`claim_task_in_list`]: crate::team::task::claim_task_in_list
    /// [`TaskStore::claim_next`]: crate::team::task::TaskStore::claim_next
    pub async fn claim_next_task(&self, agent_id: &str) -> Result<(Option<Task>, bool)> {
        /// The claim outcome, carried out of the write-through closure.
        enum Claimed {
            /// The closure is refusing the write because it mutated nothing.
            NoChange(TaskClaim),
            /// The closure mutated the list and wants it persisted.
            Applied(TaskClaim),
        }
        let mut claimed_out: Option<Claimed> = None;
        let applied = self.try_apply_to_task_list(|list| {
            let claim = crate::team::task::claim_task_in_list(list, None, agent_id)?;
            // An `AlreadyOwned`/`NoneAvailable` outcome mutated nothing, so
            // refusing the write keeps `updated_at` from moving on a no-op.
            let mutated = matches!(claim, TaskClaim::Claimed(_));
            claimed_out = Some(if mutated {
                Claimed::Applied(claim)
            } else {
                Claimed::NoChange(claim)
            });
            if mutated {
                Ok(())
            } else {
                Err(anyhow::anyhow!("claim_next made no change"))
            }
        });
        let claim = match claimed_out {
            Some(Claimed::Applied(claim)) => {
                applied?;
                claim
            }
            // The closure's refusal was the no-change signal, so the `Err` it
            // produced carries no failure; the cache is dropped by
            // `try_apply_to_task_list`, which is correct because nothing was
            // written and the next read re-loads from disk.
            Some(Claimed::NoChange(claim)) => claim,
            // The closure never ran (a load or parse failure): report it.
            None => {
                return Err(applied
                    .err()
                    .unwrap_or_else(|| anyhow::anyhow!("claim_next closure did not run")));
            }
        };
        let claimed = claim.task().cloned();
        if claim.is_already_owned() {
            return Ok((claimed, true));
        }

        if let Some(task) = claimed.as_ref() {
            self.publish_task_event(TaskEventKind::Claimed, task, agent_id);
            // FR-017: fire the claim hook from the runtime, not the tool.
            let transition = TaskTransition {
                task_id: task.id.clone(),
                from: TaskStatus::Pending,
                to: TaskStatus::InProgress,
                actor: agent_id.to_owned(),
            };
            crate::team::task::fire_task_hook(&self.team_dir, Some(&transition), task, agent_id)
                .await?;
        }
        Ok((claimed, false))
    }

    /// PERF-023: mark `task_id` as `Completed` by `agent_id` via the
    /// write-through cache.
    ///
    /// The decision (ownership, idempotency, legal transition) is
    /// [`complete_task_in_list`]'s, the same function [`TaskStore::complete`]
    /// calls, so the cached and uncached paths cannot disagree (FR-025,
    /// FR-026). This method only supplies the caching shape the manager adds:
    /// the mutation runs inside [`Self::apply_to_task_list`], which holds the
    /// `TaskStore` exclusive flock for the whole read-modify-write cycle
    /// (NFR-003) and refreshes the in-memory cache from what was written.
    ///
    /// The completion commit is followed by the
    /// [`TaskCompleted`](crate::team::HookEvent::TaskCompleted) hook, fired from
    /// here rather than from the `team_task_complete` tool so a lead-side
    /// completion fires it too (FR-017), and by the `TeamTaskCompleted` event the
    /// TUI and SSE surfaces observe. Both happen *after* the write, so a refused
    /// completion fires and publishes nothing.
    ///
    /// # Visible refusals (FR-044)
    ///
    /// An **ownership** refusal is a tightening the previous release did not
    /// have - it silently re-assigned the task to whoever completed it - so it
    /// publishes [`Event::TeamTaskCompletionRejected`] before the error is
    /// returned. The event carries the caller, the task, its real owner, the
    /// rule that fired and the exact refusal text, and the same facts are
    /// written at `warn` under the `ragent_agent::task` target so the refusal
    /// survives in `log/` when nothing subscribes. A task that does not exist is
    /// still only an error: the previous release refused it too, so it is not a
    /// behaviour change.
    ///
    /// The lead is the agent ID `"lead"` (the identifier [`TeamContext`] assigns
    /// to the lead session), so the lead can complete on an owner's behalf even
    /// though it owns no task (FR-001).
    ///
    /// # Returns
    ///
    /// The completed [`Task`] plus the completion hook's [`HookOutcome`], or the
    /// refusal [`complete_task_in_list`] produced - including the FR-001
    /// ownership error, which is returned without any mutation, so `tasks.json`
    /// is left as it was. A hook rejection (`HookOutcome::Feedback`) is returned
    /// rather than undoing the completion: the task is done, and FR-018 makes
    /// the verdict the caller's to deliver.
    ///
    /// [`Event::TeamTaskCompletionRejected`]: crate::event::Event::TeamTaskCompletionRejected
    /// [`TaskStore::complete`]: crate::team::task::TaskStore::complete
    /// [`TeamContext`]: crate::tool::TeamContext
    pub async fn complete_task(
        &self,
        task_id: &str,
        agent_id: &str,
    ) -> Result<(Task, HookOutcome)> {
        let is_lead = agent_id == "lead";
        let mut completed_out: Option<Task> = None;
        // The refusal is returned from inside the closure so the write lock is
        // dropped before it is reached: a refused completion leaves
        // `tasks.json` byte-identical (FR-002, FR-004).
        self.try_apply_to_task_list(|list| {
            let task = crate::team::task::complete_task_in_list(list, task_id, agent_id, is_lead)?;
            completed_out = Some(task);
            Ok(())
        })
        .inspect_err(|e| {
            // FR-044: publish `TeamTaskCompletionRejected` for an ownership
            // refusal so the tightening the previous release did not have is
            // visible in the event stream. A "task not found" error carries no
            // `CompletionRefusal` and is not published: nothing was accepted
            // before either, so it is not a behaviour change.
            if let Some(refusal) = e.downcast_ref::<crate::team::CompletionRefusal>() {
                crate::team::task::publish_completion_rejected(
                    refusal,
                    &self.team_name,
                    Some(&self.lead_session_id),
                    Some(&self.event_bus),
                );
            }
        })?;
        let completed =
            completed_out.ok_or_else(|| anyhow::anyhow!("task '{task_id}' not found"))?;

        self.publish_task_event(TaskEventKind::Completed, &completed, agent_id);

        // FR-017: fire the completion hook from the runtime, not the tool.
        let transition = TaskTransition {
            task_id: completed.id.clone(),
            from: TaskStatus::InProgress,
            to: TaskStatus::Completed,
            actor: agent_id.to_owned(),
        };
        let outcome = crate::team::task::fire_task_hook(
            &self.team_dir,
            Some(&transition),
            &completed,
            agent_id,
        )
        .await?;
        Ok((completed, outcome))
    }

    /// PERF-023: cancel `task_id` via the write-through cache (spec `teamssetup`
    /// FR-005, FR-006).
    ///
    /// The decision (which statuses may be cancelled, what the refusal says, the
    /// legal transition) is [`cancel_task_in_list`]'s, the same function
    /// [`TaskStore::cancel`] calls, so the cached and uncached paths cannot
    /// disagree (FR-025, FR-026). This method only supplies the caching shape the
    /// manager adds: the mutation runs inside [`Self::try_apply_to_task_list`],
    /// which holds the `TaskStore` exclusive flock for the whole read-modify-write
    /// cycle (NFR-003) and refreshes the in-memory cache from what was written. A
    /// refused cancellation returns from inside that closure, so `tasks.json` is
    /// left byte-identical (FR-004).
    ///
    /// # Mailbox notification (FR-005)
    ///
    /// When the cancelled task was `InProgress` and assigned, the owning teammate
    /// is told through its mailbox with a [`MessageType::TaskCancelled`] message so
    /// it stops working on work the lead has withdrawn rather than discovering it
    /// at completion time. The notification is best-effort and is sent *after* the
    /// status write: the cancellation is already committed, so a mailbox failure is
    /// reported in the returned [`TaskCancellation::notified`] rather than rolling
    /// the task back. A `Pending` task has nobody working on it and is not notified.
    ///
    /// # Arguments
    ///
    /// * `task_id` - ID of the task to cancel.
    /// * `agent_id` - the caller's agent ID. Only `"lead"` may cancel (FR-005);
    ///   any other actor is refused before the task list is touched.
    /// * `reason` - optional free-text reason, carried in the notification.
    ///
    /// # Returns
    ///
    /// The [`TaskCancellation`] for the cancelled task, or the refusal
    /// [`cancel_task_in_list`] produced.
    ///
    /// [`cancel_task_in_list`]: crate::team::task::cancel_task_in_list
    /// [`TaskStore::cancel`]: crate::team::task::TaskStore::cancel
    pub fn cancel_task(
        &self,
        task_id: &str,
        agent_id: &str,
        reason: Option<&str>,
    ) -> Result<TaskCancellation> {
        // FR-005: cancellation is the lead's verb. Checked here, before the task
        // list is loaded, so a teammate's attempt touches nothing at all.
        if agent_id != "lead" {
            return Err(anyhow::anyhow!(
                "task '{task_id}' cannot be cancelled by {agent_id}: only the lead may cancel a \
                 task. Ask the lead to cancel it, or complete it with `team_task_complete` if \
                 it is yours and finished"
            ));
        }

        let mut cancelled_out: Option<TaskCancellation> = None;
        // The refusal is returned from inside the closure so the write lock is
        // dropped before it is reached: a refused cancellation leaves
        // `tasks.json` byte-identical (FR-004).
        self.try_apply_to_task_list(|list| {
            let cancelled = crate::team::task::cancel_task_in_list(list, task_id, agent_id)?;
            cancelled_out = Some(cancelled);
            Ok(())
        })?;
        let mut cancelled =
            cancelled_out.ok_or_else(|| anyhow::anyhow!("task '{task_id}' not found"))?;

        // FR-005: tell the owner it no longer needs to finish the task. Sent after
        // the write so a mailbox failure cannot undo a committed cancellation; the
        // outcome is recorded on the result so the caller can report it.
        if let Some(owner) = cancelled.notify_target().map(str::to_owned) {
            let content = cancellation_notice(&cancelled.task, reason);
            // SEC-ragent-team-003: a lead-authored message carries the lead session
            // id the mailbox write boundary validates.
            let delivered = Mailbox::open(&self.team_dir, &owner).and_then(|mailbox| {
                mailbox.push(
                    MailboxMessage::new(
                        "lead".to_owned(),
                        owner.clone(),
                        MessageType::TaskCancelled,
                        content,
                    )
                    .with_sender_session(&self.lead_session_id),
                )
            });
            match delivered {
                Ok(()) => cancelled.notified = Some(owner),
                // Reported, not swallowed: a lead must not be told the teammate
                // knows when it does not. The cancellation itself stands.
                Err(e) => {
                    warn!(
                        agent_id = %owner,
                        task_id = %cancelled.task.id,
                        error = %e,
                        "failed to notify the owner of a cancelled task"
                    );
                    return Err(anyhow::anyhow!(
                        "task '{task_id}' was cancelled, but notifying {owner} failed: {e}"
                    ));
                }
            }
        }

        Ok(cancelled)
    }

    /// M6-T1: Record progress for `agent_id` (called when an event indicates
    /// the teammate did something). Resets the watchdog timer. Supplied by
    /// [`Self::watch_teammate_progress`] for the life of each turn.
    pub fn record_progress(&self, agent_id: &str) {
        // PERF-025: DashMap - `.get()` returns a short-lived shard guard; no
        // async read lock, so this method remains sync (matching its
        // pre-DashMap contract).
        if let Some(h) = self.handles.get(agent_id) {
            *h.last_progress
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = std::time::Instant::now();
        }
    }

    /// Keep `agent_id`'s watchdog timer fresh from its own session's event
    /// stream until the returned handle is dropped (FR-012).
    ///
    /// [`Self::record_progress`] used to be fed only by the TUI, which saw a
    /// teammate's work second-hand through the events it consumed. A teammate
    /// driven by a headless server, an automation run, or a lead whose panel is
    /// not open produced no such events at all, so its `last_progress` stayed
    /// where the spawn put it and the watchdog failed it after the no-progress
    /// window - while its agent loop was in the middle of a turn. The turn's own
    /// messages are the authoritative evidence that it is progressing, so the
    /// runtime subscribes here rather than trusting a display to relay it.
    ///
    /// Every `Event` that carries the child session id counts: text and
    /// reasoning deltas while the model streams, `ToolCallStart` /
    /// `ToolCallEnd` / `ToolResult` while a tool runs. That is strictly more
    /// than the TUI's coarse set, which is what a long single-tool step needs -
    /// a `Bash` call that runs for minutes publishes its start and its end and
    /// nothing in between, and either one resets the window.
    ///
    /// "No progress" is the absence of *any* activity, not a short quiet spell:
    /// a turn with no events for [`TeamManager::watchdog_timeout`] is treated as
    /// wedged again. The handle is aborted when the caller drops it - the spawn
    /// loop holds it for exactly the life of the turn - so a finished teammate
    /// leaves no listener behind.
    fn watch_teammate_progress(
        self: &Arc<Self>,
        agent_id: &str,
        child_session_id: &str,
    ) -> crate::session::AbortOnDrop {
        let mut rx = self.event_bus.subscribe();
        let cancel = Arc::clone(&self.watchdog_cancel);
        let manager = Arc::clone(self);
        let agent_id = agent_id.to_string();
        let child_session_id = child_session_id.to_string();
        let task = tokio::spawn(async move {
            loop {
                // A cancelled bus (the manager is shutting down) or a cancelled
                // watchdog means nothing will read the timer again.
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                match rx.recv().await {
                    Ok(event) => {
                        if event.session_id() == Some(child_session_id.as_str()) {
                            manager.record_progress(&agent_id);
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // A listener that fell behind still observed activity:
                        // the events it missed were progress.
                        manager.record_progress(&agent_id);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                }
            }
        });
        crate::session::AbortOnDrop(task)
    }

    /// [`Self::watch_teammate_progress`] for tests.
    ///
    /// The listener is private because the spawn loop is its only production
    /// caller; this re-export lets the watchdog tests drive the feed directly
    /// instead of standing up a whole agent turn to produce the events it
    /// consumes.
    #[doc(hidden)]
    pub fn watch_teammate_progress_for_test(
        self: &Arc<Self>,
        agent_id: &str,
        child_session_id: &str,
    ) -> impl Drop {
        self.watch_teammate_progress(agent_id, child_session_id)
    }

    // -- Plan approval -----------------------------------------------------

    /// Approve a plan for a teammate (shorthand used by the plan approval tool).
    ///
    /// The plan verdict itself is recorded through [`Self::update_member`]; the
    /// status move an approval implies (`PlanPending -> Working`) is made by the
    /// single status writer (FR-008). A rejection leaves the member in
    /// `PlanPending`: the plan was not approved, so the teammate is still
    /// waiting on the lead.
    pub fn approve_plan(&self, agent_id: &str, approved: bool) -> Result<()> {
        // FR-008: `Working` is reached only through the single status writer. A
        // member that is not `PlanPending` has no edge to `Working` from the
        // plan path, so the refusal is reported instead of silently ignored.
        if approved {
            self.set_member_status(
                agent_id,
                MemberStatus::Working,
                TransitionCause::PlanApproved,
            )?;
        }
        self.update_member(agent_id, |m| {
            m.plan_status = if approved {
                PlanStatus::Approved
            } else {
                PlanStatus::Rejected
            };
        })?;
        // PERF-027: the plan status just changed - drop the cached entry so
        // the next `is_plan_pending` query observes the new value
        // immediately instead of waiting for the TTL.
        self.invalidate_plan_pending_cache(agent_id);
        Ok(())
    }

    /// Returns `true` if the teammate has a pending plan (used by the processor
    /// to block write/bash tools while `PlanPending`).
    ///
    /// PERF-027: results are cached per-agent for `plan_pending_ttl` (2s by
    /// default) so the session processor's per-tool-call gate does not hit
    /// disk on every invocation. The cache is invalidated explicitly by
    /// `team_submit_plan` / `team_approve_plan` via
    /// [`invalidate_plan_pending_cache`].
    #[must_use]
    pub fn is_plan_pending(&self, agent_id: &str) -> bool {
        self.cached_plan_query(agent_id, |m| m.plan_status == PlanStatus::Pending)
    }

    /// Returns `true` when this teammate's write- and bash-class tool calls must
    /// be refused for want of an approved plan (spec `teamssetup` FR-014, F-10).
    ///
    /// This is the plan-approval gate the dispatch pipeline asks, and it is
    /// distinct from [`Self::is_plan_pending`] on purpose:
    ///
    /// - `require_plan_approval` must be `true`. With the setting off the gate is
    ///   inert and only [`Self::is_plan_pending`]'s `PlanPending` restriction
    ///   applies.
    /// - The member's `plan_status` must not be `Approved` - so `None` (never
    ///   submitted), `Pending` (awaiting the verdict) and `Rejected` (the lead
    ///   said no) all gate. Gating only on `Pending` is exactly the F-10 hole:
    ///   a teammate that never submits a plan was never stopped.
    /// - A member missing from the roster does not gate: the tool call belongs
    ///   to a session this manager cannot see, and a gate that fires on absent
    ///   data fails closed against the wrong agent.
    ///
    /// The result is cached with [`Self::is_plan_pending`]'s entry, which is
    /// invalidated by `team_submit_plan` and `team_approve_plan`, so the two
    /// queries never disagree about the same observation.
    #[must_use]
    pub fn is_plan_gate_active(&self, agent_id: &str) -> bool {
        let require_approval = TeamStore::load(&self.team_dir)
            .map(|s| s.config.settings.require_plan_approval)
            .unwrap_or(false);
        if !require_approval {
            return false;
        }
        self.cached_plan_query(agent_id, |m| m.plan_status != PlanStatus::Approved)
    }

    /// Answer one `plan_status` question about `agent_id`, through the PERF-027
    /// cache.
    ///
    /// `predicate` decides what the cached `pending` bit means; both callers above
    /// share one entry because both are invalidated by the same two events (plan
    /// submitted, verdict recorded). A second entry keyed on the same agent would
    /// only make the two queries disagree for up to the TTL.
    fn cached_plan_query(&self, agent_id: &str, predicate: impl Fn(&TeamMember) -> bool) -> bool {
        {
            let mut cache = self.plan_pending_cache.lock();
            if let Some(entry) = cache.get(agent_id) {
                if entry.observed_at.elapsed() < self.plan_pending_ttl {
                    return entry.pending;
                }
                // Expired - fall through and re-read from disk.
                cache.remove(agent_id);
            }
        }
        let pending = TeamStore::load(&self.team_dir)
            .ok()
            .and_then(|s| s.config.member_by_id(agent_id).map(predicate))
            .unwrap_or(false);
        let mut cache = self.plan_pending_cache.lock();
        cache.insert(
            agent_id.to_string(),
            PlanPendingEntry {
                pending,
                observed_at: Instant::now(),
            },
        );
        pending
    }
}

// -- TeamManagerInterface impl ------------------------------------------------

#[async_trait::async_trait]
impl TeamManagerInterface for TeamManager {
    async fn spawn_teammate(
        &self,
        _team_name: &str,
        teammate_name: &str,
        agent_type: &str,
        prompt: &str,
        teammate_model: Option<&crate::agent::ModelRef>,
        lead_model: Option<&crate::agent::ModelRef>,
        working_dir: &Path,
    ) -> Result<String> {
        // The spawn loop writes status through the single writer, so it needs a
        // strong handle on the manager; the trait gives only `&self`.
        let manager = self.self_handle()?;
        manager
            .spawn_teammate_internal(
                teammate_name,
                agent_type,
                prompt,
                teammate_model,
                lead_model,
                working_dir,
            )
            .await
    }

    async fn shutdown_teammate(&self, agent_id: &str, graceful: bool) -> Result<()> {
        // Delegate to the unified in-crate helper.
        Self::shutdown_teammate(self, agent_id, graceful).await
    }

    /// PERF-017 / PERF-018: expose the in-memory lead session id so team
    /// tools can avoid a per-call `TeamStore::load` when a manager is
    /// available on the `ToolContext`.
    fn lead_session_id(&self) -> Option<&str> {
        Some(&self.lead_session_id)
    }

    /// PERF-027: the plan-approval gate's query, delegated to the cached
    /// inherent method so the pipeline's per-call check does not hit disk.
    fn is_plan_pending(&self, agent_id: &str) -> bool {
        Self::is_plan_pending(self, agent_id)
    }

    /// FR-014: the same cached read, asked the wider question - has this
    /// teammate an approved plan at all?
    fn is_plan_gate_active(&self, agent_id: &str) -> bool {
        Self::is_plan_gate_active(self, agent_id)
    }

    /// FR-008 / FR-026: the team tools' only status write, delegated to the
    /// single writer.
    ///
    /// The tool holds an `Arc<dyn TeamManagerInterface>` and so cannot call the
    /// inherent `set_member_status`; this arm is what lets `team_idle`,
    /// `team_submit_plan` and `team_shutdown_ack` move their member without
    /// writing `config.json` themselves. The status and cause arrive as their
    /// canonical tokens because the trait is defined in a module that must not
    /// depend on the team config types; an unrecognised token is an error rather
    /// than a silent no-op.
    async fn set_member_status_in(
        &self,
        team_dir: &Path,
        agent_id: &str,
        to: &str,
        cause: &str,
    ) -> Result<Option<String>> {
        let to = MemberStatus::from_wire(to).ok_or_else(|| {
            // FR-033: the accepted spellings are the wire tokens, and they are
            // listed as such - a caller that typed a display label is told which
            // token it should have sent.
            anyhow::anyhow!(
                "team '{}': '{to}' is not a member status token. Use one of: {}",
                self.team_name,
                MemberStatus::ALL_TOKENS.join(", ")
            )
        })?;
        let cause = TransitionCause::from_wire(cause).ok_or_else(|| {
            anyhow::anyhow!(
                "team '{}': '{cause}' is not a transition cause token. Use one of: {}",
                self.team_name,
                TransitionCause::ALL_TOKENS.join(", ")
            )
        })?;
        // FR-015: a tool's `-> idle` is the same `Idle` transition every other
        // writer makes, so it takes the auto-claiming variant. Claiming a task
        // therefore moves the member on to `Working` before this returns, and
        // the `Some("working")` result carries that on to the tool.
        let transition = self
            .set_member_status_in_auto_claiming(team_dir, agent_id, to, cause)
            .await?;
        Ok(transition.map(|t| t.to.as_str().to_string()))
    }

    /// FR-005 / FR-025: the `team_task_cancel` tool's cancellation, delegated to
    /// the single cancellation implementation.
    ///
    /// The tool holds an `Arc<dyn TeamManagerInterface>` and so cannot call the
    /// inherent `cancel_task`; this arm is what lets it reach the one
    /// implementation instead of cancelling through a `TaskStore` of its own.
    /// The actor rule (lead only), the legal-transition check and the mailbox
    /// notice are all the inherent method's, so the tool path and a direct
    /// manager call cannot diverge (FR-026).
    ///
    /// The team comes from this manager's own `team_dir`, not from the caller:
    /// the trait is type-erased, and a manager is always bound to one team.
    async fn cancel_task(
        &self,
        task_id: &str,
        agent_id: &str,
        reason: Option<&str>,
    ) -> Result<(String, Option<String>, Option<String>)> {
        let cancelled = Self::cancel_task(self, task_id, agent_id, reason)?;
        Ok((
            cancelled.from.as_str().to_string(),
            cancelled.owner.clone(),
            cancelled.notified.clone(),
        ))
    }

    /// FR-018 / FR-025: the `team_task_complete` tool's completion, delegated to
    /// the single completion implementation so the `TaskCompleted` hook's verdict
    /// reaches the completing agent.
    ///
    /// The tool holds an `Arc<dyn TeamManagerInterface>` and so cannot call the
    /// inherent `complete_task`; this arm is what lets it reach the one
    /// implementation instead of completing through a `TaskStore` of its own.
    /// The FR-001 ownership rule, the D3 transition, the `TeamTaskCompleted`
    /// event and the hook fire are all the inherent method's, so the tool path
    /// and a lead-side runtime completion cannot diverge - which is exactly what
    /// FR-018 asks for: the same verdict on every path.
    ///
    /// The team comes from this manager's own `team_dir` in practice, but the
    /// caller's `team_dir` is checked rather than assumed: a manager bound to one
    /// team must never apply its roster, bus and cache to another team's list.
    ///
    /// The hook's `Feedback` is returned as `Option<String>` alongside the
    /// completion facts; the completion is already committed, so the verdict is
    /// the caller's to deliver (FR-018).
    async fn complete_task_in(
        &self,
        team_dir: &std::path::Path,
        task_id: &str,
        agent_id: &str,
    ) -> Result<(String, Option<String>, Option<String>, Option<String>)> {
        if team_dir != self.team_dir {
            anyhow::bail!(
                "this team manager is bound to team '{}' ({}), not {}; refusing to complete \
                 '{task_id}' there",
                self.team_name,
                self.team_dir.display(),
                team_dir.display()
            );
        }
        let (task, outcome) = Self::complete_task(self, task_id, agent_id).await?;
        let feedback = match outcome {
            HookOutcome::Feedback(feedback) => Some(feedback),
            HookOutcome::Allow => None,
        };
        Ok((
            task.id,
            Some(task.title),
            task.completed_at.map(|t| t.to_rfc3339()),
            feedback,
        ))
    }
}

// -- Helper --------------------------------------------------------------------

/// Which task lifecycle event [`TeamManager::publish_task_event`] reports.
///
/// The events themselves (`Event::TeamTaskClaimed`, `Event::TeamTaskCompleted`)
/// carry the lead session id the TUI and SSE filter on, so the publishing
/// belongs to the manager - the only object that always knows it (PERF-017,
/// PERF-018). Naming the kind here keeps the two call sites from picking the
/// wrong variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskEventKind {
    /// `Pending -> InProgress`, reported as [`Event::TeamTaskClaimed`].
    Claimed,
    /// `InProgress -> Completed`, reported as [`Event::TeamTaskCompleted`].
    Completed,
}

impl TeamManager {
    /// Publish the coarse task event a committed mutation implies (FR-017).
    ///
    /// Called after the write from [`Self::claim_next_task`] and
    /// [`Self::complete_task`], not from the `team_task_*` tools, so every path
    /// that mutates the list reports the same event. Both variants carry the
    /// lead session id, which the manager holds in memory, so this costs no disk
    /// read.
    ///
    /// The per-agent `current_task_id` bookkeeping that used to live in the
    /// tools stays there: it is roster state the tool knows the caller of, and it
    /// is not part of the task-list mutation this module owns.
    fn publish_task_event(&self, kind: TaskEventKind, task: &Task, agent_id: &str) {
        let event = match kind {
            TaskEventKind::Claimed => Event::TeamTaskClaimed {
                session_id: self.lead_session_id.clone(),
                team_name: self.team_name.clone(),
                agent_id: agent_id.to_owned(),
                task_id: task.id.clone(),
            },
            TaskEventKind::Completed => Event::TeamTaskCompleted {
                session_id: self.lead_session_id.clone(),
                team_name: self.team_name.clone(),
                agent_id: agent_id.to_owned(),
                task_id: task.id.clone(),
            },
        };
        self.event_bus.publish(event);
    }
}

// -- Helper --------------------------------------------------------------------

/// Translate an inbound mailbox message into the appropriate `Event`.
/// The body of the FR-005 mailbox notification sent to a cancelled task's owner.
///
/// Names the task, says the work is withdrawn rather than completed, and points at
/// the two verbs that apply next, so a teammate that receives it stops instead of
/// finishing work whose completion will be refused as a terminal-status transition.
fn cancellation_notice(task: &Task, reason: Option<&str>) -> String {
    let reason = reason
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .unwrap_or("no reason given");
    format!(
        "Task '{}' has been cancelled by the lead.\nTitle: {}\nReason: {reason}\n\
         Stop working on it: it will not be accepted as completed. Claim the next \
         available task with `team_task_claim`, or wait for the lead to assign you one.",
        task.id, task.title
    )
}

/// Translate an inbound mailbox message into the appropriate `Event`.
fn publish_message_event(
    event_bus: &EventBus,
    lead_session_id: &str,
    team_name: &str,
    _agent_id: &str,
    msg: &MailboxMessage,
) {
    let preview: String = msg.content.chars().take(200).collect();
    // M5-T6: snake_case message_type via serde (matches on-disk format).
    let message_type_str = serde_json::to_value(&msg.message_type)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{:?}", msg.message_type).to_lowercase());

    match msg.message_type {
        MessageType::IdleNotify => {
            event_bus.publish(Event::TeammateIdle {
                session_id: lead_session_id.to_string(),
                team_name: team_name.to_string(),
                agent_id: msg.from.clone(),
            });
        }
        _ if msg.from != "lead" && msg.to != "lead" => {
            event_bus.publish(Event::TeammateP2PMessage {
                session_id: lead_session_id.to_string(),
                team_name: team_name.to_string(),
                from: msg.from.clone(),
                to: msg.to.clone(),
                message_type: message_type_str,
                preview,
            });
        }
        _ => {
            event_bus.publish(Event::TeammateMessage {
                session_id: lead_session_id.to_string(),
                team_name: team_name.to_string(),
                from: msg.from.clone(),
                to: msg.to.clone(),
                message_type: message_type_str,
                preview,
            });
        }
    }
}
