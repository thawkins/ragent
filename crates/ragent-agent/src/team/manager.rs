//! `TeamManager` — runtime for spawning and coordinating teammate sessions.
//!
//! Implements [`crate::tool::TeamManagerInterface`] so the `team_spawn` tool
//! can call it once M3 is wired into the session processor.
//!
//! # Architecture
//!
//! ```text
//! TeamManager (Arc-shared)
//!   ├─ spawn_teammate()   → creates child session, injects team system prompt,
//!   │                       starts mailbox polling loop
//!   ├─ mailbox_poll_loop  → tokio::spawn per teammate; drains unread messages,
//!   │                       publishes Event::TeammateMessage etc.
//!   ├─ run_hook()         → exec shell hook, interpret exit code
//!   └─ shutdown_teammate()→ writes shutdown_request mailbox message, marks Stopped
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime};

use anyhow::Result;
use chrono::Utc;
use tokio::sync::{Mutex, Notify};
use tracing::{debug, warn};

use crate::Config;
use crate::agent::{AgentInfo, AgentMode, resolve_agent_with_customs};
use crate::event::{Event, EventBus};
use crate::session::processor::SessionProcessor;
use crate::team::config::{MemberStatus, PlanStatus};
use crate::team::mailbox::{
    Mailbox, MailboxMessage, MessageType, deregister_notifier, register_notifier,
};
use crate::team::store::TeamStore;
use crate::team::task::{Task, TaskList, TaskStatus};
use crate::team::{DEFAULT_AGENT_TYPE, TeamMember};
use crate::tool::TeamManagerInterface;

/// PERF-023: read the `mtime` of a path, returning `None` when the file does
/// not exist or its mtime cannot be determined. Used by the in-memory
/// `TaskList` cache for mtime-based invalidation.
fn fs_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

/// Check if an error message indicates a context-window / token-count overflow.
///
/// These errors come from Anthropic, `OpenAI`, and GitHub Copilot when the prompt
/// is too long for the model's context window. They are *not* permanent failures —
/// the session processor's compression pipeline will reduce context on retry.
fn is_token_overflow_error(error_msg: &str) -> bool {
    let msg = error_msg.to_lowercase();
    // Anthropic: "prompt token count of N exceeds the limit of M"
    // OpenAI / Copilot: "context_length_exceeded", "maximum context length"
    // Generic fallback phrases
    msg.contains("prompt token count") && msg.contains("exceeds")
        || msg.contains("context_length_exceeded")
        || msg.contains("maximum context length")
        || msg.contains("prompt is too long")
        || msg.contains("input too large")
}

/// Check if an error message indicates a permanent (non-retryable) API error.
///
/// Matches HTTP 4xx errors, excluding 429 (Too Many Requests), 408 (Timeout),
/// and token-overflow errors (handled by the compression pipeline on retry).
fn is_permanent_api_error(error_msg: &str) -> bool {
    // Token overflow is recoverable via compression — never treat as permanent.
    if is_token_overflow_error(error_msg) {
        return false;
    }
    // Match "HTTP 4xx:" patterns, excluding 429 (rate limit) and 408 (timeout)
    if let Some(rest) = error_msg.strip_prefix("HTTP ")
        && let Some(code_str) = rest
            .split(':')
            .next()
            .or_else(|| rest.split_whitespace().next())
        && let Ok(code) = code_str.trim().parse::<u16>()
    {
        return (400..500).contains(&code) && code != 429 && code != 408;
    }
    false
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
/// | 1       |  1 s    | 1.0 s – 1.5 s  |
/// | 2       |  2 s    | 2.0 s – 2.5 s  |
/// | 3       |  4 s    | 4.0 s – 4.5 s  |
/// | 4       |  8 s    | 8.0 s – 8.5 s  |
///
/// `attempt` is the 1-based retry index: `1` = first retry, `2` = second,
/// and so on.  `attempt == 0` returns [`Duration::ZERO`] so callers can
/// unconditionally `sleep(backoff)` without an `if attempt > 0` guard.
///
/// The cap (`MAX_TEAMMATE_BACKOFF_MS`) prevents an unbounded wait when the
/// caller passes a large `attempt` value by mistake.
///
/// Jitter is derived from the current monotonic clock — cheap, allocation-
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

// ── System prompt addition for teammate sessions ──────────────────────────────

/// Build the team-context section injected into every teammate's system prompt.
///
/// Template variables:
/// - `{{TEAM_NAME}}` — name of the team
/// - `{{TEAMMATE_NAME}}` — this teammate's friendly name
/// - `{{AGENT_ID}}` — this teammate's agent ID (e.g. `"tm-001"`)
/// - `{{TEAMMATE_ROSTER}}` — list of other teammates with names and agent IDs
#[must_use]
pub fn build_team_prompt_addition(
    team_name: &str,
    teammate_name: &str,
    agent_id: &str,
    teammate_roster: &[(String, String)],
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

    format!(
        r#"
## Team Context

You are a teammate in team "{team_name}". Your name is "{teammate_name}" (agent ID: {agent_id}).
The team lead is "lead". Other teammates: {others}.

### Team tool usage — CRITICAL

**Your very first action in every response MUST be a tool call.** Do NOT write planning text.
Call `team_read_messages` (team_name: "{team_name}") immediately at the start of each turn
to check for new instructions, plan approval results, or shutdown requests from the lead.

When you finish a task, call `team_task_complete` then `team_task_claim` to pick up the
next available task. If no tasks remain, call `team_idle` to notify the lead.

If you receive a `shutdown_request` message, finish your current step cleanly and call
`team_shutdown_ack` to confirm termination.

If `require_plan_approval` is enabled, call `team_submit_plan` before starting
implementation and wait for a `plan_approved` mailbox reply.

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

// ── Persistent memory injection ────────────────────────────────────────────────

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

    for mem in &memories {
        let tags = storage.get_memory_tags(mem.id).unwrap_or_default();
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

// ── Hook runner ───────────────────────────────────────────────────────────────

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
/// - Exit 0 → `HookOutcome::Allow`
/// - Exit 2 → `HookOutcome::Feedback(stdout)`
/// - Other → log warning, allow
///
/// If `stdin_data` is `Some`, it is piped to the child process on stdin.
pub async fn run_hook(command: &str, args: &[String], stdin_data: Option<&str>) -> HookOutcome {
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
            HookOutcome::Allow
        }
        Ok(mut child_proc) => {
            // Write stdin data if provided.
            if let Some(data) = stdin_data
                && let Some(mut stdin) = child_proc.stdin.take()
            {
                use tokio::io::AsyncWriteExt;
                let _ = stdin.write_all(data.as_bytes()).await;
                drop(stdin);
            }

            match child_proc.wait_with_output().await {
                Err(e) => {
                    warn!(command, error = %e, "Hook failed to complete");
                    HookOutcome::Allow
                }
                Ok(out) => match out.status.code() {
                    Some(0) => HookOutcome::Allow,
                    Some(2) => {
                        let feedback = String::from_utf8_lossy(&out.stdout).into_owned();
                        HookOutcome::Feedback(feedback)
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

/// Look up and run a quality-gate hook for the given event in a team's settings.
///
/// Returns `HookOutcome::Allow` if no hook is configured for this event.
/// The `stdin_json` parameter is piped to the hook process as stdin (useful for
/// passing task metadata to `TaskCreated` / `TaskCompleted` hooks).
pub async fn run_team_hook(
    team_dir: &Path,
    event: crate::team::config::HookEvent,
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

    run_hook(&hook.command, &[], stdin_json).await
}

// ── TeamManager ───────────────────────────────────────────────────────────────

/// Tracks the runtime state of one teammate.
#[derive(Debug)]
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
    /// DashMap provides lock-free shard-based concurrent access — readers
    /// on one agent's handle never block readers on another's. `DashMap`
    /// is already a workspace dependency (used by the orchestrator in
    /// `ragent-agent`).
    handles: Arc<dashmap::DashMap<String, TeammateHandle>>,
    /// Underlying session processor (shared with the lead).
    processor: Arc<SessionProcessor>,
    /// Event bus for publishing team lifecycle events.
    event_bus: Arc<EventBus>,
    /// Mailbox poll interval.
    poll_interval: Duration,
    /// Serialises spawn operations to avoid concurrent config read/write races.
    spawn_lock: Arc<Mutex<()>>,
    /// The lead's active model — teammates inherit this when spawned via
    /// the reconcile loop (where no `ToolContext` model is available).
    pub active_model: Option<crate::agent::ModelRef>,
    /// M6-T1: watchdog timeout. If a `Working` / `Spawning` member records no
    /// progress within this duration, it is marked `Failed` and
    /// `Event::TeammateFailed` is published.
    pub watchdog_timeout: Duration,
    /// M6-T1: cancel flag for the watchdog task (set on shutdown_all).
    watchdog_cancel: Arc<AtomicBool>,
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

impl TeamManager {
    /// Create a new `TeamManager` for an existing team on disk.
    ///
    /// M6-T2: if the team's `config.json` records a different
    /// `lead_session_id` from the one passed here, the team is *adopted* —
    /// any tasks that were `InProgress` and assigned to the old lead are
    /// reset to `Pending` so a new lead can pick them up. See
    /// [`TeamManager::adopt_orphaned_tasks`].
    pub fn new(
        team_name: impl Into<String>,
        lead_session_id: impl Into<String>,
        team_dir: PathBuf,
        processor: Arc<SessionProcessor>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        let lead_sid: String = lead_session_id.into();
        let name: String = team_name.into();

        // M6-T2: leader crash recovery. If the on-disk config was written by a
        // different lead session, adopt the team and reassign orphaned tasks.
        if let Ok(store) = TeamStore::load(&team_dir)
            && store.config.lead_session_id != lead_sid
        {
            tracing::info!(
                team = %name,
                old_lead = %store.config.lead_session_id,
                new_lead = %lead_sid,
                "M6-T2: adopting team from previous lead session"
            );
            // Reassign InProgress tasks for the old lead to Pending.
            if let Err(e) = Self::adopt_orphaned_tasks(&team_dir, &store.config.lead_session_id) {
                tracing::warn!(team = %name, error = %e, "M6-T2: failed to reassign orphaned tasks");
            }
            // Update the config to reflect the new lead.
            if let Ok(mut new_store) = TeamStore::load(&team_dir) {
                new_store.config.lead_session_id = lead_sid.clone();
                let _ = new_store.save();
            }
        }

        Self {
            team_name: name,
            lead_session_id: lead_sid,
            team_dir,
            handles: Arc::new(dashmap::DashMap::new()),
            processor,
            event_bus,
            poll_interval: Duration::from_millis(500),
            spawn_lock: Arc::new(Mutex::new(())),
            active_model: None,
            watchdog_timeout: Duration::from_mins(5),
            watchdog_cancel: Arc::new(AtomicBool::new(false)),
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
        }
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
                            // PERF-025: DashMap — no async read guard; we just
                            // check `contains_key` on each candidate's agent_id.
                            store.config.members.iter()
                                                          .filter(|m| m.status == crate::team::config::MemberStatus::Spawning)
                                                          .filter(|m| {
                                                              if m.session_id.is_some() {
                                                                  tracing::debug!(team = %manager.team_name, teammate = %m.name, "Skipping queued teammate: already has session_id");
                                                                  return false;
                                                              }
                                                              if manager.handles.contains_key(&m.agent_id) {
                                                                  tracing::debug!(team = %manager.team_name, teammate = %m.name, agent_id = %m.agent_id, "Skipping queued teammate: handle already exists");
                                                                  return false;
                                                              }
                                                                                        true
                                                                                    })                                .map(|m| (m.name.clone(), m.agent_type.clone(), m.spawn_prompt.clone().unwrap_or_default(), m.model_override.clone()))
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
                                .session_manager
                                .get_session(&manager.lead_session_id)
                                .ok()
                                .flatten()
                                .map_or_else(
                                    || std::env::current_dir().unwrap_or_default(),
                                    |s| s.directory,
                                );
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

    // ── Spawn ────────────────────────────────────────────────────────────

    /// Spawn a new teammate session.
    ///
    /// 1. Allocates an agent ID, updates `config.json`.
    /// 2. Creates a child session.
    /// 3. Resolves the agent type and augments its system prompt.
    /// 4. Starts the teammate's agent loop in a background `tokio` task.
    /// 5. Starts a mailbox polling loop for this teammate.
    /// 6. Publishes `Event::TeammateSpawned`.
    ///
    /// PERF-024: the `spawn_lock` is now held **only** for the config
    /// read → `next_agent_id()` → `add_member()` → `save()` cycle that
    /// allocates the agent ID and records the `Spawning` member. Everything
    /// afterwards (child session creation, system-prompt build, memory
    /// load, handle registration, the `tokio::spawn` of the agent loop) runs
    /// **outside** the lock, so multiple teammates can be spawned
    /// concurrently. The only state that must be serialised is the
    /// agent-ID allocation + on-disk config write, and that is captured in
    /// the short critical section below.
    pub async fn spawn_teammate_internal(
        &self,
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

        // ── PERF-024: spawn_lock held only for the config + agent-ID write ──
        //
        // This is the only section that must be serialised: it reads the
        // shared `config.json`, allocates a fresh `tm-NNN` agent ID, records
        // the new `Spawning` member, and saves. Holding the lock for just
        // this cycle lets concurrent `spawn_teammate_internal` calls proceed
        // in parallel once their agent IDs are allocated.
        let agent_id = {
            let _guard = self.spawn_lock.lock().await;
            let mut store = TeamStore::load(&self.team_dir)?;
            let id = if let Some(existing) = store.config.member_by_name(teammate_name) {
                tracing::debug!(team = %self.team_name, teammate = %teammate_name, agent_id = %existing.agent_id, "Reusing existing member record for spawn");
                existing.agent_id.clone()
            } else {
                let id = store.next_agent_id();
                tracing::info!(team = %self.team_name, teammate = %teammate_name, agent_id = %id, "Allocating new agent id and recording Spawning member");
                let mut member = TeamMember::new(teammate_name, &id, &resolved_agent_type);
                member.status = MemberStatus::Spawning;
                member.model_override = teammate_model.cloned();
                store.add_member(member)?;
                id
            };
            // Persist now so external tools (e.g., team_create) see the member.
            store.save()?;
            id
        };
        // spawn_lock released here — concurrent spawns may proceed.

        // Create isolated child session.
        tracing::info!(team = %self.team_name, agent_id = %agent_id, "Creating child session for teammate");
        let child_session = self
            .processor
            .session_manager
            .create_session(working_dir.to_path_buf())?;
        let child_sid = child_session.id.clone();

        // ── Single config reload: update session, build roster, read memory ─
        //
        // PERF-024: this second config write does not need the spawn_lock:
        // it only updates the *this* agent's* own member record (looked up
        // by `agent_id`, which is now unique and owned by this caller), and
        // `TeamStore::save` already takes the config.json `flock` for atomic
        // write safety. The only state that needs serialisation across
        // concurrent spawns — the agent-ID allocation — already happened
        // under the lock above.
        let (teammate_roster, memory_scope) = {
            let mut store = TeamStore::load(&self.team_dir)?;
            if let Some(m) = store.config.member_by_id_mut(&agent_id) {
                m.session_id = Some(child_sid.clone());
                m.status = MemberStatus::Working;
                tracing::info!(team = %self.team_name, teammate = %m.name, agent_id = %agent_id, session_id = %child_sid, "Updated team config with session id and Working status");
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
            store.save()?;
            tracing::debug!(team = %self.team_name, agent_id = %agent_id, "Team config saved after session assignment");
            (roster, mem_scope)
        };

        // Resolve agent and augment system prompt.
        let config = Config::default();
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

        let team_addition =
            build_team_prompt_addition(&self.team_name, teammate_name, &agent_id, &teammate_roster);
        // Append the team context block to the agent's system prompt.
        let base = agent.prompt.as_deref().unwrap_or("");
        Arc::make_mut(&mut agent).prompt = Some(Arc::from(format!("{base}\n{team_addition}")));

        // ── Persistent memory injection ────────────────────────────────────
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
            let storage = self.processor.session_manager.storage();
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
        // PERF-025: DashMap — direct insert, no async write guard.
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
            },
        );
        // Start agent loop in background. Capture agent_id and team_dir for error persistence.
        let proc = Arc::clone(&self.processor);
        let child_sid_clone = child_sid.clone();
        let agent_clone = agent.clone();
        let prompt_owned = prompt.to_string();
        let cancel_clone = cancel.clone();
        let agent_id_clone = agent_id.clone();
        let team_dir_clone = self.team_dir.clone();
        let team_name_clone = self.team_name.clone();
        let lead_sid_clone = self.lead_session_id.clone();
        let event_bus_clone = self.event_bus.clone();
        tokio::spawn(async move {
            // Retry with exponential backoff + jitter for transient API errors
            // (e.g. rate limits, cloud-provider cold-start).  Linear backoff
            // was insufficient — multiple teammates that failed at the same
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
                        // Teammate finished its initial prompt — mark as Idle.
                        tracing::info!(
                            team = %team_name_clone,
                            agent_id = %agent_id_clone,
                            "Teammate finished initial prompt; marking idle"
                        );
                        if let Ok(mut store) = TeamStore::load(&team_dir_clone) {
                            if let Some(m) = store.config.member_by_id_mut(&agent_id_clone) {
                                m.status = crate::team::config::MemberStatus::Idle;
                                m.current_task_id = None;
                            }
                            let _ = store.save();
                        }
                        event_bus_clone.publish(Event::TeammateIdle {
                            session_id: lead_sid_clone,
                            team_name: team_name_clone,
                            agent_id: agent_id_clone,
                        });
                        return; // success — exit the retry loop
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
                                "Token overflow — the session processor will compress history on retry"
                            );
                        }

                        // Don't retry permanent errors (4xx except 429 / 408 / token overflow).
                        if is_permanent_api_error(&last_error) {
                            tracing::error!(
                                team = %team_name_clone,
                                agent_id = %agent_id_clone,
                                "Permanent API error — skipping remaining retries"
                            );
                            break;
                        }
                    }
                }
            }
            // All retries exhausted or permanent error — persist failure.
            tracing::error!(
                team = %team_name_clone,
                agent_id = %agent_id_clone,
                "Teammate agent loop failed after {} retries",
                MAX_RETRIES
            );
            match TeamStore::load(&team_dir_clone) {
                Ok(mut store) => {
                    if let Some(m) = store.config.member_by_id_mut(&agent_id_clone) {
                        m.status = crate::team::config::MemberStatus::Failed;
                        m.last_spawn_error = Some(last_error.clone());
                    }
                    if let Err(se) = store.save() {
                        warn!(error = %se, "Failed to save team config after spawn error");
                    }
                }
                Err(se) => warn!(error = %se, "Failed to load team store to persist spawn error"),
            }
            event_bus_clone.publish(Event::TeammateFailed {
                session_id: lead_sid_clone,
                team_name: team_name_clone,
                agent_id: agent_id_clone,
                error: last_error,
            });
        });

        // Start mailbox polling loop.
        self.start_poll_loop(agent_id.clone(), poll_cancel, notify);

        // Publish TeammateSpawned event.
        self.event_bus.publish(Event::TeammateSpawned {
            session_id: self.lead_session_id.clone(),
            team_name: self.team_name.clone(),
            teammate_name: teammate_name.to_string(),
            agent_id: agent_id.clone(),
        });

        Ok(agent_id)
    }

    // ── Mailbox polling ───────────────────────────────────────────────────

    /// Start a tokio background task that polls `agent_id`'s mailbox and
    /// publishes events when new messages arrive.
    ///
    /// Uses `tokio::select!` to wake on either:
    /// - `notify.notified()` — instant push from [`Mailbox::push`], or
    /// - `sleep(poll_interval)` — fallback for external writers.
    fn start_poll_loop(&self, agent_id: String, cancel: Arc<AtomicBool>, notify: Arc<Notify>) {
        let team_dir = self.team_dir.clone();
        let team_name = self.team_name.clone();
        let lead_session_id = self.lead_session_id.clone();
        let event_bus = self.event_bus.clone();
        let interval = self.poll_interval;

        tokio::spawn(async move {
            loop {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }

                // Wait for a push notification or the fallback interval.
                tokio::select! {
                    () = notify.notified() => {}
                    () = tokio::time::sleep(interval) => {}
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
        });
    }

    // ── Shutdown ──────────────────────────────────────────────────────────

    /// Suspend (pause) a running teammate by agent ID.
    ///
    /// Marks the member as `Suspended` in config and pauses the mailbox poll
    /// loop so the teammate stops receiving new messages. The agent loop
    /// continues running but will not be woken for new mailbox messages.
    /// Use [`resume_teammate`] to restore the teammate to active status.
    pub async fn suspend_teammate(&self, agent_id: &str) -> Result<()> {
        // Pause the poll loop so no new messages wake the agent.
        // PERF-025: DashMap — `.get()` returns a short-lived shard guard.
        if let Some(handle) = self.handles.get(agent_id) {
            handle.poll_cancel.store(true, Ordering::Relaxed);
        }

        // Mark member as Suspended in config.
        let mut store = TeamStore::load(&self.team_dir)?;
        if let Some(member) = store.config.member_by_id_mut(agent_id) {
            member.status = MemberStatus::Suspended;
        }
        store.save()?;

        self.event_bus.publish(Event::TeammateSuspended {
            session_id: self.lead_session_id.clone(),
            team_name: self.team_name.clone(),
            agent_id: agent_id.to_string(),
        });
        tracing::info!(agent_id, "Teammate suspended");
        Ok(())
    }

    /// Resume a previously suspended teammate by agent ID.
    ///
    /// Restores the member to `Idle` status and restarts mailbox polling.
    /// Does nothing if the teammate is not currently `Suspended`.
    pub async fn resume_teammate(&self, agent_id: &str) -> Result<()> {
        // Check that the member is actually suspended.
        let store = TeamStore::load(&self.team_dir)?;
        let is_suspended = store
            .config
            .member_by_id(agent_id)
            .map(|m| m.status == MemberStatus::Suspended)
            .unwrap_or(false);
        if !is_suspended {
            anyhow::bail!("Teammate '{agent_id}' is not suspended");
        }
        drop(store);

        // Re-enable the poll loop.
        // PERF-025: DashMap — `.get()` returns a short-lived shard guard.
        if let Some(handle) = self.handles.get(agent_id) {
            handle.poll_cancel.store(false, Ordering::Relaxed);
            // Wake the poll loop so it starts processing again.
            handle.notify.notify_one();
        }

        // Mark member as Idle.
        let mut store = TeamStore::load(&self.team_dir)?;
        if let Some(member) = store.config.member_by_id_mut(agent_id) {
            member.status = MemberStatus::Idle;
        }
        store.save()?;

        self.event_bus.publish(Event::TeammateResumed {
            session_id: self.lead_session_id.clone(),
            team_name: self.team_name.clone(),
            agent_id: agent_id.to_string(),
        });
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
    /// - The member is marked `Stopped` on disk.
    ///
    /// In both cases the on-disk status is updated atomically via
    /// [`TeamStore::save`]. Errors from mailbox or store I/O are propagated.
    /// Missing in-memory handles (e.g. a teammate whose agent loop already
    /// exited) are tolerated: only the on-disk status is updated.
    pub async fn shutdown_teammate(&self, agent_id: &str, graceful: bool) -> Result<()> {
        // ── Handle-level cancellation (immediate path only) ───────────────
        if !graceful {
            // PERF-025: DashMap — `.get()` returns a short-lived shard guard,
            // so there's no async read lock to hold/drop.
            if let Some(handle) = self.handles.get(agent_id) {
                handle.cancel.store(true, Ordering::Relaxed);
                handle.poll_cancel.store(true, Ordering::Relaxed);
                // Wake the poll loop so it sees the cancel flag immediately.
                handle.notify.notify_one();
            }

            // Deregister the notifier now that this agent is shutting down.
            deregister_notifier(&self.team_dir, agent_id);
        }

        // ── Mailbox: push ShutdownRequest (both paths) ────────────────────
        // For graceful, this is the primary signal. For immediate, it is a
        // fallback for an agent loop that checks its mailbox before observing
        // the cancel flag.
        // M5-T4: stamp a correlation id and record it on the member so the
        // teammate's `team_shutdown_ack` reply can copy it.
        let correlation_id = uuid::Uuid::new_v4().to_string();
        let mailbox = Mailbox::open(&self.team_dir, agent_id)?;
        mailbox.push(MailboxMessage::new_correlated(
            "lead".to_string(),
            agent_id.to_string(),
            MessageType::ShutdownRequest,
            if graceful {
                "Graceful shutdown requested by TeamManager; call team_shutdown_ack to terminate."
            } else {
                "Immediate shutdown requested by TeamManager; agent loop cancelled."
            },
            &correlation_id,
        ))?;

        // ── On-disk status update ────────────────────────��────────────────
        let mut store = TeamStore::load(&self.team_dir)?;
        if let Some(member) = store.config.member_by_id_mut(agent_id) {
            member.status = if graceful {
                MemberStatus::ShuttingDown
            } else {
                MemberStatus::Stopped
            };
            if !graceful {
                member.current_task_id = None;
            }
            // M5-T4: record the correlation id for the ack reply.
            member.shutdown_request_id = Some(correlation_id);
        }
        store.save()?;

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
        // PERF-025: DashMap — `.iter()` over the map yields owned keys (or
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

    // ── Watchdog (M6-T1) & leader recovery (M6-T2) ──────────────────────

    /// M6-T1: Start the teammate watchdog.
    ///
    /// Spawns a background task that periodically checks each `Working` /
    /// `Spawning` member. If no progress had been observed for that member
    /// within [`TeamManager::watchdog_timeout`], the member is marked
    /// `Failed`, its cancel flags are set, and `Event::TeammateFailed` is
    /// published so `team_wait` can return.
    ///
    /// "Progress" is recorded via [`Self::record_progress`], which the event
    /// loop should call when it observes `TeammateIdle`, `TeamTaskClaimed`,
    /// `TeamTaskCompleted`, or `TeammateFailed` for that agent.
    pub fn start_watchdog(self: Arc<Self>) {
        let manager = Arc::clone(&self);
        let cancel = Arc::clone(&self.watchdog_cancel);
        let check_interval = self.watchdog_timeout.min(Duration::from_mins(1)) / 2;
        let timeout = self.watchdog_timeout;
        // Only spawn if a tokio runtime is available; tests that call
        // TeamManager::new outside a runtime will silently skip the watchdog.
        let handle = match tokio::runtime::Handle::try_current() {
            Ok(h) => h,
            Err(_) => {
                tracing::warn!("start_watchdog called outside a tokio runtime; skipping");
                return;
            }
        };
        handle.spawn(async move {
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
                // PERF-025: DashMap — iterate directly via `.iter()`; the
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
                            let lp = *h.last_progress.lock().unwrap();
                            if now.duration_since(lp) > timeout {
                                Some((id, lp))
                            } else {
                                None
                            }
                        })
                        .collect()
                };

                for (agent_id, last_progress) in stale {
                    // Confirm the member is still Working/Spawning on disk; if it
                    // transitioned to Idle/Failed/Stopped we should not fail it.
                    let still_active = TeamStore::load(&manager.team_dir)
                        .ok()
                        .and_then(|s| s.config.member_by_id(&agent_id).map(|m| m.status))
                        .map(|status| {
                            matches!(
                                status,
                                MemberStatus::Working
                                    | MemberStatus::Spawning
                                    | MemberStatus::PlanPending
                                    | MemberStatus::Suspended
                            )
                        })
                        .unwrap_or(false);
                    if !still_active {
                        // Mark progress so we don't re-flag it next tick.
                        if let Some(h) = manager.handles.get(&agent_id) {
                            *h.last_progress.lock().unwrap() = std::time::Instant::now();
                        }
                        continue;
                    }

                    tracing::warn!(
                        team = %manager.team_name,
                        agent_id = %agent_id,
                        ?last_progress,
                        "M6-T1 watchdog: no progress for teammate within timeout; marking Failed"
                    );

                    // Set cancel flags so any live agent loop terminates.
                    // PERF-025: DashMap — `.get()` returns a short-lived guard.
                    if let Some(h) = manager.handles.get(&agent_id) {
                        h.cancel.store(true, Ordering::Relaxed);
                        h.poll_cancel.store(true, Ordering::Relaxed);
                        h.notify.notify_one();
                    }
                    deregister_notifier(&manager.team_dir, &agent_id);

                    // Mark Failed on disk.
                    let error_msg = format!(
                        "watchdog: no progress for {agent_id} within {}s",
                        timeout.as_secs()
                    );
                    if let Ok(mut store) = TeamStore::load(&manager.team_dir) {
                        if let Some(m) = store.config.member_by_id_mut(&agent_id) {
                            m.status = MemberStatus::Failed;
                            m.last_spawn_error = Some(error_msg.clone());
                        }
                        let _ = store.save();
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
    }

    /// M6-T2: Reassign tasks that were `InProgress` and assigned to the old
    /// lead session back to `Pending` so a new lead can pick them up.
    ///
    /// Called from [`TeamManager::new`] when the on-disk `lead_session_id`
    /// differs from the new lead's session id. Tasks assigned to teammates
    /// (not the old lead) are left untouched.
    pub fn adopt_orphaned_tasks(team_dir: &Path, old_lead_sid: &str) -> Result<()> {
        let task_store = crate::team::task::TaskStore::open(team_dir)?;
        let list = task_store.read()?;
        for t in &list.tasks {
            if t.status == TaskStatus::InProgress && t.assigned_to.as_deref() == Some(old_lead_sid)
            {
                tracing::info!(
                    task_id = %t.id,
                    old_lead = old_lead_sid,
                    "M6-T2: reassigning orphaned InProgress task to Pending"
                );
                let tid = t.id.clone();
                let _ = task_store.update_task(&tid, |task| {
                    task.status = TaskStatus::Pending;
                    task.assigned_to = None;
                    task.claimed_at = None;
                });
            }
        }
        Ok(())
    }

    // ── PERF-023: in-memory TaskList cache with write-through persistence ──

    /// Return the current [`TaskList`] for this team, using the in-memory
    /// cache when the on-disk `tasks.json` `mtime` has not advanced.
    ///
    /// PERF-023: the first call loads `tasks.json` once and caches both the
    /// parsed list and the file's `mtime`. Subsequent calls return the
    /// cached list in O(1). If another process (or a `team_task_*` tool
    /// that bypassed the cache) wrote to `tasks.json`, the `mtime` will
    /// have advanced and the cache is transparently reloaded — so the
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
        let store = crate::team::task::TaskStore::open(&self.team_dir)?;
        let written = store.write_through(f)?;
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
    /// Equivalent to [`TaskStore::claim_next`] but mutates the in-memory
    /// list and writes through atomically, keeping the cache consistent.
    /// Returns `(Some(task), already_had)` where `already_had` is `true`
    /// if the agent already owned an in-progress task.
    pub fn claim_next_task(&self, agent_id: &str) -> Result<(Option<Task>, bool)> {
        let mut claimed_out: Option<(Option<Task>, bool)> = None;
        let written = self.apply_to_task_list(|list| {
            let done = list.completed_ids();
            // Guard: agent already has an in-progress task — return it as-is.
            if let Some(active) = list
                .tasks
                .iter()
                .find(|t| {
                    t.status == TaskStatus::InProgress && t.assigned_to.as_deref() == Some(agent_id)
                })
                .cloned()
            {
                claimed_out = Some((Some(active), true));
                return;
            }
            let idx = list.tasks.iter().position(|t| t.is_claimable(&done));
            if let Some(i) = idx {
                list.tasks[i].status = TaskStatus::InProgress;
                list.tasks[i].assigned_to = Some(agent_id.to_owned());
                list.tasks[i].claimed_at = Some(Utc::now());
                claimed_out = Some((Some(list.tasks[i].clone()), false));
            } else {
                claimed_out = Some((None, false));
            }
        })?;
        let _ = written;
        claimed_out.ok_or_else(|| anyhow::anyhow!("claim_next closure did not run"))
    }

    /// PERF-023: mark `task_id` as `Completed` by `agent_id` via the
    /// write-through cache (idempotent — same agent re-completing is a
    /// no-op success).
    pub fn complete_task(&self, task_id: &str, agent_id: &str) -> Result<Task> {
        let mut completed_out: Option<Task> = None;
        self.apply_to_task_list(|list| {
            let Some(task) = list.get_mut(task_id) else {
                return;
            };
            if task.status == TaskStatus::Completed {
                let owner = task.completed_by.as_deref().unwrap_or("unknown");
                if owner == agent_id {
                    completed_out = Some(task.clone());
                }
                return;
            }
            if task.assigned_to.as_deref() != Some(agent_id) {
                if task.status == TaskStatus::Pending || task.assigned_to.is_none() {
                    task.assigned_to = Some(agent_id.to_owned());
                    task.claimed_at = Some(Utc::now());
                    task.status = TaskStatus::InProgress;
                }
            }
            task.status = TaskStatus::Completed;
            task.completed_at = Some(Utc::now());
            task.completed_by = Some(agent_id.to_owned());
            completed_out = Some(task.clone());
        })?;
        completed_out.ok_or_else(|| anyhow::anyhow!("task '{task_id}' not found"))
    }

    /// M6-T1: Record progress for `agent_id` (called when an event indicates
    /// the teammate did something). Resets the watchdog timer.
    pub fn record_progress(&self, agent_id: &str) {
        // PERF-025: DashMap — `.get()` returns a short-lived shard guard; no
        // async read lock, so this method remains sync (matching its
        // pre-DashMap contract).
        if let Some(h) = self.handles.get(agent_id) {
            *h.last_progress.lock().unwrap() = std::time::Instant::now();
        }
    }

    // ── Plan approval ─────────────────────────────────────────────────────

    /// Approve a plan for a teammate (shorthand used by the plan approval tool).
    pub fn approve_plan(&self, agent_id: &str, approved: bool) -> Result<()> {
        let mut store = TeamStore::load(&self.team_dir)?;
        if let Some(m) = store.config.member_by_id_mut(agent_id) {
            if approved {
                m.plan_status = PlanStatus::Approved;
                m.status = MemberStatus::Working;
            } else {
                m.plan_status = PlanStatus::Rejected;
            }
        }
        store.save()?;
        // PERF-027: the plan status just changed — drop the cached entry so
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
        {
            let mut cache = self.plan_pending_cache.lock();
            if let Some(entry) = cache.get(agent_id) {
                if entry.observed_at.elapsed() < self.plan_pending_ttl {
                    return entry.pending;
                }
                // Expired — fall through and re-read from disk.
                cache.remove(agent_id);
            }
        }
        let pending = TeamStore::load(&self.team_dir)
            .ok()
            .and_then(|s| {
                s.config
                    .member_by_id(agent_id)
                    .map(|m| m.plan_status == PlanStatus::Pending)
            })
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

// ── TeamManagerInterface impl ────────────────────────────────────────────────

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
        self.spawn_teammate_internal(
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
}

// ── Helper ────────────────────────────────────────────────────────────────────

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
