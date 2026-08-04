//! Search specifications.
//!
//! Full-text search across all spec files (SPEC.md + PLAN.md).

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Search specs by query string.
pub struct SpecSearchTool;

#[async_trait::async_trait]
impl Tool for SpecSearchTool {
    fn name(&self) -> &'static str {
        "spec_search"
    }

    fn description(&self) -> &'static str {
        "Search all specifications by keyword. REQUIRED parameter: 'query' (string). \
         Returns matching specs with context snippets, including spec ID, title, status, \
         and relevant excerpts. Use this to find the right spec when you do not already \
         know its identifier. Common gotcha: the search is case-insensitive substring \
         matching across titles and content, not a semantic search."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query string (required)"
                }
            },
            "required": ["query"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "spec:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let query = input["query"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing required 'query' parameter"))?;

        let spec_manager = ctx.spec_manager.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Spec manager is not configured. Set up a specs/ directory first.")
        })?;

        let results = spec_manager
            .search_specs(query)
            .await
            .map_err(|e| anyhow::anyhow!("Search failed: {}", e))?;

        let mut lines = vec![format!("Search results for '{}':", query), String::new()];

        if results.is_empty() {
            lines.push("No matching specs found.".to_string());
        } else {
            for result in &results {
                lines.push(format!(
                    "## {} — {} (score: {})",
                    result.spec.id, result.spec.title, result.score
                ));
                lines.push(format!("Status: {}", result.spec.status.as_str()));
                if !result.snippets.is_empty() {
                    lines.push("### Snippets".to_string());
                    for snippet in &result.snippets {
                        lines.push(format!("> {}\n", snippet));
                    }
                }
                lines.push(String::new());
            }
        }

        let metadata = json!({
            "query": query,
            "count": results.len(),
        });

        Ok(ToolOutput {
            content: lines.join("\n"),
            metadata: Some(metadata),
        })
    }
}
