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

/// Resolve the memory directory for a given agent name and scope.
///
/// Returns `None` when `scope` is [`MemoryScope::None`].
/// The directory is **not** created — callers should create it on first write.
pub fn resolve_memory_dir(
    scope: MemoryScope,
    agent_name: &str,
    working_dir: &Path,
) -> Option<PathBuf> {
    match scope {
        MemoryScope::None => Option::None,
        MemoryScope::User => {
            let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            Some(home.join(".ragent").join("agent-memory").join(agent_name))
        }
        MemoryScope::Project => Some(
            working_dir
                .join(".ragent")
                .join("agent-memory")
                .join(agent_name),
        ),
    }
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

/// Lifecycle state of an individual teammate session.
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
    /// Graceful shutdown in progress (awaiting `team_shutdown_ack`).
    ShuttingDown,
    /// Session has terminated.
    Stopped,
    /// Spawn or startup failed; see `last_spawn_error` for details.
    Failed,
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

/// Lifecycle event that can trigger a quality-gate hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HookEvent {
    /// Fires when a teammate reports idle (no more tasks).
    TeammateIdle,
    /// Fires when a new task is added to the shared task list.
    TaskCreated,
    /// Fires when a task is marked as completed.
    TaskCompleted,
}

/// A single quality-gate hook: an event trigger paired with a shell command.
///
/// When the matching `event` fires, the `command` is executed as a shell
/// command.  Exit codes follow the quality-gate protocol:
///
/// - **Exit 0** → allow the action.
/// - **Exit 2** → reject / send feedback (stdout is returned to the agent).
/// - **Other** → log a warning, allow the action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEntry {
    /// The lifecycle event that triggers this hook.
    pub event: HookEvent,
    /// Shell command to run when the event fires.
    pub command: String,
}

/// Describes one teammate within a team.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    /// Human-friendly name for this teammate (e.g. `"security-reviewer"`).
    pub name: String,
    /// Unique agent ID assigned at spawn time (e.g. `"tm-001"`).
    pub agent_id: String,
    /// Session ID of the underlying ragent session, if spawned.
    pub session_id: Option<String>,
    /// Agent type / definition name used when spawning this session.
    pub agent_type: String,
    /// Current lifecycle state.
    pub status: MemberStatus,
    /// ID of the task currently being worked on, if any.
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
        }
    }
}

/// Team-wide settings stored inside `config.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamSettings {
    /// Maximum number of concurrent teammates allowed.
    pub max_teammates: usize,
    /// If `true`, teammates must submit a plan before starting implementation.
    pub require_plan_approval: bool,
    /// If `true`, teammates automatically claim the next available task when idle.
    pub auto_claim_tasks: bool,
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
            hooks: Vec::new(),
        }
    }
}

/// Root configuration object for a team, stored as `config.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamConfig {
    /// Unique name for this team (also the directory name).
    pub name: String,
    /// Session ID of the lead session that created this team.
    pub lead_session_id: String,
    /// When the team was created.
    pub created_at: DateTime<Utc>,
    /// Current overall status.
    pub status: TeamStatus,
    /// All registered teammates (active and stopped).
    pub members: Vec<TeamMember>,
    /// Team-wide settings.
    pub settings: TeamSettings,
}

impl TeamConfig {
    /// Create a new team config with no members.
    pub fn new(name: impl Into<String>, lead_session_id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            lead_session_id: lead_session_id.into(),
            created_at: Utc::now(),
            status: TeamStatus::Active,
            members: Vec::new(),
            settings: TeamSettings::default(),
        }
    }

    /// Return the member with the given `agent_id`, if found.
    pub fn member_by_id(&self, agent_id: &str) -> Option<&TeamMember> {
        self.members.iter().find(|m| m.agent_id == agent_id)
    }

    /// Return a mutable reference to the member with the given `agent_id`, if found.
    pub fn member_by_id_mut(&mut self, agent_id: &str) -> Option<&mut TeamMember> {
        self.members.iter_mut().find(|m| m.agent_id == agent_id)
    }

    /// Return the member with the given `name`, if found.
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
