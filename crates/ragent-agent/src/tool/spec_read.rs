//! Read a spec by ID.
//!
//! Returns the full SPEC.md content for a given spec identifier.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Read a spec by ID, returning its full markdown content and metadata.
pub struct SpecReadTool;

#[async_trait::async_trait]
impl Tool for SpecReadTool {
    fn name(&self) -> &'static str {
        "spec_read"
    }

    fn description(&self) -> &'static str {
        "Read a specification by ID. Returns the full SPEC.md content, requirements, tasks, and current status."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "spec_id": {
                    "type": "string",
                    "description": "The spec identifier (e.g. 'testspec', 'auth-refactor')"
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

        let id = ragent_specs::spec::SpecId::new(spec_id_str).ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid spec ID '{}'. Use only alphanumeric, hyphen, underscore.",
                spec_id_str
            )
        })?;

        let spec = spec_manager
            .read_spec(&id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read spec '{}': {}", spec_id_str, e))?;

        let requirements = spec
            .requirements
            .iter()
            .map(|r| {
                format!(
                    "- `{}` ({:?}) — {} {}",
                    r.id,
                    r.template,
                    r.text,
                    if r.implemented { "(implemented)" } else { "" }
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let tasks = spec
            .tasks
            .iter()
            .map(|t| {
                let reqs = if t.linked_requirements.is_empty() {
                    String::new()
                } else {
                    format!(" [links: {}]", t.linked_requirements.join(", "))
                };
                format!(
                    "- `{}` — {} ({}){}{}",
                    t.id,
                    t.title,
                    t.status.as_str(),
                    reqs,
                    if t.completed_at.is_some() { " ✓" } else { "" }
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let content = format!(
            "## Spec: {} ({})

**Status:** {}  
**Title:** {}

---

### SPEC.md

{}

---

### Requirements

{}

---

### Plan Tasks

{}

---

### Coverage: {:.0}%
",
            spec.id,
            spec.status.as_str(),
            spec.status.as_str(),
            spec.title,
            spec.spec_md,
            if requirements.is_empty() {
                "(none)"
            } else {
                &requirements
            },
            if tasks.is_empty() { "(none)" } else { &tasks },
            spec.coverage_pct()
        );

        let metadata = json!({
            "spec_id": spec_id_str,
            "status": spec.status.as_str(),
            "title": spec.title,
            "requirement_count": spec.requirements.len(),
            "task_count": spec.tasks.len(),
            "coverage_pct": spec.coverage_pct(),
        });

        Ok(ToolOutput {
            content,
            metadata: Some(metadata),
        })
    }
}
