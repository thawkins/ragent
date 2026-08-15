//! RESEARCH.md document assembly — legacy report layout and `IMRaD` layout.
//!
//! `RESEARCH.md` is the single, self-contained deliverable for each
//! research item. Two layouts are supported:
//!
//! 1. **Report** (default) — the original multi-section layout:
//!
//!    ```text
//!    # Title: <title>
//!
//!    ## Topic
//!    ## Search Queries
//!    ### Search Engine Summary   (after gathering, per-engine source counts)
//!    ## Executive Summary
//!    ## Top 5 Implications
//!    ## Findings
//!    ## Findings Relationship Diagram
//!    ## In-Project Cross-References
//!    ## Open Questions
//!    ## References Index
//!    ```
//!
//! 2. **`IMRaD`** — selected via [`OutputFormat::Imrad`](crate::run_config::OutputFormat);
//!    restructures the same content into the scientific/technical report
//!    convention (Abstract, Introduction, Methods, Results, Discussion,
//!    References Index) while preserving all existing finding paragraphs,
//!    the relationship diagram, cross-references, open questions, and
//!    references index (specs/imradreport).
//!
//! In both layouts all sections are always present (even if empty) so a
//! downstream tool that reads `RESEARCH.md` can rely on a stable structure.

use crate::contradiction::ContradictionGraph;
use crate::digest::{EvidenceDigest, TripleDraft};
use crate::io::ResearchIo;
use crate::item::{ResearchItem, strip_control_chars};
use crate::locus::{DepthInvestigation, LocusSet};
use crate::reconcile::{CrossLocusReconcile, SourceTensions};
use crate::research_name::ResearchName;
use crate::source::{LocalSourceKind, Source};
use crate::status::ResearchStatus;
use crate::synthesis::SynthesisAudit;
use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Maximum number of bytes allowed in a single untrusted source excerpt.
/// Sources larger than this are truncated to avoid blowing up RESEARCH.md
/// (NFR-006 + the size-cap risk in the PLAN.md Risks table).
pub const MAX_SOURCE_BODY_BYTES: usize = 256 * 1024;

/// The 10 sections that appear in every `RESEARCH.md`, in order (FR-010 + FR-012).
pub const REQUIRED_SECTIONS: &[&str] = &[
    "Topic",
    "Search Queries",
    "Executive Summary",
    "Top 5 Implications",
    "Findings",
    "Findings Relationship Diagram",
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
    /// Rendered under `## Executive Summary` in the report layout. In the
    /// IMRaD layout the same text is rendered under `## Abstract`.
    pub summary: String,
    /// Numbered findings — each entry is the body of one bullet under
    /// `## Findings`. References inside the body use the form `[#N]`.
    pub findings: Vec<String>,
    /// Top 5 implications — one numbered entry per implication, in rank order.
    /// In the report layout this section now appears directly under
    /// `## Executive Summary`; in the IMRaD layout it is rendered under
    /// `## Discussion`.
    pub top_implications: Vec<String>,
    /// In-project cross-references (FR-009). Each entry is one bullet under
    /// `## In-Project Cross-References`.
    pub cross_references: Vec<CrossReference>,
    /// Open questions — one bullet per question.
    pub open_questions: Vec<String>,
    /// Optional contradiction graph produced by the full / dissertation
    /// pipeline (FR-005, T-007). When `None` the report layout omits the
    /// contradiction section entirely; an empty graph is rendered with a
    /// placeholder so the section is still present for full-tier runs.
    pub contradiction_graph: Option<ContradictionGraph>,
    /// Optional loci set produced by the full / dissertation pipeline
    /// (FR-005, T-008). When `None` the report layout omits the loci section.
    pub loci: Option<LocusSet>,
    /// Optional depth investigation produced by the full / dissertation
    /// pipeline (FR-005, T-008). When `None` the report layout omits the depth
    /// section.
    pub depth_investigation: Option<Vec<DepthInvestigation>>,
    /// Optional evidence digest produced by the full / dissertation pipeline
    /// (FR-005, T-011). When `None` the report layout omits the evidence digest
    /// section.
    pub evidence_digest: Option<EvidenceDigest>,
    /// Optional triple draft produced by the full / dissertation pipeline
    /// (FR-005, T-011). When `None` the report layout omits the triple draft
    /// section.
    pub triple_draft: Option<TripleDraft>,
    /// Optional cross-locus reconciliation produced by the full / dissertation
    /// pipeline (FR-005, T-009). When `None` the report layout omits the reconcile
    /// section.
    pub cross_locus_reconcile: Option<CrossLocusReconcile>,
    /// Optional source-tensions list produced by the full / dissertation
    /// pipeline (FR-005, T-009). When `None` the report layout omits the source
    /// tensions section.
    pub source_tensions: Option<SourceTensions>,
    /// Optional synthesis audit produced by the full / dissertation pipeline
    /// (FR-005, T-012). When `None` the report layout omits the synthesis audit
    /// section.
    pub synthesis_audit: Option<SynthesisAudit>,
    /// Optional corpus-critic report produced by the full / dissertation
    /// pipeline (FR-005, T-010). When `None` the report layout omits the corpus
    /// critic section.
    pub corpus_critic: Option<crate::corpus_critic::CorpusCriticReport>,
    /// Optional gap-fill fetch result produced by the full / dissertation
    /// pipeline (FR-005, T-010). When `None` the report layout omits the gap-fill
    /// section.
    pub gap_fetch: Option<crate::corpus_critic::GapFetchResult>,
    /// Optional surgical patch result produced by the full / dissertation
    /// pipeline (FR-005, T-013). When `None` the report layout omits the
    /// surgical patch section.
    pub surgical_patch: Option<crate::patcher::PatchResult>,
    /// Optional cite-check result produced by the full / dissertation pipeline
    /// (FR-005, T-014). When `None` the report layout omits the citation
    /// check section.
    pub cite_check: Option<crate::cite_checker::CitationCheckResult>,
    /// Optional polish result produced by the final polish step (FR-005, T-015).
    /// When `None` the report layout omits the polish section.
    pub polish: Option<crate::readability::PolishResult>,
    /// Optional readability audit produced by the final audit step (FR-005, T-015).
    /// When `None` the report layout omits the readability audit section.
    pub readability_audit: Option<crate::readability::ReadabilityAudit>,
    /// Optional template body loaded from `research/_templates/<name>.md`
    /// (FR-020). When supplied, the template is used as the skeleton and
    /// `{{title}}`, `{{topic}}`, `{{date}}` placeholders are substituted
    /// from the item's metadata before the standard sections are appended.
    pub template_body: Option<String>,
    /// Sub-queries the web-gathering phase issued to the search tool. Empty
    /// when web gathering was disabled or no decomposer was configured.
    pub decomposed_queries: Vec<String>,
    /// Output artifact this document was requested as.
    pub output_format: crate::run_config::OutputFormat,
}

/// One in-project cross-reference row (FR-009).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossReference {
    /// Project-relative path (e.g. `"src/lib.rs"`).
    pub path: String,
    /// One-line note explaining why this file is relevant.
    pub relevance: String,
}

/// Result of `assemble_document` — the body text plus the rendered file
/// payload (frontmatter + body) ready for `atomic_write`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssembledDocument {
    /// Full `RESEARCH.md` payload (frontmatter + body).
    pub content: String,
    /// Just the frontmatter block (without the leading/trailing `---`).
    pub frontmatter: String,
    /// Just the body text (without the frontmatter).
    pub body: String,
}

/// Extract the headline for a finding and return the finding body with the
/// Headline paragraph removed.
///
/// If the finding contains a `**Headline:**` paragraph, its body is used as the
/// headline. Otherwise a fallback headline is derived from the first 15 words
/// of the `**Observation:**` paragraph (or the first sentence if shorter). The
/// returned headline is trimmed and never empty — it falls back to
/// "Finding {n}" when nothing else is available.
fn extract_headline(finding: &str, finding_number: usize) -> (String, String) {
    const LABEL: &str = "**Headline:**";
    let mut remainder = finding.to_string();
    let headline = if let Some(start) = finding.find(LABEL) {
        let after_label = &finding[start + LABEL.len()..];
        let (body, after_headline) = if let Some(next_pos) = after_label.find("\n\n**") {
            (
                &after_label[..next_pos],
                // Skip the blank line that separates Headline from the next label.
                &finding[start + LABEL.len() + next_pos + 2..],
            )
        } else {
            (after_label, "")
        };
        let extracted = body.trim().to_string();
        // Preserve any text that appeared before the Headline label.
        remainder = format!("{}{}", &finding[..start], after_headline)
            .trim()
            .to_string();
        if extracted.is_empty() {
            None
        } else {
            Some(extracted)
        }
    } else {
        None
    };
    let headline =
        headline.unwrap_or_else(|| derive_headline_from_observation(finding, finding_number));
    (headline, remainder)
}

/// Derive a short headline from the **Observation:** paragraph.
///
/// The derivation strips citations and backticks, takes the first 15 words,
/// and trims trailing punctuation. If there is no Observation paragraph, the
/// entire finding body is used as a last resort.
pub fn make_headline_from_observation(observation: &str) -> String {
    let cleaned = observation
        .replace("[#", " ")
        .replace([']', '`'], " ")
        .replace("**", " ");
    let words: Vec<&str> = cleaned.split_whitespace().take(15).collect();
    if words.is_empty() {
        return String::from("(no headline available)");
    }
    words
        .join(" ")
        .trim_end_matches(|c: char| c.is_ascii_punctuation())
        .to_string()
}

fn derive_headline_from_observation(finding: &str, finding_number: usize) -> String {
    let observation_body = finding.find("**Observation:**").map_or(finding, |start| {
        let after = &finding[start + "**Observation:**".len()..];
        let end = after.find("\n\n**").unwrap_or(after.len());
        &after[..end]
    });
    let headline = make_headline_from_observation(observation_body);
    if headline.is_empty() || headline == "(no headline available)" {
        return format!("Finding {finding_number}");
    }
    headline
}

/// Assemble a `RESEARCH.md` payload from a populated `ResearchDocument`.
///
/// The returned [`AssembledDocument`] always contains the YAML frontmatter,
/// the `# Title:` line, and a body whose section order depends on
/// `doc.output_format`:
///
/// * [`OutputFormat::Report`](crate::run_config::OutputFormat::Report)
///   (default) and all other existing formats emit the legacy multi-section
///   layout: Topic, Search Queries, Executive Summary, Top 5 Implications,
///   Findings, Findings Relationship Diagram, In-Project Cross-References,
///   Open Questions, References Index.
/// * [`OutputFormat::Imrad`](crate::run_config::OutputFormat::Imrad) emits
///   the `IMRaD` layout required by specs/imradreport: Abstract, Introduction,
///   Methods, Results, Discussion, References Index. The same `summary`,
///   `findings`, `cross_references`, `open_questions`, and `sources` fields feed
///   the corresponding sections, and the findings relationship diagram is
///   rendered as a sub-section of Results.
///
/// Empty sections always render a placeholder so the file structure is stable
/// for downstream tooling.
#[must_use]
pub fn assemble_document(doc: &ResearchDocument) -> AssembledDocument {
    let frontmatter = doc.item.render_frontmatter();
    let title = strip_control_chars(&doc.item.title);
    let topic = strip_control_chars(&doc.item.topic);

    let mut body = String::new();

    // FR-020 / imradreport: if a template body was supplied, use it as the
    // skeleton after substituting the standard placeholders.
    if let Some(template) = &doc.template_body {
        body.push_str(&apply_template(template, &title, &topic));
        body.push_str("\n\n");
    }

    // -- Title -----------------------------------------------------------
    body.push_str(&format!("# Title: {title}\n\n"));

    // FR-004 / specs/imradreport: choose between the legacy report layout and
    // the IMRaD layout based on the configured output format.
    if doc.output_format == crate::run_config::OutputFormat::Imrad {
        body.push_str(&assemble_imrad_body(doc, &topic));
    } else {
        body.push_str(&assemble_report_body(doc, &topic));
    }

    // Make every bare URL in the rendered body clickable, while leaving URLs
    // inside code spans, fenced blocks, and existing Markdown links untouched.
    let body = linkify_urls(&body);

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

/// Build the body of a legacy multi-section `RESEARCH.md` report.
///
/// This preserves the original section order: Topic, Search Queries, Executive
/// Summary, Top 5 Implications, Findings, Findings Relationship Diagram,
/// In-Project Cross-References, Open Questions, References Index.
/// Render the synthesis audit as a concise markdown section.
fn render_synthesis_audit(audit: &SynthesisAudit) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "**Overall score:** {}/100\n\n",
        audit.overall_score
    ));
    out.push_str(&format!(
        "**Recommendation:** {}\n\n",
        escape_pipe(&audit.recommendation)
    ));
    if !audit.summary.is_empty() {
        out.push_str(&format!(
            "{}\n\n",
            strip_control_chars(&audit.summary).trim()
        ));
    }
    if audit.critic_reports.is_empty() {
        out.push_str("_(no critic reports available)_\n\n");
        return out;
    }
    out.push_str("| Critic | Score | Status | Issue / Gap Summary |\n");
    out.push_str("|--------|-------|--------|---------------------|\n");
    for report in &audit.critic_reports {
        let status = if report.passed { "pass" } else { "review" };
        let summary = if report.issues.is_empty() {
            "none".to_string()
        } else {
            escape_pipe(&report.issues[0])
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            escape_pipe(&report.name),
            report.score,
            status,
            summary
        ));
    }
    out.push('\n');
    out
}

/// Render the evidence-digest section as a markdown table.
fn render_evidence_digest(digest: &EvidenceDigest) -> String {
    let mut out = String::new();
    if digest.claims.is_empty() {
        out.push_str("_(no evidence digest available)_\n\n");
        return out;
    }
    out.push_str("| Claim | Support | Contested | Note |\n");
    out.push_str("|-------|---------|-----------|------|\n");
    for claim in &digest.claims {
        let sources = claim
            .source_indices
            .iter()
            .map(|i| format!("#{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let contested = if claim.contested { "yes" } else { "no" };
        out.push_str(&format!(
            "| {} | {} ({}) | {} | {} |\n",
            escape_pipe(&claim.text),
            escape_pipe(&sources),
            claim.support_count,
            contested,
            escape_pipe(&claim.note)
        ));
    }
    out.push('\n');
    out
}

/// Render the triple-draft section as three labelled paragraphs.
fn render_triple_draft(draft: &TripleDraft) -> String {
    let mut out = String::new();
    if draft.candidates.is_empty() {
        out.push_str("_(no triple draft available)_\n\n");
        return out;
    }
    for candidate in &draft.candidates {
        out.push_str(&format!(
            "### Draft {} — {}\n\n{}\n\n*Sources: {}*\n\n",
            candidate.label,
            escape_pipe(&candidate.note),
            strip_control_chars(&candidate.body).trim(),
            candidate
                .source_indices
                .iter()
                .map(|i| format!("#{i}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    out
}

/// Render the cross-locus reconcile section as a markdown table.
fn render_cross_locus_reconcile(reconcile: &CrossLocusReconcile) -> String {
    let mut out = String::new();
    if reconcile.pairs.is_empty() {
        out.push_str("_(no cross-locus reconciliation available)_\n\n");
        return out;
    }
    out.push_str("| Locus A | Locus B | Shared Sources | Conflicts | Note |\n");
    out.push_str("|---------|---------|----------------|-----------|------|\n");
    for pair in &reconcile.pairs {
        let shared = pair
            .shared_source_indices
            .iter()
            .map(|i| format!("#{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            escape_pipe(&pair.locus_a),
            escape_pipe(&pair.locus_b),
            escape_pipe(&shared),
            pair.conflicting_edges,
            escape_pipe(&pair.note)
        ));
    }
    out.push('\n');
    out
}

/// Render the source-tensions section as a markdown table.
fn render_source_tensions(tensions: &SourceTensions) -> String {
    let mut out = String::new();
    if tensions.tensions.is_empty() {
        out.push_str("_(no source tensions detected)_\n\n");
        return out;
    }
    out.push_str("| Kind | Label | Sources | Note |\n");
    out.push_str("|------|-------|---------|------|\n");
    for t in &tensions.tensions {
        let sources = t
            .source_indices
            .iter()
            .map(|i| format!("#{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            escape_pipe(t.kind.as_str()),
            escape_pipe(&t.label),
            escape_pipe(&sources),
            escape_pipe(&t.note)
        ));
    }
    out.push('\n');
    out
}

/// Render the corpus-critic section as a markdown summary.
fn render_corpus_critic(report: &crate::corpus_critic::CorpusCriticReport) -> String {
    let mut out = String::new();
    let status = if report.passed { "pass" } else { "review" };
    out.push_str(&format!(
        "**Overall score:** {}/100 ({})\n\n",
        report.score, status
    ));
    out.push_str(&format!(
        "**Subscores:** coverage {} | evidence {} | balance {} | tension {}\n\n",
        report.coverage_score, report.evidence_score, report.balance_score, report.tension_score
    ));
    if !report.issues.is_empty() {
        out.push_str("**Issues:**\n");
        for issue in &report.issues {
            out.push_str(&format!("- {}\n", escape_pipe(issue)));
        }
        out.push('\n');
    }
    if !report.gaps.is_empty() {
        out.push_str("**Evidence gaps:**\n");
        for gap in &report.gaps {
            out.push_str(&format!("- {}\n", escape_pipe(gap)));
        }
        out.push('\n');
    }
    if !report.recommendations.is_empty() {
        out.push_str("**Recommendations:**\n");
        for rec in &report.recommendations {
            out.push_str(&format!("- {}\n", escape_pipe(rec)));
        }
        out.push('\n');
    }
    if !report.shallow_dimensions.is_empty() {
        out.push_str(&format!(
            "**Shallow dimensions:** {}\n\n",
            escape_pipe(&report.shallow_dimensions.join(", "))
        ));
    }
    if !report.isolated_sources.is_empty() {
        let indices: Vec<String> = report
            .isolated_sources
            .iter()
            .map(|i| format!("#{i}"))
            .collect();
        out.push_str(&format!(
            "**Isolated sources:** {}\n\n",
            escape_pipe(&indices.join(", "))
        ));
    }
    out
}

/// Render the gap-fill fetch section as a markdown summary.
fn render_gap_fetch(result: &crate::corpus_critic::GapFetchResult) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "**Attempted:** {}\n\n",
        if result.attempted { "yes" } else { "no" }
    ));
    out.push_str(&format!(
        "**New sources captured:** {}\n\n",
        result.new_sources
    ));
    if !result.queries.is_empty() {
        out.push_str("**Gap-fill queries:**\n");
        for q in &result.queries {
            out.push_str(&format!("- {}\n", escape_pipe(q)));
        }
        out.push('\n');
    }
    if !result.note.is_empty() {
        out.push_str(&format!("**Note:** {}\n\n", escape_pipe(&result.note)));
    }
    out
}

fn render_citation_check(result: &crate::cite_checker::CitationCheckResult) -> String {
    let mut out = String::new();
    let failed = if result.passed {
        0
    } else {
        result.failed_claims.len()
    };
    let passed = result.checked.saturating_sub(failed);
    out.push_str(&format!(
        "**Summary:** {} citation(s) checked, {} passed, {} failed; gate {}.\n\n",
        result.checked,
        passed,
        failed,
        if result.gate_open { "open" } else { "closed" }
    ));
    out.push_str(&format!(
        "**Result:** {} ({} citation(s) checked)\n\n",
        if result.passed {
            "pass"
        } else {
            "CITATION_VERIFICATION_FAILED"
        },
        result.checked
    ));
    if !result.issues.is_empty() {
        out.push_str("**Issues:**\n");
        for issue in &result.issues {
            out.push_str(&format!("- {}\n", escape_pipe(issue)));
        }
        out.push('\n');
    }
    if !result.failed_claims.is_empty() {
        out.push_str("**Failed claims:**\n");
        for claim in &result.failed_claims {
            out.push_str(&format!("- {}\n", escape_pipe(claim)));
        }
        out.push('\n');
    }
    out.push_str(&format!(
        "**Gate:** {}\n\n",
        if result.gate_open {
            "open — report may ship"
        } else {
            "closed — human approval required"
        }
    ));
    out
}

/// Render the polish section as a markdown summary.
fn render_polish(result: &crate::readability::PolishResult) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "**Changes:** {} control character(s) removed, {} whitespace run(s) normalized, {} empty paragraph(s) removed.\n\n",
        result.control_chars_removed,
        result.whitespace_normalized,
        result.empty_paragraphs_removed
    ));
    out.push_str(&format!("**Note:** {}\n\n", escape_pipe(&result.note)));
    if result.changes.is_empty() {
        out.push_str("_(no polish changes applied)_\n\n");
        return out;
    }
    out.push_str("**Applied changes:**\n");
    for change in &result.changes {
        out.push_str(&format!(
            "- **{}:** {}\n",
            escape_pipe(&change.field),
            escape_pipe(&change.description)
        ));
    }
    out.push('\n');
    out
}

/// Render the readability audit section as a markdown summary.
fn render_readability_audit(audit: &crate::readability::ReadabilityAudit) -> String {
    let mut out = String::new();
    let status = if audit.passed { "pass" } else { "review" };
    out.push_str(&format!("**Score:** {}/100 ({})\n\n", audit.score, status));
    out.push_str(&format!(
        "**Metrics:** average finding length {} characters, {} missing label(s), {} long paragraph(s)\n\n",
        audit.avg_finding_length,
        audit.missing_label_count,
        audit.long_paragraph_count
    ));
    if !audit.issues.is_empty() {
        out.push_str("**Issues:**\n");
        for issue in &audit.issues {
            out.push_str(&format!("- {}\n", escape_pipe(issue)));
        }
        out.push('\n');
    }
    if !audit.recommendations.is_empty() {
        out.push_str("**Recommendations:**\n");
        for rec in &audit.recommendations {
            out.push_str(&format!("- {}\n", escape_pipe(rec)));
        }
        out.push('\n');
    }
    out
}

/// Render the surgical-patch section as a markdown summary.
fn render_surgical_patch(result: &crate::patcher::PatchResult) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "**Score estimate:** {} → {}\n\n",
        result.score_before, result.score_after
    ));
    out.push_str(&format!("**Note:** {}\n\n", escape_pipe(&result.note)));
    if result.patches.is_empty() {
        out.push_str("_(no surgical patches applied)_\n\n");
        return out;
    }
    out.push_str("**Patches:**\n");
    out.push_str("| Operation | Target | Reason | Applied |\n");
    out.push_str("|-----------|--------|--------|----------|\n");
    for patch in &result.patches {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            escape_pipe(&patch.operation),
            escape_pipe(&patch.target),
            escape_pipe(&patch.reason),
            if patch.applied { "yes" } else { "no" }
        ));
    }
    out.push('\n');
    out.push_str(&format!(
        "**Patched draft:** {} finding(s), {} implication(s), {} open question(s).\n\n",
        result.patched_finding_count,
        result.patched_implication_count,
        result.patched_open_question_count
    ));
    out
}

fn assemble_report_body(doc: &ResearchDocument, topic: &str) -> String {
    let mut body = String::new();

    // -- Topic -----------------------------------------------------------
    body.push_str("## Topic\n\n");
    body.push_str(topic.trim());
    body.push_str("\n\n");

    // ── Search Queries ──────────────────────────────────────────────────
    body.push_str("## Search Queries\n\n");
    if doc.decomposed_queries.is_empty() {
        body.push_str(
            "_(no query decomposition was used — the original topic was searched as a single query)_\n\n",
        );
    } else {
        for q in &doc.decomposed_queries {
            body.push_str(&format!("- {}\n", strip_control_chars(q).trim()));
        }
        body.push('\n');
    }

    // ── Search Engine Summary ──────────────────────────────────────────
    // Per-engine breakdown of acquired web sources by media type (pages,
    // PDFs, videos). Emitted only when at least one web source carries a
    // non-empty search_engine field, so the section is absent for skeletons
    // and pre-gathering documents.
    let engine_summary = render_search_engine_summary(&doc.item.sources);
    if !engine_summary.is_empty() {
        body.push_str("### Search Engine Summary\n\n");
        body.push_str(&engine_summary);
        body.push('\n');
    }

    // ── Executive Summary ─────────────────────────────────────────────────
    body.push_str("## Executive Summary\n\n");
    if doc.summary.trim().is_empty() {
        body.push_str("_(no executive summary recorded yet — run a gathering pass to populate)_\n");
    } else {
        body.push_str(&strip_control_chars(doc.summary.trim()));
        body.push('\n');
    }
    body.push('\n');

    // ── Top 5 Implications ──────────────────────────────────────────────
    body.push_str("## Top 5 Implications\n\n");
    if doc.top_implications.is_empty() {
        body.push_str(
            "_(no ranked implications yet — the synthesis pass will populate this section)_\n\n",
        );
    } else {
        for (idx, imp) in doc.top_implications.iter().enumerate() {
            let n = idx + 1;
            let cleaned = strip_control_chars(imp).trim().to_string();
            body.push_str(&format!("{n}. {cleaned}\n"));
        }
        body.push('\n');
    }

    // -- Findings ---------------------------------------------------------
    body.push_str("## Findings\n\n");
    if doc.findings.is_empty() {
        body.push_str("_(no findings yet — the gathering pass will populate this section)_\n\n");
    } else {
        for (idx, finding) in doc.findings.iter().enumerate() {
            let n = idx + 1;
            let normalized = normalize_finding_labels(strip_control_chars(finding).trim());
            let (headline, mut remainder) = extract_headline(&normalized, n);
            if let Some(sources_list) = render_finding_sources(&remainder, &doc.item.sources) {
                remainder.push_str("\n\n");
                remainder.push_str(&sources_list);
            }
            body.push_str(&format!(
                "\n### **Finding {n}** — {headline}\n\n{remainder}\n\n"
            ));
        }
    }
    // ── Findings Relationship Diagram (FR-001 / FR-002 / FR-012) ────────────
    body.push_str(&crate::diagram::render_findings_diagram(&doc.findings));

    // ── Contradiction Graph (FR-005, T-007) ─────────────────────────────
    if let Some(graph) = &doc.contradiction_graph {
        body.push_str("## Contradiction Graph\n\n");
        if graph.is_empty() {
            body.push_str("_(no contradictions detected among the gathered sources)_\n\n");
        } else {
            body.push_str("| Pair | Dimension | Strength | Source A | Source B | Note |\n");
            body.push_str("|------|-----------|----------|----------|----------|------|\n");
            for edge in &graph.edges {
                let a = format!(
                    "#{} {}",
                    edge.claim_a.source_index, edge.claim_a.source_path
                );
                let b = format!(
                    "#{} {}",
                    edge.claim_b.source_index, edge.claim_b.source_path
                );
                body.push_str(&format!(
                    "| {} vs {} | {} | {} | {} | {} | {} |\n",
                    edge.claim_a.source_index,
                    edge.claim_b.source_index,
                    edge.dimension,
                    edge.strength,
                    escape_pipe(&a),
                    escape_pipe(&b),
                    escape_pipe(&edge.note)
                ));
            }
            body.push('\n');
        }
    }

    // ── Loci Analysis (FR-005, T-008) ───────────────────────────────────
    if let Some(loci) = &doc.loci {
        body.push_str("## Loci Analysis\n\n");
        if loci.is_empty() {
            body.push_str(
                "_(no recurring research dimensions detected among the gathered sources)_\n\n",
            );
        } else {
            body.push_str("| Locus | Sources | Mentions | Representative Snippets |\n");
            body.push_str("|-------|---------|----------|-------------------------|\n");
            for locus in &loci.loci {
                let indices: Vec<String> = locus
                    .source_indices
                    .iter()
                    .map(|i| format!("#{i}"))
                    .collect();
                let snippets = if locus.snippets.is_empty() {
                    "—".to_string()
                } else {
                    locus
                        .snippets
                        .join("; ")
                        .chars()
                        .take(120)
                        .collect::<String>()
                };
                body.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    escape_pipe(&locus.label),
                    escape_pipe(&indices.join(", ")),
                    locus.mentions,
                    escape_pipe(&snippets)
                ));
            }
            body.push('\n');
        }
    }

    // ── Depth Investigation (FR-005, T-008) ───────────────────────────────
    if let Some(investigations) = &doc.depth_investigation {
        body.push_str("## Depth Investigation\n\n");
        if investigations.is_empty() {
            body.push_str("_(no depth investigation available)_\n\n");
        } else {
            body.push_str("| Locus | Depth | Sources | Note |\n");
            body.push_str("|-------|-------|---------|------|\n");
            for inv in investigations {
                let sources = inv
                    .representative_sources
                    .iter()
                    .map(|i| format!("#{i}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                body.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    escape_pipe(&inv.label),
                    inv.depth.as_str(),
                    escape_pipe(&sources),
                    escape_pipe(&inv.note)
                ));
            }
            body.push('\n');
        }
    }

    // ── Evidence Digest (FR-005, T-011) ───────────────────────────────────
    if let Some(digest) = &doc.evidence_digest {
        body.push_str("## Evidence Digest\n\n");
        body.push_str(&render_evidence_digest(digest));
    }

    // ── Triple Draft (FR-005, T-011) ────────────────────────────────────
    if let Some(draft) = &doc.triple_draft {
        body.push_str("## Triple Draft\n\n");
        body.push_str(&render_triple_draft(draft));
    }

    // ── Cross-Locus Reconcile (FR-005, T-009) ────────────────────────��
    if let Some(reconcile) = &doc.cross_locus_reconcile {
        body.push_str("## Cross-Locus Reconcile\n\n");
        body.push_str(&render_cross_locus_reconcile(reconcile));
    }

    // ── Source Tensions (FR-005, T-009) ─────────────────────────────────
    if let Some(tensions) = &doc.source_tensions {
        body.push_str("## Source Tensions\n\n");
        body.push_str(&render_source_tensions(tensions));
    }

    // ── Synthesis Audit (FR-005, T-012) ─────────────────────────────────
    if let Some(audit) = &doc.synthesis_audit {
        body.push_str("## Synthesis Audit\n\n");
        body.push_str(&render_synthesis_audit(audit));
    }

    // ── Corpus Critic (FR-005, T-010) ─────────────────────────��──────────
    if let Some(report) = &doc.corpus_critic {
        body.push_str("## Corpus Critic\n\n");
        body.push_str(&render_corpus_critic(report));
    }

    // ── Gap-Fill Fetch (FR-005, T-010) ───────────────────────────────────
    if let Some(result) = &doc.gap_fetch {
        body.push_str("## Gap-Fill Fetch\n\n");
        body.push_str(&render_gap_fetch(result));
    }

    // ── Surgical Patch (FR-005, T-013) ─────────────────────────────────
    if let Some(result) = &doc.surgical_patch {
        body.push_str("## Surgical Patch\n\n");
        body.push_str(&render_surgical_patch(result));
    }

    // ── Citation Check (FR-005, T-014) ──────────────────────────���───────
    if let Some(result) = &doc.cite_check {
        body.push_str("## Citation Check\n\n");
        body.push_str(&render_citation_check(result));
    }

    // ── Polish (FR-005, T-015) ───────────────────────────────────────────
    if let Some(result) = &doc.polish {
        body.push_str("## Polish\n\n");
        body.push_str(&render_polish(result));
    }

    // ── Readability Audit (FR-005, T-015) ─────────────────────────────
    if let Some(audit) = &doc.readability_audit {
        body.push_str("## Readability Audit\n\n");
        body.push_str(&render_readability_audit(audit));
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
                escape_pipe(&strip_control_chars(&cr.path)),
                escape_pipe(&strip_control_chars(&cr.relevance)),
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
            body.push_str(&format!("- {}\n", strip_control_chars(q).trim()));
        }
        body.push('\n');
    }

    // ── References Index (FR-011) ────────────────────────────────────────
    body.push_str(&ResearchIo::render_references_index(
        &doc.item.sources,
        Utc::now(),
    ));

    body
}

/// Build the body of an IMRaD-compliant `RESEARCH.md` report.
///
/// Section order follows the `IMRaD` convention: Abstract, Introduction, Methods,
/// Results, Discussion, References Index. The same `ResearchDocument` fields are
/// reused; only the headings and grouping change.
fn assemble_imrad_body(doc: &ResearchDocument, topic: &str) -> String {
    let mut body = String::new();

    // ── Abstract (FR-005) ───────────────────────────────────────────────
    body.push_str("## Abstract\n\n");
    if doc.summary.trim().is_empty() {
        body.push_str(
            "_(no abstract recorded yet — run a gathering pass to populate this section)_\n\n",
        );
    } else {
        body.push_str(&strip_control_chars(doc.summary.trim()));
        body.push_str("\n\n");
    }

    // ── Introduction (FR-006) ─────────────────────────────────────────────
    body.push_str("## Introduction\n\n");
    if topic.trim().is_empty() {
        body.push_str("_(no research topic specified)_\n\n");
    } else {
        body.push_str(strip_control_chars(topic).trim());
        body.push_str("\n\n");
        body.push_str(
            "This research item investigates the topic above by gathering and synthesizing \
             web sources, local project files, and related specifications. The objective is \
             to produce evidence-based findings that can be mapped back to the project \
             context.\n\n",
        );
    }

    // ── Methods (FR-007) ──────────────────────────────────────────────────
    body.push_str("## Methods\n\n");
    body.push_str("### Search Queries\n\n");
    if doc.decomposed_queries.is_empty() {
        body.push_str(
            "_(no query decomposition was used — the original topic was searched as a single query)_\n\n",
        );
    } else {
        for q in &doc.decomposed_queries {
            body.push_str(&format!("- {}\n", strip_control_chars(q).trim()));
        }
        body.push('\n');
    }

    // ── Search Engine Summary (IMRaD Methods sub-section) ─────────────
    // Per-engine breakdown of acquired web sources by media type. Only
    // emitted when at least one web source has a non-empty search_engine.
    let engine_summary = render_search_engine_summary(&doc.item.sources);
    if !engine_summary.is_empty() {
        body.push_str("### Search Engine Summary\n\n");
        body.push_str(&engine_summary);
        body.push('\n');
    }

    body.push_str("### Research Configuration\n\n");
    body.push_str(
        "Evidence was gathered through automated web search and local cross-reference \
                 scanning; the resulting corpus was synthesized into structured findings. \
                 Empty sections below indicate that the corresponding evidence was not yet \
                 produced by the gathering pass.\n\n",
    );

    // -- Results (FR-008) -----------------------------------------------
    body.push_str("## Results\n\n");
    body.push_str("### Findings\n\n");
    if doc.findings.is_empty() {
        body.push_str("_(no findings yet — the gathering pass will populate this section)_\n\n");
    } else {
        for (idx, finding) in doc.findings.iter().enumerate() {
            let n = idx + 1;
            let normalized = normalize_finding_labels(strip_control_chars(finding).trim());
            let (headline, mut remainder) = extract_headline(&normalized, n);
            if let Some(sources_list) = render_finding_sources(&remainder, &doc.item.sources) {
                remainder.push_str("\n\n");
                remainder.push_str(&sources_list);
            }
            body.push_str(&format!(
                "\n### **Finding {n}** — {headline}\n\n{remainder}\n\n"
            ));
        }
    }
    // ── Findings Relationship Diagram (FR-001 / FR-002 / FR-012). In the
    // IMRaD layout it is a sub-section of Results, so we use a `###` heading
    // and ask the diagram renderer to return only the body.
    body.push_str("### Findings Relationship Diagram\n\n");
    body.push_str(&crate::diagram::render_findings_diagram_body(&doc.findings));

    // ── Discussion (FR-009) ────────────────────────────────────────────────
    body.push_str("## Discussion\n\n");

    // ── Contradiction Graph (FR-005, T-007) ─────────────────────────────
    if let Some(graph) = &doc.contradiction_graph {
        body.push_str("### Contradiction Graph\n\n");
        if graph.is_empty() {
            body.push_str("_(no contradictions detected among the gathered sources)_\n\n");
        } else {
            body.push_str("| Pair | Dimension | Strength | Source A | Source B | Note |\n");
            body.push_str("|------|-----------|----------|----------|----------|------|\n");
            for edge in &graph.edges {
                let a = format!(
                    "#{} {}",
                    edge.claim_a.source_index, edge.claim_a.source_path
                );
                let b = format!(
                    "#{} {}",
                    edge.claim_b.source_index, edge.claim_b.source_path
                );
                body.push_str(&format!(
                    "| {} vs {} | {} | {} | {} | {} | {} |\n",
                    edge.claim_a.source_index,
                    edge.claim_b.source_index,
                    edge.dimension,
                    edge.strength,
                    escape_pipe(&a),
                    escape_pipe(&b),
                    escape_pipe(&edge.note)
                ));
            }
            body.push('\n');
        }
    }

    // ── Loci Analysis (FR-005, T-008) ───────────────────────────────────
    if let Some(loci) = &doc.loci {
        body.push_str("### Loci Analysis\n\n");
        if loci.is_empty() {
            body.push_str(
                "_(no recurring research dimensions detected among the gathered sources)_\n\n",
            );
        } else {
            body.push_str("| Locus | Sources | Mentions | Representative Snippets |\n");
            body.push_str("|-------|---------|----------|-------------------------|\n");
            for locus in &loci.loci {
                let indices: Vec<String> = locus
                    .source_indices
                    .iter()
                    .map(|i| format!("#{i}"))
                    .collect();
                let snippets = if locus.snippets.is_empty() {
                    "—".to_string()
                } else {
                    locus
                        .snippets
                        .join("; ")
                        .chars()
                        .take(120)
                        .collect::<String>()
                };
                body.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    escape_pipe(&locus.label),
                    escape_pipe(&indices.join(", ")),
                    locus.mentions,
                    escape_pipe(&snippets)
                ));
            }
            body.push('\n');
        }
    }

    // ── Depth Investigation (FR-005, T-008) ───────────────────────────────
    if let Some(investigations) = &doc.depth_investigation {
        body.push_str("### Depth Investigation\n\n");
        if investigations.is_empty() {
            body.push_str("_(no depth investigation available)_\n\n");
        } else {
            body.push_str("| Locus | Depth | Sources | Note |\n");
            body.push_str("|-------|-------|---------|------|\n");
            for inv in investigations {
                let sources = inv
                    .representative_sources
                    .iter()
                    .map(|i| format!("#{i}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                body.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    escape_pipe(&inv.label),
                    inv.depth.as_str(),
                    escape_pipe(&sources),
                    escape_pipe(&inv.note)
                ));
            }
            body.push('\n');
        }
    }

    // ── Evidence Digest (FR-005, T-011) ────��──────────────────────────────
    if let Some(digest) = &doc.evidence_digest {
        body.push_str("### Evidence Digest\n\n");
        body.push_str(&render_evidence_digest(digest));
    }

    // ── Triple Draft (FR-005, T-011) ────────────────────────────────────
    if let Some(draft) = &doc.triple_draft {
        body.push_str("### Triple Draft\n\n");
        body.push_str(&render_triple_draft(draft));
    }

    // ── Cross-Locus Reconcile (FR-005, T-009) ──────────────────────────
    if let Some(reconcile) = &doc.cross_locus_reconcile {
        body.push_str("### Cross-Locus Reconcile\n\n");
        body.push_str(&render_cross_locus_reconcile(reconcile));
    }

    // ── Source Tensions (FR-005, T-009) ─────────────────────────────────
    if let Some(tensions) = &doc.source_tensions {
        body.push_str("### Source Tensions\n\n");
        body.push_str(&render_source_tensions(tensions));
    }

    // Synthesis Audit (FR-005, T-012)
    if let Some(audit) = &doc.synthesis_audit {
        body.push_str("### Synthesis Audit\n\n");
        body.push_str(&render_synthesis_audit(audit));
    }

    // Corpus Critic (FR-005, T-010)
    if let Some(report) = &doc.corpus_critic {
        body.push_str("### Corpus Critic\n\n");
        body.push_str(&render_corpus_critic(report));
    }

    // Gap-Fill Fetch (FR-005, T-010)
    if let Some(result) = &doc.gap_fetch {
        body.push_str("### Gap-Fill Fetch\n\n");
        body.push_str(&render_gap_fetch(result));
    }

    // Surgical Patch (FR-005, T-013)
    if let Some(result) = &doc.surgical_patch {
        body.push_str("### Surgical Patch\n\n");
        body.push_str(&render_surgical_patch(result));
    }

    // Citation Check (FR-005, T-014)
    if let Some(result) = &doc.cite_check {
        body.push_str("### Citation Check\n\n");
        body.push_str(&render_citation_check(result));
    }

    // Polish (FR-005, T-015)
    if let Some(result) = &doc.polish {
        body.push_str("### Polish\n\n");
        body.push_str(&render_polish(result));
    }

    // Readability Audit (FR-005, T-015)
    if let Some(audit) = &doc.readability_audit {
        body.push_str("### Readability Audit\n\n");
        body.push_str(&render_readability_audit(audit));
    }

    body.push_str("### In-Project Cross-References\n\n");
    if doc.cross_references.is_empty() {
        body.push_str(
            "_(no relevant in-project files were identified during the gathering pass)_\n\n",
        );
    } else {
        body.push_str("| Path | Relevance |\n|------|-----------|\n");
        for cr in &doc.cross_references {
            body.push_str(&format!(
                "| `{}` | {} |\n",
                escape_pipe(&strip_control_chars(&cr.path)),
                escape_pipe(&strip_control_chars(&cr.relevance)),
            ));
        }
        body.push('\n');
    }
    body.push_str("### Open Questions\n\n");
    if doc.open_questions.is_empty() {
        body.push_str("_(none)_\n\n");
    } else {
        for q in &doc.open_questions {
            body.push_str(&format!("- {}\n", strip_control_chars(q).trim()));
        }
        body.push('\n');
    }

    // ── References Index (FR-010) ─────────────────────────────────────────
    body.push_str(&ResearchIo::render_references_index(
        &doc.item.sources,
        Utc::now(),
    ));

    body
}

/// Render the empty `RESEARCH.md` skeleton that [`crate::manager::ResearchManager::create`]
/// writes before any gathering has run (FR-005 / FR-011). All sections are
/// present in the placeholder form so the file is well-formed from the moment
/// it lands on disk.
///
/// The `output_format` argument selects between the legacy report layout and
/// the `IMRaD` layout; callers that do not care should pass
/// [`OutputFormat::Report`].
#[must_use]
pub fn render_skeleton(
    name: &ResearchName,
    title: &str,
    topic: &str,
    output_format: crate::run_config::OutputFormat,
) -> String {
    let mut placeholder = ResearchItem::new(name.clone(), title, topic);
    // Persist non-default formats in the frontmatter (FR-012) so the skeleton
    // records the requested artifact from the moment it is created.
    if output_format != crate::run_config::OutputFormat::Report {
        placeholder.output_format = Some(output_format.as_str().to_string());
    }
    let doc = ResearchDocument {
        item: placeholder,
        summary: String::new(),
        findings: Vec::new(),
        top_implications: Vec::new(),
        cross_references: Vec::new(),
        open_questions: Vec::new(),
        contradiction_graph: None,
        loci: None,
        depth_investigation: None,
        evidence_digest: None,
        triple_draft: None,
        cross_locus_reconcile: None,
        source_tensions: None,
        synthesis_audit: None,
        corpus_critic: None,
        gap_fetch: None,
        surgical_patch: None,
        cite_check: None,
        polish: None,
        readability_audit: None,
        template_body: None,
        decomposed_queries: Vec::new(),
        output_format,
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
#[must_use]
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

/// Extract 1-based source indices from `[#N]` citations in a finding body.
/// Returns a sorted, deduplicated list suitable for rendering a Sources list.
fn extract_cited_source_indices(finding: &str) -> Vec<usize> {
    let re = Regex::new(r"\[#(\d+)\]").expect("valid citation regex");
    let mut indices: Vec<usize> = re
        .captures_iter(finding)
        .filter_map(|cap| cap[1].parse::<usize>().ok())
        .filter(|n| *n > 0)
        .collect();
    indices.sort_unstable();
    indices.dedup();
    indices
}

/// Build a `**Sources:**` paragraph for a finding that cites one or more
/// captured sources. Each bullet contains the citation number, source title,
/// and path/URL so the reader can map the finding back to the References
/// Index. Returns `None` when there are no citations or none of the cited
/// indices map to a known source.
///
/// A `**Source date range:**` line is appended after the bullet list showing
/// the earliest and latest publication dates of the cited *web* sources, so
/// the reader can judge the relative age of the evidence backing the finding.
/// The line reads `—` when no cited web source exposes a publication date.
fn render_finding_sources(finding: &str, sources: &[Source]) -> Option<String> {
    // If the finding already contains a Sources paragraph (e.g. produced by
    // the LLM itself), don't append a duplicate list.
    if finding.to_lowercase().contains("**sources:**") {
        return None;
    }

    let indices = extract_cited_source_indices(finding);
    if indices.is_empty() {
        return None;
    }

    let mut out = String::from("**Sources:**\n");
    let mut any = false;
    for idx in &indices {
        if let Some(src) = sources.get(idx - 1) {
            any = true;
            out.push_str(&format!(
                "- [{idx}] {title} — {path}{published}\n",
                idx = idx,
                title = src.title(),
                path = src.path_or_url(),
                published = src
                    .published_at()
                    .map(|dt| format!(" (published {})", dt.format("%Y-%m-%d")))
                    .unwrap_or_default()
            ));
        }
    }
    if !any {
        return None;
    }
    // Append the date-range line summarising the publication dates of the
    // cited web sources, so the relative age of the evidence is visible at
    // a glance per finding.
    if let Some(range) = render_finding_date_range(&indices, sources) {
        out.push_str(&format!("\n{range}"));
    }
    // Trim trailing newline; the caller inserts blank lines.
    out.truncate(out.trim_end().len());
    Some(out)
}

/// Compute a `**Source date range:**` summary line for the cited sources.
///
/// Considers only web sources that expose a `published_at` value. Returns
/// `None` when none of the cited sources is a dated web source. The returned
/// line uses the form `earliest..latest` (both inclusive, `YYYY-MM-DD`), or a
/// single date when all dated sources share the same publication date.
fn render_finding_date_range(indices: &[usize], sources: &[Source]) -> Option<String> {
    let mut dates: Vec<chrono::DateTime<chrono::Utc>> = Vec::new();
    let mut total_web = 0usize;
    for idx in indices {
        let src = sources.get(idx - 1)?;
        if matches!(src, Source::Web { .. }) {
            total_web += 1;
            if let Some(dt) = src.published_at() {
                dates.push(dt);
            }
        }
    }
    if dates.is_empty() {
        // No dated web sources: still emit a line when the finding cites web
        // sources, so the reader knows the dates were unavailable rather than
        // absent.
        return total_web.checked_sub(0).map(|_| {
            if total_web > 0 {
                "**Source date range:** — (cited web sources did not expose a publication date)"
                    .to_string()
            } else {
                "**Source date range:** — (no web sources cited)".to_string()
            }
        });
    }
    dates.sort();
    let earliest = dates.first()?;
    let latest = dates.last()?;
    let span = if earliest == latest {
        format!("{}", earliest.format("%Y-%m-%d"))
    } else {
        format!(
            "{}..{}",
            earliest.format("%Y-%m-%d"),
            latest.format("%Y-%m-%d")
        )
    };
    let with_dates = dates.len();
    Some(format!(
        "**Source date range:** {span} ({with_dates} of {total_web} cited web sources dated)"
    ))
}
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

/// Make every bare `http://`/`https://` URL in Markdown text clickable,
/// returning a new string where each raw URL is rewritten as `[url](url)`.
///
/// URLs that already sit inside code fences, inline backtick spans, or
/// existing Markdown links (`[text](url)`) are left untouched so the output
/// stays valid Markdown.
pub(crate) fn linkify_urls(text: &str) -> String {
    let fence_re = Regex::new(r"(?m)^```.*$").expect("valid fence regex");

    let mut out = String::with_capacity(text.len() * 2);
    let mut in_fence = false;
    let mut last_end = 0;
    for m in fence_re.find_iter(text) {
        let segment = &text[last_end..m.start()];
        if in_fence {
            out.push_str(segment);
        } else {
            out.push_str(&linkify_outside_code(segment));
        }
        out.push_str(m.as_str());
        in_fence = !in_fence;
        last_end = m.end();
    }
    let segment = &text[last_end..];
    if in_fence {
        out.push_str(segment);
    } else {
        out.push_str(&linkify_outside_code(segment));
    }
    out
}

/// URL-matching regex. The allowed character set is conservative: it includes
/// the unreserved/sub-delimiters plus the common path/query characters that
/// appear in real URLs, while stopping at whitespace and Markdown delimiter
/// characters.
fn url_regex() -> Regex {
    Regex::new(r"(?i)\bhttps?://[a-zA-Z0-9_~:/.?#@!$&'()*+,;=%-]+").expect("valid url regex")
}

/// Convert bare URLs in a segment that is known to be outside fenced code
/// blocks. Inline backtick spans and existing Markdown links are still
/// protected.
fn linkify_outside_code(text: &str) -> String {
    let url_re = url_regex();
    let chars: Vec<(usize, char)> = text.char_indices().collect();

    let mut out = String::with_capacity(text.len() * 2);
    let mut i = 0;
    while i < chars.len() {
        let (byte, ch) = chars[i];

        // Protect inline code spans: `...`.
        if ch == '`' {
            let after = &text[byte + ch.len_utf8()..];
            if let Some(skip) = after.find('`') {
                let end_byte = byte + ch.len_utf8() + skip + 1;
                out.push_str(&text[byte..end_byte]);
                i = char_index_at(&chars, end_byte);
                continue;
            }
        }

        // Protect existing Markdown links: [text](url).
        if ch == '[' {
            let after = &text[byte + 1..];
            if let Some(close_text) = after.find(']') {
                let close_byte = byte + 1 + close_text;
                if text.as_bytes().get(close_byte + 1) == Some(&b'(')
                    && let Some(close_url) = text[close_byte + 2..].find(')')
                {
                    let end_byte = close_byte + 2 + close_url + 1;
                    out.push_str(&text[byte..end_byte]);
                    i = char_index_at(&chars, end_byte);
                    continue;
                }
            }
        }

        // If a URL starts here, rewrite it.
        if let Some(m) = url_re.find_at(text, byte)
            && m.start() == byte
        {
            let raw = m.as_str();
            let (url, trailing) = trim_url_trailing(raw);

            // Leave autolink-style `<url>` untouched; it is already
            // clickable in most Markdown renderers.
            let prev = text[..byte].chars().next_back();
            let next_char = text[m.end()..].chars().next();
            if prev == Some('<') && next_char == Some('>') {
                out.push_str(raw);
                out.push_str(trailing);
                i = char_index_at(&chars, m.end());
                continue;
            }
            out.push_str(&format!("[{url}]({url}){trailing}"));
            i = char_index_at(&chars, m.end() + trailing.len());
            continue;
        }

        out.push(ch);
        i += 1;
    }
    out
}
/// Find the index in `chars` whose byte offset equals `target_byte`.
/// If `target_byte` is past the end, return `chars.len()`.
fn char_index_at(chars: &[(usize, char)], target_byte: usize) -> usize {
    chars
        .binary_search_by_key(&target_byte, |(b, _)| *b)
        .unwrap_or_else(|e| e)
}

/// Trim trailing punctuation that is unlikely to be part of a URL.
///
/// Trailing `.`, `,`, `;`, `:`, `!`, `?`, quotes, and unbalanced closing
/// parentheses are returned as a separate suffix so the punctuation stays
/// outside the link.
fn trim_url_trailing(raw: &str) -> (&str, &str) {
    let mut end = raw.len();
    while end > 0 {
        let c = raw.as_bytes()[end - 1] as char;
        if c == ')' {
            let opens = raw[..end].chars().filter(|&cc| cc == '(').count();
            let closes = raw[..end].chars().filter(|&cc| cc == ')').count();
            if closes > opens {
                end -= 1;
                continue;
            }
        }
        if matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '"' | '\'') {
            end -= 1;
            continue;
        }
        break;
    }
    (&raw[..end], &raw[end..])
}

/// Truncate a source body to [`MAX_SOURCE_BODY_BYTES`] if necessary,
/// returning a markdown-safe fenced version safe to embed in a supporting
/// file (NFR-006).
#[must_use]
pub fn fence_source_body(body: &str) -> String {
    let trimmed = body.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() <= MAX_SOURCE_BODY_BYTES {
        return trimmed.to_string();
    }
    truncate_body_to_bytes(trimmed, MAX_SOURCE_BODY_BYTES)
}

/// Truncate `body` to at most `max_bytes` UTF-8 bytes, cutting at the nearest
/// char boundary, and append a marker so the reader knows content was capped.
#[must_use]
pub fn truncate_body_to_bytes(body: &str, max_bytes: usize) -> String {
    let bytes = body.as_bytes();
    if bytes.len() <= max_bytes {
        return body.to_string();
    }
    let mut cut = max_bytes;
    while cut > 0 && !body.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut out = String::with_capacity(cut + 64);
    out.push_str(&body[..cut]);
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
#[must_use]
pub fn render_supporting_file(source: &Source) -> Option<String> {
    match source {
        Source::Web {
            url,
            title,
            captured_at,
            published_at,
            body,
            relevance,
            oa_recovery,
            ..
        } => {
            let recovery_note = match oa_recovery {
                Some(r) => {
                    let version = r.version.as_deref().unwrap_or("unspecified");
                    let license = r.license.as_deref().unwrap_or("unspecified");
                    format!(
                        "- Open-access recovery: full text fetched from {source} ({url}); version={version}, license={license}",
                        source = r.source,
                        url = r.url
                    )
                }
                None => String::new(),
            };
            Some(format!(
                "# Web source\n\n\
                 - URL: {url}\n\
                 - Title: {title}\n\
                 - Published (UTC): {published}\n\
                 - Captured (UTC): {captured}\n\
                 - Relevance: {relevance}\n\
                 {recovery_note}\n\n\
                 ```text\n{body}\n```\n",
                url = url,
                title = title,
                published = published_at.map_or_else(|| "—".to_string(), |dt| dt.to_rfc3339()),
                captured = captured_at.to_rfc3339(),
                relevance = if relevance.is_empty() {
                    "—"
                } else {
                    relevance.as_str()
                },
                body = if body.is_empty() {
                    "(no body captured for this source)"
                } else {
                    body.as_str()
                },
            ))
        }
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

/// Lazy-initialized regexes used to strip inline text attributes from the
/// **Analysis:** body so that HTML tags and strikethrough markers do not bleed
/// into the rendered `RESEARCH.md`.
static HTML_TAG_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
static STRIKE_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

/// Remove inline text attributes (HTML tags and `~~...~~` strikethrough) from a
/// paragraph so raw formatting does not leak into the report.
///
/// The content between tags/markers is preserved; only the wrapping markers are
/// removed. This handles crossed-out text (`<s>`, `<del>`, `~~...~~`) and any
/// inline HTML styling attributes (`<span class="...">`, etc.).
fn strip_inline_text_attributes(text: &str) -> String {
    let html = HTML_TAG_RE.get_or_init(|| {
        // HTML/XML-style tags, case-insensitive, preserving inner text.
        Regex::new(r"(?i)</?[a-z][a-z0-9]*(?:\s[^>]*)?/?>").expect("valid regex")
    });
    let strike = STRIKE_RE.get_or_init(|| {
        // Markdown strikethrough: ~~...~~
        Regex::new(r"~~(.+?)~~").expect("valid regex")
    });
    let mut out = html.replace_all(text, "").to_string();
    out = strike.replace_all(&out, "$1").to_string();
    out
}

/// Known abbreviations whose trailing period should not be treated as a
/// sentence boundary (e.g. "e.g.", "i.e.", "etc."). Compared case-insensitively
/// against the token immediately preceding the terminator.
const SENTENCE_ABBREVIATIONS: &[&str] = &[
    "e.g", "i.e", "etc", "vs", "versus", "cf", "approx", "fig", "no", "vol", "pp", "ch", "sec",
    "ref", "eq", "al", "inc", "ltd", "co", "st", "dr", "mr", "mrs", "ms", "prof", "sr", "jr",
];

/// Split the body of an **Analysis:** paragraph into sentences and place each
/// sentence on its own line using a blank line separator. Whitespace in the
/// input — including any embedded newlines — is collapsed to single spaces before
/// splitting so the output is stable regardless of how the analysis was
/// generated.
///
/// Only the **Analysis:** label receives this treatment; other finding labels
/// keep their original (single-paragraph) body. Sentences are split after a
/// `.`, `!`, or `?` that is followed by whitespace and a capital letter or digit,
/// skipping common abbreviations so mid-sentence periods do not create spurious
/// breaks. Single-letter uppercase initials (e.g. "J. P. Morgan") are treated as
/// part of the same sentence when the word after the period is another
/// uppercase initial or capitalized name.
fn split_analysis_sentences(body: &str) -> String {
    // Remove raw HTML tags and markdown emphasis/strikethrough markers first.
    let body = strip_inline_text_attributes(body);

    // Collapse all whitespace (including embedded newlines) to single spaces.
    let collapsed: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let collapsed = collapsed.trim();
    if collapsed.is_empty() {
        return String::new();
    }

    // Walk the text collecting sentence boundaries. A boundary occurs after a
    // terminator character when the following non-whitespace char starts a new
    // sentence (uppercase ASCII letter or digit) and the token ending at the
    // terminator is not a known abbreviation.
    let chars: Vec<char> = collapsed.chars().collect();
    let mut sentences: Vec<String> = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c == '.' || c == '!' || c == '?' {
            // Look ahead past whitespace for the next non-space char.
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            // End of string or a capital letter / digit means a likely boundary.
            let next_starts_sentence =
                j >= chars.len() || chars[j].is_ascii_uppercase() || chars[j].is_ascii_digit();
            if next_starts_sentence {
                // Extract the token immediately preceding the terminator to
                // check against the abbreviation list.
                let mut tok_start = i;
                while tok_start > start && !chars[tok_start - 1].is_whitespace() {
                    tok_start -= 1;
                }
                let token: String = chars[tok_start..=i].iter().collect();
                let lower = token.trim_end_matches(['.', '!', '?']);

                // Single uppercase initials followed by another uppercase word
                // or initial should stay attached (e.g. "J. P. Morgan").
                let is_initial = lower.len() == 1
                    && lower
                        .chars()
                        .next()
                        .is_some_and(|ch| ch.is_ascii_uppercase());
                let next_is_uppercase = j < chars.len() && chars[j].is_ascii_uppercase();
                let skip_boundary = is_initial && next_is_uppercase;

                if !skip_boundary
                    && !SENTENCE_ABBREVIATIONS.contains(&lower.to_lowercase().as_str())
                {
                    let sentence: String = chars[start..=i].iter().collect();
                    let trimmed = sentence.trim();
                    if !trimmed.is_empty() {
                        sentences.push(trimmed.to_string());
                    }
                    start = j;
                    i = j;
                    continue;
                }
            }
        }
        i += 1;
    }
    // Trailing fragment after the last terminator (no final period, etc.).
    let tail: String = chars[start..].iter().collect();
    let tail = tail.trim();
    if !tail.is_empty() {
        sentences.push(tail.to_string());
    }

    if sentences.len() <= 1 {
        // Zero or one sentence: nothing to break apart.
        return sentences.into_iter().collect::<String>();
    }
    // Separate sentences with a blank line so each one stands alone.
    sentences.join("\n\n")
}

fn normalize_finding_labels(finding: &str) -> String {
    let mut text = finding.trim().replace("\n\n\n", "\n\n");

    // Strip stale "Paragraph N — " prefixes before any label.
    let paragraph_prefix = Regex::new(r"Paragraph\s+\d+\s*—\s*").expect("valid regex");
    text = paragraph_prefix.replace_all(&text, "").to_string();

    // Match bold or italic labels ending in a colon, e.g. **Observation:** or
    // *Analysis:*. The colon is required so we don't split random emphasis.
    // Hyphens and slashes are allowed so labels like **Cross-reference / Dependencies:**
    // are recognised correctly.
    let label_re = Regex::new(r"(\*\*[-A-Za-z/\s]+:\*\*|\*[-A-Za-z/\s]+:\*)").expect("valid regex");

    // Split into alternating non-label / label segments. The first segment is
    // text before the first label (usually empty).
    let mut labels: Vec<String> = Vec::new();
    let mut bodies: Vec<String> = Vec::new();
    let mut last_end = 0;
    for mat in label_re.find_iter(&text) {
        let preceding = text[last_end..mat.start()].trim().to_string();
        if labels.is_empty() {
            // Text before the first label is discarded unless it is non-empty,
            // in which case it becomes a leading unlabeled paragraph.
            if !preceding.is_empty() {
                bodies.push(preceding);
                labels.push(String::new());
            }
        } else {
            bodies.push(preceding);
        }
        labels.push(mat.as_str().to_string());
        last_end = mat.end();
    }
    // Trailing text after the last label.
    if last_end < text.len() {
        let trailing = text[last_end..].trim().to_string();
        bodies.push(trailing);
    } else {
        bodies.push(String::new());
    }

    // Pair each label with its body. If labels and bodies are mismatched,
    // fall back to the cleaned text unchanged.
    if labels.len() != bodies.len() || labels.is_empty() {
        return text.trim().to_string();
    }

    let mut out = String::new();
    for (raw_label, body) in labels.iter().zip(bodies.iter()) {
        // Normalize legacy italic labels (*Label:*) to bold (**Label:**).
        let label = if raw_label.starts_with("**") {
            raw_label.clone()
        } else if raw_label.len() >= 2 && raw_label.starts_with('*') && raw_label.ends_with('*') {
            format!("**{}**", &raw_label[1..raw_label.len() - 1])
        } else {
            raw_label.clone()
        };
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        if label.is_empty() {
            // First paragraph may be unlabeled leading text.
            out.push_str(body);
        } else {
            // Put the label on its own line, then the body indented as the
            // next paragraph so each labeled paragraph is clearly separated.
            // The **Analysis:** body is further split into one line per
            // sentence for readability (see split_analysis_sentences).
            let rendered_body = if label == "**Analysis:**" {
                split_analysis_sentences(body)
            } else {
                body.to_string()
            };
            out.push_str(&label);
            if !rendered_body.is_empty() {
                out.push('\n');
                out.push_str(&rendered_body);
            }
        }
    }
    out.trim().to_string()
}
/// Render a "Search Engine Summary" table showing, per backend engine, the
/// number of web sources acquired broken down by media type (pages, PDFs,
/// videos).
///
/// Each [`Source::Web`] carries a comma-separated `search_engine` field (e.g.
/// `"duckduckgo, brave"`). This function splits that field, counts the
/// `media_type` (`"page"`, `"pdf"`, `"youtube"`) per engine, and emits a
/// Markdown table:
///
/// ```text
/// | Engine | Pages | PDFs | Videos | Total |
/// |--------|-------|------|--------|-------|
/// | duckduckgo | 5 | 1 | 0 | 6 |
/// ```
///
/// Returns an empty string when no web sources have a non-empty
/// `search_engine` value, so callers can unconditionally append the result
/// without producing a stray empty section.
#[must_use]
pub fn render_search_engine_summary(sources: &[Source]) -> String {
    use std::collections::BTreeMap;

    /// Per-engine media-type counts.
    #[derive(Default, Clone, Copy)]
    struct Counts {
        pages: usize,
        pdfs: usize,
        videos: usize,
    }

    impl Counts {
        const fn total(&self) -> usize {
            self.pages + self.pdfs + self.videos
        }
    }

    let mut by_engine: BTreeMap<String, Counts> = BTreeMap::new();
    for source in sources {
        if let Source::Web {
            search_engine,
            media_type,
            ..
        } = source
        {
            for engine in search_engine.split(',') {
                let name = engine.trim();
                if name.is_empty() {
                    continue;
                }
                let entry = by_engine.entry(name.to_string()).or_default();
                match media_type.as_str() {
                    "pdf" => entry.pdfs += 1,
                    "youtube" => entry.videos += 1,
                    _ => entry.pages += 1,
                }
            }
        }
    }

    if by_engine.is_empty() {
        return String::new();
    }

    let mut out = String::from("| Engine | Pages | PDFs | Videos | Total |\n");
    out.push_str("|--------|-------|------|--------|-------|\n");
    for (engine, counts) in &by_engine {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            escape_pipe(engine),
            counts.pages,
            counts.pdfs,
            counts.videos,
            counts.total(),
        ));
    }
    out
}

/// Render a standalone sources appendix / bibliography for the
/// `--format source-bibliography` artifact (T-011).
///
/// The output is a markdown document listing every source with its type,
/// title, path/URL, captured timestamp, and (when available) the first 240
/// characters of its body.
#[must_use]
pub fn render_bibliography(sources: &[Source]) -> String {
    if sources.is_empty() {
        return "# Sources Bibliography\n\n_(no sources captured)_\n".to_string();
    }
    let mut out = String::from("# Sources Bibliography\n\n");
    for (idx, source) in sources.iter().enumerate() {
        let n = idx + 1;
        let kind = source.type_str();
        let path = source.path_or_url();
        let title = source.title();
        let captured = source.captured_at().to_rfc3339();
        out.push_str(&format!(
            "## [{n}] {title}\n\n- **Type:** {kind}\n- **Path/URL:** {path}\n- **Captured:** {captured}\n"
        ));
        if let Some(rel) = source.relevance()
            && !rel.is_empty()
        {
            out.push_str(&format!("- **Relevance:** {rel}\n"));
        }
        if let Some(body) = source.body() {
            let preview = if body.chars().count() > 240 {
                format!("{}…", body.chars().take(240).collect::<String>())
            } else {
                body.to_string()
            };
            if !preview.is_empty() {
                out.push_str("- **Preview:**\n\n  ```text\n  ");
                out.push_str(&preview.replace('\n', "\n  "));
                out.push_str("\n  ```\n");
            }
        }
        out.push('\n');
    }
    out
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

    fn sample_doc(item: ResearchItem) -> ResearchDocument {
        ResearchDocument {
            item,
            summary: String::new(),
            findings: Vec::new(),
            top_implications: Vec::new(),
            cross_references: Vec::new(),
            open_questions: Vec::new(),
            contradiction_graph: None,
            loci: None,
            depth_investigation: None,
            evidence_digest: None,
            triple_draft: None,
            cross_locus_reconcile: None,
            source_tensions: None,
            synthesis_audit: None,
            corpus_critic: None,
            gap_fetch: None,
            surgical_patch: None,
            cite_check: None,
            polish: None,
            readability_audit: None,
            template_body: None,
            decomposed_queries: Vec::new(),
            output_format: crate::run_config::OutputFormat::Report,
        }
    }

    #[test]
    fn assemble_document_renders_contradiction_graph_section() {
        use crate::contradiction::{ContradictionClaim, ContradictionEdge, ContradictionGraph};
        let sources = vec![
            Source::Web {
                url: "https://a.example".into(),
                title: "A".into(),
                captured_at: chrono::Utc::now(),
                published_at: None,
                body_path: PathBuf::new(),
                body: "The intervention improves performance.".into(),
                relevance: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
                content_type: None,
                page_type: None,
                media_type: "page".into(),
                language: None,
                oa_recovery: None,
            },
            Source::Web {
                url: "https://b.example".into(),
                title: "B".into(),
                captured_at: chrono::Utc::now(),
                published_at: None,
                body_path: PathBuf::new(),
                body: "The intervention degrades performance.".into(),
                relevance: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
                content_type: None,
                page_type: None,
                media_type: "page".into(),
                language: None,
                oa_recovery: None,
            },
        ];
        let mut graph = ContradictionGraph::empty();
        graph.add_edge(ContradictionEdge {
            claim_a: ContradictionClaim::from_source("claims better performance", 1, &sources[0]),
            claim_b: ContradictionClaim::from_source("claims worse performance", 2, &sources[1]),
            dimension: "performance".into(),
            note: "opposing performance claims".into(),
            strength: 50,
        });
        let mut doc = sample_doc(sample_item());
        doc.contradiction_graph = Some(graph);
        let assembled = assemble_document(&doc);
        assert!(assembled.body.contains("## Contradiction Graph"));
        assert!(assembled.body.contains("performance"));
        assert!(assembled.body.contains("opposing performance claims"));
        assert!(assembled.body.contains("#1"));
        assert!(assembled.body.contains("#2"));
    }

    #[test]
    fn assemble_document_contradiction_graph_placeholder_when_empty() {
        let mut doc = sample_doc(sample_item());
        doc.contradiction_graph = Some(crate::contradiction::ContradictionGraph::empty());
        let assembled = assemble_document(&doc);
        assert!(assembled.body.contains("## Contradiction Graph"));
        assert!(
            assembled
                .body
                .contains("no contradictions detected among the gathered sources")
        );
    }

    #[test]
    fn assemble_document_omits_contradiction_section_when_none() {
        let doc = sample_doc(sample_item());
        let assembled = assemble_document(&doc);
        assert!(!assembled.body.contains("## Contradiction Graph"));
    }

    #[test]
    fn assemble_document_renders_corpus_critic_and_gap_fetch_sections() {
        let mut doc = sample_doc(sample_item());
        doc.corpus_critic = Some(crate::corpus_critic::CorpusCriticReport {
            score: 72,
            coverage_score: 80,
            evidence_score: 70,
            balance_score: 85,
            tension_score: 55,
            issues: vec!["shallow evidence on Cost".into()],
            gaps: vec!["Add cost evidence".into()],
            recommendations: vec!["Broaden the width sweep".into()],
            contested_ratio: 10,
            shallow_dimensions: vec!["Cost".into()],
            isolated_sources: vec![3],
            passed: true,
        });
        doc.gap_fetch = Some(crate::corpus_critic::GapFetchResult {
            queries: vec!["topic cost evidence".into()],
            new_sources: 2,
            failed_queries: 0,
            attempted: true,
            note: String::new(),
        });
        let assembled = assemble_document(&doc);
        assert!(
            assembled.body.contains("## Corpus Critic"),
            "report layout should render corpus critic section"
        );
        assert!(
            assembled.body.contains("## Gap-Fill Fetch"),
            "report layout should render gap-fill section"
        );
        assert!(assembled.body.contains("72/100"));
        assert!(assembled.body.contains("Broaden the width sweep"));
        assert!(assembled.body.contains("**New sources captured:** 2"));
        assert!(assembled.body.contains("topic cost evidence"));
    }

    #[test]
    fn assemble_document_renders_surgical_patch_section() {
        let mut doc = sample_doc(sample_item());
        doc.surgical_patch = Some(crate::patcher::PatchResult {
            patches: vec![
                crate::patcher::SurgicalPatch {
                    operation: "append_finding".to_string(),
                    target: "Cost".to_string(),
                    reason: "Dimension 'Cost' not addressed".to_string(),
                    applied: true,
                },
                crate::patcher::SurgicalPatch {
                    operation: "noop".to_string(),
                    target: "logic".to_string(),
                    reason: "logic critic passed".to_string(),
                    applied: false,
                },
            ],
            patched_analysis: crate::analysis::AnalysisResult::default(),
            score_before: 55,
            score_after: 70,
            note: "Applied 1 surgical patch".to_string(),
            patched_finding_count: 1,
            patched_implication_count: 0,
            patched_open_question_count: 1,
        });
        let assembled = assemble_document(&doc);
        assert!(
            assembled.body.contains("## Surgical Patch"),
            "report layout should render surgical patch section"
        );
        assert!(assembled.body.contains("55 → 70"));
        assert!(assembled.body.contains("Applied 1 surgical patch"));
        assert!(assembled.body.contains("append_finding"));
        assert!(assembled.body.contains("Cost"));
    }

    #[test]
    fn assemble_document_omits_surgical_patch_section_when_none() {
        let doc = sample_doc(sample_item());
        let assembled = assemble_document(&doc);
        assert!(!assembled.body.contains("## Surgical Patch"));
    }

    #[test]
    fn assemble_document_frontmatter_discloses_open_access_recovery() {
        let mut item = sample_item();
        item.open_access_recovery = true;
        let doc = sample_doc(item);
        let assembled = assemble_document(&doc);
        assert!(
            assembled.frontmatter.contains("open_access_recovery: true"),
            "frontmatter should disclose OA recovery; got:\n{}",
            assembled.frontmatter
        );
    }

    #[test]
    fn assemble_document_supporting_file_discloses_recovery_version_and_license() {
        use crate::open_access::{RecoveredOpenAccess, RecoverySource};
        let mut item = sample_item();
        item.add_source(Source::Web {
            url: "https://doi.org/10.1234/example".into(),
            title: "Example paper".into(),
            captured_at: chrono::Utc::now(),
            published_at: None,
            body_path: std::path::PathBuf::from("sources/web-01.md"),
            body: "full text".into(),
            relevance: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
            content_type: None,
            page_type: None,
            media_type: "page".into(),
            language: None,
            oa_recovery: Some(Box::new(RecoveredOpenAccess {
                url: "https://pmc.ncbi.nlm.nih.gov/articles/PMC123456/".into(),
                source: RecoverySource::EuropePmc,
                license: Some("CC-BY-4.0".into()),
                version: Some("publishedVersion".into()),
            })),
        });
        let rendered = render_supporting_file(&item.sources[0]).expect("web source renders");
        assert!(rendered.contains("Open-access recovery"));
        assert!(rendered.contains("europepmc"));
        assert!(rendered.contains("publishedVersion"));
        assert!(rendered.contains("CC-BY-4.0"));
    }

    #[test]
    fn assemble_document_includes_all_ten_sections() {
        let doc = sample_doc(sample_item());
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
    fn assemble_document_renders_top_implications() {
        let mut doc = sample_doc(sample_item());
        doc.top_implications = vec![
            "Adopt async/await for I/O-bound concurrency.".into(),
            "Profile blocking calls before migration.".into(),
        ];
        let assembled = assemble_document(&doc);
        let body = &assembled.body;
        assert!(
            body.contains("## Top 5 Implications"),
            "section heading must be present"
        );
        assert!(body.contains("1. Adopt async/await for I/O-bound concurrency."));
        assert!(body.contains("2. Profile blocking calls before migration."));
    }

    #[test]
    fn assemble_document_starts_with_frontmatter_block() {
        let doc = sample_doc(sample_item());
        let assembled = assemble_document(&doc);
        assert!(assembled.content.starts_with("---\n"));
        assert!(assembled.content.contains("name: rust-async"));
        assert!(assembled.content.contains("status: draft"));
    }

    #[test]
    fn assemble_document_normalizes_paragraph_prefixes() {
        let mut doc = sample_doc(sample_item());
        doc.findings = vec![
            "Paragraph 1 — **Observation:** observation text. *Paragraph 2 — Analysis:* analysis text. **Cross-reference / Dependencies:** deps. **Implication:** implication text. **Caveat:** caveat text.".into(),
        ];
        let assembled = assemble_document(&doc);
        let body = &assembled.body;
        assert!(
            !body.contains("Paragraph 1"),
            "paragraph prefixes should be stripped: {body}"
        );
        assert!(
            body.contains("**Observation:**\nobservation text."),
            "observation label should be on its own line: {body}"
        );
        assert!(
            body.contains("**Analysis:**\nanalysis text."),
            "analysis label should be on its own line: {body}"
        );
        assert!(
            body.contains("**Cross-reference / Dependencies:**\ndeps."),
            "cross-reference label should be on its own line: {body}"
        );
        assert!(
            body.contains("**Caveat:**\ncaveat text."),
            "extra caveat label should be preserved and separated: {body}"
        );
    }

    #[test]
    fn assemble_document_splits_run_on_finding_into_paragraphs() {
        let mut doc = sample_doc(sample_item());
        doc.findings = vec![
            "**Headline:** Observation summary

**Observation:** obs **Analysis:** analysis **Cross-reference / Dependencies:** none **Implication:** impl **Caveat:** caveat".into(),
        ];
        let assembled = assemble_document(&doc);
        let finding = assembled
            .body
            .split("### **Finding 1** — Observation summary\n\n")
            .nth(1)
            .unwrap();
        // After "### Finding N — headline\n\n" the required labels should be separated by blank lines.
        assert!(
            finding.contains("**Observation:**\nobs\n\n**Analysis:**\nanalysis"),
            "labels should be separated by blank lines: {finding}"
        );
        assert!(
            finding.contains("**Implication:**\nimpl\n\n**Caveat:**\ncaveat"),
            "caveat should be separated from implication: {finding}"
        );
        assert!(
            finding.contains("**Cross-reference / Dependencies:**\nnone\n\n**Implication:**\nimpl"),
            "cross-reference label should be on its own line: {finding}"
        );
        assert!(
            finding
                .trim_start()
                .starts_with("**Observation:**\nobs\n\n**Analysis:**\nanalysis"),
            "label and its body should be on separate lines: {finding}"
        );
    }

    #[test]
    fn assemble_document_emits_one_finding_block_per_entry() {
        let mut doc = sample_doc(sample_item());
        doc.findings = vec![
            "**Headline:** Observation summary

**Observation:** first observation\n\n**Analysis:** first analysis\n\n**Cross-reference / Dependencies:** No direct dependencies.\n\n**Implication:** first implication\n\n**Related work:** extra context for finding one.".into(),
            "**Headline:** Observation summary

**Observation:** second observation\n\n**Analysis:** second analysis\n\n**Cross-reference / Dependencies:** Related to Finding 1.\n\n**Implication:** second implication".into(),
        ];
        let assembled = assemble_document(&doc);
        assert!(assembled.body.contains(
            "### **Finding 1** — Observation summary\n\n**Observation:**\nfirst observation"
        ));
        assert!(assembled.body.contains(
            "### **Finding 2** — Observation summary\n\n**Observation:**\nsecond observation"
        ));
        assert!(assembled.body.contains("Related to Finding 1."));
        assert!(
            assembled
                .body
                .contains("**Related work:**\nextra context for finding one."),
            "extra labeled paragraph beyond the five required ones should be preserved: {}",
            assembled.body
        );
    }

    #[test]
    fn assemble_document_renders_cross_reference_table() {
        let mut doc = sample_doc(sample_item());
        doc.cross_references = vec![CrossReference {
            path: "src/lib.rs".into(),
            relevance: "Main library entry".into(),
        }];
        let assembled = assemble_document(&doc);
        assert!(assembled.body.contains("| Path | Relevance |"));
        assert!(
            assembled
                .body
                .contains("| `src/lib.rs` | Main library entry |")
        );
    }

    /// Helper: build a `Source::Web` with the given search engine and media type.
    fn web_source(search_engine: &str, media_type: &str) -> Source {
        use chrono::Utc;
        Source::Web {
            url: format!("https://example.com/{media_type}"),
            title: format!("Test {media_type}"),
            captured_at: Utc::now(),
            published_at: None,
            body_path: PathBuf::from("sources/web-01.md"),
            body: String::new(),
            relevance: String::new(),
            search_tool: "mf_search".into(),
            search_engine: search_engine.into(),
            content_type: None,
            page_type: None,
            media_type: media_type.into(),
            language: None,
            oa_recovery: None,
        }
    }

    #[test]
    fn render_search_engine_summary_counts_pages_pdfs_videos() {
        let sources = vec![
            web_source("duckduckgo, brave", "page"),
            web_source("duckduckgo", "page"),
            web_source("brave", "pdf"),
            web_source("exa", "youtube"),
            web_source("exa", "page"),
        ];
        let table = render_search_engine_summary(&sources);
        assert!(table.contains("| Engine | Pages | PDFs | Videos | Total |"));
        // brave: 1 page (from multi-engine source) + 1 pdf = 1 page, 1 pdf, 0 videos, 2 total
        assert!(table.contains("| brave | 1 | 1 | 0 | 2 |"));
        // duckduckgo: 2 pages (one from multi-engine, one single) = 2 pages
        assert!(table.contains("| duckduckgo | 2 | 0 | 0 | 2 |"));
        // exa: 1 youtube + 1 page = 1 page, 0 pdfs, 1 video, 2 total
        assert!(table.contains("| exa | 1 | 0 | 1 | 2 |"));
    }

    #[test]
    fn render_search_engine_summary_empty_when_no_web_sources() {
        let sources: Vec<Source> = vec![];
        let table = render_search_engine_summary(&sources);
        assert!(table.is_empty());
    }

    #[test]
    fn render_search_engine_summary_empty_when_no_engine_field() {
        // Web sources with empty search_engine should produce no table.
        let sources = vec![web_source("", "page")];
        let table = render_search_engine_summary(&sources);
        assert!(table.is_empty());
    }

    #[test]
    fn assemble_document_renders_search_engine_summary_after_queries() {
        let mut item = sample_item();
        item.sources = vec![web_source("duckduckgo", "page"), web_source("brave", "pdf")];
        let mut doc = sample_doc(item);
        doc.decomposed_queries = vec!["test query".into()];
        let assembled = assemble_document(&doc);
        // The summary heading should appear after Search Queries and before
        // Executive Summary.
        let queries_pos = assembled.body.find("## Search Queries").unwrap();
        let summary_pos = assembled
            .body
            .find("### Search Engine Summary")
            .expect("Search Engine Summary section should be present");
        let exec_pos = assembled.body.find("## Executive Summary").unwrap();
        assert!(
            queries_pos < summary_pos,
            "Search Engine Summary should come after Search Queries"
        );
        assert!(
            summary_pos < exec_pos,
            "Search Engine Summary should come before Executive Summary"
        );
        assert!(assembled.body.contains("| duckduckgo | 1 | 0 | 0 | 1 |"));
        assert!(assembled.body.contains("| brave | 0 | 1 | 0 | 1 |"));
    }

    #[test]
    fn assemble_document_imrad_renders_search_engine_summary() {
        let mut item = sample_item();
        item.sources = vec![web_source("exa", "page"), web_source("exa", "youtube")];
        let mut doc = sample_doc(item);
        doc.decomposed_queries = vec!["test query".into()];
        doc.output_format = crate::run_config::OutputFormat::Imrad;
        let assembled = assemble_document(&doc);
        // In IMRaD layout the summary appears under Methods after Search Queries.
        let methods_pos = assembled.body.find("## Methods").unwrap();
        let queries_pos = assembled.body.find("### Search Queries").unwrap();
        let summary_pos = assembled
            .body
            .find("### Search Engine Summary")
            .expect("Search Engine Summary should be present in IMRaD layout");
        let config_pos = assembled.body.find("### Research Configuration").unwrap();
        assert!(methods_pos < queries_pos);
        assert!(
            queries_pos < summary_pos,
            "Search Engine Summary should come after Search Queries in IMRaD"
        );
        assert!(
            summary_pos < config_pos,
            "Search Engine Summary should come before Research Configuration in IMRaD"
        );
        assert!(assembled.body.contains("| exa | 1 | 0 | 1 | 2 |"));
    }

    #[test]
    fn assemble_document_omits_search_engine_summary_for_skeleton() {
        // A skeleton (no sources) should NOT contain the Search Engine Summary.
        let doc = sample_doc(sample_item());
        let assembled = assemble_document(&doc);
        assert!(
            !assembled.body.contains("### Search Engine Summary"),
            "skeleton should not contain Search Engine Summary: {}",
            assembled.body
        );
    }

    #[test]
    fn assemble_document_escapes_pipes_in_cross_reference_relevance() {
        let mut doc = sample_doc(sample_item());
        doc.cross_references = vec![CrossReference {
            path: "src/lib.rs".into(),
            relevance: "Has | pipes".into(),
        }];
        let assembled = assemble_document(&doc);
        assert!(
            assembled.body.contains(r"Has \| pipes"),
            "expected escaped pipe in: {}",
            assembled.body
        );
    }

    #[test]
    fn assemble_document_preserves_inline_citation_markers() {
        let mut doc = sample_doc(sample_item());
        doc.findings = vec!["Use Tokio [#1] for async runtimes.".into()];
        let assembled = assemble_document(&doc);
        assert!(assembled.body.contains("[#1]"));
    }

    #[test]
    fn render_skeleton_produces_well_formed_document() {
        let skeleton = render_skeleton(
            &sample_name(),
            "Rust Async",
            "topic",
            crate::run_config::OutputFormat::Report,
        );
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
            published_at: None,
            url: "https://example.com".into(),
            title: "Example".into(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/web-01.md"),
            relevance: String::new(),
            body: "page body content".into(),
            search_tool: String::new(),
            search_engine: String::new(),
            content_type: None,
            page_type: None,
            media_type: "page".into(),
            language: None,
            oa_recovery: None,
        };
        let out = render_supporting_file(&source).expect("web must produce a body");
        assert!(out.contains("# Web source"));
        assert!(out.contains("URL: https://example.com"));
        assert!(out.contains("page body content"));
    }

    #[test]
    fn render_supporting_file_produces_web_placeholder_when_body_empty() {
        let source = Source::Web {
            published_at: None,
            url: "https://example.com".into(),
            title: "Example".into(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/web-01.md"),
            relevance: String::new(),
            body: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
            content_type: None,
            page_type: None,
            media_type: "page".into(),
            language: None,
            oa_recovery: None,
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
    fn render_bibliography_empty_state() {
        let out = render_bibliography(&[]);
        assert!(out.contains("Sources Bibliography"));
        assert!(out.contains("no sources captured"));
    }

    #[test]
    fn render_bibliography_includes_source_preview() {
        let source = Source::Web {
            published_at: None,
            url: "https://example.com".into(),
            title: "Example".into(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/web-01.md"),
            relevance: String::new(),
            body: "page body content".into(),
            search_tool: String::new(),
            search_engine: String::new(),
            content_type: None,
            page_type: None,
            media_type: "page".into(),
            language: None,
            oa_recovery: None,
        };
        let out = render_bibliography(&[source]);
        assert!(out.contains("Example"));
        assert!(out.contains("https://example.com"));
        assert!(out.contains("page body content"));
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

    #[test]
    fn assemble_document_appends_sources_list_for_citations() {
        let mut item = sample_item();
        item.add_source(Source::Web {
            published_at: None,
            url: "https://example.com".into(),
            title: "Example Article".into(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/web-01.md"),
            relevance: String::new(),
            body: "body".into(),
            search_tool: String::new(),
            search_engine: String::new(),
            content_type: None,
            page_type: None,
            media_type: "page".into(),
            language: None,
            oa_recovery: None,
        });
        let mut doc = sample_doc(item);
        doc.findings = vec![
            "**Headline:** Observation summary

**Observation:** Something important [#1].

**Analysis:** Why it matters.

**Cross-reference / Dependencies:** No direct dependencies.

**Implication:** Do this."
                .into(),
        ];
        let assembled = assemble_document(&doc);
        let finding = assembled.body.split("### **Finding 1**").nth(1).unwrap();
        assert!(
            finding.contains("**Sources:**"),
            "finding should contain Sources paragraph: {finding}"
        );
        assert!(
            finding.contains("- [1] Example Article — [https://example.com](https://example.com)"),
            "Sources bullet should map citation to source title/URL: {finding}"
        );
    }

    #[test]
    fn assemble_document_dedupes_and_sorts_citation_indices() {
        let mut item = sample_item();
        item.add_source(Source::Web {
            published_at: None,
            url: "https://a".into(),
            title: "A".into(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/web-01.md"),
            relevance: String::new(),
            body: "body".into(),
            search_tool: String::new(),
            search_engine: String::new(),
            content_type: None,
            page_type: None,
            media_type: "page".into(),
            language: None,
            oa_recovery: None,
        });
        item.add_source(Source::Web {
            published_at: None,
            url: "https://b".into(),
            title: "B".into(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/web-02.md"),
            relevance: String::new(),
            body: "body".into(),
            search_tool: String::new(),
            search_engine: String::new(),
            content_type: None,
            page_type: None,
            media_type: "page".into(),
            language: None,
            oa_recovery: None,
        });
        let mut doc = sample_doc(item);
        doc.findings = vec!["Mixed [#2] and [#1] and again [#2].".into()];
        let assembled = assemble_document(&doc);
        let finding = assembled.body.split("### **Finding 1**").nth(1).unwrap();
        // Sources should be in index order, not citation order, and deduped.
        let sources_idx = finding.find("**Sources:**").unwrap();
        let sources_block = &finding[sources_idx..];
        let first = sources_block.find("- [1] A").unwrap();
        let second = sources_block.find("- [2] B").unwrap();
        assert!(
            first < second,
            "sources should be sorted by index: {finding}"
        );
    }

    #[test]
    fn assemble_document_appends_source_date_range_for_cited_web_sources() {
        let mut item = sample_item();
        item.add_source(Source::Web {
            published_at: Some(
                chrono::DateTime::parse_from_rfc3339("2023-01-10T00:00:00Z")
                    .unwrap()
                    .with_timezone(&chrono::Utc),
            ),
            url: "https://a.example".into(),
            title: "A".into(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/web-01.md"),
            relevance: String::new(),
            body: "body".into(),
            search_tool: String::new(),
            search_engine: String::new(),
            content_type: None,
            page_type: None,
            media_type: "page".into(),
            language: None,
            oa_recovery: None,
        });
        item.add_source(Source::Web {
            published_at: Some(
                chrono::DateTime::parse_from_rfc3339("2024-06-01T00:00:00Z")
                    .unwrap()
                    .with_timezone(&chrono::Utc),
            ),
            url: "https://b.example".into(),
            title: "B".into(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/web-02.md"),
            relevance: String::new(),
            body: "body".into(),
            search_tool: String::new(),
            search_engine: String::new(),
            content_type: None,
            page_type: None,
            media_type: "page".into(),
            language: None,
            oa_recovery: None,
        });
        item.add_source(Source::Web {
            published_at: None,
            url: "https://c.example".into(),
            title: "C".into(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/web-03.md"),
            relevance: String::new(),
            body: "body".into(),
            search_tool: String::new(),
            search_engine: String::new(),
            content_type: None,
            page_type: None,
            media_type: "page".into(),
            language: None,
            oa_recovery: None,
        });
        let mut doc = sample_doc(item);
        doc.findings = vec![
            "**Headline:** Observation summary

**Observation:** spans [#1], [#2], and [#3]."
                .into(),
        ];
        let assembled = assemble_document(&doc);
        let finding = assembled.body.split("### **Finding 1**").nth(1).unwrap();
        assert!(
            finding.contains(
                "**Source date range:** 2023-01-10..2024-06-01 (2 of 3 cited web sources dated)"
            ),
            "finding should carry a source date range line: {finding}"
        );
        // The bullet for the dated source should include its publication date.
        assert!(
            finding.contains("(published 2023-01-10)"),
            "dated source bullet should include its publication date: {finding}"
        );
    }

    #[test]
    fn assemble_document_notes_undated_web_sources_in_date_range() {
        let mut item = sample_item();
        item.add_source(Source::Web {
            published_at: None,
            url: "https://a.example".into(),
            title: "A".into(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/web-01.md"),
            relevance: String::new(),
            body: "body".into(),
            search_tool: String::new(),
            search_engine: String::new(),
            content_type: None,
            page_type: None,
            media_type: "page".into(),
            language: None,
            oa_recovery: None,
        });
        let mut doc = sample_doc(item);
        doc.findings = vec![
            "**Headline:** Observation summary

**Observation:** only [#1]."
                .into(),
        ];
        let assembled = assemble_document(&doc);
        let finding = assembled.body.split("### **Finding 1**").nth(1).unwrap();
        assert!(
            finding.contains(
                "**Source date range:** — (cited web sources did not expose a publication date)"
            ),
            "finding should note that cited web sources had no date: {finding}"
        );
    }

    #[test]
    fn assemble_document_omits_sources_paragraph_without_citations() {
        let doc = sample_doc(sample_item());
        let assembled = assemble_document(&doc);
        assert!(
            !assembled.body.contains("**Sources:**"),
            "no citations means no Sources paragraph: {}",
            assembled.body
        );
    }
    #[test]
    fn assemble_document_skips_sources_list_when_finding_already_has_one() {
        let mut item = sample_item();
        item.add_source(Source::Web {
            published_at: None,
            url: "https://example.com".into(),
            title: "Example Article".into(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/web-01.md"),
            relevance: String::new(),
            body: "body".into(),
            search_tool: String::new(),
            search_engine: String::new(),
            content_type: None,
            page_type: None,
            media_type: "page".into(),
            language: None,
            oa_recovery: None,
        });
        let mut doc = sample_doc(item);
        // The LLM already produced its own Sources paragraph.
        doc.findings = vec![
            "**Headline:** Observation summary

**Observation:** Something important [#1].

**Sources:**
- Article A — https://a"
                .into(),
        ];
        let assembled = assemble_document(&doc);
        let finding = assembled.body.split("### **Finding 1**").nth(1).unwrap();
        let count = finding.matches("**Sources:**").count();
        assert_eq!(
            count, 1,
            "should not add a duplicate Sources paragraph: {finding}"
        );
    }

    #[test]
    fn assemble_document_puts_cross_reference_label_on_own_line() {
        let mut doc = sample_doc(sample_item());
        doc.findings = vec![
              "**Headline:** Observation summary

**Observation:** obs\n\n**Analysis:** analysis\n\n**Cross-reference / Dependencies:** none\n\n**Implication:** impl".into(),
          ];
        let assembled = assemble_document(&doc);
        let finding = assembled.body.split("### **Finding 1**").nth(1).unwrap();
        assert!(
            finding.contains("**Cross-reference / Dependencies:**\nnone"),
            "cross-reference label should stand on its own line: {finding}"
        );
    }

    #[test]
    fn linkify_urls_rewrites_bare_http_urls() {
        let input = "Visit https://example.com for details.";
        assert_eq!(
            linkify_urls(input),
            "Visit [https://example.com](https://example.com) for details."
        );
    }

    #[test]
    fn linkify_urls_leaves_existing_markdown_links_unchanged() {
        let input = "See [example](https://example.com).";
        assert_eq!(linkify_urls(input), input);
    }

    #[test]
    fn linkify_urls_leaves_autolink_style_unchanged() {
        let input = "See <https://example.com>.";
        assert_eq!(linkify_urls(input), input);
    }

    #[test]
    fn linkify_urls_protects_inline_code() {
        let input = "Use `curl https://example.com` to test.";
        assert_eq!(linkify_urls(input), input);
    }

    #[test]
    fn linkify_urls_protects_fenced_code_blocks() {
        let input = "```\ncurl https://example.com\n```\nThen visit https://site.org.";
        assert_eq!(
            linkify_urls(input),
            "```\ncurl https://example.com\n```\nThen visit [https://site.org](https://site.org)."
        );
    }

    #[test]
    fn linkify_urls_keeps_trailing_punctuation_outside_link() {
        let input = "Read https://example.com.";
        assert_eq!(
            linkify_urls(input),
            "Read [https://example.com](https://example.com)."
        );
    }

    #[test]
    fn linkify_urls_keeps_unbalanced_closing_paren_outside_link() {
        let input = "(see https://example.com))";
        assert_eq!(
            linkify_urls(input),
            "(see [https://example.com](https://example.com)))"
        );
    }

    #[test]
    fn linkify_urls_leaves_supporting_file_url_lines_raw() {
        // This mirrors the supporting-file table rows produced by
        // render_supporting_file; linkification is applied only to the
        // assembled RESEARCH.md body.
        let input = "URL: https://example.com";
        assert_eq!(
            linkify_urls(input),
            "URL: [https://example.com](https://example.com)"
        );
    }

    #[test]
    fn split_analysis_sentences_places_each_sentence_on_its_own_line() {
        let body = "This is the first sentence. This is the second one! And a third?";
        let out = split_analysis_sentences(body);
        // Three sentences, separated by blank lines.
        assert_eq!(
            out,
            "This is the first sentence.\n\nThis is the second one!\n\nAnd a third?"
        );
    }

    #[test]
    fn split_analysis_sentences_single_sentence_has_no_break() {
        let body = "Only one sentence here.";
        let out = split_analysis_sentences(body);
        assert_eq!(out, "Only one sentence here.");
    }

    #[test]
    fn split_analysis_sentences_collapses_embedded_newlines() {
        let body = "First sentence.\n\nSecond sentence that\nspans lines. Third.";
        let out = split_analysis_sentences(body);
        assert_eq!(
            out,
            "First sentence.\n\nSecond sentence that spans lines.\n\nThird."
        );
    }

    #[test]
    fn split_analysis_sentences_skips_abbreviation_periods() {
        let body = "Use e.g. short examples. Then move on. See i.e. the next part.";
        let out = split_analysis_sentences(body);
        // "e.g." and "i.e." should not create sentence breaks; only the real
        // sentence terminators after "examples" and "on" should split.
        assert_eq!(
            out,
            "Use e.g. short examples.\n\nThen move on.\n\nSee i.e. the next part."
        );
    }

    #[test]
    fn split_analysis_sentences_keeps_initials_together() {
        let body = "J. P. Morgan founded the firm. Later he expanded it.";
        let out = split_analysis_sentences(body);
        assert_eq!(
            out,
            "J. P. Morgan founded the firm.\n\nLater he expanded it."
        );
    }

    #[test]
    fn assemble_document_renders_citation_check_section() {
        use crate::cite_checker::CitationCheckResult;
        let mut doc = sample_doc(sample_item());
        doc.cite_check = Some(CitationCheckResult {
            passed: true,
            checked: 2,
            failed_claims: Vec::new(),
            issues: Vec::new(),
            gate_open: true,
        });
        let assembled = assemble_document(&doc);
        assert!(
            assembled.body.contains("## Citation Check"),
            "report layout must contain Citation Check section"
        );
        assert!(
            assembled
                .body
                .contains("**Summary:** 2 citation(s) checked, 2 passed, 0 failed; gate open.")
        );
        assert!(assembled.body.contains("pass (2 citation(s) checked)"));
        assert!(assembled.body.contains("open — report may ship"));
    }

    #[test]
    fn assemble_document_renders_source_tensions_section() {
        use crate::reconcile::{SourceTensions, TensionKind, TensionRecord};
        let mut doc = sample_doc(sample_item());
        doc.source_tensions = Some(SourceTensions {
            tensions: vec![TensionRecord {
                kind: TensionKind::Contradiction,
                label: "performance".into(),
                source_indices: vec![1, 2],
                note: "opposing performance claims".into(),
            }],
            sources_scanned: 2,
        });
        let assembled = assemble_document(&doc);
        assert!(
            assembled.body.contains("## Source Tensions"),
            "report layout must contain Source Tensions section"
        );
        assert!(assembled.body.contains("contradiction"));
        assert!(assembled.body.contains("performance"));
        assert!(assembled.body.contains("#1, #2"));
        assert!(assembled.body.contains("opposing performance claims"));
    }

    #[test]
    fn assemble_document_imrad_renders_source_tensions_subsection() {
        use crate::reconcile::{SourceTensions, TensionKind, TensionRecord};
        let mut doc = sample_doc(sample_item());
        doc.output_format = crate::run_config::OutputFormat::Imrad;
        doc.source_tensions = Some(SourceTensions {
            tensions: vec![TensionRecord {
                kind: TensionKind::ShallowEvidence,
                label: "cost".into(),
                source_indices: vec![3],
                note: "thin coverage".into(),
            }],
            sources_scanned: 5,
        });
        let assembled = assemble_document(&doc);
        assert!(
            assembled.body.contains("### Source Tensions"),
            "IMRaD layout must render Source Tensions as a subsection"
        );
        assert!(assembled.body.contains("shallow evidence"));
        assert!(assembled.body.contains("cost"));
    }

    #[test]
    fn assemble_document_renders_polish_and_readability_audit_sections() {
        use crate::readability::{PolishChange, PolishResult, ReadabilityAudit};
        let mut doc = sample_doc(sample_item());
        doc.polish = Some(PolishResult {
            changes: vec![PolishChange {
                field: "summary".into(),
                description: "normalized whitespace".into(),
            }],
            control_chars_removed: 1,
            whitespace_normalized: 2,
            empty_paragraphs_removed: 3,
            note: "Polished draft".into(),
        });
        doc.readability_audit = Some(ReadabilityAudit {
            score: 85,
            passed: true,
            issues: vec!["issue".into()],
            recommendations: vec!["rec".into()],
            avg_finding_length: 400,
            missing_label_count: 0,
            long_paragraph_count: 0,
        });
        let assembled = assemble_document(&doc);
        assert!(
            assembled.body.contains("## Polish"),
            "report layout must contain Polish section"
        );
        assert!(assembled.body.contains("1 control character(s) removed"));
        assert!(
            assembled.body.contains("## Readability Audit"),
            "report layout must contain Readability Audit section"
        );
        assert!(assembled.body.contains("85/100"));
        assert!(
            assembled
                .body
                .contains("average finding length 400 characters")
        );
    }

    #[test]
    fn assemble_document_renders_failed_citation_check_with_marker() {
        use crate::cite_checker::CitationCheckResult;
        let mut doc = sample_doc(sample_item());
        doc.cite_check = Some(CitationCheckResult {
            passed: false,
            checked: 1,
            failed_claims: vec!["CITATION_VERIFICATION_FAILED: [#1] missing body".into()],
            issues: vec!["[#1] has no captured body".into()],
            gate_open: false,
        });
        let assembled = assemble_document(&doc);
        assert!(assembled.body.contains("CITATION_VERIFICATION_FAILED"));
        assert!(assembled.body.contains("closed — human approval required"));
    }

    #[test]
    fn assemble_document_imrad_renders_citation_check_subsection() {
        use crate::cite_checker::CitationCheckResult;
        let mut doc = sample_doc(sample_item());
        doc.output_format = crate::run_config::OutputFormat::Imrad;
        doc.cite_check = Some(CitationCheckResult {
            passed: true,
            checked: 1,
            failed_claims: Vec::new(),
            issues: Vec::new(),
            gate_open: true,
        });
        let assembled = assemble_document(&doc);
        assert!(
            assembled.body.contains("### Citation Check"),
            "IMRaD layout must render Citation Check as a subsection"
        );
    }

    #[test]
    fn split_analysis_sentences_strips_html_and_strikethrough_attributes() {
        let body = "The claim <del>was wrong</del> is plausible. ~~Crossed out~~ text remains.";
        let out = split_analysis_sentences(body);
        assert_eq!(
            out,
            "The claim was wrong is plausible.\n\nCrossed out text remains."
        );
    }

    #[test]
    fn assemble_document_splits_analysis_sentences_onto_separate_lines() {
        let mut doc = sample_doc(sample_item());
        doc.findings = vec![
            "**Headline:** Observation summary\n\n\
             **Observation:** First observation. [#1]\n\n\
             **Analysis:** Sentence one. Sentence two. Sentence three.\n\n\
             **Cross-reference / Dependencies:** none\n\n\
             **Implication:** do something."
                .into(),
        ];
        let assembled = assemble_document(&doc);
        let finding = assembled.body.split("### **Finding 1**").nth(1).unwrap();
        // Each sentence of the Analysis body must be on its own paragraph.
        assert!(
            finding.contains("**Analysis:**\nSentence one.\n\nSentence two.\n\nSentence three."),
            "analysis sentences should be split onto separate lines: {finding}"
        );
        // The next label should still be separated from the analysis by a blank
        // line, preserving the existing label separation.
        assert!(
            finding.contains("Sentence three.\n\n**Cross-reference / Dependencies:**"),
            "cross-reference label should remain separated by a blank line: {finding}"
        );
    }
}
