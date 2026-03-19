//! LSP diagnostics tool.
//!
//! Provides [`LspDiagnosticsTool`], which surfaces compiler errors, warnings,
//! and hints accumulated from connected LSP servers.

use anyhow::{Context as _, Result};
use lsp_types::DiagnosticSeverity;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Show compiler errors, warnings, and hints from connected LSP servers.
///
/// Reads accumulated `textDocument/publishDiagnostics` notifications that the
/// LSP servers have pushed since startup. Optionally filtered to a specific
/// file path.
pub struct LspDiagnosticsTool;

#[async_trait::async_trait]
impl Tool for LspDiagnosticsTool {
    fn name(&self) -> &str {
        "lsp_diagnostics"
    }

    /// Returns the tool description.
    fn description(&self) -> &str {
        "Show compiler errors, warnings, and hints accumulated from connected LSP servers. \
         Pass a file path to filter results, or omit for all diagnostics. \
         Note: diagnostics are pushed by the server on file open — use lsp_symbols or lsp_hover \
         first to trigger analysis on a file. \
         Requires an LSP server configured for the file's language."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Optional file path to filter diagnostics (omit for all files)"
                },
                "severity": {
                    "type": "string",
                    "enum": ["error", "warning", "information", "hint", "all"],
                    "description": "Minimum severity to include (default: all)"
                }
            }
        })
    }

    fn permission_category(&self) -> &str {
        "lsp:read"
    }

    /// Executes the LSP diagnostics query.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No LSP manager is configured in the context
    /// - The file path (if provided) cannot be resolved or canonicalized
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_filter = input["path"].as_str();
        let severity_filter = input["severity"].as_str().unwrap_or("all");

        let lsp = ctx
            .lsp_manager
            .as_ref()
            .context("No LSP manager — add a server to ragent.json 'lsp' section")?;

        let filter_path = if let Some(p) = path_filter {
            let full = ctx.working_dir.join(p);
            let canonical = full
                .canonicalize()
                .with_context(|| format!("Cannot resolve path: {p}"))?;
            Some(canonical)
        } else {
            None
        };

        let all_diags = {
            let guard = lsp.read().await;
            guard.diagnostics_for(filter_path.as_deref()).await
        };

        if all_diags.is_empty() {
            let scope = path_filter.unwrap_or("all files");
            return Ok(ToolOutput {
                content: format!("No diagnostics for {scope}"),
                metadata: Some(json!({ "total": 0 })),
            });
        }

        let min_severity = parse_severity(severity_filter);
        let mut out = String::new();
        let mut total = 0usize;
        let mut diag_json = Vec::new();

        for (uri, diags) in &all_diags {
            let filtered: Vec<_> = diags
                .iter()
                .filter(|d| {
                    let sev = d.severity.unwrap_or(DiagnosticSeverity::INFORMATION);
                    severity_passes(sev, min_severity)
                })
                .collect();

            if filtered.is_empty() {
                continue;
            }

            // Show a short display name (strip absolute path prefix if inside cwd).
            let display = shorten_uri(uri, &ctx.working_dir.to_string_lossy());
            out.push_str(&format!("{display}:\n"));

            for d in &filtered {
                let sev = severity_label(d.severity.unwrap_or(DiagnosticSeverity::INFORMATION));
                let line = d.range.start.line + 1;
                let col = d.range.start.character + 1;
                let msg = d.message.replace('\n', " ");
                out.push_str(&format!("  [{sev}] {line}:{col}  {msg}\n"));
                diag_json.push(json!({
                    "file": display,
                    "line": line,
                    "column": col,
                    "severity": sev,
                    "message": d.message,
                    "code": d.code,
                }));
                total += 1;
            }
            out.push('\n');
        }

        if total == 0 {
            let scope = path_filter.unwrap_or("all files");
            return Ok(ToolOutput {
                content: format!("No {severity_filter} diagnostics for {scope}"),
                metadata: Some(json!({ "total": 0 })),
            });
        }

        out = format!("{total} diagnostic(s):\n\n") + &out;

        Ok(ToolOutput {
            content: out,
            metadata: Some(json!({ "total": total, "diagnostics": diag_json })),
        })
    }
}

fn parse_severity(s: &str) -> DiagnosticSeverity {
    match s {
        "error" => DiagnosticSeverity::ERROR,
        "warning" => DiagnosticSeverity::WARNING,
        "information" => DiagnosticSeverity::INFORMATION,
        "hint" => DiagnosticSeverity::HINT,
        _ => DiagnosticSeverity::HINT, // "all" → include everything (hint is lowest)
    }
}

fn severity_passes(actual: DiagnosticSeverity, min: DiagnosticSeverity) -> bool {
    // Lower integer = higher severity in LSP (1=error, 4=hint).
    // DiagnosticSeverity wraps u32 but doesn't expose the inner field,
    // so compare by matching both on the same named constants.
    use DiagnosticSeverity as D;
    let rank = |s: DiagnosticSeverity| match s {
        D::ERROR => 1u8,
        D::WARNING => 2,
        D::INFORMATION => 3,
        D::HINT => 4,
        _ => 5,
    };
    rank(actual) <= rank(min)
}

fn severity_label(s: DiagnosticSeverity) -> &'static str {
    match s {
        DiagnosticSeverity::ERROR => "error",
        DiagnosticSeverity::WARNING => "warning",
        DiagnosticSeverity::INFORMATION => "info",
        DiagnosticSeverity::HINT => "hint",
        _ => "unknown",
    }
}

fn shorten_uri(uri: &str, cwd: &str) -> String {
    // Convert file:///path to relative path if inside cwd, else keep as-is.
    if let Ok(url) = uri.parse::<url::Url>() {
        if let Ok(path) = url.to_file_path() {
            let s = path.to_string_lossy();
            if let Some(rel) = s.strip_prefix(cwd).and_then(|r| r.strip_prefix('/')) {
                return rel.to_string();
            }
            return s.into_owned();
        }
    }
    uri.to_string()
}
