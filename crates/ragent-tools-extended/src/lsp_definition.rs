//! LSP go-to-definition tool.
//!
//! Provides [`LspDefinitionTool`], which jumps to where a symbol is defined
//! using the LSP `textDocument/definition` request.

use anyhow::{Context as _, Result};
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

        let lsp = ctx
            .lsp_backend
            .as_ref()
            .context("No LSP backend — add a server to ragent.json 'lsp' section")?;
        let locations = lsp
            .definition(&path, lsp_line, lsp_char)
            .await
            .context("LSP definition request failed")?;

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
