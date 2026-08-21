//! Codebase index god-nodes tool — top-N most-connected symbols (FR-014).
//!
//! Returns the highest-degree symbols in the semantic code graph, useful for
//! identifying hub functions, central types, and architectural bottleneck
//! symbols. When the index is busy (FR-017), returns a `codeindex_busy`
//! response instead of stalling the agent loop.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::codeindex_utils::codeindex_not_available;
use crate::codeindex_utils::{busy_output, with_retry};

/// Display the top-N most-connected symbols (highest degree) with their
/// names, source files, and edge counts (FR-014).
pub struct CodeIndexGodnodesTool;

/// Default number of god-nodes to return when the caller does not specify `n`.
const DEFAULT_LIMIT: usize = 10;

/// Maximum number of god-nodes to return, even if the caller requests more.
const MAX_LIMIT: usize = 100;

#[async_trait::async_trait]
impl Tool for CodeIndexGodnodesTool {
    fn name(&self) -> &'static str {
        "codeindex_godnodes"
    }

    fn description(&self) -> &'static str {
        "Display the top-N most-connected symbols (highest degree) in the codebase \
         graph with their names, source files, and edge counts. Optional parameter: \
         'n' (default 10, max 100). Requires the code index to be active and the \
         graph to be built (run `/codeindex graph build` first). Returns a \
         `codeindex_busy` response if the index is temporarily locked (FR-014, FR-017)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "n": {
                    "type": "integer",
                    "description": "Maximum number of god-nodes to return (default: 10, max: 100)",
                    "minimum": 1
                }
            },
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "codeindex:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let idx = match &ctx.code_index {
            Some(idx) => idx,
            None => {
                return Ok(codeindex_not_available(
                    "Use `grep` or `glob` as fallback tools for code search.",
                    &["grep", "glob"],
                ));
            }
        };

        let n = input["n"]
            .as_u64()
            .map_or(DEFAULT_LIMIT, |v| (v as usize).clamp(1, MAX_LIMIT));

        let nodes = match with_retry(|| idx.try_godnodes(n)).await? {
            Some(nodes) => nodes,
            None => return Ok(busy_output("codeindex_godnodes")),
        };

        if nodes.is_empty() {
            return Ok(ToolOutput {
                content: "No graph data available. Run `/codeindex graph build` first to \
                          build the semantic edge graph."
                    .to_string(),
                metadata: Some(json!({
                    "error": "codeindex_empty_graph",
                    "fallback_tools": ["grep", "glob"]
                })),
            });
        }

        let mut output = String::from("## God Nodes (Top Most-Connected Symbols)\n\n");
        output.push_str("| # | Symbol | Source File | Degree |\n");
        output.push_str("|---|--------|-------------|--------|\n");
        for (i, node) in nodes.iter().enumerate() {
            output.push_str(&format!(
                "| {} | `{}` | `{}` | {} |\n",
                i + 1,
                node.name,
                node.source_file,
                node.degree,
            ));
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "total_results": nodes.len(),
                "limit": n,
                "nodes": nodes.iter().map(|n| json!({
                    "name": n.name,
                    "source_file": n.source_file,
                    "degree": n.degree,
                })).collect::<Vec<_>>(),
            })),
        })
    }
}
