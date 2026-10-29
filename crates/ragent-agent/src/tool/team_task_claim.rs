//! `team_task_claim` - Atomically claim the next available task.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{TaskStatus, TaskStore, find_team_dir};

/// Atomically claims the next pending task with no unresolved dependencies.
pub struct TeamTaskClaimTool;

#[async_trait::async_trait]
impl Tool for TeamTaskClaimTool {
    fn name(&self) -> &'static str {
        "team_task_claim"
    }

    fn description(&self) -> &'static str {
        "Claim a task to work on. REQUIRED parameter: 'team_name' (string). Optional: \
             'task_id' (string) to claim a specific task; if omitted, the next available \
             pending task is claimed atomically using file locking. Returns the task details \
             or a message if the task is unavailable. Only a PENDING task whose dependencies are \
             all completed can be claimed: a task that is in progress, completed or cancelled is \
             refused with a message naming the status it actually holds (a cancelled task reports \
             'cancelled' and is terminal - it can never be claimed again). Common gotcha: if no \
             task_id is provided and no pending tasks remain, the result indicates no work is \
             available - call team_idle when appropriate."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team (required)"
                },
                "task_id": {
                    "type": "string",
                    "description": "Optional: specific task ID to claim. If provided, claims this task; if not provided, claims the next available task."
                }
            },
            "required": ["team_name"],
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

        let agent_id = ctx
            .team_context
            .as_ref()
            .map_or_else(|| ctx.session_id.clone(), |tc| tc.agent_id.clone());

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        let store = TaskStore::open(&team_dir)?;

        // PERF-017: gate the debug read behind `tracing::enabled!(DEBUG)`
        // so the per-claim `store.read()` (a full file read + deserialise)
        // only happens when debug logging is actually enabled, instead of
        // on every `team_task_claim` call.
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
                    tasks = ?task_summary,
                    "team_task_claim: available tasks"
                );
            }
        }
        // Check if a specific task_id was requested
        let specific_task_id = input.get("task_id").and_then(|v| v.as_str());

        if let Some(task_id) = specific_task_id {
            // Claim a specific task by ID. The claim hook and the
            // `TeamTaskClaimed` event fire from the claim itself (FR-017), so
            // nothing here has to publish or run a hook.
            match store.claim_specific(task_id, &agent_id).await {
                Ok(task) => {
                    // M8-T3: update the claiming member's current_task_id
                    // so team_status and the TUI can show what they're working on.
                    // PERF-017: combine this config write with the claim
                    // write by doing a single `TeamStore::load` + `save`
                    // cycle instead of a separate load per field.
                    if let Ok(mut store) = crate::team::TeamStore::load(&team_dir) {
                        if let Some(m) = store.config.member_by_id_mut(&agent_id) {
                            m.current_task_id = Some(task.id.clone());
                        }
                        let _ = store.save();
                    }
                    Ok(ToolOutput {
                        content: format!(
                            "Claimed task '{}'.\nTitle: {}\nDescription: {}\nDependencies: {}",
                            task.id,
                            task.title,
                            task.description,
                            if task.depends_on.is_empty() {
                                "none".to_string()
                            } else {
                                task.depends_on.join(", ")
                            }
                        ),
                        metadata: Some(json!({
                            "team_name": team_name,
                            "claimed": true,
                            "task_id": task.id,
                            "title": task.title,
                            "description": task.description,
                            "agent_id": agent_id
                        })),
                    })
                }
                Err(e) => {
                    let err_msg = e.to_string();

                    // Check if this is a dependency issue (common for pre-assigned tasks)
                    let is_dependency_issue = err_msg.contains("unsatisfied dependencies");
                    let guidance = if is_dependency_issue {
                        "\n\n**Tip:** This task has dependencies that aren't complete yet. \
                         Wait for the prerequisite tasks to complete, then retry this claim."
                    } else {
                        ""
                    };

                    Ok(ToolOutput {
                        content: format!(
                            "Failed to claim task '{task_id}': {err_msg}{guidance}\n\
                             If this task doesn't exist or is unavailable, check the task ID and try again."
                        ),
                        metadata: Some(json!({
                            "team_name": team_name,
                            "claimed": false,
                            "task_id": task_id,
                            "error": err_msg,
                            "blocked_by_dependencies": is_dependency_issue
                        })),
                    })
                }
            }
        } else {
            // Claim the next available task
            let (claimed, already_had) = store.claim_next(&agent_id).await?;

            match claimed {
                None => {
                    // No unclaimed, unblocked tasks available. A cancelled task
                    // is reported as `cancelled`, never as pending (FR-006): the
                    // count reads the same status the claim predicate refuses,
                    // so this summary cannot describe a cancelled task as
                    // waiting work.
                    let cancelled = store
                        .read()
                        .map(|list| {
                            list.tasks
                                .iter()
                                .filter(|t| t.status == TaskStatus::Cancelled)
                                .count()
                        })
                        .unwrap_or(0);
                    let cancelled_note = if cancelled == 0 {
                        String::new()
                    } else {
                        format!(
                            "\n{cancelled} cancelled task(s) are terminal and cannot be claimed."
                        )
                    };
                    Ok(ToolOutput {
                        content: format!(
                            "No more tasks available. All tasks are either in progress, \
                                                completed, cancelled, or blocked by dependencies.{cancelled_note}\n\n\
                                                Call `team_idle` to signal you are done and ready for reassignment."
                        ),
                        metadata: Some(json!({
                            "team_name": team_name,
                            "claimed": false,
                            "ready_for_idle": true,
                            "cancelled_tasks": cancelled
                        })),
                    })
                }
                Some(task) if already_had => {
                    // Agent already has a task in progress
                    Ok(ToolOutput {
                        content: format!(
                            "[!] You already have task '{}' in progress.\n\
                                           Title: {}\nDescription: {}\n\
                                           You must call `team_task_complete` for this task before claiming another.",
                            task.id, task.title, task.description
                        ),
                        metadata: Some(json!({
                            "team_name": team_name,
                            "claimed": false,
                            "already_in_progress": true,
                            "task_id": task.id,
                            "title": task.title,
                            "agent_id": agent_id
                        })),
                    })
                }
                Some(task) => {
                    // M8-T3: update the claiming member's current_task_id.
                    // The `TeamTaskClaimed` event and the `TaskClaimed` hook
                    // fire from the claim itself (FR-017).
                    if let Ok(mut store) = crate::team::TeamStore::load(&team_dir) {
                        if let Some(m) = store.config.member_by_id_mut(&agent_id) {
                            m.current_task_id = Some(task.id.clone());
                        }
                        let _ = store.save();
                    } // Successfully claimed a new task
                    Ok(ToolOutput {
                        content: format!(
                            "Claimed task '{}'.\nTitle: {}\nDescription: {}\nDependencies: {}",
                            task.id,
                            task.title,
                            task.description,
                            if task.depends_on.is_empty() {
                                "none".to_string()
                            } else {
                                task.depends_on.join(", ")
                            }
                        ),
                        metadata: Some(json!({
                            "team_name": team_name,
                            "claimed": true,
                            "task_id": task.id,
                            "title": task.title,
                            "description": task.description,
                            "agent_id": agent_id
                        })),
                    })
                }
            }
        }
    }
}
