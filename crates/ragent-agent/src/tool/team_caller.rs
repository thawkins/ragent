//! Caller identity and role checks shared by the `team_*` tools.
//!
//! Every team tool acts on the team named by its `team_name` argument, so it
//! must first establish who is calling *in that team*. Identity is taken from
//! the session's resolved [`TeamContext`](super::TeamContext) when it names
//! the same team, and otherwise from the team's own `config.json` (the lead
//! session id, or a member whose `session_id` is this session). A session that
//! is neither the lead nor a member of the named team is refused, so a
//! teammate of team A cannot act on team B and a session outside every team
//! cannot impersonate the lead.
//!
//! Depends on [`crate::team::TeamStore`] for the on-disk roster.

use std::path::Path;

use anyhow::{Result, bail};

use super::ToolContext;
use crate::team::TeamStore;

/// The calling session's role in one team.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeamCaller {
    /// The session is the team lead.
    Lead,
    /// The session is the teammate with this agent id (e.g. `"tm-001"`).
    Member(String),
}

impl TeamCaller {
    /// The mailbox / task-store identity of the caller: `"lead"` or the
    /// teammate's agent id.
    #[must_use]
    pub fn agent_id(&self) -> &str {
        match self {
            Self::Lead => "lead",
            Self::Member(id) => id,
        }
    }

    /// `true` when the caller is the team lead.
    #[must_use]
    pub fn is_lead(&self) -> bool {
        matches!(self, Self::Lead)
    }
}

/// Resolve the calling session's role in team `team_name`.
///
/// # Arguments
/// * `ctx` - the tool context (session id and resolved team context).
/// * `team_name` - the team named in the tool call.
/// * `team_dir` - that team's directory.
///
/// # Returns
/// The caller's role in the team.
///
/// # Errors
/// Returns an error when the session is neither the lead nor a member of
/// the team, or when the team's `config.json` cannot be read.
pub fn resolve_caller(ctx: &ToolContext, team_name: &str, team_dir: &Path) -> Result<TeamCaller> {
    if let Some(tc) = ctx.team_context.as_deref()
        && tc.team_name == team_name
    {
        return Ok(if tc.is_lead {
            TeamCaller::Lead
        } else {
            TeamCaller::Member(tc.agent_id.clone())
        });
    }
    let store = TeamStore::load(team_dir)?;
    if store.config.lead_session_id == ctx.session_id {
        return Ok(TeamCaller::Lead);
    }
    if let Some(member) = store
        .config
        .members
        .iter()
        .find(|m| m.session_id.as_deref() == Some(ctx.session_id.as_str()))
    {
        return Ok(TeamCaller::Member(member.agent_id.clone()));
    }
    bail!(
        "This session is not a member of team '{team_name}'; only its lead and teammates can use it"
    )
}

/// Refuse the call unless the calling session is the lead of `team_name`.
///
/// # Arguments
/// * `ctx` - the tool context.
/// * `team_name` - the team named in the tool call.
/// * `team_dir` - that team's directory.
/// * `tool` - the tool name, used in the refusal message.
///
/// # Errors
/// Returns an error when the caller is a teammate, or is not in the team.
pub fn require_lead(ctx: &ToolContext, team_name: &str, team_dir: &Path, tool: &str) -> Result<()> {
    match resolve_caller(ctx, team_name, team_dir)? {
        TeamCaller::Lead => Ok(()),
        TeamCaller::Member(id) => {
            bail!("{tool} is lead-only: teammate '{id}' cannot use it in team '{team_name}'")
        }
    }
}
