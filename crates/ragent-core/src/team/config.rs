//! Team configuration types: `TeamConfig`, `TeamMember`, `TeamStatus`, `MemberStatus`.
//!
//! These types are serialised to/from `config.json` inside the team directory.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
    /// Graceful shutdown in progress (awaiting `team_shutdown_ack`).
    ShuttingDown,
    /// Session has terminated.
    Stopped,
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
}

impl TeamMember {
    /// Create a new member record in `Spawning` state.
    pub fn new(name: impl Into<String>, agent_id: impl Into<String>, agent_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            agent_id: agent_id.into(),
            session_id: None,
            agent_type: agent_type.into(),
            status: MemberStatus::Spawning,
            current_task_id: None,
            plan_status: PlanStatus::None,
            created_at: Utc::now(),
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
}

impl Default for TeamSettings {
    fn default() -> Self {
        Self {
            max_teammates: 8,
            require_plan_approval: false,
            auto_claim_tasks: true,
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
        self.members.iter().filter(|m| m.status != MemberStatus::Stopped)
    }
}
