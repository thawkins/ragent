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
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::agent::{AgentInfo, AgentMode, resolve_agent_with_customs};
use crate::config::Config;
use crate::event::{Event, EventBus};
use crate::session::processor::SessionProcessor;
use crate::team::config::{MemberStatus, PlanStatus};
use crate::team::mailbox::{Mailbox, MailboxMessage, MessageType};
use crate::team::store::TeamStore;
use crate::team::TeamMember;
use crate::tool::TeamManagerInterface;

// ── System prompt addition for teammate sessions ──────────────────────────────

/// Build the team-context section injected into every teammate's system prompt.
///
/// Template variables:
/// - `{{TEAM_NAME}}` — name of the team
/// - `{{TEAMMATE_NAME}}` — this teammate's friendly name
/// - `{{AGENT_ID}}` — this teammate's agent ID (e.g. `"tm-001"`)
/// - `{{TEAMMATE_LIST}}` — comma-separated list of other teammate names
pub fn build_team_prompt_addition(
    team_name: &str,
    teammate_name: &str,
    agent_id: &str,
    teammate_list: &[String],
) -> String {
    let others = if teammate_list.is_empty() {
        "none yet".to_string()
    } else {
        teammate_list.join(", ")
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
"#,
    )
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
pub async fn run_hook(command: &str, args: &[String]) -> HookOutcome {
    let output = tokio::process::Command::new(command)
        .args(args)
        .output()
        .await;

    match output {
        Err(e) => {
            warn!(command, error = %e, "Hook failed to execute");
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
        }
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
        working_dir: &Path,
    ) -> Result<String> {
        // Allocate agent ID and update config.
        let agent_id = {
            let mut store = TeamStore::load(&self.team_dir)?;
            let id = store.next_agent_id();
            let mut member = TeamMember::new(teammate_name, &id, agent_type);
            member.status = MemberStatus::Spawning;
            store.add_member(member)?;
            id
        };

        // Create isolated child session.
        let child_session = self
            .processor
            .session_manager
            .create_session(working_dir.to_path_buf())?;
        let child_sid = child_session.id.clone();

        // Update config with child session ID + set Working.
        {
            let mut store = TeamStore::load(&self.team_dir)?;
            if let Some(m) = store.config.member_by_id_mut(&agent_id) {
                m.session_id = Some(child_sid.clone());
                m.status = MemberStatus::Working;
            }
            store.save()?;
        }

        // Build teammate roster (other members).
        let teammate_list: Vec<String> = {
            let store = TeamStore::load(&self.team_dir)?;
            store
                .config
                .members
                .iter()
                .filter(|m| m.agent_id != agent_id)
                .map(|m| m.name.clone())
                .collect()
        };

        // Resolve agent and augment system prompt.
        let config = Config::default();
        let mut agent = resolve_agent_with_customs(agent_type, &config, working_dir)
            .unwrap_or_else(|_| AgentInfo::new(agent_type, "Teammate agent"));
        agent.mode = AgentMode::Subagent;

        let team_addition = build_team_prompt_addition(
            &self.team_name,
            teammate_name,
            &agent_id,
            &teammate_list,
        );
        // Append the team context block to the agent's system prompt.
        let base = agent.prompt.as_deref().unwrap_or("");
        agent.prompt = Some(format!("{base}\n{team_addition}"));

        let cancel = Arc::new(AtomicBool::new(false));
        let poll_cancel = Arc::new(AtomicBool::new(false));

        // Register handle.
        self.handles.write().await.insert(
            agent_id.clone(),
                          TeammateHandle {
                              _name: teammate_name.to_string(),
                              _agent_id: agent_id.clone(),
                              _child_session_id: child_sid.clone(),
                              cancel: cancel.clone(),
                              poll_cancel: poll_cancel.clone(),
                          },        );

        // Start agent loop in background.
        let proc = Arc::clone(&self.processor);
        let child_sid_clone = child_sid.clone();
        let agent_clone = agent.clone();
        let prompt_owned = prompt.to_string();
        let cancel_clone = cancel.clone();
        tokio::spawn(async move {
            if let Err(e) = proc
                .process_message(&child_sid_clone, &prompt_owned, &agent_clone, cancel_clone)
                .await
            {
                warn!(child_session = %child_sid_clone, error = %e, "Teammate agent loop error");
            }
        });

        // Start mailbox polling loop.
        self.start_poll_loop(agent_id.clone(), poll_cancel);

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
    fn start_poll_loop(&self, agent_id: String, cancel: Arc<AtomicBool>) {
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
                tokio::time::sleep(interval).await;
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
        }
        drop(handles);

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

// ── TeamManagerInterface impl ────────────────────────────────────────────────

#[async_trait::async_trait]
impl TeamManagerInterface for TeamManager {
    async fn spawn_teammate(
        &self,
        _team_name: &str,
        teammate_name: &str,
        agent_type: &str,
        prompt: &str,
        working_dir: &Path,
    ) -> Result<String> {
        self.spawn_teammate_internal(teammate_name, agent_type, prompt, working_dir)
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
