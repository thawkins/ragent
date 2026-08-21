//! Codebase index communities tool — community detection (FR-013).
//!
//! Runs label-propagation community detection over the semantic code graph,
//! displaying each detected community with its auto-generated label and
//! member count. When the index is busy (FR-017), returns a `codeindex_busy`
//! response instead of stalling the agent loop.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::codeindex_utils::codeindex_not_available;
use crate::codeindex_utils::{busy_output, with_retry};

/// Run community detection and list detected communities with labels and
/// member counts (FR-013, FR-017).
pub struct CodeIndexCommunitiesTool;

#[async_trait::async_trait]
impl Tool for CodeIndexCommunitiesTool {
    fn name(&self) -> &'static str {
        "codeindex_communities"
    }

    fn description(&self) -> &'static str {
        "Run community detection over the codebase graph and display each \
         detected community with its auto-generated label and member count. \
         Requires the code index to be active and the graph to be built (run \
         `/codeindex graph build` first). Returns a `codeindex_busy` response \
         if the index is temporarily locked (FR-013, FR-017)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "codeindex:read"
    }

    async fn execute(&self, _input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let idx = match &ctx.code_index {
            Some(idx) => idx,
            None => {
                return Ok(codeindex_not_available(
                    "Use `grep` or `glob` as fallback tools for code search.",
                    &["grep", "glob"],
                ));
            }
        };

        let communities = match with_retry(|| idx.try_communities()).await? {
            Some(communities) => communities,
            None => return Ok(busy_output("codeindex_communities")),
        };

        if communities.is_empty() {
            return Ok(ToolOutput {
                content: "No graph data available. Run `/codeindex graph build` first to \
                          build the semantic edge graph, then retry."
                    .to_string(),
                metadata: Some(json!({
                    "error": "codeindex_empty_graph",
                    "fallback_tools": ["grep", "glob"]
                })),
            });
        }

        let mut output = String::from("## Communities\n\n");
        output.push_str("| Community | Label | Members |\n");
        output.push_str("|-----------|-------|--------|\n");
        for comm in &communities {
            let label = comm.label.as_deref().unwrap_or("—");
            output.push_str(&format!(
                "| {} | {} | {} |\n",
                comm.id, label, comm.member_count,
            ));
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "total_communities": communities.len(),
                "communities": communities.iter().map(|c| json!({
                    "id": c.id,
                    "label": c.label,
                    "member_count": c.member_count,
                })).collect::<Vec<_>>(),
            })),
        })
    }
}
