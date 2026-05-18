//! List and filter specifications.
//!
//! Returns a table of all (or filtered) specs with status, title, and coverage.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// List specs with optional status filtering.
pub struct SpecListTool;

#[async_trait::async_trait]
impl Tool for SpecListTool {
    fn name(&self) -> &'static str {
        "spec_list"
    }

    fn description(&self) -> &'static str {
        "List all specifications. Optionally filter by status (draft, in_review, approved, in_progress, implemented, verified, archived)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "description": "Filter by spec status (optional). One of: draft, in_review, approved, in_progress, implemented, verified, archived"
                }
            },
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "spec:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let spec_manager = ctx
            .spec_manager
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Spec manager is not configured. Set up a specs/ directory first."))?;

        let mut filter = ragent_specs::manager::SpecFilter::new();

        if let Some(status_str) = input["status"].as_str() {
            let status = ragent_specs::spec::SpecStatus::parse(status_str)
                .ok_or_else(|| anyhow::anyhow!("Unknown status '{}'. Valid: draft, in_review, approved, in_progress, implemented, verified, archived", status_str))?;
            filter = filter.with_status(status);
        }

        let specs = spec_manager
            .list_specs(&filter)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list specs: {}", e))?;

        let mut lines = vec![format!("Found {} spec(s):", specs.len()), String::new()];
        lines.push(format!("| {:<20} | {:<14} | {:<30} | {:>6} |", "ID", "Status", "Title", "Cov%"));
        lines.push(format!("|{:-<22}|{:-<16}|{:-<32}|{:-<8}|", "", "", "", ""));

        let count = specs.len();
        for spec in &specs {
            let title = if spec.title.len() > 28 {
                format!("{}…", &spec.title[..27])
            } else {
                spec.title.clone()
            };
            lines.push(format!(
                "| {:<20} | {:<14} | {:<30} | {:>5.0}% |",
                spec.id.as_str(),
                spec.status.as_str(),
                title,
                spec.coverage_pct()
            ));
        }

        let metadata = json!({
            "count": count,
            "status_filter": input["status"].as_str(),
        });

        Ok(ToolOutput {
            content: lines.join("\n"),
            metadata: Some(metadata),
        })
    }
}
