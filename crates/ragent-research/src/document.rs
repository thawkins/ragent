//! RESEARCH.md document assembly — the 8 required sections (T-020, T-021, T-022, T-024, T-049).
//!
//! `RESEARCH.md` is the single, self-contained deliverable for each
//! research item. The shape is fixed by FR-010:
//!
//! ```text
//! ---
//! <YAML frontmatter>
//! ---
//!
//! # Title: <title>
//!
//! ## Topic
//!
//! <topic description>
//!
//! ## Summary
//!
//! <one-paragraph summary of the gathered evidence>
//!
//! ## Findings
//!
//! <numbered findings, each citing references>
//!
//! ## In-Project Cross-References
//!
//! <relevant in-project files with one-line relevance notes>
//!
//! ## Open Questions
//!
//! <bulleted open questions>
//!
//! ## References Index
//!
//! | # | Type | Path/URL | Title | Relevance | Captured |
//! |---|------|----------|-------|-----------|----------|
//! | 1 | web  | https://... | ... | ... | ... |
//! | 2 | local | src/lib.rs | ... | ... | ... |
//! | 3 | spec  | foo | ... | ... | ... |
//! ```
//!
//! All sections are always present (even if empty) so a downstream tool that
//! reads `RESEARCH.md` can rely on a stable structure.

use crate::io::ResearchIo;
use crate::item::ResearchItem;
use crate::research_name::ResearchName;
use crate::source::{LocalSourceKind, Source};
use crate::status::ResearchStatus;
use chrono::Utc;

/// Maximum number of bytes allowed in a single untrusted source excerpt.
/// Sources larger than this are truncated to avoid blowing up RESEARCH.md
/// (NFR-006 + the size-cap risk in the PLAN.md Risks table).
pub const MAX_SOURCE_BODY_BYTES: usize = 256 * 1024;

/// The 8 sections that appear in every `RESEARCH.md`, in order (FR-010).
pub const REQUIRED_SECTIONS: &[&str] = &[
    "Topic",
    "Summary",
    "Findings",
    "In-Project Cross-References",
    "Open Questions",
    "References Index",
];

/// Inputs the caller supplies when assembling a fresh `RESEARCH.md` after a
/// gathering pass. The fields are intentionally separate from
/// `ResearchItem` so the session engine can fill them in incrementally
/// before committing them to disk.
#[derive(Debug, Clone)]
pub struct ResearchDocument {
    /// The item this document belongs to.
    pub item: ResearchItem,
    /// Optional human-written summary; falls back to a placeholder when
    /// empty so the section is never blank.
    pub summary: String,
    /// Numbered findings — each entry is the body of one bullet under
    /// `## Findings`. References inside the body use the form `[#N]`.
    pub findings: Vec<String>,
    /// In-project cross-references (FR-009). Each entry is one bullet under
    /// `## In-Project Cross-References`.
    pub cross_references: Vec<CrossReference>,
    /// Open questions — one bullet per question.
    pub open_questions: Vec<String>,
    /// Optional template body loaded from `research/_templates/<name>.md`
    /// (FR-020). When supplied, the template is used as the skeleton and
    /// `{{title}}`, `{{topic}}`, `{{date}}` placeholders are substituted
    /// from the item's metadata before the standard sections are appended.
    pub template_body: Option<String>,
}

/// One in-project cross-reference row (FR-009).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossReference {
    /// Project-relative path (e.g. `"src/lib.rs"`).
    pub path: String,
    /// One-line note explaining why this file is relevant.
    pub relevance: String,
}

/// Result of `assemble_document` — the body text plus the rendered file
/// payload (frontmatter + body) ready for atomic_write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssembledDocument {
    /// Full `RESEARCH.md` payload (frontmatter + body).
    pub content: String,
    /// Just the frontmatter block (without the leading/trailing `---`).
    pub frontmatter: String,
    /// Just the body text (without the frontmatter).
    pub body: String,
}

/// Assemble a `RESEARCH.md` document from the supplied [`ResearchDocument`].
///
/// The output always contains the 8 FR-010 sections, in order, even when
/// the caller passes no findings/questions — empty sections render as
/// "(none yet)" placeholders so downstream tooling can rely on the
/// structure.
///
/// References are numbered 1..=N in the order they appear in
/// `ResearchItem::sources`. Any `[#N]` marker in the body is preserved as
/// written by the caller (T-022); the assembler does not invent or rewrite
/// citation numbers.
pub fn assemble_document(doc: &ResearchDocument) -> AssembledDocument {
    let frontmatter = doc.item.render_frontmatter();
    let title = doc.item.title.clone();
    let topic = doc.item.topic.clone();

    let mut body = String::new();

    // FR-020: if a template body was supplied, use it as the skeleton after
    // substituting the standard placeholders.
    if let Some(template) = &doc.template_body {
        body.push_str(&apply_template(template, &title, &topic));
        body.push_str("\n\n");
    }

    // ── Title ────────────────────────────────────────────────────────────
    body.push_str(&format!("# Title: {}\n\n", title));

    // ── Topic ────────────────────────────────────────────────────────────
    body.push_str("## Topic\n\n");
    body.push_str(topic.trim());
    body.push_str("\n\n");

    // ── Summary ──────────────────────────────────────────────────────────
    body.push_str("## Summary\n\n");
    if doc.summary.trim().is_empty() {
        body.push_str("(no summary recorded yet — run a gathering pass to populate)\n");
    } else {
        body.push_str(doc.summary.trim());
        body.push('\n');
    }
    body.push('\n');

    // ── Findings ─────────────────────────────────────────────────────────
    body.push_str("## Findings\n\n");
    if doc.findings.is_empty() {
        body.push_str("_(no findings yet — the gathering pass will populate this section)_\n\n");
    } else {
        for (idx, finding) in doc.findings.iter().enumerate() {
            body.push_str(&format!("### Finding {}\n\n", idx + 1));
            body.push_str(finding.trim());
            body.push_str("\n\n");
        }
    }

    // ── In-Project Cross-References ─────────────────────────────────────
    body.push_str("## In-Project Cross-References\n\n");
    if doc.cross_references.is_empty() {
        body.push_str(
            "_(no relevant in-project files were identified during the gathering pass)_\n\n",
        );
    } else {
        body.push_str("| Path | Relevance |\n|------|-----------|\n");
        for cr in &doc.cross_references {
            body.push_str(&format!(
                "| `{}` | {} |\n",
                escape_pipe(&cr.path),
                escape_pipe(&cr.relevance),
            ));
        }
        body.push('\n');
    }

    // ── Open Questions ───────────────────────────────────────────────────
    body.push_str("## Open Questions\n\n");
    if doc.open_questions.is_empty() {
        body.push_str("_(none)_\n\n");
    } else {
        for q in &doc.open_questions {
            body.push_str(&format!("- {}\n", q.trim()));
        }
        body.push('\n');
    }

    // ── References Index (FR-011) ────────────────────────────────────────
    body.push_str(&ResearchIo::render_references_index(
        &doc.item.sources,
        Utc::now(),
    ));

    let content = format!("{frontmatter}\n{body}");
    AssembledDocument {
        content,
        frontmatter: frontmatter
            .trim_start_matches("---\n")
            .trim_end_matches("---\n")
            .to_string(),
        body,
    }
}

/// Render the empty `RESEARCH.md` skeleton that [`crate::manager::ResearchManager::create`]
/// writes before any gathering has run (FR-005). All sections are present
/// in the placeholder form so the file is well-formed from the moment it
/// lands on disk.
pub fn render_skeleton(name: &ResearchName, title: &str, topic: &str) -> String {
    let placeholder = ResearchItem::new(name.clone(), title, topic);
    let doc = ResearchDocument {
        item: placeholder,
        summary: String::new(),
        findings: Vec::new(),
        cross_references: Vec::new(),
        open_questions: Vec::new(),
        template_body: None,
    };
    assemble_document(&doc).content
}

/// Apply the FR-020 template substitution to `template_body`.
///
/// Recognised placeholders:
///
/// - `{{title}}` — the research item title.
/// - `{{topic}}` — the topic description.
/// - `{{date}}` — the current UTC date (`YYYY-MM-DD`).
/// - `{{name}}` — the research name.
///
/// Unknown placeholders are left untouched so authors can use other
/// `{{var}}` syntax (e.g. inside a code fence) without surprises.
pub fn apply_template(template: &str, title: &str, topic: &str) -> String {
    let date = Utc::now().format("%Y-%m-%d").to_string();
    template
        .replace("{{title}}", title)
        .replace("{{topic}}", topic)
        .replace("{{date}}", &date)
        // {{name}} is substituted only when present in the template — we
        // don't have the name here, so we use a placeholder-friendly default
        // that the caller can replace after the fact if needed.
        .replace("{{name}}", "")
}

/// Escape `|` so the value doesn't break a markdown table row (NFR-005).
fn escape_pipe(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '|' => out.push_str(r"\|"),
            '\n' | '\r' => out.push(' '),
            _ => out.push(ch),
        }
    }
    out
}

/// Truncate a source body to [`MAX_SOURCE_BODY_BYTES`] if necessary,
/// returning a markdown-safe fenced version safe to embed in a supporting
/// file (NFR-006).
pub fn fence_source_body(body: &str) -> String {
    let trimmed = body.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() <= MAX_SOURCE_BODY_BYTES {
        return trimmed.to_string();
    }
    // Find a UTF-8 char boundary at or before the limit so we don't slice
    // through a multi-byte sequence.
    let mut cut = MAX_SOURCE_BODY_BYTES;
    while cut > 0 && !trimmed.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut out = String::with_capacity(cut + 64);
    out.push_str(&trimmed[..cut]);
    out.push_str("\n\n… _(truncated — body exceeded the per-source size cap)_\n");
    out
}

/// Build the on-disk content of a numbered supporting file for a captured
/// `Source`. The format is intentionally simple: a YAML-ish header so the
/// file is self-describing, followed by the fenced body.
///
/// Returns `None` when the variant has no body content to write (currently
/// just `Source::Spec`, which points at the spec directory itself rather
/// than capturing an excerpt).
///
/// When the captured `body` is empty (e.g. older research items loaded from
/// disk that predate the body field, or a fetch that returned no text) we
/// emit a clearly-marked placeholder so the file is still self-describing.
pub fn render_supporting_file(source: &Source) -> Option<String> {
    match source {
        Source::Web {
            url,
            title,
            captured_at,
            body,
            ..
        } => Some(format!(
            "# Web source\n\n\
             - URL: {url}\n\
             - Title: {title}\n\
             - Captured (UTC): {captured}\n\n\
             ```text\n{body}\n```\n",
            url = url,
            title = title,
            captured = captured_at.to_rfc3339(),
            body = if body.is_empty() {
                "(no body captured for this source)"
            } else {
                body.as_str()
            },
        )),
        Source::Local {
            path,
            kind,
            captured_at,
            relevance,
            body,
            ..
        } => {
            let kind_label = match kind {
                LocalSourceKind::InProject => "in-project",
                LocalSourceKind::Extra => "extra (--sources-dir)",
            };
            Some(format!(
                "# Local source ({kind_label})\n\n\
                 - Path: {path}\n\
                 - Relevance: {relevance}\n\
                 - Captured (UTC): {captured}\n\n\
                 ```text\n{body}\n```\n",
                path = path,
                relevance = relevance,
                captured = captured_at.to_rfc3339(),
                body = if body.is_empty() {
                    "(no excerpt captured — file could not be read)"
                } else {
                    body.as_str()
                },
            ))
        }
        Source::Spec { .. } => None,
        Source::Other {
            label,
            captured_at,
            body,
            ..
        } => Some(format!(
            "# Other source\n\n\
             - Label: {label}\n\
             - Captured (UTC): {captured}\n\n\
             ```text\n{body}\n```\n",
            label = label,
            captured = captured_at.to_rfc3339(),
            body = if body.is_empty() {
                "(no body captured for this source)"
            } else {
                body.as_str()
            },
        )),
    }
}

/// Helper for the manager: bump an item's status to `InProgress` once
/// gathering starts (FR-013).
pub fn mark_in_progress(item: &mut ResearchItem) {
    if item.status != ResearchStatus::Archived {
        item.set_status(ResearchStatus::InProgress);
    }
}

/// Helper for the manager: mark an item as `Complete` after a successful
/// write of `RESEARCH.md` (FR-013).
pub fn mark_complete(item: &mut ResearchItem) {
    item.set_status(ResearchStatus::Complete);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::Source;
    use std::path::PathBuf;

    fn sample_name() -> ResearchName {
        ResearchName::new("rust-async").expect("name must validate")
    }

    fn sample_item() -> ResearchItem {
        ResearchItem::new(sample_name(), "Rust Async Patterns", "async/await idioms")
    }

    #[test]
    fn assemble_document_includes_all_eight_sections() {
        let doc = ResearchDocument {
            item: sample_item(),
            summary: String::new(),
            findings: Vec::new(),
            cross_references: Vec::new(),
            open_questions: Vec::new(),
            template_body: None,
        };
        let assembled = assemble_document(&doc);
        for section in REQUIRED_SECTIONS {
            assert!(
                assembled.body.contains(&format!("## {section}")),
                "missing required section `{section}` in assembled document:\n{}",
                assembled.body
            );
        }
        // The Title heading is rendered as an H1 (`# Title: ...`) rather than
        // an H2, so it isn't part of REQUIRED_SECTIONS but must still be
        // present.
        assert!(
            assembled.body.contains("# Title:"),
            "missing H1 Title heading"
        );
    }

    #[test]
    fn assemble_document_starts_with_frontmatter_block() {
        let doc = ResearchDocument {
            item: sample_item(),
            summary: String::new(),
            findings: Vec::new(),
            cross_references: Vec::new(),
            open_questions: Vec::new(),
            template_body: None,
        };
        let assembled = assemble_document(&doc);
        assert!(assembled.content.starts_with("---\n"));
        assert!(assembled.content.contains("name: rust-async"));
        assert!(assembled.content.contains("status: draft"));
    }

    #[test]
    fn assemble_document_emits_one_finding_block_per_entry() {
        let doc = ResearchDocument {
            item: sample_item(),
            summary: String::new(),
            findings: vec!["first finding".into(), "second finding".into()],
            cross_references: Vec::new(),
            open_questions: Vec::new(),
            template_body: None,
        };
        let assembled = assemble_document(&doc);
        assert!(assembled.body.contains("### Finding 1\n\nfirst finding"));
        assert!(assembled.body.contains("### Finding 2\n\nsecond finding"));
    }

    #[test]
    fn assemble_document_renders_cross_reference_table() {
        let doc = ResearchDocument {
            item: sample_item(),
            summary: String::new(),
            findings: Vec::new(),
            cross_references: vec![CrossReference {
                path: "src/lib.rs".into(),
                relevance: "Main library entry".into(),
            }],
            open_questions: Vec::new(),
            template_body: None,
        };
        let assembled = assemble_document(&doc);
        assert!(assembled.body.contains("| Path | Relevance |"));
        assert!(
            assembled
                .body
                .contains("| `src/lib.rs` | Main library entry |")
        );
    }

    #[test]
    fn assemble_document_escapes_pipes_in_cross_reference_relevance() {
        let doc = ResearchDocument {
            item: sample_item(),
            summary: String::new(),
            findings: Vec::new(),
            cross_references: vec![CrossReference {
                path: "src/lib.rs".into(),
                relevance: "Has | pipes".into(),
            }],
            open_questions: Vec::new(),
            template_body: None,
        };
        let assembled = assemble_document(&doc);
        assert!(
            assembled.body.contains(r"Has \| pipes"),
            "expected escaped pipe in: {}",
            assembled.body
        );
    }

    #[test]
    fn assemble_document_preserves_inline_citation_markers() {
        let doc = ResearchDocument {
            item: sample_item(),
            summary: String::new(),
            findings: vec!["Use Tokio [#1] for async runtimes.".into()],
            cross_references: Vec::new(),
            open_questions: Vec::new(),
            template_body: None,
        };
        let assembled = assemble_document(&doc);
        assert!(assembled.body.contains("[#1]"));
    }

    #[test]
    fn render_skeleton_produces_well_formed_document() {
        let skeleton = render_skeleton(&sample_name(), "Rust Async", "topic");
        assert!(skeleton.starts_with("---\n"));
        assert!(skeleton.contains("status: draft"));
        assert!(skeleton.contains("## Topic"));
        assert!(skeleton.contains("## References Index"));
    }

    #[test]
    fn template_substitution_replaces_known_placeholders() {
        let tmpl = "# {{title}}\n\nTopic: {{topic}}\nDate: {{date}}\n";
        let out = apply_template(tmpl, "Title", "Topic");
        assert!(out.contains("# Title\n"));
        assert!(out.contains("Topic: Topic"));
        assert!(out.contains("Date: 20"));
    }

    #[test]
    fn template_substitution_leaves_unknown_placeholders_alone() {
        let tmpl = "Hello {{name}}, unknown {{foo}}";
        let out = apply_template(tmpl, "Title", "Topic");
        assert!(out.contains("Hello , unknown {{foo}}"));
    }

    #[test]
    fn fence_source_body_truncates_oversize_input() {
        let huge = "x".repeat(MAX_SOURCE_BODY_BYTES + 1024);
        let fenced = fence_source_body(&huge);
        assert!(fenced.len() < huge.len());
        assert!(fenced.contains("truncated"));
    }

    #[test]
    fn fence_source_body_preserves_small_input() {
        let small = "hello world";
        let fenced = fence_source_body(small);
        assert_eq!(fenced, small);
    }

    #[test]
    fn render_supporting_file_returns_none_for_spec() {
        let source = Source::Spec {
            spec_id: "foo".into(),
            captured_at: Utc::now(),
            relevance: "Related".into(),
        };
        assert!(render_supporting_file(&source).is_none());
    }

    #[test]
    fn render_supporting_file_produces_web_block() {
        let source = Source::Web {
            url: "https://example.com".into(),
            title: "Example".into(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/web-01.md"),
            body: "page body content".into(),
        };
        let out = render_supporting_file(&source).expect("web must produce a body");
        assert!(out.contains("# Web source"));
        assert!(out.contains("URL: https://example.com"));
        assert!(out.contains("page body content"));
    }

    #[test]
    fn render_supporting_file_produces_web_placeholder_when_body_empty() {
        let source = Source::Web {
            url: "https://example.com".into(),
            title: "Example".into(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/web-01.md"),
            body: String::new(),
        };
        let out = render_supporting_file(&source).expect("web must produce a body");
        assert!(out.contains("no body captured"));
    }

    #[test]
    fn render_supporting_file_produces_local_block_for_extra() {
        let source = Source::Local {
            path: "notes/extra.md".into(),
            kind: LocalSourceKind::Extra,
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/local-01.md"),
            relevance: "External notes".into(),
            body: "excerpt text".into(),
        };
        let out = render_supporting_file(&source).expect("local must produce a body");
        assert!(out.contains("# Local source (extra (--sources-dir))"));
        assert!(out.contains("Path: notes/extra.md"));
        assert!(out.contains("excerpt text"));
    }

    #[test]
    fn render_supporting_file_produces_local_placeholder_when_body_empty() {
        let source = Source::Local {
            path: "missing.md".into(),
            kind: LocalSourceKind::InProject,
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/local-01.md"),
            relevance: "could not read".into(),
            body: String::new(),
        };
        let out = render_supporting_file(&source).expect("local must produce a body");
        assert!(out.contains("no excerpt captured"));
    }

    #[test]
    fn mark_in_progress_only_when_not_archived() {
        let mut item = sample_item();
        mark_in_progress(&mut item);
        assert_eq!(item.status, ResearchStatus::InProgress);

        item.set_status(ResearchStatus::Archived);
        mark_in_progress(&mut item);
        // Archived is a terminal state — gathering cannot restart it.
        assert_eq!(item.status, ResearchStatus::Archived);
    }

    #[test]
    fn mark_complete_overrides_in_progress() {
        let mut item = sample_item();
        mark_in_progress(&mut item);
        mark_complete(&mut item);
        assert_eq!(item.status, ResearchStatus::Complete);
    }
}
