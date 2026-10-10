//! Shared CLI report-body rewriting for the slash-command parity surfaces
//! (`ragent connectors`, `ragent plugins`).
//!
//! Both `ragent connectors` and `ragent plugins` render the same report bodies
//! the TUI slash commands produce and then rewrite the TUI attribution and usage
//! heading for the non-TUI surface (FR-006, FR-021). Hoisting the rewrite into one
//! function keeps the two surfaces byte-for-byte in step so one wording serves both.

/// Rewrite a shared slash-command report for a `ragent <verb>` CLI surface (FR-021).
///
/// The TUI `From: /<verb> ...` attribution becomes `ragent <verb> ...`, the
/// `## /<verb>` usage heading becomes `## ragent <verb>`, and the usage-table rows
/// swap the `/<verb>` trigger for `ragent <verb>`. The body is otherwise returned
/// verbatim so both surfaces stay in step.
#[must_use]
pub fn rewrite_report_body(report: &str, verb: &str) -> String {
    let from = format!("From: /{verb}");
    let heading = format!("## /{verb}");
    let row = format!("`/{verb} ");
    let rewritten = report
        .strip_prefix(from.as_str())
        .map(|tail| format!("ragent {verb}{tail}"))
        .unwrap_or_else(|| report.to_string());
    let rewritten = rewritten
        .replace(&heading, &format!("## ragent {verb}"))
        .replace(&row, &format!("`ragent {verb} "));
    format!("{rewritten}\n")
}
