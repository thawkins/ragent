//! Update a task status within a spec.
//!
//! Transitions a task from pending → in_progress → completed (or blocked).
//!
//! The call is also mirrored into the session task tracker: if a session
//! task tagged with the same `spec_id`/`task_id` exists it is updated,
//! otherwise one is created. This keeps the tracker in sync without any
//! pre-created tasks from the `/spec impl` driver.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Update a task's status in a spec's PLAN.md.
pub struct SpecTaskUpdateTool;

#[async_trait::async_trait]
impl Tool for SpecTaskUpdateTool {
    fn name(&self) -> &'static str {
        "spec_task_update"
    }

    fn description(&self) -> &'static str {
        "Update the status of a task within a spec. REQUIRED parameters: 'spec_id' \
         (string, e.g. 'auth-refactor'), 'task_id' (string within the plan, e.g. 'T-001'), \
         and 'status' (string enum: pending, in_progress, completed, blocked). This writes \
         the updated spec back to disk. Use it to track progress against a specification. \
         The call is also mirrored into the session task tracker: a session task tagged \
         with the same spec_id/task_id is updated if it exists, or created if it does \
         not. Common gotcha: the spec must exist in the configured specs directory, and \
         the task ID must match one defined in that spec."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "spec_id": {
                    "type": "string",
                    "description": "The spec identifier (required)"
                },
                "task_id": {
                    "type": "string",
                    "description": "The task identifier within the plan, e.g. 'T-001' (required)"
                },
                "status": {
                    "type": "string",
                    "description": "New status: pending, in_progress, completed, blocked (required)",
                    "enum": ["pending", "in_progress", "completed", "blocked"]
                }
            },
            "required": ["spec_id", "task_id", "status"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "spec:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let spec_id_str = input["spec_id"]
            .as_str()
            .context("Missing required 'spec_id' parameter")?;
        let task_id = input["task_id"]
            .as_str()
            .context("Missing required 'task_id' parameter")?;
        let status_str = input["status"]
            .as_str()
            .context("Missing required 'status' parameter")?;

        let new_status = ragent_specs::spec::TaskStatus::parse(status_str).ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown task status '{}'. Use: pending, in_progress, completed, blocked",
                status_str
            )
        })?;

        let spec_manager = ctx.spec_manager.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Spec manager is not configured. Set up a specs/ directory first.")
        })?;

        let id = ragent_specs::spec::SpecId::new(spec_id_str)
            .ok_or_else(|| anyhow::anyhow!("Invalid spec ID '{}'", spec_id_str))?;

        let mut spec = spec_manager
            .read_spec(&id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read spec '{}': {}", spec_id_str, e))?;

        spec_manager
            .update_task_status(&mut spec, task_id, new_status)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to update task status: {}", e))?;

        // Mirror the status into the session task tracker: update the
        // matching task if it exists, create one if it does not. A mirror
        // failure must not fail the tool call — the spec write already
        // succeeded — so it is logged and noted in the output instead.
        let mirror_note = mirror_session_task(ctx, &spec, task_id, new_status);

        let content = match mirror_note {
            Some(action) => format!(
                "Updated task `{}` in spec `{}` to status `{}`. Session task tracker \
                 synced ({}).",
                task_id, spec_id_str, status_str, action
            ),
            None => format!(
                "Updated task `{}` in spec `{}` to status `{}`. [warn] session task \
                 tracker unavailable or mirror failed.",
                task_id, spec_id_str, status_str
            ),
        };

        let metadata = json!({
            "spec_id": spec_id_str,
            "task_id": task_id,
            "status": status_str,
        });

        Ok(ToolOutput {
            content,
            metadata: Some(metadata),
        })
    }
}

/// Maps a spec `TaskStatus` to the session-tracker status strings
/// (`pending`, `in_progress`, `completed`). The tracker has no `blocked`
/// status — it is derived from `blocked_by` — so a blocked spec task maps
/// to `pending` (not done, not being worked on).
fn tracker_status(status: ragent_specs::spec::TaskStatus) -> &'static str {
    match status {
        ragent_specs::spec::TaskStatus::Pending => "pending",
        ragent_specs::spec::TaskStatus::InProgress => "in_progress",
        ragent_specs::spec::TaskStatus::Completed => "completed",
        ragent_specs::spec::TaskStatus::Blocked => "pending",
    }
}

/// Mirrors a spec task status change into the session task tracker.
///
/// Looks for a tracker task in the current session whose metadata carries
/// the same `spec_id` and `spec_task_id`. If one exists its status is
/// updated; otherwise a new tracker task is created seeded from the
/// spec's PLAN.md task row.
///
/// On success returns `Some(action)` describing the action taken
/// (`updated` or `created`); `None` means the tracker is unavailable or
/// the mirror failed (already logged) — the caller surfaces a warning
/// note in its output. Storage being unavailable is not an error:
/// headless/CLI runs without a database still succeed.
fn mirror_session_task(
    ctx: &ToolContext,
    spec: &ragent_specs::spec::Spec,
    task_id: &str,
    new_status: ragent_specs::spec::TaskStatus,
) -> Option<std::string::String> {
    use uuid::Uuid;

    let storage = ctx.storage.as_ref()?;

    let wanted = tracker_status(new_status);

    // Locate an existing tracker task for this spec task (metadata match).
    let rows = match storage.list_tasks(&ctx.session_id, None) {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!(error = %e, "spec task mirror: failed to list session tasks");
            return None;
        }
    };
    let existing = rows.iter().find(|row| {
        serde_json::from_str::<Value>(&row.metadata).is_ok_and(|meta| {
            meta["spec_id"].as_str() == Some(spec.id.as_str())
                && meta["spec_task_id"].as_str() == Some(task_id)
        })
    });

    if let Some(row) = existing {
        let params = ragent_storage::TaskUpdateParams {
            subject: None,
            status: Some(wanted),
            description: None,
            active_form: None,
            owner: None,
            metadata: None,
            blocked_by: None,
        };
        if let Err(e) = storage.update_task(&row.id, &ctx.session_id, &params) {
            tracing::warn!(
                error = %e,
                task_id,
                "spec task mirror: failed to update session task"
            );
            return None;
        }
        return Some("updated".to_string());
    }

    // No tracker task yet — create one seeded from the spec's task row.
    let plan_task = spec.tasks.iter().find(|t| t.id == task_id);
    let subject = plan_task
        .map(|t| t.title.clone())
        .unwrap_or_else(|| format!("Spec task {task_id}"));
    let description = plan_task
        .map(|t| t.description.clone())
        .unwrap_or_else(|| format!("Task {task_id} for spec {}", spec.id));
    let active_form = format!("Implementing task {task_id}");
    let metadata = json!({
        "spec_id": spec.id.as_str(),
        "spec_task_id": task_id,
        "kind": "spec-impl",
    })
    .to_string();
    let new_id = format!("task-{}", Uuid::new_v4().simple());
    if let Err(e) = storage.create_task(
        &new_id,
        &ctx.session_id,
        &subject,
        &description,
        wanted,
        Some(&active_form),
        None,
        &metadata,
        &[],
    ) {
        tracing::warn!(
            error = %e,
            task_id,
            "spec task mirror: failed to create session task"
        );
        return None;
    }
    Some("created".to_string())
}
