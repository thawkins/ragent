//! LSP go-to-definition tool.
//!
//! Provides [`LspDefinitionTool`], which jumps to where a symbol is defined
//! using the LSP `textDocument/definition` request.

use anyhow::{Context as _, Result};
use lsp_types::{
    GotoDefinitionParams, GotoDefinitionResponse, Position, TextDocumentPositionParams,
    WorkDoneProgressParams,
};
use serde_json::{Value, json};
use url::Url;

use super::{Tool, ToolContext, ToolOutput};

/// Find where a symbol is defined using the Language Server Protocol.
///
/// Returns the file path and line number where the symbol under the cursor is
/// declared. For symbols with multiple definitions (e.g. overloaded functions)
/// all locations are returned.
pub struct LspDefinitionTool;

#[async_trait::async_trait]
impl Tool for LspDefinitionTool {
    /// Returns the tool name.
    fn name(&self) -> &'static str {
        "lsp_definition"
    }

    fn description(&self) -> &'static str {
        "Find where a symbol is defined using the Language Server Protocol. \
         Provide the file and cursor position; returns the definition's file path and line. \
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
                }
            },
            "required": ["path", "line", "column"]
        })
    }

    /// Returns the permission category.
    fn permission_category(&self) -> &'static str {
        "lsp:read"
    }

    /// Executes the LSP definition query.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Required parameters (`path`, `line`, `column`) are missing or invalid
    /// - The file path cannot be resolved or canonicalized
    /// - No LSP manager is configured in the context
    /// - No LSP server is available for the file's language/extension
    /// - The document cannot be opened by the LSP server
    /// - The LSP definition request fails
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"]
            .as_str()
            .context("Missing required 'path' parameter")?;
        let line = input["line"]
            .as_u64()
            .context("Missing required 'line' parameter")? as u32;
        let column = input["column"]
            .as_u64()
            .context("Missing required 'column' parameter")? as u32;

        let lsp_line = line.saturating_sub(1);
        let lsp_char = column.saturating_sub(1);

        let path = ctx.working_dir.join(path_str);
        let path = path
            .canonicalize()
            .with_context(|| format!("Cannot resolve path: {path_str}"))?;

        let client = {
            let lsp = ctx
                .lsp_manager
                .as_ref()
                .context("No LSP manager — add a server to ragent.json 'lsp' section")?;
            let guard = lsp.read().await;
            guard.client_for_path(&path).with_context(|| {
                format!(
                    "No LSP server for '{}' files",
                    path.extension().and_then(|e| e.to_str()).unwrap_or("?")
                )
            })?
        };

        client
            .open_document(&path)
            .await
            .with_context(|| format!("LSP: failed to open {}", path.display()))?;

        let uri = client.text_document_id(&path)?;
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: uri,
                position: Position {
                    line: lsp_line,
                    character: lsp_char,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        };

        let result: Option<GotoDefinitionResponse> = client
            .request("textDocument/definition", params)
            .await
            .context("LSP definition request failed")?;

        let locations: Vec<lsp_types::Location> = match result {
            None => vec![],
            Some(GotoDefinitionResponse::Scalar(loc)) => vec![loc],
            Some(GotoDefinitionResponse::Array(locs)) => locs,
            Some(GotoDefinitionResponse::Link(links)) => links
                .into_iter()
                .map(|l| lsp_types::Location {
                    uri: l.target_uri,
                    range: l.target_range,
                })
                .collect(),
        };

        if locations.is_empty() {
            return Ok(ToolOutput {
                content: format!("No definition found at {path_str}:{line}:{column}"),
                metadata: Some(json!({ "count": 0 })),
            });
        }

        let mut out = format!("Definition(s) for symbol at {path_str}:{line}:{column}:\n\n");
        let mut locs_json = Vec::new();

        for loc in &locations {
            let file = uri_to_display(&loc.uri);
            let def_line = loc.range.start.line + 1;
            let def_col = loc.range.start.character + 1;
            out.push_str(&format!("  {file}:{def_line}:{def_col}\n"));
            locs_json.push(json!({ "file": file, "line": def_line, "column": def_col }));
        }

        Ok(ToolOutput {
            content: out,
            metadata: Some(json!({ "count": locations.len(), "locations": locs_json })),
        })
    }
}

fn uri_to_display(uri: &lsp_types::Uri) -> String {
    // Try to parse as URL and extract the path component for display.
    uri.as_str()
        .parse::<Url>()
        .ok()
        .and_then(|u| u.to_file_path().ok())
        .map_or_else(
            || uri.as_str().to_string(),
            |p| p.to_string_lossy().into_owned(),
        )
}
