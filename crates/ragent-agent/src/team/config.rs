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
/// The directory is **not** created — callers should create it on first write.
#[must_use]
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
