//! LSP find-references tool.
//!
//! Provides [`LspReferencesTool`], which finds all usages of a symbol
//! throughout the workspace using the LSP `textDocument/references` request.

use anyhow::{Context as _, Result};
use lsp_types::{Position, ReferenceContext, ReferenceParams, TextDocumentPositionParams, WorkDoneProgressParams};
use serde_json::{Value, json};
use url::Url;

use super::{Tool, ToolContext, ToolOutput};

/// Find all references to a symbol across the workspace.
///
/// Delegates to the LSP server's `textDocument/references` request. Returns
/// file paths and line numbers for every usage of the symbol under the cursor.
/// Optionally includes the declaration itself.
pub struct LspReferencesTool;

#[async_trait::async_trait]
impl Tool for LspReferencesTool {
    fn name(&self) -> &str {
        "lsp_references"
    }

    fn description(&self) -> &str {
        "Find all usages (references) of a symbol in the workspace using the Language \
         Server Protocol. Returns file paths and line numbers. \
         Requires an LSP server configured for the file's language."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the source file containing the symbol"
                },
                "line": {
                    "type": "integer",
                    "description": "1-based line number where the symbol appears"
                },
                "column": {
                    "type": "integer",
                    "description": "1-based column (character) number of the symbol"
                },
                "include_declaration": {
                    "type": "boolean",
                    "description": "Include the symbol's own declaration in results (default: true)"
                }
            },
            "required": ["path", "line", "column"]
        })
    }

    fn permission_category(&self) -> &str {
        "lsp:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"].as_str().context("Missing required 'path' parameter")?;
        let line = input["line"].as_u64().context("Missing required 'line' parameter")? as u32;
        let column = input["column"].as_u64().context("Missing required 'column' parameter")? as u32;
        let include_declaration = input["include_declaration"].as_bool().unwrap_or(true);

        let lsp_line = line.saturating_sub(1);
        let lsp_char = column.saturating_sub(1);

        let path = ctx.working_dir.join(path_str);
        let path = path.canonicalize()
            .with_context(|| format!("Cannot resolve path: {path_str}"))?;

        let client = {
            let lsp = ctx.lsp_manager.as_ref()
                .context("No LSP manager — add a server to ragent.json 'lsp' section")?;
            let guard = lsp.read().await;
            guard.client_for_path(&path)
                .with_context(|| format!(
                    "No LSP server for '{}' files",
                    path.extension().and_then(|e| e.to_str()).unwrap_or("?")
                ))?
        };

        client.open_document(&path).await
            .with_context(|| format!("LSP: failed to open {}", path.display()))?;

        let uri = client.text_document_id(&path)?;
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: uri,
                position: Position { line: lsp_line, character: lsp_char },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
            context: ReferenceContext { include_declaration },
        };

        let result: Option<Vec<lsp_types::Location>> = client
            .request("textDocument/references", params)
            .await
            .context("LSP references request failed")?;

        let locations = result.unwrap_or_default();

        if locations.is_empty() {
            return Ok(ToolOutput {
                content: format!("No references found for symbol at {}:{}:{}", path_str, line, column),
                metadata: Some(json!({ "count": 0 })),
            });
        }

        let mut out = format!(
            "{} reference(s) to symbol at {}:{}:{}:\n\n",
            locations.len(), path_str, line, column
        );
        let mut locs_json = Vec::new();

        // Group by file for readability.
        let mut by_file: std::collections::BTreeMap<String, Vec<(u32, u32)>> = Default::default();
        for loc in &locations {
            let file = uri_to_display(&loc.uri);
            let entry = by_file.entry(file).or_default();
            entry.push((loc.range.start.line + 1, loc.range.start.character + 1));
        }

        for (file, positions) in &by_file {
            out.push_str(&format!("  {file}\n"));
            for (l, c) in positions {
                out.push_str(&format!("    line {l}:{c}\n"));
                locs_json.push(json!({ "file": file, "line": l, "column": c }));
            }
        }

        Ok(ToolOutput {
            content: out,
            metadata: Some(json!({ "count": locations.len(), "locations": locs_json })),
        })
    }
}

fn uri_to_display(uri: &lsp_types::Uri) -> String {
    uri.as_str()
        .parse::<Url>()
        .ok()
        .and_then(|u| u.to_file_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| uri.as_str().to_string())
}
