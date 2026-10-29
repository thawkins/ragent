//! `team_status` - Returns a formatted team status report.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{
    MemberStatus, STATUS_HISTORY_LIMIT, StatusHistoryEntry, TaskStatus, TaskStore, TeamStore,
    find_team_dir, pending_plans, recent_status_history, render_hook_failures,
    render_pending_plans, render_status_history, render_undelivered, status_word,
    undelivered_messages,
};

/// Returns human-readable team status.
pub struct TeamStatusTool;

#[async_trait::async_trait]
impl Tool for TeamStatusTool {
    fn name(&self) -> &'static str {
        "team_status"
    }

    fn description(&self) -> &'static str {
        "Get the current status of a team: member list, their states, and task \
         progress summary. REQUIRED parameter: 'team_name' (string). Use this \
         whenever you need to know which teammates are active, idle, stopped, or \
         failed, or how many tasks are pending/in-progress/completed. Each member \
         row is followed by a `Status history` block listing that member's last \
         five transitions with timestamps and causes (oldest first, from \
         `status-history.jsonl`), so \"why did this teammate stop?\" is answerable \
         from this report alone rather than by inference from the coarse events. Also reports \
         any message still unread in a terminated teammate's mailbox under \
         `Undelivered messages`, because no teammate will ever drain it, and \
         any plan parked in plan_pending under `Plans awaiting a verdict` with \
         the team_approve_plan call that resolves it. Also reports under \
         `Hook failures` any configured quality-gate hook that could not run \
         (refused command, spawn failure, or timeout): an inert hook never \
         approved anything, so a non-zero count here means a gate is not \
         working. Common \
         gotcha: this tool does not read YOUR mailbox - use team_read_messages for \
         that, and use this report to spot messages stranded for a teammate that \
         has already stopped."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team (required)"
                }
            },
            "required": ["team_name"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "team:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let team_name = input
            .get("team_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: team_name"))?;

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        let store = TeamStore::load(&team_dir)?;

        // Task store is supplementary - don't fail the whole report if
        // tasks.json is missing or corrupt.
        let task_list = TaskStore::open(&team_dir)
            .and_then(|ts| ts.read())
            .unwrap_or_default();

        let total_tasks = task_list.tasks.len();
        let done_tasks = task_list
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .count();
        let in_progress_tasks = task_list
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::InProgress)
            .count();
        let pending_tasks = task_list
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Pending)
            .count();

        let mut lines = vec![
            format!(
                "Team: {} [{}]",
                store.config.name,
                store.config.status.as_str()
            ),
            format!("Lead session: {}", store.config.lead_session_id),
            format!(
                "Tasks: {done_tasks}/{total_tasks} done | {in_progress_tasks} in progress | {pending_tasks} pending"
            ),
            String::new(),
            format!("Members ({}):", store.config.members.len()),
        ];

        for m in &store.config.members {
            let status_icon = match m.status {
                MemberStatus::Working => "[refresh]",
                MemberStatus::Idle => "[||]",
                MemberStatus::PlanPending => "[notice]",
                MemberStatus::Blocked => "[locked]",
                MemberStatus::Suspended => "[||]",
                MemberStatus::ShuttingDown => "[stop]",
                MemberStatus::Stopped => "#",
                MemberStatus::Spawning => "[rocket]",
                MemberStatus::Failed => "[x]",
            };
            let task_info = m
                .current_task_id
                .as_deref()
                .map(|id| format!(" (task: {id})"))
                .unwrap_or_default();
            // FR-032/FR-033: every row names the member, its id and its state,
            // with the status word taken from `display_label()` so this report
            // and the Teams panel spell a state the same way.
            lines.push(format!(
                "  {status_icon} {} [{}] {}{task_info}",
                m.name,
                m.agent_id,
                status_word(m.status)
            ));
        }

        // FR-036: per-member status history. A roster row says what a member is
        // now; this block says how it got there, which is the question a lead
        // actually asks ("why did this teammate stop?").
        let histories: Vec<Vec<StatusHistoryEntry>> = store
            .config
            .members
            .iter()
            .map(|m| recent_status_history(&team_dir, &m.agent_id, STATUS_HISTORY_LIMIT))
            .filter(|entries| !entries.is_empty())
            .collect();
        if !histories.is_empty() {
            lines.push(String::new());
            let mut block = format!(
                "# Status history\n\nLast {STATUS_HISTORY_LIMIT} transition(s) per member, \
                 oldest first.\n"
            );
            for entries in &histories {
                block.push('\n');
                block.push_str(render_status_history(entries, 2).trim_end());
                block.push('\n');
            }
            lines.push(block.trim_end().to_string());
        }

        // FR-020: a message whose recipient terminated before reading it is
        // reported here rather than dropped - nothing else will ever read it.
        let undelivered = undelivered_messages(&store);
        let undelivered_block = render_undelivered(&store.config.name, &undelivered, 1);
        if !undelivered_block.is_empty() {
            lines.push(String::new());
            lines.push(undelivered_block.trim_end().to_string());
        }

        // FR-034: a member parked in `plan_pending` is reported with the plan it
        // is waiting on and the action that resolves it, never as a bare status
        // word the lead has to decode.
        let pending = pending_plans(&store);
        let pending_block = render_pending_plans(&store.config.name, &pending, 1);
        if !pending_block.is_empty() {
            lines.push(String::new());
            lines.push(pending_block.trim_end().to_string());
        }

        // FR-019: a hook that could not be executed never produced a verdict,
        // so it must not read as a passing gate. The ledger is the record and
        // this count is the surface.
        let hook_failures = crate::team::read_hook_failures(&team_dir);
        let hook_block = render_hook_failures(&store.config.name, &hook_failures, 1);
        if !hook_block.is_empty() {
            lines.push(String::new());
            lines.push(hook_block.trim_end().to_string());
        }

        let members_json: Vec<Value> = store
            .config
            .members
            .iter()
            .map(|m| {
                // FR-036: each member row carries its own last five transitions,
                // oldest first, so a programmatic consumer gets the same answer
                // the prose block gives.
                let history: Vec<Value> =
                    recent_status_history(&team_dir, &m.agent_id, STATUS_HISTORY_LIMIT)
                        .iter()
                        .map(|entry| {
                            json!({
                                "timestamp": entry.timestamp.to_rfc3339(),
                                "from": entry.from,
                                "to": entry.to,
                                "cause": entry.cause,
                            })
                        })
                        .collect();
                json!({
                    "name": m.name,
                    "agent_id": m.agent_id,
                    "status": m.status.as_str(),
                    "current_task_id": m.current_task_id,
                    // FR-034: a `plan_pending` row states its plan state so a
                    // reader never has to infer it from the status word alone.
                    "plan_status": m.plan_status.as_str(),
                    "plan_request_id": m.plan_request_id,
                    "status_history": history
                })
            })
            .collect();

        let undelivered_json: Vec<Value> = undelivered
            .iter()
            .map(|u| {
                json!({
                    "agent_id": u.agent_id,
                    "recipient": u.recipient,
                    "recipient_status": u.recipient_status.as_str(),
                    "message_id": u.message_id,
                    "from": u.from,
                    "type": u.message_type,
                    "sent_at": u.sent_at.to_rfc3339(),
                    "content": u.content,
                })
            })
            .collect();

        let pending_json: Vec<Value> = pending
            .iter()
            .map(|p| {
                json!({
                    "agent_id": p.agent_id,
                    "name": p.name,
                    "plan": p.plan,
                    "submitted_at": p.submitted_at.map(|at| at.to_rfc3339()),
                    "request_id": p.request_id,
                    "approve": p.approve_action(team_name),
                    "reject": p.reject_action(team_name),
                })
            })
            .collect();

        let hook_failures_json: Vec<Value> = hook_failures
            .iter()
            .map(|f| {
                json!({
                    "timestamp": f.timestamp.to_rfc3339(),
                    "event": f.event.as_str(),
                    "hook": f.hook,
                    "reason": f.reason.as_str(),
                    "detail": f.detail,
                })
            })
            .collect();

        Ok(ToolOutput {
            content: lines.join("\n"),
            metadata: Some(json!({
                "team_name": team_name,
                "team_status": store.config.status.as_str(),
                "member_count": store.config.members.len(),
                "members": members_json,
                "undelivered_count": undelivered.len(),
                "undelivered": undelivered_json,
                "pending_plan_count": pending.len(),
                "pending_plans": pending_json,
                "hook_failure_count": hook_failures.len(),
                "hook_failures": hook_failures_json,
                "tasks": {
                    "total": total_tasks,
                    "done": done_tasks,
                    "in_progress": in_progress_tasks,
                    "pending": pending_tasks
                }
            })),
        })
    }
}
