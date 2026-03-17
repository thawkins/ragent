//! LSP hover tool.
//!
//! Provides [`LspHoverTool`], which queries the LSP server for type information
//! and documentation at a specific position in a source file.

use anyhow::{Context as _, Result};
use lsp_types::{HoverParams, Position, TextDocumentPositionParams, WorkDoneProgressParams};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Returns type information and documentation at a specific position in a file.
///
/// Delegates to the LSP server's `textDocument/hover` request. Useful for
/// understanding the type of an expression, reading doc-comments, or
/// investigating what a function signature looks like without navigating to
/// its definition.
pub struct LspHoverTool;

#[async_trait::async_trait]
impl Tool for LspHoverTool {
    fn name(&self) -> &str {
        "lsp_hover"
    }

    fn description(&self) -> &str {
        "Get type information and documentation for a symbol at a specific line and column \
         in a source file using the Language Server Protocol. \
         Requires an LSP server configured for the file's language."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the source file"
                },
                "line": {
                    "type": "integer",
                    "description": "1-based line number"
                },
                "column": {
                    "type": "integer",
                    "description": "1-based column (character) number"
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

        // LSP positions are 0-based; ragent exposes 1-based to the LLM.
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
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: uri,
                position: Position { line: lsp_line, character: lsp_char },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };

        let result: Option<lsp_types::Hover> = client
            .request("textDocument/hover", params)
            .await
            .context("LSP hover request failed")?;

        let content = match result {
            None => format!("No hover information available at {}:{}:{}", path_str, line, column),
            Some(hover) => {
                let text = match &hover.contents {
                    lsp_types::HoverContents::Scalar(markup) => markup_to_text(markup),
                    lsp_types::HoverContents::Array(markups) => markups
                        .iter()
                        .map(markup_to_text)
                        .collect::<Vec<_>>()
                        .join("\n"),
                    lsp_types::HoverContents::Markup(markup) => markup.value.clone(),
                };
                format!("Hover at {}:{}:{}\n\n{}", path_str, line, column, text)
            }
        };

        Ok(ToolOutput {
            content,
            metadata: Some(json!({
                "path": path_str,
                "line": line,
                "column": column,
            })),
        })
    }
}

fn markup_to_text(markup: &lsp_types::MarkedString) -> String {
    match markup {
        lsp_types::MarkedString::String(s) => s.clone(),
        lsp_types::MarkedString::LanguageString(ls) => {
            format!("```{}\n{}\n```", ls.language, ls.value)
        }
    }
}
