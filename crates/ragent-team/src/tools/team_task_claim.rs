//! `team_task_claim` — Atomically claim the next available task.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::event::Event;
use crate::team::{TaskStore, find_team_dir};

/// Atomically claims the next pending task with no unresolved dependencies.
pub struct TeamTaskClaimTool;

#[async_trait::async_trait]
impl Tool for TeamTaskClaimTool {
    fn name(&self) -> &'static str {
        "team_task_claim"
    }

    fn description(&self) -> &'static str {
        "Claim a task to work on. Either claim the next available task (if no task_id provided), \
         or claim a specific task by ID (if the lead assigned you to one). \
         Uses file locking to prevent race conditions between teammates. \
         Returns the task details, or a message if the task is unavailable."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team"
                },
                "task_id": {
                    "type": "string",
                    "description": "Optional: specific task ID to claim. If provided, claims this task. \
                                   If not provided, claims the next available task."
                }
            },
            "required": ["team_name"]
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
            // Claim a specific task by ID
            match store.claim_specific(task_id, &agent_id) {
                Ok(task) => {
                    // M5-T7: publish TeamTaskClaimed.
                    // PERF-017: prefer the in-memory `TeamManager` (when
                    // available on the `ToolContext`) for the lead session
                    // id instead of loading `TeamStore` from disk on every
                    // claim. Falls back to the disk read only when no
                    // manager is wired in (e.g. in tests).
                    let lead_sid = ctx
                        .team_manager
                        .as_ref()
                        .and_then(|tm| tm.lead_session_id().map(str::to_string))
                        .or_else(|| {
                            crate::team::TeamStore::load(&team_dir)
                                .ok()
                                .map(|s| s.config.lead_session_id.clone())
                        })
                        .unwrap_or_else(|| ctx.session_id.clone());
                    ctx.event_bus.publish(Event::TeamTaskClaimed {
                        session_id: lead_sid,
                        team_name: team_name.to_string(),
                        agent_id: agent_id.clone(),
                        task_id: task.id.clone(),
                    });
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
            let (claimed, already_had) = store.claim_next(&agent_id)?;

            match claimed {
                None => {
                    // No unclaimed, unblocked tasks available
                    Ok(ToolOutput {
                                      content: "No more tasks available. All tasks are either in progress, \
                                                completed, or blocked by dependencies.\n\n\
                                                Call `team_idle` to signal you are done and ready for reassignment."
                                          .to_string(),
                                      metadata: Some(json!({
                                          "team_name": team_name,
                                          "claimed": false,
                                          "ready_for_idle": true
                                      })),
                                  })
                }
                Some(task) if already_had => {
                    // Agent already has a task in progress
                    Ok(ToolOutput {
                        content: format!(
                            "⚠ You already have task '{}' in progress.\n\
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
                    // M5-T7: publish TeamTaskClaimed so the TUI/SSE observe
                    // the claim (the event variant already existed but was
                    // never published).
                    // PERF-017: prefer the in-memory `TeamManager` for the
                    // lead session id to avoid a per-claim `TeamStore::load`.
                    let lead_sid = ctx
                        .team_manager
                        .as_ref()
                        .and_then(|tm| tm.lead_session_id().map(str::to_string))
                        .or_else(|| {
                            crate::team::TeamStore::load(&team_dir)
                                .ok()
                                .map(|s| s.config.lead_session_id.clone())
                        })
                        .unwrap_or_else(|| ctx.session_id.clone());
                    ctx.event_bus.publish(Event::TeamTaskClaimed {
                        session_id: lead_sid,
                        team_name: team_name.to_string(),
                        agent_id: agent_id.clone(),
                        task_id: task.id.clone(),
                    });
                    // M8-T3: update the claiming member's current_task_id.
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
