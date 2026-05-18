//! Update a task status within a spec.
//!
//! Transitions a task from pending → in_progress → completed (or blocked).

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
        "Update the status of a task within a spec. Statuses: pending, in_progress, completed, blocked."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "spec_id": {
                    "type": "string",
                    "description": "The spec identifier"
                },
                "task_id": {
                    "type": "string",
                    "description": "The task identifier within the plan (e.g. 'T-001')"
                },
                "status": {
                    "type": "string",
                    "description": "New status: pending, in_progress, completed, blocked",
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

        let new_status = ragent_specs::spec::TaskStatus::parse(status_str)
            .ok_or_else(|| anyhow::anyhow!("Unknown task status '{}'. Use: pending, in_progress, completed, blocked", status_str))?;

        let spec_manager = ctx
            .spec_manager
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Spec manager is not configured. Set up a specs/ directory first."))?;

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

        let content = format!(
            "Updated task `{}` in spec `{}` to status `{}`.",
            task_id, spec_id_str, status_str
        );

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
