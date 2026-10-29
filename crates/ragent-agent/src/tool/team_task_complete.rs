//! `team_task_complete` - Mark a task as completed and unblock its dependents.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput, complete_task_through_manager};
use crate::team::find_team_dir;

/// Marks a task as completed by the calling agent.
pub struct TeamTaskCompleteTool;

#[async_trait::async_trait]
impl Tool for TeamTaskCompleteTool {
    fn name(&self) -> &'static str {
        "team_task_complete"
    }

    fn description(&self) -> &'static str {
        "Mark a TEAM task as completed (used inside a team session). REQUIRED \
             parameters: 'team_name' (string) and 'task_id' (string, the ID of the task \
             you claimed via team_task_claim, e.g. 'task-001'). Completing it unblocks any \
             dependent tasks. \
             \\\n\\\n\
             OWNERSHIP RULE (enforced): only the task's assigned agent or the lead may \
             complete it. A task assigned to another teammate is refused and is NOT \
             re-assigned to you; the refusal names the current owner and the two ways \
             forward - the lead assigns it with `team_assign_task`, or you claim it with \
             `team_task_claim`. A task that is still 'pending' is refused as unclaimed (also \
             not re-assigned): claim or assign it first. Completing a task that is already \
             completed is a no-op success for its owner or the lead, and refused for anyone \
             else. Every refusal leaves the task list untouched and reports \
             `rejection_reason` ('not_owner', 'not_claimed', 'already_completed') together \
             with the task's `owner` in the result metadata. \
             \\\n\\\n\
             IDLE RULE (FR-014): completing a task parks you as idle, through the same 
             status writer `team_idle` uses, so you do not have to call `team_idle` \
             afterwards. With `auto_claim_tasks` on (the default) a member whose loop is 
             still running is handed the next claimable task instead and goes straight 
             back to `working`; with it off you stop `idle` and wait for the lead. A lead \
             completing on a teammate's behalf leaves the lead's own status alone. \
             \\\n\\\n\
             HOOK VERDICT (FR-018): when the team configures a `TaskCompleted` hook, its \
             verdict is returned to you in this call's result. A veto arrives as \
             `hook_rejected: true` plus the hook's `feedback` text: read it, address it with \
             the lead, and do not re-complete the task - a completed task is terminal, so \
             the completion stands. \
             \\\n\\\n\
             WARNING: DO NOT confuse with `agent_complete` (a different tool used OUTSIDE teams to \
             signal the end of the autonomous loop, which takes `summary` as its only parameter). \
             This tool takes `team_name` + `task_id` - NOT `summary`. \
             \\\n\\\n\
             Example: team_task_complete(team_name: \"audit-team\", task_id: \"task-001\")"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "REQUIRED. Name of the team. If you are NOT inside a team session, this tool will fail - use `agent_complete(summary: ...)` instead to end the autonomous loop."
                },
                "task_id": {
                    "type": "string",
                    "description": "REQUIRED. ID of the task to mark as completed (e.g. 'task-001'). This must be a task ID you claimed via `team_task_claim`."
                }
            },
            "required": ["team_name", "task_id"],
            "additionalProperties": false
        })
    }
    fn permission_category(&self) -> &'static str {
        "team:tasks"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let team_name = input
            .get("team_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: team_name"))?;

        let task_id = input
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: task_id"))?;

        let agent_id = ctx
            .team_context
            .as_ref()
            .map_or_else(|| ctx.session_id.clone(), |tc| tc.agent_id.clone());

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        let store = crate::team::TaskStore::open(&team_dir)?;

        // PERF-018: gate the debug read behind `tracing::enabled!(DEBUG)` so
        // the per-completion `store.read()` (a full file read + deserialise)
        // only happens when debug logging is actually enabled, instead of
        // on every `team_task_complete` call.
        if tracing::enabled!(tracing::Level::DEBUG) {
            if let Ok(list) = store.read() {
                let task_summary: Vec<String> = list
                    .tasks
                    .iter()
                    .map(|t| {
                        format!(
                            "{} ({})",
                            t.id,
                            match t.status {
                                crate::team::TaskStatus::Pending => "pending",
                                crate::team::TaskStatus::InProgress => "in_progress",
                                crate::team::TaskStatus::Completed => "completed",
                                crate::team::TaskStatus::Cancelled => "cancelled",
                            }
                        )
                    })
                    .collect();
                tracing::debug!(
                    agent_id = %agent_id,
                    team_name = %team_name,
                    task_id = %task_id,
                    tasks = ?task_summary,
                    "team_task_complete: attempting to complete"
                );
            }
        }
        // FR-018 / FR-026: complete through the one runtime completion path, so
        // a lead-side completion and this tool run the same FR-001 rule, the
        // same D3 transition, the same `TeamTaskCompleted` event and the same
        // `TaskCompleted` hook - and the hook's verdict reaches the completing
        // agent here as a tool result, whichever path produced it.
        let (task_id_out, title, completed_at, feedback) =
            match complete_task_through_manager(ctx, &team_dir, task_id, &agent_id).await {
                Ok(done) => done,
                Err(e) => {
                    // Return a tool output explaining why completion failed, rather than an error.
                    // This gives the teammate a clear error in the TUI instead of generic failure.
                    let err_msg = e.to_string();
                    tracing::warn!(
                        agent_id = %agent_id,
                        task_id = %task_id,
                        team_name = %team_name,
                        error = %err_msg,
                        "team_task_complete failed"
                    );
                    // FR-044: name the rule that refused the completion in the
                    // tool's JSON metadata, so a caller that reads the structured
                    // result (rather than the prose) can tell an ownership refusal
                    // (`not_owner`) from an unclaimed task (`not_claimed`). The
                    // `TeamTaskCompletionRejected` event carries the same token.
                    let (reason, owner) = match e.downcast_ref::<crate::team::CompletionRefusal>() {
                        Some(refusal) => {
                            (Some(refusal.reason.as_str()), Some(refusal.owner.clone()))
                        }
                        None => (None, None),
                    };
                    return Ok(ToolOutput {
                        content: format!(
                            "Failed to mark task '{task_id}' as completed: {err_msg}\n\
                         This usually means the task doesn't exist, is already completed, \
                         or is assigned to a different agent."
                        ),
                        metadata: Some(json!({
                            "team_name": team_name,
                            "task_id": task_id,
                            "completed": false,
                            "agent_id": agent_id,
                            "rejection_reason": reason,
                            "owner": owner,
                            "error": err_msg
                        })),
                    });
                }
            };

        // FR-017: the `TaskCompleted` hook fires from the completion itself, so
        // the verdict arrives here rather than being asked for here. That is
        // what makes a lead-side completion run the same gate as this tool.
        //
        // FR-018: the verdict is delivered - a veto is reported to the agent
        // that completed the task, in the tool result it reads. The completion
        // is *not* undone:
        //
        //   - the `TaskCompleted` hook is the runtime's observer of a commit,
        //     not the gatekeeper of it (the pre-commit gate is the
        //     `TaskClaimed` hook, which runs before the transition is written);
        //   - silently reverting a committed completion is a second writer of
        //     the task status, which FR-008 and FR-026 forbid, and it would
        //     leave `TeamTaskCompleted` published for a task that reads
        //     `in_progress`;
        //   - `Completed` is a terminal status (D3), so the revert itself would
        //     be an illegal transition - a hook that vetoed a completion could
        //     not be satisfied by "completing again".
        //
        // What the verdict must do is reach the agent, which the earlier release
        // did not do on the runtime path: a lead-side completion ran the hook and
        // threw the answer away.
        if let Some(feedback) = feedback {
            tracing::warn!(
                team = %team_name,
                task_id = %task_id,
                agent_id = %agent_id,
                "TaskCompleted hook vetoed the completion; delivering its feedback"
            );
            return Ok(ToolOutput {
                content: format!(
                    "Task '{task_id}' was marked completed by '{agent_id}', but the \
                     TaskCompleted hook did not accept the work.\n\
                     Hook feedback:\n{feedback}\n\
                     The completion stands (Completed is terminal). Address the feedback \
                     with the lead - for example, the lead can raise a follow-up task with \
                     `team_task_create`, or cancel this one and re-issue it."
                ),
                metadata: Some(json!({
                    "team_name": team_name,
                    "task_id": task_id,
                    "completed": true,
                    "completed_by": agent_id,
                    "completed_at": completed_at,
                    "hook_rejected": true,
                    "feedback": feedback
                })),
            });
        }

        // M8-T3 / FR-014: completing a task parks the completing member as
        // idle. Without this a teammate that completed its only task stayed
        // `Working` with `current_task_id = None` for ever - the swarm roster
        // showed a done task beside a member that never reported idle, and no
        // route (`team_idle` refuses only while a task is held, the agent loop's
        // own idle move had already happened) ever moved it back.
        //
        // The move goes through the single status writer, exactly as
        // `team_idle` does, and not by rewriting `config.json` here. That keeps
        // one transition, one history row and one `TeammateStatusChanged` event
        // per move, and it applies FR-015's `auto_claim_tasks` rule: a member
        // with a claimable task left and a live agent loop is moved on to
        // `Working` with that task instead of parking. `team_task_claim` is then
        // how a teammate picks up more work; an `Idle` member claims for itself
        // or waits for the lead to assign.
        //
        // A lead completing on a teammate's behalf leaves the *lead* alone: the
        // lead is not a roster member and its status is not the swarm's
        // problem. A member whose loop has already returned also stays put -
        // `auto_claim_on_idle` refuses to hand work to a loop that has ended,
        // and any status the member already holds here is a legal destination
        // from `Working` (or an unknown id, reported by the writer as "no
        // teammate with id", which is logged rather than failing a completion
        // that has already been committed).
        if agent_id != "lead" {
            if let Err(e) = super::set_member_status_through_manager(
                ctx,
                &team_dir,
                &agent_id,
                "idle",
                "task_completed",
            )
            .await
            {
                tracing::warn!(
                    team = %team_name,
                    agent = %agent_id,
                    error = %e,
                    "completed task but could not park the member as idle"
                );
            }
        }
        Ok(ToolOutput {
            content: format!(
                "Task '{}' marked as completed by '{}'.\nTitle: {}",
                task_id_out,
                agent_id,
                title.clone().unwrap_or_default()
            ),
            metadata: Some(json!({
                "team_name": team_name,
                "task_id": task_id_out,
                "title": title,
                "completed_by": agent_id,
                "completed_at": completed_at,
                "completed": true
            })),
        })
    }
}
