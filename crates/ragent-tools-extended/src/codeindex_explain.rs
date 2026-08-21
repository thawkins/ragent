//! Codebase index explain tool — node metadata and connections (FR-011).
//!
//! Displays a symbol's node metadata (source file, line, community, degree)
//! and its incoming/outgoing edges with kind and confidence tags, limited to
//! the top 50 connections. When the index is busy (FR-017), returns a
//! `codeindex_busy` response instead of stalling the agent loop.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::codeindex_utils::codeindex_not_available;
use crate::codeindex_utils::{busy_output, with_retry};

/// Explain a symbol: show its node metadata and connections (FR-011, FR-017).
pub struct CodeIndexExplainTool;

#[async_trait::async_trait]
impl Tool for CodeIndexExplainTool {
    fn name(&self) -> &'static str {
        "codeindex_explain"
    }

    fn description(&self) -> &'static str {
        "Explain a symbol in the codebase graph: show its node metadata (source \
         file, line, community, degree) and its incoming/outgoing edges with \
         kind and confidence tags. Limited to the top 50 connections. Requires \
         the code index to be active. Returns a `codeindex_busy` response if the \
         index is temporarily locked (FR-011, FR-017)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "The name of the symbol to explain"
                }
            },
            "required": ["symbol"],
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

        let symbol = match input["symbol"].as_str() {
            Some(s) if !s.is_empty() => s,
            _ => {
                return Ok(ToolOutput {
                    content: "The `symbol` parameter is required and must be a \
                              non-empty string specifying the symbol name."
                        .to_string(),
                    metadata: Some(json!({
                        "error": "codeindex_missing_parameter",
                        "parameter": "symbol",
                        "fallback_tools": ["grep", "glob"]
                    })),
                });
            }
        };

        // with_retry returns Option<Option<ExplainResult>>:
        //   None              = index busy (lock not acquired after retries)
        //   Some(None)        = lock acquired but symbol not found
        //   Some(Some(result)) = symbol found
        let opt_result: Option<Option<ragent_codeindex::graph::ExplainResult>> =
            with_retry(|| idx.try_explain(symbol)).await?;

        match opt_result {
            None => Ok(busy_output("codeindex_explain")),
            Some(None) => Ok(ToolOutput {
                content: format!(
                    "Symbol `{symbol}` was not found in the index. Run \
                     `/codeindex reindex` to ensure the codebase is indexed, then \
                     retry."
                ),
                metadata: Some(json!({
                    "error": "codeindex_symbol_not_found",
                    "symbol": symbol,
                    "fallback_tools": ["grep", "glob"]
                })),
            }),
            Some(Some(result)) => {
                let mut output = String::from("## Symbol Explanation\n\n");
                output.push_str(&format!("**{}** `{}`\n\n", result.name, result.source_file));
                output.push_str(&format!("- **Line:** {}\n", result.line));
                output.push_str(&format!("- **Degree:** {}\n", result.degree));
                if let Some(community) = result.community {
                    output.push_str(&format!("- **Community:** {}\n", community));
                }
                output.push('\n');

                // Incoming edges.
                output.push_str("### Incoming\n\n");
                if result.incoming.is_empty() {
                    output.push_str("_(none)_\n\n");
                } else {
                    output.push_str("| Symbol | File | Kind | Confidence | Line |\n");
                    output.push_str("|--------|------|------|------------|------|\n");
                    for conn in &result.incoming {
                        output.push_str(&format!(
                            "| `{}` | `{}` | {} | {} | {} |\n",
                            conn.symbol,
                            conn.source_file,
                            conn.kind,
                            conn.confidence,
                            conn.line.map_or("—".to_string(), |l| l.to_string()),
                        ));
                    }
                    output.push('\n');
                }

                // Outgoing edges.
                output.push_str("### Outgoing\n\n");
                if result.outgoing.is_empty() {
                    output.push_str("_(none)_\n\n");
                } else {
                    output.push_str("| Symbol | File | Kind | Confidence | Line |\n");
                    output.push_str("|--------|------|------|------------|------|\n");
                    for conn in &result.outgoing {
                        output.push_str(&format!(
                            "| `{}` | `{}` | {} | {} | {} |\n",
                            conn.symbol,
                            conn.source_file,
                            conn.kind,
                            conn.confidence,
                            conn.line.map_or("—".to_string(), |l| l.to_string()),
                        ));
                    }
                }

                Ok(ToolOutput {
                    content: output,
                    metadata: Some(json!({
                        "name": result.name,
                        "source_file": result.source_file,
                        "line": result.line,
                        "community": result.community,
                        "degree": result.degree,
                        "incoming": result.incoming.iter().map(|c| json!({
                            "symbol": c.symbol,
                            "source_file": c.source_file,
                            "kind": c.kind,
                            "confidence": c.confidence.to_string(),
                            "line": c.line,
                        })).collect::<Vec<_>>(),
                        "outgoing": result.outgoing.iter().map(|c| json!({
                            "symbol": c.symbol,
                            "source_file": c.source_file,
                            "kind": c.kind,
                            "confidence": c.confidence.to_string(),
                            "line": c.line,
                        })).collect::<Vec<_>>(),
                    })),
                })
            }
        }
    }
}
