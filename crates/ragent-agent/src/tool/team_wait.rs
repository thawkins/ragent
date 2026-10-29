//! `team_wait` - block the lead agent until every awaited teammate settles.
//!
//! Subscribes to [`Event::TeammateIdle`], [`Event::TeammateFailed`],
//! [`Event::TeammateSuspended`], and [`Event::TeammateResumed`] so there is no
//! polling. A teammate settles when it reaches a state it cannot leave on its
//! own: `Idle` (finished and waiting for work), any parked state (waiting on
//! something external), or any terminal state (gone). Only a `is_live()`
//! member - one with an outstanding action it must finish by itself - is still
//! being waited on.
//!
//! On return the tool reports, for every member it was waiting on, how that
//! member ended the wait: `Idle`, parked (with the parked status named),
//! failed, or still running when the timeout fired (spec `teamssetup` FR-031).
//! That distinction is the point of the report: "the wait timed out" alone
//! cannot tell an operator whether a teammate is deliberately paused or wedged.
//!
//! This tool is the team-aware counterpart to `wait_agents`.  Use it after
//! spawning teammates so the lead does not race ahead and duplicate their work.

use std::collections::{BTreeMap, HashSet};
use std::time::Duration;

use anyhow::Result;
use serde_json::{Value, json};

use crate::event::Event;
use crate::team::TeamStore;
use crate::team::config::{MemberStatus, TeamMember};

use super::{Tool, ToolContext, ToolOutput};

/// How a teammate ended a `team_wait` call (spec `teamssetup` FR-031).
///
/// The four outcomes are mutually exclusive and, together with the
/// `still_running` outcome carrying the member's live status, cover every
/// [`MemberStatus`]:
///
/// | Outcome | Statuses |
/// | --- | --- |
/// | [`WaitOutcome::Idle`] | `Idle` |
/// | [`WaitOutcome::Parked`] | `PlanPending`, `Blocked`, `Suspended` |
/// | [`WaitOutcome::Failed`] | `Failed` |
/// | [`WaitOutcome::StillRunning`] | `Spawning`, `Working`, `ShuttingDown`, `Stopped` |
///
/// `Idle` is reported by its own name rather than as "parked" because it is the
/// outcome `team_wait` exists to reach: the teammate finished and is ready for
/// more work. `Stopped` lands in `StillRunning`: a graceful shutdown was
/// requested but not acknowledged, and `team_wait` does not treat an
/// unacknowledged shutdown as settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum WaitOutcome {
    /// The teammate is `Idle` and ready for more work.
    Idle,
    /// The teammate is parked, waiting on something external.
    Parked,
    /// The teammate is `Failed`.
    Failed,
    /// The teammate never settled before the wait deadline.
    StillRunning,
}

impl WaitOutcome {
    /// Classify a member's final status into the outcome `team_wait` reports.
    fn of(status: MemberStatus) -> Self {
        match status {
            MemberStatus::Idle => Self::Idle,
            MemberStatus::Failed => Self::Failed,
            MemberStatus::PlanPending | MemberStatus::Blocked | MemberStatus::Suspended => {
                Self::Parked
            }
            MemberStatus::Spawning
            | MemberStatus::Working
            | MemberStatus::ShuttingDown
            | MemberStatus::Stopped => Self::StillRunning,
        }
    }

    /// The metadata token for this outcome.
    fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Parked => "parked",
            Self::Failed => "failed",
            Self::StillRunning => "still_running",
        }
    }

    /// Whether this outcome settles the wait, so the member is not awaited
    /// further. Only a still-running member keeps the wait open.
    fn is_settled(&self) -> bool {
        !matches!(self, Self::StillRunning)
    }

    /// The sentence fragment naming this outcome, used in the per-member report.
    fn describe(&self, status: MemberStatus) -> String {
        let label = status.display_label();
        match self {
            Self::Idle => format!("idle ({label})"),
            Self::Parked => format!("parked ({label}) - waiting on something external"),
            Self::Failed => "failed".to_string(),
            Self::StillRunning => format!("still running ({label}) at timeout"),
        }
    }
}

/// Blocks until all (or specified) teammates in a team become idle.
pub struct TeamWaitTool;

#[async_trait::async_trait]
impl Tool for TeamWaitTool {
    fn name(&self) -> &'static str {
        "team_wait"
    }

    fn description(&self) -> &'static str {
        "Block until all awaited teammates (or specific ones) in the active team settle. A \
             teammate settles when it is idle, parked (planning, blocked, or suspended - \
             waiting on something external), or failed; only a still-running teammate keeps \
             the wait open. REQUIRED parameter: 'team_name' (string) is \
             recommended; the active team is used if omitted. Optional: 'agent_ids' (array \
             of strings) to wait for a subset, and 'timeout_secs' (number, default 300). Use \
             this after team_spawn so the lead does not race ahead and duplicate the \
             teammates' work. Returns a per-teammate outcome - idle, parked, failed, or \
             still running at timeout - so a paused teammate is never confused with a wedged \
             one. \
             Common gotcha: this is for team members only; do NOT use wait_agents for \
             teammates (it is for new_agent sub-agents)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team to wait on. Uses the active team if omitted."
                },
                "agent_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Specific agent IDs (e.g. 'tm-001') to wait for. Omit to wait for ALL working teammates."
                },
                "timeout_secs": {
                    "type": "number",
                    "description": "Maximum seconds to wait before returning partial results. Default: 300."
                }
            },
            "additionalProperties": false
        })
    }
    fn permission_category(&self) -> &'static str {
        "agent:spawn"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let timeout_secs = input
            .get("timeout_secs")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(300);

        let working_dir = ctx.working_dir.clone();

        // Resolve team name - use provided value or infer from on-disk teams.
        let team_name: Option<String> = input
            .get("team_name")
            .and_then(|v| v.as_str())
            .map(std::string::ToString::to_string);

        // M3-T1: subscribe to the event bus BEFORE reading the team store so
        // that any `TeammateIdle` / `TeammateFailed` events that fire between
        // the store read and the wait loop are captured in `rx` rather than
        // lost. Events that arrive while we are computing the initial
        // `waiting_for` set are reconciled below.
        let mut rx = ctx.event_bus.subscribe();

        // Load the team store to discover current membership.
        let store = if let Some(ref name) = team_name {
            TeamStore::load_by_name(name, &working_dir)
                .map_err(|e| anyhow::anyhow!("Cannot load team '{name}': {e}"))?
        } else {
            // Find the most recently modified team.
            let teams = TeamStore::list_teams(&working_dir);
            let (_, dir, _) = teams
                .into_iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("No teams found in this project."))?;
            TeamStore::load(&dir).map_err(|e| anyhow::anyhow!("Cannot load team store: {e}"))?
        };

        let resolved_team_name = store.config.name.clone();
        let lead_session_id = store.config.lead_session_id.clone();

        // Determine which agent IDs to wait for.
        let requested_ids: HashSet<String> = input
            .get("agent_ids")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        // The full selection: every member this call must report on, whether or
        // not it is waitable (FR-031). Captured before the wait starts removing
        // settled members, and kept intact for the rest of the call so the
        // outcome report answers for each teammate the caller asked about - not
        // just the ones still blocking at the end.
        //
        // An explicit `agent_ids` list is the selection. Otherwise the selection
        // is the whole team, so a teammate that is already parked is still
        // reported as parked instead of silently omitted.
        let mut awaited_ids: Vec<String> = if requested_ids.is_empty() {
            store
                .config
                .members
                .iter()
                .map(|m| m.agent_id.clone())
                .collect()
        } else {
            requested_ids.iter().cloned().collect()
        };
        awaited_ids.sort();

        // Build the set of agent IDs we are waiting for.
        //
        // FR-031: only a member that is still running can be waited on. A parked
        // member (`PlanPending`, `Blocked`, `Suspended`) is settled the moment we
        // look at it - it is waiting on something external and no amount of time
        // moves it - so it is reported as parked rather than awaited. `Idle`,
        // `Failed`, and `Stopped` are settled for the same reason. The previous
        // spelling of this filter listed `PlanPending` and `Suspended` as
        // waitable, which made every deliberately paused teammate cost the lead a
        // full timeout (F-17).
        let mut waiting_for: HashSet<String> = awaited_ids
            .iter()
            .filter(|id| {
                store
                    .config
                    .member_by_id(id)
                    .is_none_or(|m| !WaitOutcome::of(m.status).is_settled())
            })
            .cloned()
            .collect();

        // M3-T1: drain any events that arrived between the subscribe and the
        // store read. These may report teammates that the store scan still
        // considered active. Reconcile them into `waiting_for`.
        //
        // FR-031: a member that parked or resumed before the loop started is
        // reconciled the same way, so a pause that landed during the store read
        // does not leave `team_wait` blocking on a teammate that never moves.
        while let Ok(event) = rx.try_recv() {
            match event {
                Event::TeammateIdle {
                    session_id,
                    team_name: ev_team,
                    agent_id,
                } if session_id == lead_session_id
                    && ev_team == resolved_team_name
                    && waiting_for.contains(&agent_id) =>
                {
                    waiting_for.remove(&agent_id);
                }
                // M3-T2: handle TeammateFailed events that arrived pre-loop.
                Event::TeammateFailed {
                    session_id,
                    team_name: ev_team,
                    agent_id,
                    ..
                } if session_id == lead_session_id
                    && ev_team == resolved_team_name
                    && waiting_for.contains(&agent_id) =>
                {
                    waiting_for.remove(&agent_id);
                }
                // FR-031: a suspension parks the member; it is settled.
                Event::TeammateSuspended {
                    session_id,
                    team_name: ev_team,
                    agent_id,
                } if session_id == lead_session_id
                    && ev_team == resolved_team_name
                    && waiting_for.contains(&agent_id) =>
                {
                    waiting_for.remove(&agent_id);
                }
                _ => continue,
            }
        }

        if waiting_for.is_empty() {
            let summary = summarise_store(&store.config.members);
            let report = outcome_report(&store.config.members, &awaited_ids);
            let outcomes = outcome_metadata(&store.config.members, &awaited_ids);
            return Ok(ToolOutput {
                content: format!(
                    "All awaited teammates in team '{resolved_team_name}' are already settled.\n\n{report}{summary}"
                ),
                metadata: Some(json!({
                    "team_name": resolved_team_name,
                    "timed_out": false,
                    "success": true,
                    "outcomes": outcomes,
                })),
            });
        }

        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);

        tracing::info!(
            team = %resolved_team_name,
            waiting_for = ?waiting_for,
            timeout_secs,
            "team_wait: waiting for teammates to settle"
        );

        loop {
            if waiting_for.is_empty() {
                break;
            }
            match tokio::time::timeout_at(deadline, rx.recv()).await {
                Ok(Ok(Event::TeammateIdle {
                    session_id,
                    team_name: ev_team,
                    agent_id,
                })) if session_id == lead_session_id
                    && ev_team == resolved_team_name
                    && waiting_for.contains(&agent_id) =>
                {
                    tracing::info!(team = %resolved_team_name, agent_id = %agent_id, "team_wait: teammate became idle");
                    waiting_for.remove(&agent_id);
                }
                // M3-T2: a failed teammate will never become idle; treat it
                // as finished and remove it from the waiting set so the lead
                // does not wait the full timeout for a dead agent.
                Ok(Ok(Event::TeammateFailed {
                    session_id,
                    team_name: ev_team,
                    agent_id,
                    error,
                })) if session_id == lead_session_id
                    && ev_team == resolved_team_name
                    && waiting_for.contains(&agent_id) =>
                {
                    tracing::info!(
                        team = %resolved_team_name,
                        agent_id = %agent_id,
                        error = %error,
                        "team_wait: teammate failed; treating as finished"
                    );
                    waiting_for.remove(&agent_id);
                }
                // FR-031: a teammate the lead parks mid-wait is settled, not
                // wedged - stop waiting for it so a deliberate pause does not
                // burn the timeout. `TeammateResumed` is deliberately ignored: a
                // resume moves a member to `Idle`, which the post-loop disk
                // reconcile observes as settled anyway.
                Ok(Ok(Event::TeammateSuspended {
                    session_id,
                    team_name: ev_team,
                    agent_id,
                })) if session_id == lead_session_id
                    && ev_team == resolved_team_name
                    && waiting_for.contains(&agent_id) =>
                {
                    tracing::info!(
                        team = %resolved_team_name,
                        agent_id = %agent_id,
                        "team_wait: teammate was parked mid-wait; treating as settled"
                    );
                    waiting_for.remove(&agent_id);
                }
                Ok(Ok(_)) => continue,
                Ok(Err(_)) => break, // channel closed
                Err(_) => break,     // timeout
            }
        }

        // M3-T3: before declaring a timeout, re-read the team store and treat
        // any member that has settled on disk as finished. This covers the case
        // where an `EventBus` event was dropped (buffer full / no subscribers)
        // but the teammate legitimately reached a settled state, and it is also
        // how a member parked without an event (a direct config write) is
        // recognised as parked rather than still running (FR-031).
        let mut final_members = store.config.members.clone();
        if !waiting_for.is_empty() {
            if let Ok(fresh) = TeamStore::load_by_name(&resolved_team_name, &working_dir) {
                let settled_on_disk: Vec<String> = fresh
                    .config
                    .members
                    .iter()
                    .filter(|m| {
                        WaitOutcome::of(m.status).is_settled() && waiting_for.contains(&m.agent_id)
                    })
                    .map(|m| m.agent_id.clone())
                    .collect();
                for id in &settled_on_disk {
                    waiting_for.remove(id);
                }
                tracing::info!(
                    team = %resolved_team_name,
                    reconciled = ?settled_on_disk,
                    remaining = ?waiting_for,
                    "team_wait: reconciled settled teammates from disk after timeout window"
                );
                final_members = fresh.config.members;
            }
        }

        let timed_out = !waiting_for.is_empty();

        let summary = summarise_store(&final_members);
        let report = outcome_report(&final_members, &awaited_ids);
        let outcomes = outcome_metadata(&final_members, &awaited_ids);

        let mut output = String::new();
        if timed_out {
            output.push_str(&format!(
                "[!] Timed out after {timeout_secs}s. {} teammate(s) still running: {}\n\n",
                waiting_for.len(),
                sorted_ids(&waiting_for).join(", ")
            ));
        } else {
            output.push_str(&format!(
                "All awaited teammates in team '{resolved_team_name}' are now settled.\n\n"
            ));
        }
        output.push_str(&report);
        output.push_str(&summary);

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "team_name": resolved_team_name,
                "timed_out": timed_out,
                "success": !timed_out,
                "idle_count": final_members.iter().filter(|m| m.status == MemberStatus::Idle).count(),
                "still_working_count": waiting_for.len(),
                "still_working": sorted_ids(&waiting_for),
                "outcomes": outcomes,
            })),
        })
    }
}

/// Sorted agent IDs from a set, so reports and metadata are stable across runs.
fn sorted_ids(ids: &HashSet<String>) -> Vec<String> {
    let mut sorted: Vec<String> = ids.iter().cloned().collect();
    sorted.sort();
    sorted
}

/// Render the per-member outcome block `team_wait` reports (FR-031).
///
/// `awaited` is the set captured before the wait loop started: every member the
/// caller asked about, whether it settled during the wait or was found settled
/// on the first read. Every member in it appears exactly once, so the report is
/// a complete answer to "how did each of my teammates end up?".
fn outcome_report(members: &[TeamMember], awaited: &[String]) -> String {
    let mut out = String::from("## Wait outcome\n\n");
    for member in members {
        if !awaited.contains(&member.agent_id) {
            continue;
        }
        out.push_str(&format!(
            "- {} ({}) - {}\n",
            member.name,
            member.agent_id,
            WaitOutcome::of(member.status).describe(member.status)
        ));
    }
    out.push('\n');
    out
}

/// Build the machine-readable outcome map for the tool metadata (FR-031).
///
/// Keyed by agent ID so a caller can look up one member without scanning the
/// rendered report: each entry carries the member's name, its final status, and
/// the outcome token (`idle`, `parked`, `failed`, `still_running`).
fn outcome_metadata(members: &[TeamMember], awaited: &[String]) -> Value {
    let mut per_member = BTreeMap::new();
    for member in members {
        if !awaited.contains(&member.agent_id) {
            continue;
        }
        per_member.insert(
            member.agent_id.clone(),
            json!({
                "name": member.name,
                "status": member.status.as_str(),
                "outcome": WaitOutcome::of(member.status).as_str(),
            }),
        );
    }
    Value::Object(per_member.into_iter().collect())
}

fn summarise_store(members: &[TeamMember]) -> String {
    let mut out = String::from("## Teammate status\n\n");
    for m in members {
        let status_icon = match m.status {
            MemberStatus::Idle => "[sleep]",
            MemberStatus::Working => "[cfg]",
            MemberStatus::Spawning => "[yellow]",
            MemberStatus::Blocked => "[locked]",
            MemberStatus::Suspended => "[||]",
            MemberStatus::Failed => "[x]",
            MemberStatus::PlanPending => "[notice]",
            MemberStatus::ShuttingDown => "[refresh]",
            MemberStatus::Stopped => "[stop]",
        };
        let status_str = m.status.as_str();
        out.push_str(&format!(
            "- {} **{}** ({}) - {}\n",
            status_icon, m.name, m.agent_id, status_str
        ));
    }
    out
}
