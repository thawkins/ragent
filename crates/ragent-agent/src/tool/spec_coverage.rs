//! Coverage report for a specification.
//!
//! Shows requirement coverage: which requirements are linked to completed tasks.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Generate a coverage report for a spec.
pub struct SpecCoverageTool;

#[async_trait::async_trait]
impl Tool for SpecCoverageTool {
    fn name(&self) -> &'static str {
        "spec_coverage"
    }

    fn description(&self) -> &'static str {
        "Generate a requirement coverage report for a spec. Shows which requirements are linked to completed tasks."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "spec_id": {
                    "type": "string",
                    "description": "The spec identifier"
                }
            },
            "required": ["spec_id"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "spec:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let spec_id_str = input["spec_id"]
            .as_str()
            .context("Missing required 'spec_id' parameter")?;

        let spec_manager = ctx.spec_manager.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Spec manager is not configured. Set up a specs/ directory first.")
        })?;

        let id = ragent_specs::spec::SpecId::new(spec_id_str)
            .ok_or_else(|| anyhow::anyhow!("Invalid spec ID '{}'", spec_id_str))?;

        let spec = spec_manager
            .read_spec(&id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read spec '{}': {}", spec_id_str, e))?;

        let mut lines = vec![
            format!("## Coverage Report: {}", spec.id),
            String::new(),
            format!("**Overall Coverage:** {:.1}%", spec.coverage_pct()),
            String::new(),
        ];

        // Build a map of requirement ID → linked completed tasks
        let mut req_to_completed: std::collections::HashMap<&str, Vec<&str>> =
            std::collections::HashMap::new();
        let mut req_to_total: std::collections::HashMap<&str, Vec<&str>> =
            std::collections::HashMap::new();

        for task in &spec.tasks {
            for req_id in &task.linked_requirements {
                req_to_total
                    .entry(req_id.as_str())
                    .or_default()
                    .push(task.id.as_str());
                if task.status == ragent_specs::spec::TaskStatus::Completed {
                    req_to_completed
                        .entry(req_id.as_str())
                        .or_default()
                        .push(task.id.as_str());
                }
            }
        }

        lines.push("### Requirements".to_string());
        for req in &spec.requirements {
            let completed = req_to_completed.get(req.id.as_str()).map_or(0, |v| v.len());
            let total = req_to_total.get(req.id.as_str()).map_or(0, |v| v.len());
            let covered = completed > 0 && completed == total;
            let symbol = if covered { "✅" } else { "⚪" };
            let detail = if total > 0 {
                format!(" ({} of {} linked tasks completed)", completed, total)
            } else {
                " (no linked tasks)".to_string()
            };
            lines.push(format!("{} `{}` — {}{}", symbol, req.id, req.text, detail));
        }

        lines.push(String::new());
        lines.push("### Tasks".to_string());
        for task in &spec.tasks {
            let reqs = if task.linked_requirements.is_empty() {
                "(unlinked)".to_string()
            } else {
                format!("[{}]", task.linked_requirements.join(", "))
            };
            let symbol = match task.status {
                ragent_specs::spec::TaskStatus::Completed => "✅",
                ragent_specs::spec::TaskStatus::InProgress => "🔄",
                ragent_specs::spec::TaskStatus::Blocked => "🚫",
                ragent_specs::spec::TaskStatus::Pending => "⏳",
            };
            lines.push(format!(
                "{} `{}` — {} ({}) {}",
                symbol,
                task.id,
                task.title,
                task.status.as_str(),
                reqs
            ));
        }

        let metadata = json!({
            "spec_id": spec_id_str,
            "coverage_pct": spec.coverage_pct(),
            "requirement_count": spec.requirements.len(),
            "task_count": spec.tasks.len(),
        });

        Ok(ToolOutput {
            content: lines.join("\n"),
            metadata: Some(metadata),
        })
    }
}
