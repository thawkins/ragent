//! LSP document-symbols tool.
//!
//! Provides [`LspSymbolsTool`], which queries the connected LSP server for all
//! symbols in a document (functions, structs, enums, constants, etc.) and returns
//! them as a structured list ordered by line number.

use anyhow::{Context as _, Result};
use lsp_types::{DocumentSymbolParams, DocumentSymbolResponse, PartialResultParams, WorkDoneProgressParams};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Returns all symbols (functions, types, constants, etc.) defined in a file.
///
/// Queries the LSP server registered for the file's extension and returns a
/// flat list of symbol names, kinds, and line numbers. Useful for quickly
/// understanding a file's structure without reading it line-by-line.
pub struct LspSymbolsTool;

#[async_trait::async_trait]
impl Tool for LspSymbolsTool {
    fn name(&self) -> &str {
        "lsp_symbols"
    }

    fn description(&self) -> &str {
        "List all symbols (functions, types, constants, etc.) defined in a source file \
         using the Language Server Protocol. Returns names, kinds, and line numbers. \
         Requires an LSP server configured for the file's language."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the source file to query"
                }
            },
            "required": ["path"]
        })
    }

    fn permission_category(&self) -> &str {
        "lsp:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"].as_str().context("Missing required 'path' parameter")?;
        let path = ctx.working_dir.join(path_str);
        let path = path.canonicalize()
            .with_context(|| format!("Cannot resolve path: {path_str}"))?;

        let client = {
            let lsp = ctx.lsp_manager.as_ref()
                .context("No LSP manager — add a server to ragent.json 'lsp' section")?;
            let guard = lsp.read().await;
            guard.client_for_path(&path)
                .with_context(|| format!(
                    "No LSP server for '{}' files — check your ragent.json 'lsp' configuration",
                    path.extension().and_then(|e| e.to_str()).unwrap_or("?")
                ))?
        };

        client.open_document(&path).await
            .with_context(|| format!("LSP: failed to open {}", path.display()))?;

        let uri = client.text_document_id(&path)?;
        let params = DocumentSymbolParams {
            text_document: uri,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        let result: Option<DocumentSymbolResponse> = client
            .request("textDocument/documentSymbol", params)
            .await
            .context("LSP documentSymbol request failed")?;

        let Some(response) = result else {
            return Ok(ToolOutput {
                content: format!("No symbols found in {path_str}"),
                metadata: Some(json!({ "symbol_count": 0 })),
            });
        };

        let mut lines = vec![format!("Symbols in {path_str}:\n")];
        let mut count = 0usize;

        match response {
            DocumentSymbolResponse::Nested(syms) => {
                let mut flat = Vec::new();
                for sym in &syms {
                    flatten_symbol(sym, 0, &mut flat);
                }
                for (indent, kind, name, line) in flat {
                    let prefix = "  ".repeat(indent);
                    lines.push(format!("  {prefix}{kind:<12} {name:<40} line {}", line + 1));
                    count += 1;
                }
            }
            DocumentSymbolResponse::Flat(mut syms) => {
                syms.sort_by_key(|s| s.location.range.start.line);
                for sym in &syms {
                    let kind = symbol_kind_name(sym.kind);
                    let line = sym.location.range.start.line + 1;
                    lines.push(format!("  {kind:<12} {:<40} line {line}", sym.name));
                    count += 1;
                }
            }
        }

        if count == 0 {
            lines.push("  (no symbols found)".to_string());
        }

        Ok(ToolOutput {
            content: lines.join("\n"),
            metadata: Some(json!({ "symbol_count": count, "path": path_str })),
        })
    }
}

fn flatten_symbol(
    sym: &lsp_types::DocumentSymbol,
    depth: usize,
    out: &mut Vec<(usize, &'static str, String, u32)>,
) {
    let kind = symbol_kind_name(sym.kind);
    let line = sym.selection_range.start.line;
    out.push((depth, kind, sym.name.clone(), line));
    if let Some(children) = &sym.children {
        for child in children {
            flatten_symbol(child, depth + 1, out);
        }
    }
}

fn symbol_kind_name(kind: lsp_types::SymbolKind) -> &'static str {
    use lsp_types::SymbolKind;
    match kind {
        SymbolKind::FILE => "file",
        SymbolKind::MODULE => "module",
        SymbolKind::NAMESPACE => "namespace",
        SymbolKind::PACKAGE => "package",
        SymbolKind::CLASS => "class",
        SymbolKind::METHOD => "method",
        SymbolKind::PROPERTY => "property",
        SymbolKind::FIELD => "field",
        SymbolKind::CONSTRUCTOR => "constructor",
        SymbolKind::ENUM => "enum",
        SymbolKind::INTERFACE => "interface",
        SymbolKind::FUNCTION => "function",
        SymbolKind::VARIABLE => "variable",
        SymbolKind::CONSTANT => "constant",
        SymbolKind::STRING => "string",
        SymbolKind::NUMBER => "number",
        SymbolKind::BOOLEAN => "boolean",
        SymbolKind::ARRAY => "array",
        SymbolKind::OBJECT => "object",
        SymbolKind::KEY => "key",
        SymbolKind::NULL => "null",
        SymbolKind::ENUM_MEMBER => "enum-member",
        SymbolKind::STRUCT => "struct",
        SymbolKind::EVENT => "event",
        SymbolKind::OPERATOR => "operator",
        SymbolKind::TYPE_PARAMETER => "type-param",
        _ => "symbol",
    }
}
