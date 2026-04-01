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
use std::time::Duration;

use anyhow::Result;
use tokio::sync::{Mutex, Notify, RwLock};
use tracing::{debug, warn};

use crate::agent::{AgentInfo, AgentMode, resolve_agent_with_customs};
use crate::config::Config;
use crate::event::{Event, EventBus};
use crate::session::processor::SessionProcessor;
use crate::team::config::{MemberStatus, PlanStatus};
use crate::team::mailbox::{Mailbox, MailboxMessage, MessageType, register_notifier, deregister_notifier};
use crate::team::store::TeamStore;
use crate::team::TeamMember;
use crate::tool::TeamManagerInterface;

/// Check if an error message indicates a context-window / token-count overflow.
///
/// These errors come from Anthropic, OpenAI, and GitHub Copilot when the prompt
/// is too long for the model's context window.  They are *not* permanent failures —
/// the session can be compacted and then retried successfully.
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
/// Matches HTTP 4xx errors (except 429 Too Many Requests, 408 Timeout, and
/// token-overflow errors which can be resolved by compacting the session).
fn is_permanent_api_error(error_msg: &str) -> bool {
    // Token overflow is recoverable via compaction — never treat as permanent.
    if is_token_overflow_error(error_msg) {
        return false;
    }
    // Match "HTTP 4xx:" patterns, excluding 429 (rate limit) and 408 (timeout)
    if let Some(rest) = error_msg.strip_prefix("HTTP ") {
        if let Some(code_str) = rest.split(':').next().or_else(|| rest.split_whitespace().next()) {
            if let Ok(code) = code_str.trim().parse::<u16>() {
                return (400..500).contains(&code) && code != 429 && code != 408;
            }
        }
    }
    false
}

/// Compact a teammate session's history by running the compaction agent.
///
/// Runs the compaction agent against `session_id`, then replaces the entire
/// session message history with the resulting summary.  Returns `true` if
/// compaction succeeded, `false` if it failed (caller should still retry the
/// original task — even a partial compact may help).
async fn compact_teammate_session(
    proc: &Arc<crate::session::processor::SessionProcessor>,
    session_id: &str,
    agent: &AgentInfo,
) -> bool {
    tracing::info!(session_id, "Compacting teammate session due to token overflow");

    // Use the compaction agent with the same provider/model as the teammate so
    // it works regardless of which provider is active (Copilot, OpenAI, Ollama …).
    let mut compact_agent =
        crate::agent::resolve_agent("compaction", &Default::default()).unwrap_or_else(|_| agent.clone());
    if let Some(model_ref) = agent.model.clone() {
        compact_agent.model = Some(model_ref);
    }

    let summary_prompt =
        "Summarise the conversation so far into a concise representation that \
         preserves all important context, decisions, code changes, file paths, \
         and outstanding tasks. Output only the summary — no preamble."
            .to_string();

    let cancel = Arc::new(AtomicBool::new(false));
    let compact_result = proc
        .process_message(session_id, &summary_prompt, &compact_agent, cancel)
        .await;

    match compact_result {
        Err(e) => {
            tracing::warn!(session_id, error = %e, "Compaction LLM call failed");
            return false;
        }
        Ok(_) => {
            tracing::info!(session_id, "Compaction LLM call completed — replacing history");
        }
    }

    // Read back the summary that was just produced (last assistant message).
    let storage = proc.session_manager.storage().clone();
    let sid_owned = session_id.to_string();
    let messages_result = tokio::task::spawn_blocking(move || storage.get_messages(&sid_owned))
        .await
        .ok()
        .and_then(|r| r.ok());

    let summary_text = messages_result
        .as_ref()
        .and_then(|msgs| {
            msgs.iter()
                .rev()
                .find(|m| m.role == crate::message::Role::Assistant)
                .map(|m| m.text_content())
        });

    let Some(summary) = summary_text else {
        tracing::warn!(session_id, "Compaction produced no assistant message");
        return false;
    };
    if summary.trim().is_empty() {
        tracing::warn!(session_id, "Compaction summary is empty");
        return false;
    }

    // Replace history: delete all messages, then insert the summary.
    let storage = proc.session_manager.storage().clone();
    let sid_owned = session_id.to_string();
    let summary_msg = crate::message::Message::new(
        &sid_owned,
        crate::message::Role::Assistant,
        vec![crate::message::MessagePart::Text {
            text: format!("[Conversation compacted]\n\n{}", summary),
        }],
    );

    let replace_result = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        storage.delete_messages(&sid_owned)?;
        storage.create_message(&summary_msg)?;
        Ok(())
    })
    .await;

    match replace_result {
        Ok(Ok(())) => {
            tracing::info!(session_id, "Teammate session history replaced with summary");
            true
        }
        Ok(Err(e)) => {
            tracing::warn!(session_id, error = %e, "Failed to replace teammate session history");
            false
        }
        Err(e) => {
            tracing::warn!(session_id, error = %e, "Storage task panicked during compaction");
            false
        }
    }
}

// ── System prompt addition for teammate sessions ──────────────────────────────

/// Build the team-context section injected into every teammate's system prompt.
///
/// Template variables:
/// - `{{TEAM_NAME}}` — name of the team
/// - `{{TEAMMATE_NAME}}` — this teammate's friendly name
/// - `{{AGENT_ID}}` — this teammate's agent ID (e.g. `"tm-001"`)
/// - `{{TEAMMATE_ROSTER}}` — list of other teammates with names and agent IDs
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

### Team tool usage

At the start of each turn, call `team_read_messages` (team_name: "{team_name}") to check
for new instructions, plan approval results, or shutdown requests from the lead.

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

fn apply_teammate_model_override(
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

/// Maximum number of lines to inject from `MEMORY.md`.
const MEMORY_MAX_LINES: usize = 200;
/// Maximum bytes to inject from `MEMORY.md`.
const MEMORY_MAX_BYTES: usize = 25 * 1024; // 25 KB

/// Load the persistent-memory prompt block for an agent.
///
/// If `MEMORY.md` exists in `mem_dir`, its content is read (truncated to
/// [`MEMORY_MAX_LINES`] lines / [`MEMORY_MAX_BYTES`] bytes) and wrapped in
/// a labelled section.  The block also tells the agent where its memory
/// directory lives so it can use `team_memory_read` / `team_memory_write`.
///
/// Returns an empty string when memory is unavailable.
fn load_memory_block(mem_dir: &Path) -> String {
    let memory_file = mem_dir.join("MEMORY.md");
    let content = if memory_file.is_file() {
        match std::fs::read_to_string(&memory_file) {
            Ok(raw) => {
                let mut taken = 0usize;
                let truncated: String = raw
                    .lines()
                    .take(MEMORY_MAX_LINES)
                    .take_while(|line| {
                        taken += line.len() + 1; // +1 for newline
                        taken <= MEMORY_MAX_BYTES
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                truncated
            }
            Err(e) => {
                tracing::warn!(path = %memory_file.display(), error = %e, "Failed to read MEMORY.md");
                String::new()
            }
        }
    } else {
        String::new()
    };

    let dir_display = mem_dir.display();
    let preamble = format!(
        "\n\n## Persistent Memory\n\
         \n\
         Your memory directory: `{dir_display}`\n\
         Use `team_memory_read` to read files and `team_memory_write` to write files in this directory.\n\
         Write important findings, decisions, and context to `MEMORY.md` so you can recall them in future sessions.\n"
    );
    if content.is_empty() {
        format!(
            "{preamble}\n\
             _No prior memory found — this is your first session.  Start writing notes to `MEMORY.md`._\n"
        )
    } else {
        format!(
            "{preamble}\n\
             ### Prior Memory (MEMORY.md)\n\
             \n\
             {content}\n"
        )
    }
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
            if let Some(data) = stdin_data {
                if let Some(mut stdin) = child_proc.stdin.take() {
                    use tokio::io::AsyncWriteExt;
                    let _ = stdin.write_all(data.as_bytes()).await;
                    drop(stdin);
                }
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
                        warn!(command, code, "Hook returned unexpected exit code; allowing");
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

    let hook = store.config.settings.hooks.iter().find(|h| h.event == event);
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
          _child_session_id: String,    /// Cancel flag; set to `true` to terminate the teammate's agent loop.
    cancel: Arc<AtomicBool>,
    /// Cancel flag for the mailbox polling task.
    poll_cancel: Arc<AtomicBool>,
    /// Notify handle for push-based mailbox wakeup.
    notify: Arc<Notify>,
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
    handles: Arc<RwLock<HashMap<String, TeammateHandle>>>,
    /// Underlying session processor (shared with the lead).
    processor: Arc<SessionProcessor>,
    /// Event bus for publishing team lifecycle events.
    event_bus: Arc<EventBus>,
    /// Mailbox poll interval.
    poll_interval: Duration,
    /// Serialises spawn operations to avoid concurrent config read/write races.
    spawn_lock: Arc<Mutex<()>>,
    /// The lead's active model — teammates inherit this when spawned via
    /// the reconcile loop (where no ToolContext model is available).
    pub active_model: Option<crate::agent::ModelRef>,
}

impl TeamManager {
    /// Create a new `TeamManager` for an existing team on disk.
    pub fn new(
        team_name: impl Into<String>,
        lead_session_id: impl Into<String>,
        team_dir: PathBuf,
        processor: Arc<SessionProcessor>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self {
            team_name: team_name.into(),
            lead_session_id: lead_session_id.into(),
            team_dir,
            handles: Arc::new(RwLock::new(HashMap::new())),
            processor,
            event_bus,
            poll_interval: Duration::from_millis(500),
            spawn_lock: Arc::new(Mutex::new(())),
            active_model: None,
        }
    }

    /// Reconcile any members recorded on-disk with `Spawning` status by
    /// attempting to spawn them now that the TeamManager exists.
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
                        let to_spawn: Vec<(String, String, String, Option<crate::agent::ModelRef>)> = {
                            let existing_handles = manager.handles.read().await;
                            store.config.members.iter()
                                .filter(|m| m.status == crate::team::config::MemberStatus::Spawning)
                                .filter(|m| {
                                    if m.session_id.is_some() {
                                        tracing::debug!(team = %manager.team_name, teammate = %m.name, "Skipping queued teammate: already has session_id");
                                        return false;
                                    }
                                    if existing_handles.contains_key(&m.agent_id) {
                                        tracing::debug!(team = %manager.team_name, teammate = %m.name, agent_id = %m.agent_id, "Skipping queued teammate: handle already exists");
                                        return false;
                                    }
                                    true
                                })
                                .map(|m| (m.name.clone(), m.agent_type.clone(), m.spawn_prompt.clone().unwrap_or_default(), m.model_override.clone()))
                                .collect()
                        }; // handles read lock dropped here

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
                                .map(|s| s.directory.clone())
                                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                            match manager.spawn_teammate_internal(&name, &agent_type, &spawn_prompt, member_model.as_ref(), manager.active_model.as_ref(), &lead_wd).await {
                                Ok(agent_id) => tracing::info!(team = %manager.team_name, teammate = %name, agent_id = %agent_id, "Successfully reconciled queued teammate"),
                                Err(e) => tracing::warn!(team = %manager.team_name, teammate = %name, error = %e, "Failed to spawn queued teammate"),
                            }
                        }
                    }
                    Err(e) => tracing::warn!(team = %manager.team_name, error = %e, "Cannot load team store to reconcile spawning members"),
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
    pub async fn spawn_teammate_internal(
        &self,
        teammate_name: &str,
        agent_type: &str,
        prompt: &str,
        teammate_model: Option<&crate::agent::ModelRef>,
        lead_model: Option<&crate::agent::ModelRef>,
        working_dir: &Path,
    ) -> Result<String> {
        let _guard = self.spawn_lock.lock().await;

        // ── Single config load: allocate agent ID ──────────────────────────
        let mut store = TeamStore::load(&self.team_dir)?;
        let agent_id = if let Some(existing) = store.config.member_by_name(teammate_name) {
            tracing::debug!(team = %self.team_name, teammate = %teammate_name, agent_id = %existing.agent_id, "Reusing existing member record for spawn");
            existing.agent_id.clone()
        } else {
            let id = store.next_agent_id();
            tracing::info!(team = %self.team_name, teammate = %teammate_name, agent_id = %id, "Allocating new agent id and recording Spawning member");
            let mut member = TeamMember::new(teammate_name, &id, agent_type);
            member.status = MemberStatus::Spawning;
            member.model_override = teammate_model.cloned();
            store.add_member(member)?;
            id
        };
        // Persist now so external tools (e.g., team_create) see the member.
        store.save()?;
        drop(store);

        // Create isolated child session.
        tracing::info!(team = %self.team_name, agent_id = %agent_id, "Creating child session for teammate");
        let child_session = self
            .processor
            .session_manager
            .create_session(working_dir.to_path_buf())?;
        let child_sid = child_session.id.clone();

        // ── Single config reload: update session, build roster, read memory ─
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
                .map(|m| m.memory_scope)
                .unwrap_or(super::config::MemoryScope::None);
            store.save()?;
            tracing::debug!(team = %self.team_name, agent_id = %agent_id, "Team config saved after session assignment");
            (roster, mem_scope)
        };

        // Resolve agent and augment system prompt.
        let config = Config::default();
        let mut agent = resolve_agent_with_customs(agent_type, &config, working_dir)
            .unwrap_or_else(|_| AgentInfo::new(agent_type, "Teammate agent"));
        agent.mode = AgentMode::Subagent;
        apply_teammate_model_override(&mut agent, teammate_model, lead_model);

        // Ensure the agent has a model configured. Some custom agent names may
        // not resolve to a configured model; fall back to the built-in "general"
        // agent's model to avoid immediate startup failures in the agent loop.
        if agent.model.is_none() {
            if let Ok(default_agent) = crate::agent::resolve_agent("general", &config) {
                agent.model = default_agent.model;
                tracing::info!(team = %self.team_name, teammate = %teammate_name, agent_type = %agent_type, "No model on agent; falling back to 'general' model");
            }
        }

        let team_addition = build_team_prompt_addition(
            &self.team_name,
            teammate_name,
            &agent_id,
            &teammate_roster,
        );
        // Append the team context block to the agent's system prompt.
        let base = agent.prompt.as_deref().unwrap_or("");
        agent.prompt = Some(format!("{base}\n{team_addition}"));

        // ── Persistent memory injection ────────────────────────────────────
        // Resolve memory scope: member-level config (from blueprint) takes
        // priority, then the agent profile's setting, then None.
        let effective_scope = if memory_scope != super::config::MemoryScope::None {
            memory_scope
        } else {
            agent.memory
        };
        if let Some(mem_dir) = super::config::resolve_memory_dir(
            effective_scope,
            teammate_name,
            working_dir,
        ) {
            let memory_block = load_memory_block(&mem_dir);
            let current = agent.prompt.as_deref().unwrap_or("");
            agent.prompt = Some(format!("{current}\n{memory_block}"));
        }

        let cancel = Arc::new(AtomicBool::new(false));
        let poll_cancel = Arc::new(AtomicBool::new(false));
        let notify = Arc::new(Notify::new());

        // Register notifier so Mailbox::push() can wake this agent's poll loop.
        register_notifier(&self.team_dir, &agent_id, Arc::clone(&notify));

        // Register handle.
        self.handles.write().await.insert(
            agent_id.clone(),
                          TeammateHandle {
                              _name: teammate_name.to_string(),
                              _agent_id: agent_id.clone(),
                              _child_session_id: child_sid.clone(),
                              cancel: cancel.clone(),
                              poll_cancel: poll_cancel.clone(),
                              notify: Arc::clone(&notify),
                          },        );

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
            // Retry with linear backoff for transient API errors (e.g. rate limits).
            // Token overflow errors get one free compaction attempt before counting
            // as a retry, so the teammate can resume with a smaller context window.
            const MAX_RETRIES: u32 = 3;
            let mut last_error = String::new();
            let mut compacted = false; // only compact once per spawn
            for attempt in 0..=MAX_RETRIES {
                if attempt > 0 {
                    let backoff = std::time::Duration::from_millis(500 * attempt as u64);
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
                    .process_message(&child_sid_clone, &prompt_owned, &agent_clone, cancel_clone.clone())
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
                        last_error = format!("{}", e);
                        warn!(
                            child_session = %child_sid_clone,
                            error = %last_error,
                            attempt,
                            max_retries = MAX_RETRIES,
                            "Teammate agent loop error"
                        );

                        // Token overflow: compact the session history then retry
                        // without burning a retry attempt (first overflow only).
                        if is_token_overflow_error(&last_error) && !compacted {
                            compacted = true;
                            tracing::warn!(
                                team = %team_name_clone,
                                agent_id = %agent_id_clone,
                                "Token overflow — compacting teammate session before retry"
                            );
                            event_bus_clone.publish(Event::AgentError {
                                session_id: child_sid_clone.clone(),
                                error: format!(
                                    "Context window full — compacting history and retrying ({agent_id_clone})"
                                ),
                            });
                            compact_teammate_session(&proc, &child_sid_clone, &agent_clone).await;
                            // Don't increment attempt — retry immediately after compact
                            continue;
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
                    _ = notify.notified() => {}
                    _ = tokio::time::sleep(interval) => {}
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

    /// Request graceful shutdown of a teammate by agent ID.
    ///
    /// Sets the cancel flag (terminates the agent loop) and the poll cancel
    /// flag. Also pushes a `ShutdownRequest` to the teammate's mailbox and
    /// marks the member `Stopped` in config.
    pub async fn shutdown_teammate(&self, agent_id: &str) -> Result<()> {
        let handles = self.handles.read().await;
        if let Some(handle) = handles.get(agent_id) {
            handle.cancel.store(true, Ordering::Relaxed);
            handle.poll_cancel.store(true, Ordering::Relaxed);
            // Wake the poll loop so it sees the cancel flag immediately.
            handle.notify.notify_one();
        }
        drop(handles);

        // Deregister the notifier now that this agent is shutting down.
        deregister_notifier(&self.team_dir, agent_id);

        // Push shutdown_request to mailbox (in case the agent loop has not
        // yet terminated and checks its mailbox).
        let mailbox = Mailbox::open(&self.team_dir, agent_id)?;
        mailbox.push(MailboxMessage::new(
            "lead".to_string(),
            agent_id.to_string(),
            MessageType::ShutdownRequest,
            "Session shutdown requested by TeamManager.",
        ))?;

        // Mark member as Stopped.
        let mut store = TeamStore::load(&self.team_dir)?;
        if let Some(member) = store.config.member_by_id_mut(agent_id) {
            member.status = MemberStatus::Stopped;
        }
        store.save()?;

        Ok(())
    }

    /// Shut down all active teammates and clean up.
    pub async fn shutdown_all(&self) -> Result<()> {
        let agent_ids: Vec<String> = {
            let handles = self.handles.read().await;
            handles.keys().cloned().collect()
        };
        for id in agent_ids {
            if let Err(e) = self.shutdown_teammate(&id).await {
                warn!(agent_id = %id, error = %e, "Error during teammate shutdown");
            }
        }
        Ok(())
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
        store.save()
    }

    /// Returns `true` if the teammate has a pending plan (used by the processor
    /// to block write/bash tools while `PlanPending`).
    pub fn is_plan_pending(&self, agent_id: &str) -> bool {
        TeamStore::load(&self.team_dir)
            .ok()
            .and_then(|s| s.config.member_by_id(agent_id).map(|m| m.plan_status == PlanStatus::Pending))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::apply_teammate_model_override;
    use crate::agent::{AgentInfo, ModelRef};

    #[test]
    fn test_teammate_model_takes_priority_over_lead() {
        let mut agent = AgentInfo::new("explore", "test");
        agent.model = Some(ModelRef {
            provider_id: "anthropic".to_string(),
            model_id: "claude-3-5-haiku-latest".to_string(),
        });

        let teammate = ModelRef {
            provider_id: "openai".to_string(),
            model_id: "gpt-4o".to_string(),
        };
        let lead = ModelRef {
            provider_id: "copilot".to_string(),
            model_id: "claude-sonnet-4.6".to_string(),
        };
        apply_teammate_model_override(&mut agent, Some(&teammate), Some(&lead));

        let model = agent.model.expect("model set");
        assert_eq!(model.provider_id, "openai");
        assert_eq!(model.model_id, "gpt-4o");
    }

    #[test]
    fn test_lead_model_used_when_no_teammate_model() {
        let mut agent = AgentInfo::new("explore", "test");
        agent.model = Some(ModelRef {
            provider_id: "anthropic".to_string(),
            model_id: "claude-3-5-haiku-latest".to_string(),
        });

        let lead = ModelRef {
            provider_id: "copilot".to_string(),
            model_id: "claude-sonnet-4.6".to_string(),
        };
        apply_teammate_model_override(&mut agent, None, Some(&lead));

        let model = agent.model.expect("model set");
        assert_eq!(model.provider_id, "copilot");
        assert_eq!(model.model_id, "claude-sonnet-4.6");
    }

    #[test]
    fn test_agent_default_preserved_when_no_overrides() {
        let mut agent = AgentInfo::new("explore", "test");
        agent.model = Some(ModelRef {
            provider_id: "anthropic".to_string(),
            model_id: "claude-3-5-haiku-latest".to_string(),
        });

        apply_teammate_model_override(&mut agent, None, None);

        let model = agent.model.expect("model set");
        assert_eq!(model.provider_id, "anthropic");
        assert_eq!(model.model_id, "claude-3-5-haiku-latest");
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
                preview,
            });
        }
        _ => {
            event_bus.publish(Event::TeammateMessage {
                session_id: lead_session_id.to_string(),
                team_name: team_name.to_string(),
                from: msg.from.clone(),
                to: msg.to.clone(),
                preview,
            });
        }
    }
}
