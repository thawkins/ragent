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
        "Generate a requirement coverage report for a spec. REQUIRED parameter: \
         'spec_id' (string, the spec identifier, e.g. 'auth-refactor'). Shows which \
         requirements are linked to completed tasks. Use this to verify that a \
         specification's tasks cover its requirements before marking it complete. \
         Common gotcha: the spec must exist in the configured specs directory."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "spec_id": {
                    "type": "string",
                    "description": "The spec identifier (required)"
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

        let metadata = json!({
            "spec_id": spec_id_str,
            "coverage_pct": spec.coverage_pct(),
            "requirement_count": spec.requirements.len(),
            "task_count": spec.tasks.len(),
        });

        Ok(ToolOutput {
            // Rendering lives in `Spec::coverage_report` (ragent-specs) so
            // this tool and the TUI /spec coverage arm always agree on
            // format and status symbols.
            content: spec.coverage_report(),
            metadata: Some(metadata),
        })
    }
}
