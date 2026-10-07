//! RESEARCH.md document assembly - legacy report layout and `IMRaD` layout.
//!
//! `RESEARCH.md` is the single, self-contained deliverable for each
//! research item. Two layouts are supported:
//!
//! 1. **Report** (default) - the original multi-section layout:
//!
//!    ```text
//!    # Title: <title>
//!
//!    ## Topic
//!    ## Search Queries
//!    ### Search Engine Summary   (after gathering, per-engine source counts)
//!    ## Executive Summary
//!    ## Top 10 Implications
//!    ## Open Questions
//!    ## Findings
//!    ## Findings Relationship Diagram
//!    ## In-Project Cross-References
//!    ## References Index
//!    ```
//!
//! 2. **`IMRaD`** - selected via [`OutputFormat::Imrad`](crate::run_config::OutputFormat);
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
use crate::io::{ResearchIo, cloak_url};
use crate::item::{ResearchItem, strip_control_chars};
use crate::locus::{DepthInvestigation, LocusSet};
use crate::reconcile::{CrossLocusReconcile, SourceTensions};
use crate::research_name::ResearchName;
use crate::source::{LocalSourceKind, Source};
use crate::status::ResearchStatus;
use crate::synthesis::SynthesisAudit;
use chrono::{Datelike, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Maximum number of bytes allowed in a single untrusted source excerpt.
/// Sources larger than this are truncated to avoid blowing up RESEARCH.md
/// (NFR-006 + the size-cap risk in the PLAN.md Risks table).
pub const MAX_SOURCE_BODY_BYTES: usize = 256 * 1024;

/// Rough per-source byte estimate used to pre-size the `--format
/// source-bibliography` output buffer (ANTIPAT F-22).
///
/// Each rendered entry is a short header plus a capped `Preview` block, so 512
/// bytes is a deliberately loose upper-bound hint that avoids a reallocation or
/// two in the common case. This is an allocation hint only: it never affects
/// the emitted output.
const BIBLIOGRAPHY_ENTRY_ESTIMATE_BYTES: usize = 512;

/// The 9 sections that appear in every `RESEARCH.md`, in order (FR-010 + FR-012).
pub const REQUIRED_SECTIONS: &[&str] = &[
    "Topic",
    "Search Queries",
    "Executive Summary",
    "Top 10 Implications",
    "Open Questions",
    "Findings",
    "Findings Relationship Diagram",
    "In-Project Cross-References",
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
    /// Numbered findings - each entry is the body of one bullet under
    /// `## Findings`. References inside the body use the form `[#N]`.
    pub findings: Vec<String>,
    /// Top 10 implications - one numbered entry per implication, in rank order.
    /// In the report layout this section now appears directly under
    /// `## Executive Summary`; in the IMRaD layout it is rendered under
    /// `## Discussion`.
    pub top_implications: Vec<String>,
    /// In-project cross-references (FR-009). Each entry is one bullet under
    /// `## In-Project Cross-References`.
    pub cross_references: Vec<CrossReference>,
    /// Open questions - one bullet per question. In the report layout this
    /// section appears directly under `## Top 10 Implications`; in the IMRaD
    /// layout it is rendered under `## Discussion`.
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
    /// Optional concept-extraction section produced by the `/research create`
    /// pipeline (spec researchcluster). The raw markdown is the normalized
    /// LLM concept list: `### N. Name` subsections with `[#N]` citations that
    /// resolve against the References Index. Rendered as `## Concepts`
    /// directly above `## Findings` (report layout) or `### Concepts` directly
    /// above `### Findings` (IMRaD layout); omitted entirely when `None`.
    pub concepts: Option<String>,
    /// Optional template body loaded from `research/_templates/<name>.md`
    /// (FR-020). When supplied, the template is used as the skeleton and
    /// `{{title}}`, `{{topic}}`, `{{date}}` placeholders are substituted
    /// from the item's metadata before the standard sections are appended.
    pub template_body: Option<String>,
    /// Explicit research brief used as the mission statement for synthesis and
    /// rendered as `## Research Brief` in the report (FR-004 / T-004).
    pub brief: Option<String>,
    /// Sub-queries the web-gathering phase issued to the search tool. Empty
    /// when web gathering was disabled or no decomposer was configured.
    pub decomposed_queries: Vec<String>,
    /// Output artifact this document was requested as.
    pub output_format: crate::run_config::OutputFormat,
    /// Pre-rendered comparison-table body for `OutputFormat::ComparisonTable`
    /// competitive-analysis runs (FR-014 / FR-016 / T-011). Contains the
    /// comparison criteria, cross-entity Markdown table, and per-entity
    /// profiles built from researcher summaries.
    pub comparison_table: Option<String>,
    /// Pre-rendered self-evaluation scorecard markdown (FR-008 / T-015).
    /// When `Some`, the `## Self-Evaluation Scorecard` section is inserted
    /// before the References Index.
    pub evaluation_scorecard: Option<String>,
    /// Pre-rendered per-provider search-request summary markdown. When
    /// `Some`, the `### Search Provider Requests` section is rendered next to
    /// the Search Engine Summary. `None` omits the section entirely.
    pub provider_stats: Option<String>,
}

/// One in-project cross-reference row (FR-009).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossReference {
    /// Project-relative path (e.g. `"src/lib.rs"`).
    pub path: String,
    /// One-line note explaining why this file is relevant.
    pub relevance: String,
}

/// Result of `assemble_document` - the body text plus the rendered file
/// payload (frontmatter + body) ready for `atomic_write`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssembledDocument {
    /// Full `RESEARCH.md` payload (frontmatter + body).
    pub content: String,
    /// Just the frontmatter block (without the leading/trailing `---`).
    pub frontmatter: String,
    /// Just the body text (without the frontmatter).
    pub body: String,
    /// Full `CORPA.md` companion payload. Carries the QA render sections
    /// (Contradiction Graph, Loci Analysis, Depth Investigation, Cross-Locus
    /// Reconcile, Source Tensions, Synthesis Audit, Corpus Critic) plus a
    /// `Sources Reference` copy of the references table. Written alongside
    /// `RESEARCH.md` by [`crate::manager::ResearchManager::write_document`].
    pub corpa: String,
}

/// Extract the headline for a finding and return the finding body with the
/// Headline paragraph removed.
///
/// If the finding contains a `**Headline:**` paragraph, its body is used as the
/// headline. Otherwise a fallback headline is derived from the first 15 words
/// of the `**Observation:**` paragraph (or the first sentence if shorter). The
/// returned headline is trimmed and never empty - it falls back to
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
/// * `OutputFormat::Report`
///   (default) and all other existing formats emit the legacy multi-section
///   layout: Topic, Search Queries, Executive Summary, Top 10 Implications,
///   Open Questions, Findings, Findings Relationship Diagram,
///   In-Project Cross-References, References Index.
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

    let mut body = String::with_capacity(8192);

    // FR-020 / imradreport: if a template body was supplied, use it as the
    // skeleton after substituting the standard placeholders.
    if let Some(template) = &doc.template_body {
        body.push_str(&apply_template(template, &title, &topic));
        body.push_str("\n\n");
    }

    // -- Title -----------------------------------------------------------
    body.push_str("# Title: ");
    body.push_str(&title);
    body.push_str("\n\n");

    // FR-004 / specs/imradreport: choose between the legacy report layout and
    // the IMRaD layout based on the configured output format.
    if doc.output_format == crate::run_config::OutputFormat::Imrad {
        body.push_str(&assemble_imrad_body(doc, &topic));
    } else if doc.output_format == crate::run_config::OutputFormat::ComparisonTable {
        body.push_str(&assemble_comparison_table_body(doc, &topic));
    } else {
        body.push_str(&assemble_report_body(doc, &topic));
    }

    // Make every bare URL in the rendered body clickable, while leaving URLs
    // inside code spans, fenced blocks, and existing Markdown links untouched.
    let mut body = linkify_urls(&body);

    // -- Self-Evaluation Scorecard (FR-008 / T-015) ------------------------
    // Insert the rendered scorecard before the References Index when available,
    // so it appears as a regular body section rather than inside frontmatter.
    if let Some(scorecard) = doc
        .evaluation_scorecard
        .as_deref()
        .filter(|s| !s.is_empty())
    {
        let scorecard = format!("\n{scorecard}");
        if let Some(pos) = body.rfind("\n## References Index") {
            body.insert_str(pos, &scorecard);
        } else {
            body.push_str(&scorecard);
        }
    }

    // -- CORPA.md companion document --------------------------------------
    // The QA render sections are split into this separate per-research
    // `CORPA.md` file so RESEARCH.md keeps its narrative focus (spec
    // corpusAnalysis). It reuses the same references table so `[#N]` source
    // indices resolve in both files.
    let mut corpa = String::with_capacity(4096);
    corpa.push_str("# Corpus Analysis Companion (CORPA.md)\n\n");
    corpa.push_str(&format!(
        "Quality-assurance companion document for `{title}`. Generated \
         together with `RESEARCH.md`; the `[#N]` source indices reference \
         the Sources Reference table at the bottom of this file.\n\n"
    ));
    corpa.push_str(&assemble_corpa_body(doc));
    corpa.push_str("## Sources Reference\n\n");
    corpa.push_str(&ResearchIo::render_references_index_table(
        &doc.item.sources,
        Utc::now(),
        doc.item.url_cloak,
    ));
    let corpa = linkify_urls(&corpa);

    let mut content = String::with_capacity(frontmatter.len() + 1 + body.len());
    content.push_str(&frontmatter);
    content.push('\n');
    content.push_str(&body);
    AssembledDocument {
        content,
        frontmatter: frontmatter
            .trim_start_matches("---\n")
            .trim_end_matches("---\n")
            .to_string(),
        body,
        corpa,
    }
}

/// Build the body of the per-research `CORPA.md` companion file.
///
/// CORPA.md carries the QA render sections that used to inline at the bottom
/// of `RESEARCH.md`, in a fixed order:
///
/// 1. Contradiction Graph
/// 2. Loci Analysis
/// 3. Depth Investigation
/// 4. Cross-Locus Reconcile
/// 5. Source Tensions
/// 6. Synthesis Audit
/// 7. Corpus Critic
///
/// Each section is emitted only when its artifact was produced, so a light
/// tier run yields a short file. The `Sources Reference` table is appended by
/// [`assemble_document`]. The report and IMRaD layouts share one CORPA body -
/// the companion document always uses top-level `##` headings so it renders
/// identically regardless of the `RESEARCH.md` layout in use.
fn assemble_corpa_body(doc: &ResearchDocument) -> String {
    let mut body = String::new();

    // -- Contradiction Graph (FR-005, T-007) -----------------------------
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

    // -- Loci Analysis (FR-005, T-008) -----------------------------------
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
                let indices = format_source_refs(&locus.source_indices);
                let snippets = if locus.snippets.is_empty() {
                    "-".to_string()
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
                    escape_pipe(&indices),
                    locus.mentions,
                    escape_pipe(&snippets)
                ));
            }
            body.push('\n');
        }
    }

    // -- Depth Investigation (FR-005, T-008) -------------------------------
    if let Some(investigations) = &doc.depth_investigation {
        body.push_str("## Depth Investigation\n\n");
        if investigations.is_empty() {
            body.push_str("_(no depth investigation available)_\n\n");
        } else {
            body.push_str("| Locus | Depth | Sources | Note |\n");
            body.push_str("|-------|-------|---------|------|\n");
            for inv in investigations {
                let sources = format_source_refs(&inv.representative_sources);
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

    // -- Cross-Locus Reconcile (FR-005, T-009) ---------------------------
    if let Some(reconcile) = &doc.cross_locus_reconcile {
        body.push_str("## Cross-Locus Reconcile\n\n");
        body.push_str(&render_cross_locus_reconcile(reconcile));
    }

    // -- Source Tensions (FR-005, T-009) --------------------------------
    if let Some(tensions) = &doc.source_tensions {
        body.push_str("## Source Tensions\n\n");
        body.push_str(&render_source_tensions(tensions));
    }

    // -- Synthesis Audit (FR-005, T-012) ---------------------------------
    if let Some(audit) = &doc.synthesis_audit {
        body.push_str("## Synthesis Audit\n\n");
        body.push_str(&render_synthesis_audit(audit));
    }

    // -- Corpus Critic (FR-005, T-010) -----------------------------------
    if let Some(report) = &doc.corpus_critic {
        body.push_str("## Corpus Critic\n\n");
        body.push_str(&render_corpus_critic(report));
    }

    body
}

/// Format a slice of source indices as a comma-separated list of `#N`
/// reference tokens (e.g. `&[1, 3]` -> `"#1, #3"`).
fn format_source_refs(indices: &[usize]) -> String {
    indices
        .iter()
        .map(|i| format!("#{i}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Return the strength of the strongest contradiction edge in `graph`, or `0`
/// when the graph has no edges.
///
/// Both the Data Quality summary and the scoreboard use this helper so their
/// strongest-edge figures always agree.
fn strongest_edge_strength(graph: &ContradictionGraph) -> u32 {
    graph
        .edges
        .iter()
        .map(|e| u32::from(e.strength))
        .max()
        .unwrap_or(0)
}

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
        let sources = format_source_refs(&claim.source_indices);
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
            "### Draft {} - {}\n\n{}\n\n*Sources: {}*\n\n",
            candidate.label,
            escape_pipe(&candidate.note),
            strip_control_chars(&candidate.body).trim(),
            format_source_refs(&candidate.source_indices)
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
        let shared = format_source_refs(&pair.shared_source_indices);
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
        let sources = format_source_refs(&t.source_indices);
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
        let indices = format_source_refs(&report.isolated_sources);
        out.push_str(&format!(
            "**Isolated sources:** {}\n\n",
            escape_pipe(&indices)
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

/// Render a concise "Data Quality & Consistency" summary that synthesizes the
/// five QA artifacts - corpus critic, contradiction graph, source tensions,
/// cross-locus reconcile, and synthesis audit - into a single overview block.
///
/// Returns an empty string when none of the five artifacts are present, so the
/// caller can omit the section heading entirely. When at least one artifact is
/// available the body is returned (without a heading); the caller is
/// responsible for emitting the appropriate `##` or `###` heading.
fn render_data_quality_summary(doc: &ResearchDocument) -> String {
    let has_corpus = doc.corpus_critic.is_some();
    let has_contradictions = doc.contradiction_graph.is_some();
    let has_tensions = doc.source_tensions.is_some();
    let has_reconcile = doc.cross_locus_reconcile.is_some();
    let has_audit = doc.synthesis_audit.is_some();
    if !(has_corpus || has_contradictions || has_tensions || has_reconcile || has_audit) {
        return String::new();
    }

    let mut out = String::new();

    // -- Verdict line ------------------------------------------------------
    // Prefer the synthesis-audit recommendation; fall back to the corpus-critic
    // pass/fail status; finally emit a neutral line when only the graph or
    // tensions are present.
    let verdict = if let Some(audit) = &doc.synthesis_audit {
        if !audit.recommendation.is_empty() {
            escape_pipe(&audit.recommendation)
        } else {
            format!("Synthesis audit scored {}/100.", audit.overall_score)
        }
    } else if let Some(report) = &doc.corpus_critic {
        let status = if report.passed { "pass" } else { "review" };
        format!("Corpus critic scored {}/100 ({}).", report.score, status)
    } else {
        "Quality data available - see metrics below.".to_string()
    };
    out.push_str(&format!("**Overall verdict:** {}\n\n", verdict));

    // -- Metrics table -----------------------------------------------------
    // Only rows with data are emitted, so a sparse QA run produces a compact
    // table rather than a row of placeholders.
    let mut rows: Vec<(String, String, String)> = Vec::new();

    if let Some(report) = &doc.corpus_critic {
        let status = if report.passed { "pass" } else { "review" };
        rows.push((
            "Corpus critic".into(),
            format!("{}/100 ({})", report.score, status),
            format!(
                "coverage {} * evidence {} * balance {} * tension {}",
                report.coverage_score,
                report.evidence_score,
                report.balance_score,
                report.tension_score
            ),
        ));
    }

    if let Some(graph) = &doc.contradiction_graph {
        let count = graph.edges.len();
        // Use the true maximum so the Data Quality row agrees with the
        // scoreboard's strongest-edge computation below.
        let strongest = strongest_edge_strength(graph);
        rows.push((
            "Contradictions".into(),
            format!("{count} edge(s)"),
            if strongest > 0 {
                format!("strongest = {strongest}/100")
            } else {
                "no edges".into()
            },
        ));
    }

    if let Some(tensions) = &doc.source_tensions {
        let total = tensions.tensions.len();
        let contradictions = tensions
            .tensions
            .iter()
            .filter(|t| t.kind == crate::reconcile::TensionKind::Contradiction)
            .count();
        let shallow = tensions
            .tensions
            .iter()
            .filter(|t| t.kind == crate::reconcile::TensionKind::ShallowEvidence)
            .count();
        let isolated = tensions
            .tensions
            .iter()
            .filter(|t| t.kind == crate::reconcile::TensionKind::IsolatedSource)
            .count();
        rows.push((
            "Source tensions".into(),
            format!("{total} tension(s)"),
            format!(
                "{} contradiction * {} shallow * {} isolated",
                contradictions, shallow, isolated
            ),
        ));
    }

    if let Some(reconcile) = &doc.cross_locus_reconcile {
        let pairs = reconcile.pairs.len();
        let conflicts: usize = reconcile.pairs.iter().map(|p| p.conflicting_edges).sum();
        rows.push((
            "Cross-locus reconcile".into(),
            format!("{pairs} pair(s)"),
            format!("{conflicts} conflicting edge(s)"),
        ));
    }

    if let Some(audit) = &doc.synthesis_audit {
        let status = if audit.overall_score >= 80 {
            "proceed"
        } else if audit.overall_score >= 50 {
            "caution"
        } else {
            "revise"
        };
        rows.push((
            "Synthesis audit".into(),
            format!("{}/100 ({})", audit.overall_score, status),
            format!("{} source(s) cited", audit.sources_used),
        ));
    }

    if !rows.is_empty() {
        out.push_str("| Metric | Value | Detail |\n");
        out.push_str("|--------|-------|--------|\n");
        for (metric, value, detail) in &rows {
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                escape_pipe(metric),
                escape_pipe(value),
                escape_pipe(detail)
            ));
        }
        out.push('\n');
    }

    // -- Key concerns ------------------------------------------------------
    // Collect the most salient issue from each artifact so the reader can scan
    // the headline problems without opening each detailed section.
    let mut concerns: Vec<String> = Vec::new();
    if let Some(report) = &doc.corpus_critic {
        for issue in report.issues.iter().take(2) {
            concerns.push(format!("Corpus: {}", escape_pipe(issue)));
        }
    }
    if let Some(graph) = &doc.contradiction_graph {
        for edge in graph.edges.iter().take(2) {
            concerns.push(format!(
                "Contradiction: {} vs {} - {}",
                edge.claim_a.source_index,
                edge.claim_b.source_index,
                escape_pipe(&edge.note)
            ));
        }
    }
    if let Some(tensions) = &doc.source_tensions {
        for t in tensions.tensions.iter().take(2) {
            let sources = format_source_refs(&t.source_indices);
            concerns.push(format!(
                "Tension ({}): {} [{}] - {}",
                t.kind.as_str(),
                escape_pipe(&t.label),
                sources,
                escape_pipe(&t.note)
            ));
        }
    }
    if let Some(reconcile) = &doc.cross_locus_reconcile {
        for pair in reconcile.pairs.iter().take(2) {
            if pair.conflicting_edges > 0 {
                concerns.push(format!(
                    "Reconcile: {} <-> {} - {} conflicting edge(s)",
                    escape_pipe(&pair.locus_a),
                    escape_pipe(&pair.locus_b),
                    pair.conflicting_edges
                ));
            }
        }
    }
    if let Some(audit) = &doc.synthesis_audit
        && !audit.summary.is_empty()
    {
        concerns.push(format!("Audit: {}", escape_pipe(audit.summary.trim())));
    }

    if !concerns.is_empty() {
        out.push_str("**Key concerns:**\n");
        for concern in &concerns {
            out.push_str(&format!("- {concern}\n"));
        }
        out.push('\n');
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
            "open - report may ship"
        } else {
            "closed - human approval required"
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
        "**Score estimate:** {} -> {}\n\n",
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

/// Extract the distinct hostnames of all gathered web sources.
///
/// The hostname is taken from the URL by stripping the scheme and everything
/// from the first `/`, `?`, or `#`; a leading `www.` is removed so
/// `www.example.com` and `example.com` count as one distinct domain.
fn distinct_web_domains(sources: &[Source]) -> Vec<String> {
    // HashSet dedup avoids the O(n^2) `Vec::contains` scan; sorting at the end
    // keeps the previous deterministic output order.
    let mut domains: std::collections::HashSet<String> = std::collections::HashSet::new();
    for source in sources {
        let Source::Web { url, .. } = source else {
            continue;
        };
        let host = url
            .split_once("://")
            .map_or(url.as_str(), |(_, rest)| rest)
            .split(['/', '?', '#'])
            .next()
            .unwrap_or("")
            .trim()
            .to_lowercase();
        if host.is_empty() {
            continue;
        }
        let host = host.strip_prefix("www.").unwrap_or(&host);
        if !host.is_empty() {
            domains.insert(host.to_string());
        }
    }
    let mut domains: Vec<String> = domains.into_iter().collect();
    domains.sort();
    domains
}

/// Render the Corpus Quality Scoreboard section (spec `corpusAnalysis`).
///
/// A deterministic, LLM-free at-a-glance block rendered immediately after the
/// document title and before the first body section (FR-011). The scoreboard
/// aggregates quality indicators the pipeline already computed:
///
/// - Score line (FR-002): `Quality: **74/100** - Grade B (Good)` with the
///   grade band from [`crate::scoreboard::GradeBand`]. Score precedence is
///   corpus critic, then synthesis audit (FR-006); with neither artifact the
///   line reads `Quality: Not graded` and no meter is rendered (FR-007).
/// - Meter bar (FR-003): 20-cell ASCII bar inside a fenced code block.
/// - Critic subscore line (FR-005): coverage / evidence / balance / tension
///   when a corpus critic report is present.
/// - Source-facts line (FR-004): gathered, cited, full-text, and
///   distinct-domain counts, plus average web relevance. For local-only runs
///   (zero web sources) the distinct-domain count and the average relevance
///   are omitted (FR-014), as is the cited date span.
/// - Tension/citation line (FR-009, FR-010): contradiction-edge count with
///   the strongest edge strength, and the citation-check status when a
///   cite-check result is present.
/// - Abbreviated formats (FR-013): for `executive-summary`,
///   `comparison-table`, and `source-bibliography` documents the critic
///   subscore line and the tension/citation line are omitted, leaving the
///   score line, meter bar, and source-facts block.
///
/// The existing `render_data_quality_summary` section is untouched (FR-012);
/// the scoreboard is an at-a-glance summary, not a replacement. No scoring
/// computation is modified (FR-015) and all output is ASCII-only (FR-016).
///
/// Returns an empty string when no quality artifact is available at all
/// (no gathered sources, no critic report, no audit), so skeleton documents
/// do not grow an empty section (FR-001).
fn render_scoreboard(doc: &ResearchDocument) -> String {
    let sources = &doc.item.sources;
    let has_critic = doc.corpus_critic.is_some();
    let has_audit = doc.synthesis_audit.is_some();
    if sources.is_empty() && !has_critic && !has_audit {
        return String::new();
    }

    // FR-013: abbreviated formats render only the score line, meter bar, and
    // source facts; the critic subscore and tension/citation lines are held
    // back for the full report and IMRaD layouts.
    let abbreviated = matches!(
        doc.output_format,
        crate::run_config::OutputFormat::ExecutiveSummary
            | crate::run_config::OutputFormat::ComparisonTable
            | crate::run_config::OutputFormat::SourceBibliography
    );

    let mut out = String::new();
    out.push_str("## Corpus Quality Scoreboard\n\n");

    // -- Score line + meter bar (FR-002, FR-003, FR-006, FR-007) ---------
    // Score precedence is corpus critic, then synthesis audit (FR-006).
    let score = doc
        .corpus_critic
        .as_ref()
        .map(|r| r.score)
        .or_else(|| doc.synthesis_audit.as_ref().map(|a| a.overall_score));
    match score {
        Some(s) => {
            let band = crate::scoreboard::GradeBand::from_score(s);
            out.push_str(&format!(
                "Quality: **{s}/100** - Grade {} ({})\n\n",
                band,
                band.meaning()
            ));
            out.push_str(&render_scoreboard_meter(s));
        }
        None => {
            out.push_str("Quality: Not graded\n\n");
        }
    }

    // -- Critic subscore line (FR-005, suppressed in abbreviated formats) -
    if !abbreviated && let Some(report) = &doc.corpus_critic {
        out.push_str(&format!(
            "- Critic: {} (coverage {} | evidence {} | balance {} | tension {})\n",
            if report.passed { "pass" } else { "review" },
            report.coverage_score,
            report.evidence_score,
            report.balance_score,
            report.tension_score
        ));
    }

    // -- Source-facts line (FR-004, FR-014) ------------------------------
    let cited = cited_source_indices(doc);
    let cited_count = cited.len();
    let full_text = sources.iter().filter(|s| s.has_body()).count();
    let web_domains = distinct_web_domains(sources);
    out.push_str(&format!(
        "- Sources: {} gathered | {} cited | {} full text",
        sources.len(),
        cited_count,
        full_text
    ));
    if !web_domains.is_empty() {
        out.push_str(&format!(
            " | {} distinct domains | {:.1}/8 average relevance",
            web_domains.len(),
            average_web_relevance(sources)
        ));
    }
    out.push('\n');

    // -- Cited date-span line (FR-004) -----------------------------------
    if let Some((earliest, latest, undated)) = cited_date_span(doc, &cited) {
        out.push_str(&format!(
            "- Cited date span: {earliest}-{latest} ({undated} undated)\n"
        ));
    }

    // -- Tension/citation line (FR-009, FR-010, suppressed by FR-013) ----
    if !abbreviated {
        let mut tension_parts: Vec<String> = Vec::new();
        if let Some(graph) = &doc.contradiction_graph
            && !graph.edges.is_empty()
        {
            let strongest = strongest_edge_strength(graph);
            tension_parts.push(format!(
                "Contradictions: {} edges (strongest {strongest}/100)",
                graph.edges.len()
            ));
        }
        if let Some(check) = &doc.cite_check {
            let status = if check.passed { "passed" } else { "failed" };
            tension_parts.push(format!("Citation check: {status}"));
        }
        if !tension_parts.is_empty() {
            out.push_str(&format!("- {}\n", tension_parts.join(" | ")));
        }
    }

    out.push('\n');
    out
}

/// Render the fenced-code-block meter bar for the scoreboard (FR-003).
fn render_scoreboard_meter(score: u32) -> String {
    format!(
        "```\n{}\n```\n\n",
        crate::scoreboard::render_meter_bar(score)
    )
}

/// Distinct, in-range source indices cited by the document narrative.
///
/// Citations are collected from the summary, findings, top implications, and
/// open questions via the shared `[#N]` regex; indices outside the gathered
/// source range are ignored so the count never exceeds the corpus size.
fn cited_source_indices(doc: &ResearchDocument) -> Vec<usize> {
    let mut texts = String::new();
    texts.push_str(&doc.summary);
    for finding in &doc.findings {
        texts.push('\n');
        texts.push_str(finding);
    }
    for implication in &doc.top_implications {
        texts.push('\n');
        texts.push_str(implication);
    }
    for question in &doc.open_questions {
        texts.push('\n');
        texts.push_str(question);
    }
    let total = doc.item.sources.len();
    crate::polarity::cited_indices(&texts)
        .into_iter()
        .filter(|n| *n <= total)
        .collect()
}

/// Average relevance rank (`x.x`) across gathered web sources (FR-004).
///
/// Local, spec, and other sources have no relevance label and are excluded;
/// callers must not invoke this when the run gathered zero web sources
/// (FR-014 omission is handled by [`render_scoreboard`]).
fn average_web_relevance(sources: &[Source]) -> f64 {
    // Single pass: count web sources and sum their ranks together.
    let (web_count, rank_sum) = sources
        .iter()
        .filter(|s| matches!(s, Source::Web { .. }))
        .fold((0usize, 0u32), |(n, sum), s| {
            (n + 1, sum + u32::from(s.relevance_rank()))
        });
    if web_count == 0 {
        return 0.0;
    }
    f64::from(rank_sum) / web_count as f64
}

/// Publication-date span of the cited web sources (FR-004).
///
/// Returns `(earliest_year, latest_year, undated_count)` over the cited web
/// sources that carry a publication date; `undated_count` covers cited web
/// sources without one. `None` when no cited source exposes a date.
fn cited_date_span(doc: &ResearchDocument, cited: &[usize]) -> Option<(i32, i32, usize)> {
    let sources = &doc.item.sources;
    let mut earliest: Option<i32> = None;
    let mut latest: Option<i32> = None;
    let mut undated = 0usize;
    for index in cited {
        let Some(source) = sources.get(index - 1) else {
            continue;
        };
        let Source::Web { published_at, .. } = source else {
            continue;
        };
        match published_at {
            Some(date) => {
                let year = date.year();
                earliest = Some(earliest.map_or(year, |e: i32| e.min(year)));
                latest = Some(latest.map_or(year, |l: i32| l.max(year)));
            }
            None => undated += 1,
        }
    }
    Some((earliest?, latest?, undated))
}

/// Shared section emitters used by both report layouts.
///
/// The legacy report layout (`assemble_report_body`) and the IMRaD layout
/// (`assemble_imrad_body`) render the same document data; the only structural
/// difference is the heading level (`##` top-level sections vs `###`
/// sub-sections) and the surrounding section order. These helpers carry the
/// shared rendering so each layout stays a thin, ordering-only function.
#[allow(clippy::redundant_pub_crate)]
mod layout {
    use super::{
        ResearchDocument, ResearchIo, Utc, escape_pipe, extract_headline, normalize_finding_labels,
        render_finding_sources, render_search_engine_summary, strip_control_chars,
    };

    /// Heading prefix for a section at the given nesting level (`1` = `##`).
    fn heading(level: u8) -> &'static str {
        if level == 1 { "## " } else { "### " }
    }

    /// `## Search Queries` block (shared verbatim between layouts).
    pub(crate) fn push_search_queries(body: &mut String, doc: &ResearchDocument, level: u8) {
        body.push_str(heading(level));
        body.push_str("Search Queries\n\n");
        if doc.decomposed_queries.is_empty() {
            body.push_str(
                "_(no query decomposition was used - the original topic was searched as a single query)_\n\n",
            );
        } else {
            for q in &doc.decomposed_queries {
                body.push_str(&format!("- {}\n", strip_control_chars(q).trim()));
            }
            body.push('\n');
        }
    }

    /// `Search Engine Summary` block - per-engine breakdown of acquired web
    /// sources by media type, emitted only when at least one web source
    /// carries a non-empty `search_engine` field (absent for skeletons and
    /// pre-gathering documents).
    pub(crate) fn push_search_engine_summary(body: &mut String, doc: &ResearchDocument, level: u8) {
        let engine_summary = render_search_engine_summary(&doc.item.sources);
        if !engine_summary.is_empty() {
            body.push_str(heading(level));
            body.push_str("Search Engine Summary\n\n");
            body.push_str(&engine_summary);
            body.push('\n');
        }
    }

    /// `Search Provider Requests` block - per-provider search-request totals
    /// for the run, rendered only when the session recorded at least one
    /// provider call.
    pub(crate) fn push_provider_requests(body: &mut String, doc: &ResearchDocument, level: u8) {
        if let Some(provider_stats) = &doc.provider_stats
            && !provider_stats.trim().is_empty()
        {
            body.push_str(heading(level));
            body.push_str("Search Provider Requests\n\n");
            body.push_str(provider_stats.trim_end());
            body.push_str("\n\n");
        }
    }

    /// `Open Questions` block (shared verbatim between layouts).
    pub(crate) fn push_open_questions(body: &mut String, doc: &ResearchDocument, level: u8) {
        body.push_str(heading(level));
        body.push_str("Open Questions\n\n");
        if doc.open_questions.is_empty() {
            body.push_str("_(none)_\n\n");
        } else {
            for q in &doc.open_questions {
                body.push_str(&format!("- {}\n", strip_control_chars(q).trim()));
            }
            body.push('\n');
        }
    }

    /// `Findings` section body with per-finding headline extraction and
    /// source attribution. The caller emits the section heading itself (the
    /// two layouts differ: `## Findings` vs `## Results` + `### Findings`).
    pub(crate) fn push_findings(body: &mut String, doc: &ResearchDocument) {
        if doc.findings.is_empty() {
            body.push_str(
                "_(no findings yet - the gathering pass will populate this section)_\n\n",
            );
            return;
        }
        let cloak_urls = doc.item.url_cloak;
        for (idx, finding) in doc.findings.iter().enumerate() {
            let n = idx + 1;
            let normalized = normalize_finding_labels(strip_control_chars(finding).trim());
            let (headline, mut remainder) = extract_headline(&normalized, n);
            if let Some(sources_list) =
                render_finding_sources(&remainder, &doc.item.sources, cloak_urls)
            {
                remainder.push_str("\n\n");
                remainder.push_str(&sources_list);
            }
            body.push_str(&format!(
                "\n### **Finding {n}** - {headline}\n\n{remainder}\n\n"
            ));
        }
    }

    /// `In-Project Cross-References` block (shared verbatim between layouts).
    pub(crate) fn push_cross_references(body: &mut String, doc: &ResearchDocument, level: u8) {
        body.push_str(heading(level));
        body.push_str("In-Project Cross-References\n\n");
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
    }

    /// QA artifact sub-sections rendered identically in both layouts:
    /// Evidence Digest, Triple Draft, Gap-Fill Fetch, Surgical Patch,
    /// Citation Check, Polish, and Readability Audit. Each is omitted
    /// entirely when its artifact is absent.
    pub(crate) fn push_qa_sections(body: &mut String, doc: &ResearchDocument, level: u8) {
        if let Some(digest) = &doc.evidence_digest {
            body.push_str(heading(level));
            body.push_str("Evidence Digest\n\n");
            body.push_str(&super::render_evidence_digest(digest));
        }

        if let Some(draft) = &doc.triple_draft {
            body.push_str(heading(level));
            body.push_str("Triple Draft\n\n");
            body.push_str(&super::render_triple_draft(draft));
        }

        if let Some(result) = &doc.gap_fetch {
            body.push_str(heading(level));
            body.push_str("Gap-Fill Fetch\n\n");
            body.push_str(&super::render_gap_fetch(result));
        }

        if let Some(result) = &doc.surgical_patch {
            body.push_str(heading(level));
            body.push_str("Surgical Patch\n\n");
            body.push_str(&super::render_surgical_patch(result));
        }

        if let Some(result) = &doc.cite_check {
            body.push_str(heading(level));
            body.push_str("Citation Check\n\n");
            body.push_str(&super::render_citation_check(result));
        }

        if let Some(result) = &doc.polish {
            body.push_str(heading(level));
            body.push_str("Polish\n\n");
            body.push_str(&super::render_polish(result));
        }

        if let Some(audit) = &doc.readability_audit {
            body.push_str(heading(level));
            body.push_str("Readability Audit\n\n");
            body.push_str(&super::render_readability_audit(audit));
        }
    }

    /// `References Index` block (shared verbatim; date captured at call time).
    pub(crate) fn push_references_index(body: &mut String, doc: &ResearchDocument) {
        body.push_str(&ResearchIo::render_references_index(
            &doc.item.sources,
            Utc::now(),
            doc.item.url_cloak,
        ));
    }
}

use layout::{
    push_cross_references, push_findings, push_open_questions, push_provider_requests,
    push_qa_sections, push_references_index, push_search_engine_summary, push_search_queries,
};

/// Build the body of a legacy multi-section `RESEARCH.md` report.
///
/// This preserves the original section order: Topic, Search Queries, Executive
/// Summary, Top 10 Implications, Open Questions, Findings, Findings
/// Relationship Diagram, In-Project Cross-References, References Index.
fn assemble_report_body(doc: &ResearchDocument, topic: &str) -> String {
    let mut body = String::new();

    // -- Corpus Quality Scoreboard (spec corpusAnalysis, FR-011 / FR-012) --
    // Rendered immediately after the title and before the first body section
    // (## Topic). Omitted entirely for skeleton documents with no gathered
    // sources and no QA artifacts. The detailed Data Quality & Consistency
    // section below is untouched (FR-012).
    body.push_str(&render_scoreboard(doc));

    // -- Topic -----------------------------------------------------------
    body.push_str("## Topic\n\n");
    body.push_str(topic.trim());
    body.push_str("\n\n");

    // -- Research Brief (FR-004 / T-004) -------------------------------
    if let Some(brief) = doc.brief.as_deref().filter(|b| !b.is_empty()) {
        body.push_str("## Research Brief\n\n");
        body.push_str(&strip_control_chars(brief).trim());
        body.push_str("\n\n");
    }

    // -- Search Queries -------------------------------------------------
    push_search_queries(&mut body, doc, 1);

    // -- Search Engine Summary ------------------------------------------
    // Per-engine breakdown of acquired web sources by media type (pages,
    // PDFs, videos). Rendered as a `###` sub-section under Search Queries
    // even in the legacy layout (pre-existing behaviour).
    push_search_engine_summary(&mut body, doc, 2);

    // -- Search Provider Requests ----------------------------------------
    // Per-provider search-request totals for the run (how many requests were
    // sent to each search tool/engine), rendered as a `###` sub-section
    // (pre-existing behaviour) only when the session recorded at least one
    // provider call.
    push_provider_requests(&mut body, doc, 2);

    // -- Executive Summary -------------------------------------------------
    body.push_str("## Executive Summary\n\n");
    if doc.summary.trim().is_empty() {
        body.push_str("_(no executive summary recorded yet - run a gathering pass to populate)_\n");
    } else {
        body.push_str(&strip_control_chars(doc.summary.trim()));
        body.push('\n');
    }
    body.push('\n');

    // -- Top 10 Implications ----------------------------------------------
    body.push_str("## Top 10 Implications\n\n");
    if doc.top_implications.is_empty() {
        body.push_str(
            "_(no ranked implications yet - the synthesis pass will populate this section)_\n\n",
        );
    } else {
        for (idx, imp) in doc.top_implications.iter().enumerate() {
            let n = idx + 1;
            let cleaned = strip_control_chars(imp).trim().to_string();
            body.push_str(&format!("{n}. {cleaned}\n"));
        }
        body.push('\n');
    }

    // -- Open Questions --------------------------------------------------
    push_open_questions(&mut body, doc, 1);

    // -- Data Quality & Consistency --------------------------------------
    // Synthesized overview of the QA artifacts, placed between the Top 10
    // Implications and Findings so the reader sees the quality summary before
    // diving into the detailed sections. Omitted entirely when no QA data
    // is present (standard gathering run without full-tier QA).
    let dq_summary = render_data_quality_summary(doc);
    if !dq_summary.is_empty() {
        body.push_str("## Data Quality & Consistency\n\n");
        body.push_str(&dq_summary);
    }

    // -- Concepts (spec researchcluster) --------------------------------
    // LLM-extracted concept list from the gathered corpus, rendered directly
    // above Findings so the reader sees the cross-source theme map before the
    // per-finding detail. Omitted entirely when concept extraction did not
    // run (no LLM engine wired, or the extraction produced no sections).
    if let Some(concepts) = &doc.concepts {
        body.push_str("## Concepts\n\n");
        body.push_str(concepts.trim());
        body.push_str("\n\n");
    }

    // -- Findings ---------------------------------------------------------
    body.push_str("## Findings\n\n");
    push_findings(&mut body, doc);
    // -- Findings Relationship Diagram (FR-001 / FR-002 / FR-012) ------------
    body.push_str(&crate::diagram::render_findings_diagram(&doc.findings));
    // NOTE: the QA render sections (Contradiction Graph, Loci Analysis,
    // Depth Investigation, Cross-Locus Reconcile, Source Tensions,
    // Synthesis Audit, Corpus Critic) moved to the per-research CORPA.md
    // companion file -- see `assemble_corpa_body`.

    // QA artifact sub-sections (Digest -> Readability Audit) share one
    // renderer with the IMRaD layout.
    push_qa_sections(&mut body, doc, 1);
    // NOTE: Cross-Locus Reconcile, Source Tensions, Synthesis Audit, and
    // Corpus Critic also moved to CORPA.md (`assemble_corpa_body`).

    // -- In-Project Cross-References -------------------------------------
    push_cross_references(&mut body, doc, 1);

    // -- References Index (FR-011) ----------------------------------------
    push_references_index(&mut body, doc);

    body
}

/// Build the body of an IMRaD-compliant `RESEARCH.md` report.
///
/// Section order follows the `IMRaD` convention: Abstract, Introduction, Methods,
/// Results, Discussion, References Index. The same `ResearchDocument` fields are
/// reused; only the headings and grouping change.
fn assemble_imrad_body(doc: &ResearchDocument, topic: &str) -> String {
    let mut body = String::new();

    // -- Corpus Quality Scoreboard (spec corpusAnalysis, FR-011 / FR-012) --
    // Rendered immediately after the title and before the Abstract (IMRaD
    // layout). Omitted entirely for skeleton documents with no gathered
    // sources and no QA artifacts. The detailed Data Quality & Consistency
    // subsection inside Discussion below is untouched (FR-012).
    body.push_str(&render_scoreboard(doc));

    // -- Abstract (FR-005) -----------------------------------------------
    body.push_str("## Abstract\n\n");
    if doc.summary.trim().is_empty() {
        body.push_str(
            "_(no abstract recorded yet - run a gathering pass to populate this section)_\n\n",
        );
    } else {
        body.push_str(&strip_control_chars(doc.summary.trim()));
        body.push_str("\n\n");
    }

    // -- Introduction (FR-006) ---------------------------------------------
    body.push_str("## Introduction\n\n");
    if topic.trim().is_empty() {
        body.push_str("_(no research topic specified)_\n\n");
    } else {
        body.push_str(strip_control_chars(topic).trim());
        body.push_str("\n\n");
        // -- Research Brief (FR-004 / T-004) ------------------------------
        if let Some(brief) = doc.brief.as_deref().filter(|b| !b.is_empty()) {
            body.push_str("### Research Brief\n\n");
            body.push_str(&strip_control_chars(brief).trim());
            body.push_str("\n\n");
        }
        body.push_str(
            "This research item investigates the topic above by gathering and synthesizing \
             web sources, local project files, and related specifications. The objective is \
             to produce evidence-based findings that can be mapped back to the project \
             context.\n\n",
        );
    }

    // -- Methods (FR-007) -------------------------------------------------
    body.push_str("## Methods\n\n");
    push_search_queries(&mut body, doc, 2);

    // -- Search Engine Summary (IMRaD Methods sub-section) -------------
    // Per-engine breakdown of acquired web sources by media type. Only
    // emitted when at least one web source has a non-empty search_engine.
    push_search_engine_summary(&mut body, doc, 2);

    // -- Search Provider Requests (IMRaD Methods sub-section) -----------
    // Per-provider search-request totals for the run, rendered only when
    // the session recorded at least one provider call.
    push_provider_requests(&mut body, doc, 2);

    body.push_str("### Research Configuration\n\n");
    body.push_str(
        "Evidence was gathered through automated web search and local cross-reference \
                 scanning; the resulting corpus was synthesized into structured findings. \
                 Empty sections below indicate that the corresponding evidence was not yet \
                 produced by the gathering pass.\n\n",
    );

    // -- Concepts (spec researchcluster) -------------------------------
    // In the IMRaD layout the concept list is a Results sub-section rendered
    // directly above the Findings subsection.
    if let Some(concepts) = &doc.concepts {
        body.push_str("### Concepts\n\n");
        body.push_str(concepts.trim());
        body.push_str("\n\n");
    }

    // -- Results (FR-008) -----------------------------------------------
    body.push_str("## Results\n\n");
    body.push_str("### Findings\n\n");
    push_findings(&mut body, doc);
    // -- Findings Relationship Diagram (FR-001 / FR-002 / FR-012). In the
    // IMRaD layout it is a sub-section of Results, so we use a `###` heading
    // and ask the diagram renderer to return only the body.
    body.push_str("### Findings Relationship Diagram\n\n");
    body.push_str(&crate::diagram::render_findings_diagram_body(&doc.findings));

    // -- Discussion (FR-009) ------------------------------------------------
    body.push_str("## Discussion\n\n");

    // -- Data Quality & Consistency --------------------------------------
    // Synthesized QA overview at the start of Discussion, before the detailed
    // contradiction/reconcile/audit subsections. Omitted entirely when no QA
    // data is present.
    let dq_summary = render_data_quality_summary(doc);
    if !dq_summary.is_empty() {
        body.push_str("### Data Quality & Consistency\n\n");
        body.push_str(&dq_summary);
    }

    // NOTE: the QA subsections (Contradiction Graph, Loci Analysis, Depth
    // Investigation) moved to the per-research CORPA.md companion file --
    // see `assemble_corpa_body`.

    // QA artifact sub-sections share one renderer with the legacy report
    // layout; in IMRaD they are Discussion sub-sections.
    push_qa_sections(&mut body, doc, 2);
    // NOTE: Cross-Locus Reconcile, Source Tensions, Synthesis Audit, and
    // Corpus Critic also moved to CORPA.md (`assemble_corpa_body`).

    push_cross_references(&mut body, doc, 2);
    push_open_questions(&mut body, doc, 2);

    // -- References Index (FR-010) -----------------------------------------
    push_references_index(&mut body, doc);

    body
}

/// Build the body of a dedicated comparison-table `RESEARCH.md` artifact.
fn assemble_comparison_table_body(doc: &ResearchDocument, topic: &str) -> String {
    let mut body = String::new();

    body.push_str(&render_scoreboard(doc));

    body.push_str("## Topic\n\n");
    body.push_str(topic.trim());
    body.push_str("\n\n");

    if let Some(brief) = doc.brief.as_deref().filter(|b| !b.is_empty()) {
        body.push_str("## Research Brief\n\n");
        body.push_str(&strip_control_chars(brief).trim());
        body.push_str("\n\n");
    }

    // -- Executive Summary ----------------------------------------------
    // Rendered before the comparison table (like the report layout places it
    // before the findings) so readers see the synthesis at the top of the
    // artifact instead of after the entity profiles.
    body.push_str("## Executive Summary\n\n");
    if doc.summary.trim().is_empty() {
        body.push_str("_(no executive summary recorded yet - the synthesis pass will populate)_\n");
    } else {
        body.push_str(&strip_control_chars(doc.summary.trim()));
        body.push('\n');
    }
    body.push('\n');

    if let Some(table) = doc.comparison_table.as_deref().filter(|t| !t.is_empty()) {
        // Trim the block's trailing blank lines (the builder already ends with
        // `\n\n`) so a doubled blank-line gap does not precede Findings.
        body.push_str(strip_control_chars(table).trim_end());
        body.push_str("\n\n");
    } else {
        body.push_str(
            "## Comparison Table\n\n_(no comparison table was produced for this run)_\n\n",
        );
    }

    body.push_str("## Findings\n\n");
    if doc.findings.is_empty() {
        body.push_str("_(no findings yet - the gathering pass will populate this section)_\n\n");
    } else {
        for (idx, finding) in doc.findings.iter().enumerate() {
            let n = idx + 1;
            let normalized = normalize_finding_labels(strip_control_chars(finding).trim());
            let (headline, mut remainder) = extract_headline(&normalized, n);
            if let Some(sources_list) =
                render_finding_sources(&remainder, &doc.item.sources, doc.item.url_cloak)
            {
                remainder.push_str("\n\n");
                remainder.push_str(&sources_list);
            }
            body.push_str(&format!(
                "\n### **Finding {n}** - {headline}\n\n{remainder}\n\n"
            ));
        }
    }

    body.push_str(&ResearchIo::render_references_index(
        &doc.item.sources,
        Utc::now(),
        doc.item.url_cloak,
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
/// `OutputFormat::Report`.
#[must_use]
pub fn render_skeleton(
    name: &ResearchName,
    title: &str,
    topic: &str,
    output_format: crate::run_config::OutputFormat,
) -> String {
    assemble_document(&placeholder_document(name, title, topic, output_format)).content
}

/// Render the `CORPA.md` companion skeleton written alongside the
/// `RESEARCH.md` skeleton by [`crate::manager::ResearchManager::create`].
/// No QA sections have artifacts at creation time, so the skeleton carries
/// only the header and the `Sources Reference` placeholder table - this
/// keeps the file well-formed the moment it lands on disk.
#[must_use]
pub fn render_corpa_skeleton(
    name: &ResearchName,
    title: &str,
    topic: &str,
    output_format: crate::run_config::OutputFormat,
) -> String {
    assemble_document(&placeholder_document(name, title, topic, output_format)).corpa
}

/// Build the empty placeholder `ResearchDocument` shared by [`render_skeleton`]
/// and [`render_corpa_skeleton`]. Non-default output formats are persisted in
/// the frontmatter (FR-012) so the skeleton records the requested artifact.
fn placeholder_document(
    name: &ResearchName,
    title: &str,
    topic: &str,
    output_format: crate::run_config::OutputFormat,
) -> ResearchDocument {
    let mut placeholder = ResearchItem::new(name.clone(), title, topic);
    // Persist non-default formats in the frontmatter (FR-012) so the skeleton
    // records the requested artifact from the moment it is created.
    if output_format != crate::run_config::OutputFormat::Report {
        placeholder.output_format = Some(output_format.as_str().to_string());
    }
    ResearchDocument {
        item: placeholder,
        summary: String::new(),
        findings: Vec::new(),
        top_implications: Vec::new(),
        cross_references: Vec::new(),
        open_questions: Vec::new(),
        concepts: None,
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
        brief: None,
        decomposed_queries: Vec::new(),
        output_format,
        comparison_table: None,
        evaluation_scorecard: None,
        provider_stats: None,
    }
}

/// Apply the FR-020 template substitution to `template_body`.
///
/// Recognised placeholders:
///
/// - `{{title}}` - the research item title.
/// - `{{topic}}` - the topic description.
/// - `{{date}}` - the current UTC date (`YYYY-MM-DD`).
/// - `{{name}}` - the research name.
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
        // {{name}} is substituted only when present in the template - we
        // don't have the name here, so we use a placeholder-friendly default
        // that the caller can replace after the fact if needed.
        .replace("{{name}}", "")
}

/// Build a `**Sources:**` paragraph for a finding that cites one or more
/// captured sources. Each bullet contains the citation number, source title,
/// author (when known), and path/URL so the reader can map the finding back to
/// the References Index. Returns `None` when there are no citations or none of
/// the cited indices map to a known source.
///
/// A `**Source date range:**` line is appended after the bullet list showing
/// the earliest and latest publication dates of the cited *web* sources, so
/// the reader can judge the relative age of the evidence backing the finding.
/// The line reads `-` when no cited web source exposes a publication date.
///
/// When `cloak_urls` is `true`, web sources have their URL defanged with
/// [`cloak_url`] so the bullet emits it as plain text rather than a clickable
/// link (`/research create --url-cloak`).
fn render_finding_sources(finding: &str, sources: &[Source], cloak_urls: bool) -> Option<String> {
    // If the finding already contains a Sources paragraph (e.g. produced by
    // the LLM itself), don't append a duplicate list. Match the `**Sources:**`
    // literal in its three case variants directly to avoid allocating a full
    // lowercase copy of the finding body on every call.
    if finding.contains("**Sources:**")
        || finding.contains("**sources:**")
        || finding.contains("**SOURCES:**")
    {
        return None;
    }

    let indices = crate::polarity::cited_indices(finding);
    if indices.is_empty() {
        return None;
    }

    let mut out = String::from("**Sources:**\n");
    use std::fmt::Write;
    let mut any = false;
    for idx in &indices {
        if let Some(src) = sources.get(idx - 1) {
            any = true;
            let author = src.author().filter(|a| !a.is_empty());
            let _ = write!(out, "- [{idx}] {}", src.title()); // INTENTIONAL: write to a String buffer is infallible
            if let Some(a) = author {
                let _ = write!(out, " [{a}]"); // INTENTIONAL: write to a String buffer is infallible
            }
            // `--url-cloak`: defang web URLs so this bullet carries them as
            // plain text rather than a clickable link.
            let location = if cloak_urls && matches!(src, Source::Web { .. }) {
                cloak_url(src.path_or_url())
            } else {
                src.path_or_url().to_string()
            };
            let _ = write!(out, " - {location}"); // INTENTIONAL: write to a String buffer is infallible
            if let Some(dt) = src.published_at() {
                let _ = write!(out, " (published {})", dt.format("%Y-%m-%d")); // INTENTIONAL: write to a String buffer is infallible
            }
            out.push('\n');
        }
    }
    if !any {
        return None;
    }
    // Append the date-range line summarising the publication dates of the
    // cited web sources, so the relative age of the evidence is visible at
    // a glance per finding.
    if let Some(range) = render_finding_date_range(&indices, sources) {
        out.push('\n');
        out.push_str(&range);
    }
    // Trim trailing newline; the caller inserts blank lines.
    out.truncate(out.trim_end().len());
    Some(out)
}

/// Compute a `**Source date range:**` summary line for the cited sources.
///
/// Considers only web sources that expose a `published_at` value. Unknown
/// citation indices are skipped (matching [`render_finding_sources`]) instead
/// of aborting the whole line. The returned line uses the form
/// `earliest..latest` (both inclusive, `YYYY-MM-DD`), or a single date when
/// all dated sources share the same publication date; when the finding cites
/// web sources but none of them expose a date it reads `-` with an
/// explanatory suffix.
fn render_finding_date_range(indices: &[usize], sources: &[Source]) -> Option<String> {
    let mut dates: Vec<chrono::DateTime<chrono::Utc>> = Vec::new();
    let mut total_web = 0usize;
    for idx in indices {
        // Tolerate stale citations (e.g. a source list that shrank since the
        // finding was written): skip the unknown index instead of dropping
        // the whole date-range line.
        let Some(src) = sources.get(idx.checked_sub(1)?) else {
            continue;
        };
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
        return Some(if total_web > 0 {
            "**Source date range:** - (cited web sources did not expose a publication date)"
                .to_string()
        } else {
            "**Source date range:** - (no web sources cited)".to_string()
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
    let fence_re = fence_re();

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
fn url_regex() -> &'static Regex {
    static URL_RE: OnceLock<Regex> = OnceLock::new();
    URL_RE.get_or_init(|| {
        // INVARIANT: compile-time-constant regex; the call cannot fail at runtime.
        Regex::new(r"(?i)\bhttps?://[a-zA-Z0-9_~:/.?#@!$&'()*+,;=%-]+").expect("valid url regex")
    })
}

/// Cached fenced-code-block line pattern used by [`linkify_urls`].
fn fence_re() -> &'static Regex {
    static FENCE_RE: OnceLock<Regex> = OnceLock::new();
    // INVARIANT: compile-time-constant regex; the call cannot fail at runtime.
    FENCE_RE.get_or_init(|| Regex::new(r"(?m)^```.*$").expect("valid fence regex"))
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
    // Count parens once and adjust the counter as `end` retreats so the
    // imbalance check stays O(1) per step instead of rescanning the prefix
    // (which made the loop worst-case O(n^2) per URL).
    let opens = raw[..end].chars().filter(|&cc| cc == '(').count();
    let mut closes = raw[..end].chars().filter(|&cc| cc == ')').count();
    while end > 0 {
        let c = raw.as_bytes()[end - 1] as char;
        if c == ')' {
            if closes > opens {
                end -= 1;
                closes -= 1;
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
///
/// The body is additionally neutralised for use inside a ```` ```text ````
/// fence: any backtick run is broken up so an embedded fence delimiter cannot
/// close the block, and any line that would render as a `#### Source [#N]`
/// header is escaped (SEC-ragent-research-006, SECTASKS T-063).
#[must_use]
pub fn fence_source_body(body: &str) -> String {
    let trimmed = body.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() <= MAX_SOURCE_BODY_BYTES {
        return neutralise_fenced_body(trimmed);
    }
    neutralise_fenced_body(&truncate_body_to_bytes(trimmed, MAX_SOURCE_BODY_BYTES))
}

/// Make a fetched body safe to embed inside a ```` ```text ```` fence.
///
/// Two escapes are neutralised: a backtick run (which could close the fence and
/// inject prompt text) is split with a zero-width space, and a line that
/// would render as a `#### Source [#...]` header or a `**Sources:**` heading is
/// escaped so it cannot spoof a citation block. The body is otherwise returned
/// verbatim.
#[must_use]
pub fn neutralise_fenced_body(body: &str) -> String {
    let mut out = String::with_capacity(body.len() + 64);
    for (idx, line) in body.split('\n').enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        let trimmed_start = line.trim_start();
        // Spoofed source headers (`#### Source [#999] (web) ...`) are the
        // citation-forgery vector; escape the leading `#` run so markdown no
        // longer sees a heading while the text stays readable.
        if trimmed_start.starts_with("#### Source [#") || trimmed_start.starts_with("**Sources:**")
        {
            out.push_str("\\");
        }
        // Break every backtick run so no ```` ``` ```` delimiter survives.
        for ch in line.chars() {
            if ch == '`' {
                out.push('`');
                out.push('\u{200B}');
            } else {
                out.push(ch);
            }
        }
    }
    out
}

/// Strip userinfo and the query string from a URL for safe logging.
///
/// SEC-ragent-research-007 (SECTASKS T-063): search-hit and OA-recovered URLs
/// regularly carry credentials or presigned tokens in the query string
/// (`?access_token=...`). The gather log keeps the scheme/host/path so it stays
/// diagnostically useful while the secret-bearing components are dropped.
/// Non-URL input is returned unchanged.
#[must_use]
pub fn mask_url_credentials(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    let (authority, tail) = match rest.find(['/', '?', '#']) {
        Some(pos) => (&rest[..pos], &rest[pos..]),
        None => (rest, ""),
    };
    let host = authority.rsplit('@').next().unwrap_or(authority);
    let path = match tail.find(['?', '#']) {
        Some(pos) => &tail[..pos],
        None => tail,
    };
    format!("{scheme}://{host}{path}")
}

/// Truncate `body` to at most `max_bytes` UTF-8 bytes, cutting at the nearest
/// char boundary, and append a marker so the reader knows content was capped.
#[must_use]
pub fn truncate_body_to_bytes(body: &str, max_bytes: usize) -> String {
    if body.len() <= max_bytes {
        return body.to_string();
    }
    let cut = ragent_types::strutil::floor_char_boundary(body, max_bytes);
    let mut out = String::with_capacity(cut + 64);
    out.push_str(&body[..cut]);
    out.push_str("\n\n... _(truncated - body exceeded the per-source size cap)_\n");
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
///
/// When `cloak_urls` is `true`, web-source URLs (the header `- URL:` line and
/// any open-access recovery URL) are defanged with [`cloak_url`] so the
/// supporting file emits them as plain text rather than clickable links,
/// matching the `Sources` bullets and the References Index (`/research create
/// --url-cloak`; ANTIPAT F-13).
#[must_use]
pub fn render_supporting_file(source: &Source, cloak_urls: bool) -> Option<String> {
    match source {
        Source::Web {
            url,
            title,
            captured_at,
            published_at,
            body,
            relevance,
            oa_recovery,
            language,
            author,
            ..
        } => {
            let header_url = if cloak_urls {
                cloak_url(url)
            } else {
                url.clone()
            };
            let recovery_note = match oa_recovery {
                Some(r) => {
                    let version = r.version.as_deref().unwrap_or("unspecified");
                    let license = r.license.as_deref().unwrap_or("unspecified");
                    let recovery_url = if cloak_urls {
                        cloak_url(&r.url)
                    } else {
                        r.url.clone()
                    };
                    format!(
                        "- Open-access recovery: full text fetched from {source} ({recovery_url}); version={version}, license={license}",
                        source = r.source,
                    )
                }
                None => String::new(),
            };
            Some(format!(
                "# Web source\n\n\
                 - URL: {url}\n\
                 - Title: {title}\n\
                 - Author(s): {author}\n\
                 - Language: {language}\n\
                 - Published (UTC): {published}\n\
                 - Captured (UTC): {captured}\n\
                 - Relevance: {relevance}\n\
                 {recovery_note}\n\n\
                 ```text\n{body}\n```\n",
                url = header_url,
                title = title,
                author = author.as_deref().unwrap_or("-"),
                language = language.as_deref().unwrap_or("-"),
                published = published_at.map_or_else(|| "-".to_string(), |dt| dt.to_rfc3339()),
                captured = captured_at.to_rfc3339(),
                relevance = if relevance.is_empty() {
                    "-"
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
                    "(no excerpt captured - file could not be read)"
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
        // INVARIANT: compile-time-constant regex; the call cannot fail at runtime.
        Regex::new(r"(?i)</?[a-z][a-z0-9]*(?:\s[^>]*)?/?>").expect("valid regex")
    });
    let strike = STRIKE_RE.get_or_init(|| {
        // Markdown strikethrough: ~~...~~
        // INVARIANT: compile-time-constant regex; the call cannot fail at runtime.
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
/// input - including any embedded newlines - is collapsed to single spaces before
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
    let mut collapsed = String::with_capacity(body.len());
    let mut first = true;
    for word in body.split_whitespace() {
        if !first {
            collapsed.push(' ');
        }
        collapsed.push_str(word);
        first = false;
    }
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
    static PARAGRAPH_PREFIX_RE: OnceLock<Regex> = OnceLock::new();
    static LABEL_RE: OnceLock<Regex> = OnceLock::new();
    let paragraph_prefix = PARAGRAPH_PREFIX_RE
        // INVARIANT: compile-time-constant regex; the call cannot fail at runtime.
        .get_or_init(|| Regex::new(r"Paragraph\s+\d+\s*-\s*").expect("valid regex"));
    let label_re = LABEL_RE.get_or_init(|| {
        // INVARIANT: compile-time-constant regex; the call cannot fail at runtime.
        Regex::new(r"(\*\*[-A-Za-z/\s]+:\*\*|\*[-A-Za-z/\s]+:\*)").expect("valid regex")
    });

    let mut text = finding.trim().replace("\n\n\n", "\n\n");

    // Strip stale "Paragraph N - " prefixes before any label.
    text = paragraph_prefix.replace_all(&text, "").to_string();

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

    let mut out = String::with_capacity(finding.len());
    for (raw_label, body) in labels.iter().zip(bodies.iter()) {
        // Normalize legacy italic labels (*Label:*) to bold (**Label:**).
        let label = if raw_label.starts_with("**") {
            raw_label.clone()
        } else if raw_label.len() >= 2 && raw_label.starts_with('*') && raw_label.ends_with('*') {
            let mut s = String::with_capacity(raw_label.len() + 2);
            s.push_str("**");
            s.push_str(&raw_label[1..raw_label.len() - 1]);
            s.push_str("**");
            s
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
/// `"openalex, wikipedia"`). This function splits that field, counts the
/// `media_type` (`"page"`, `"pdf"`, `"youtube"`) per engine, and emits a
/// Markdown table:
///
/// ```text
/// | Engine | Pages | PDFs | Videos | Total |
/// |--------|-------|------|--------|-------|
/// | openalex | 5 | 1 | 0 | 6 |
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

/// Render the per-provider search-request summary section.
///
/// `tool_calls` holds `(search tool, call count)` pairs as snapshotted from
/// the run's [`crate::provider_stats::ProviderCallStats`] counter. The output
/// is a Markdown table with one row per provider; returns an empty string
/// when the list is empty so callers can unconditionally attach the result.
///
/// # Example
///
/// ```text
/// | Search Provider | Requests |
/// |-----------------|----------|
/// | mf_search       | 12       |
/// ```
#[must_use]
pub fn render_provider_stats_summary(tool_calls: &[(String, usize)]) -> String {
    if tool_calls.is_empty() {
        return String::new();
    }
    let mut out = String::from("| Search Provider | Requests |\n");
    out.push_str("|-----------------|----------|\n");
    for (tool, count) in tool_calls {
        out.push_str(&format!("| {} | {} |\n", escape_pipe(tool), count));
    }
    out
}

/// Render the end-of-run per-provider search-request suffix, e.g.
/// `, 12 search request(s) (mf_search: 12)`.
///
/// Returns an empty string when `tool_calls` is empty. Shared by the `ragent
/// research` CLI and the TUI `/research` surface so their summaries cannot
/// drift (see `ANTIPAT.md` M3.10).
#[must_use]
pub fn provider_calls_suffix(tool_calls: &[(String, usize)]) -> String {
    if tool_calls.is_empty() {
        return String::new();
    }
    let per_tool = tool_calls
        .iter()
        .map(|(tool, count)| format!("{tool}: {count}"))
        .collect::<Vec<_>>()
        .join(", ");
    let total: usize = tool_calls.iter().map(|(_, count)| count).sum();
    format!(", {total} search request(s) ({per_tool})")
}

/// Render a standalone sources appendix / bibliography for the
/// `--format source-bibliography` artifact (T-011).
///
/// The output is a markdown document listing every source with its type,
/// title, path/URL, captured timestamp, and (when available) the first
/// [`BIBLIOGRAPHY_PREVIEW_CHARS`] characters of its body.
///
/// When `cloak_urls` is `true`, web-source URLs are defanged with [`cloak_url`]
/// so the emitted URL is plain text rather than a clickable link, matching the
/// `Sources` bullets, the References Index, and the supporting files
/// (`/research create --url-cloak`; ANTIPAT F-13).
///
/// [`BIBLIOGRAPHY_PREVIEW_CHARS`]: crate::limits::BIBLIOGRAPHY_PREVIEW_CHARS
#[must_use]
pub fn render_bibliography(sources: &[Source], cloak_urls: bool) -> String {
    if sources.is_empty() {
        return "# Sources Bibliography\n\n_(no sources captured)_\n".to_string();
    }
    let mut out = String::with_capacity(sources.len() * BIBLIOGRAPHY_ENTRY_ESTIMATE_BYTES);
    out.push_str("# Sources Bibliography\n\n");
    use std::fmt::Write;
    // Shared limit resolution: `0` would mean "unbounded" (FR-016), a positive
    // cap truncates to that many characters (ANTIPAT F-15).
    let preview_cap = crate::limits::effective_limit(crate::limits::BIBLIOGRAPHY_PREVIEW_CHARS);
    for (idx, source) in sources.iter().enumerate() {
        let n = idx + 1;
        let kind = source.type_str();
        let path = if cloak_urls && matches!(source, Source::Web { .. }) {
            cloak_url(source.path_or_url())
        } else {
            source.path_or_url().to_string()
        };
        let title = source.title();
        let captured = source.captured_at().to_rfc3339();
        let _ = write!(
            // INTENTIONAL: write to a String buffer is infallible
            out,
            "## [{n}] {title}\n\n- **Type:** {kind}\n- **Path/URL:** {path}\n- **Captured:** {captured}\n"
        );
        if let Some(rel) = source.relevance()
            && !rel.is_empty()
        {
            let _ = writeln!(out, "- **Relevance:** {rel}"); // INTENTIONAL: write to a String buffer is infallible
        }
        if let Some(body) = source.body()
            && let Some(cap) = preview_cap
        {
            // Short-circuit: only count up to `cap + 1` chars to decide whether
            // to truncate, avoiding a full O(n) scan of large bodies.
            let needs_truncation = body.chars().take(cap + 1).count() > cap;
            let preview = if needs_truncation {
                let mut p = body.chars().take(cap).collect::<String>();
                p.push('\u{2026}');
                p
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
#[path = "../tests/inline/document_tests.rs"]
mod tests;
