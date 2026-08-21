//! Codebase index path tool — shortest path between two symbols (FR-012).
//!
//! Computes the shortest path (by hop count) between two symbols in the
//! semantic code graph, showing each hop as `A --kind--> B` with
//! confidence tags. When the index is busy (FR-017), returns a
//! `codeindex_busy` response instead of stalling the agent loop.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::codeindex_utils::codeindex_not_available;
use crate::codeindex_utils::{busy_output, with_retry};

/// Compute the shortest path (by hop count) between two symbols in the
/// semantic code graph (FR-012, FR-017).
pub struct CodeIndexPathTool;

#[async_trait::async_trait]
impl Tool for CodeIndexPathTool {
    fn name(&self) -> &'static str {
        "codeindex_path"
    }

    fn description(&self) -> &'static str {
        "Find the shortest path (by hop count) between two symbols in the codebase \
         graph, displaying each hop as `A --kind--> B` with confidence tags. \
         Requires the code index to be active and the graph to be built (run \
         `/codeindex graph build` first). Returns a `codeindex_busy` response if \
         the index is temporarily locked (FR-012, FR-017)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "from": {
                    "type": "string",
                    "description": "The name of the source symbol"
                },
                "to": {
                    "type": "string",
                    "description": "The name of the target symbol"
                }
            },
            "required": ["from", "to"],
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

        let from = match input["from"].as_str() {
            Some(s) if !s.is_empty() => s,
            _ => {
                return Ok(ToolOutput {
                    content: "The `from` parameter is required and must be a non-empty \
                              string specifying the source symbol name."
                        .to_string(),
                    metadata: Some(json!({
                        "error": "codeindex_missing_parameter",
                        "parameter": "from",
                        "fallback_tools": ["grep", "glob"]
                    })),
                });
            }
        };

        let to = match input["to"].as_str() {
            Some(s) if !s.is_empty() => s,
            _ => {
                return Ok(ToolOutput {
                    content: "The `to` parameter is required and must be a non-empty \
                              string specifying the target symbol name."
                        .to_string(),
                    metadata: Some(json!({
                        "error": "codeindex_missing_parameter",
                        "parameter": "to",
                        "fallback_tools": ["grep", "glob"]
                    })),
                });
            }
        };

        // with_retry returns Option<Option<Option<PathResult>>>:
        //   None              = index busy (lock not acquired after retries)
        //   Some(None)        = lock acquired but no path between the symbols
        //   Some(Some(result)) = path found
        let opt_path: Option<Option<ragent_codeindex::graph::PathResult>> =
            with_retry(|| idx.try_path(from, to)).await?;

        match opt_path {
            None => Ok(busy_output("codeindex_path")),
            Some(None) => Ok(ToolOutput {
                content: format!(
                    "No path found between `{from}` and `{to}`. Either one of the \
                     symbols does not exist in the index, or there is no connection \
                     between them in the graph. Run `/codeindex graph build` to \
                     (re)build the semantic edge graph."
                ),
                metadata: Some(json!({
                    "error": "codeindex_no_path",
                    "from": from,
                    "to": to,
                    "fallback_tools": ["grep", "glob"]
                })),
            }),
            Some(Some(result)) => {
                let mut output = String::from("## Shortest Path\n\n");
                output.push_str(&format!(
                    "**{} hops** from `{}` to `{}`\n\n",
                    result.hops, from, to
                ));

                // Render each step as `A --kind--> B`.
                if result.steps.is_empty() {
                    output.push_str("(empty path)\n");
                } else {
                    for (i, (sym, kind)) in result.steps.iter().enumerate() {
                        if i == 0 {
                            output.push_str(&format!("`{sym}`"));
                        } else if let Some(k) = kind {
                            output.push_str(&format!(" --{k}--> `{sym}`"));
                        } else {
                            output.push_str(&format!(" --> `{sym}`"));
                        }
                    }
                    output.push('\n');
                }

                Ok(ToolOutput {
                    content: output,
                    metadata: Some(json!({
                        "hops": result.hops,
                        "from": from,
                        "to": to,
                        "steps": result.steps.iter().map(|(sym, kind)| {
                            json!({
                                "symbol": sym,
                                "kind": kind
                            })
                        }).collect::<Vec<_>>(),
                    })),
                })
            }
        }
    }
}
