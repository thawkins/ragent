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
//!    ## Summary
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

use crate::io::ResearchIo;
use crate::item::{ResearchItem, strip_control_chars};
use crate::research_name::ResearchName;
use crate::source::{LocalSourceKind, Source};
use crate::status::ResearchStatus;
use chrono::Utc;
use regex::Regex;

/// Maximum number of bytes allowed in a single untrusted source excerpt.
/// Sources larger than this are truncated to avoid blowing up RESEARCH.md
/// (NFR-006 + the size-cap risk in the PLAN.md Risks table).
pub const MAX_SOURCE_BODY_BYTES: usize = 256 * 1024;

/// The 9 sections that appear in every `RESEARCH.md`, in order (FR-010 + FR-012).
pub const REQUIRED_SECTIONS: &[&str] = &[
    "Topic",
    "Search Queries",
    "Summary",
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
    /// Sub-queries the web-gathering phase issued to the search tool. Empty
    /// when web gathering was disabled or no decomposer was configured.
    pub decomposed_queries: Vec<String>,
    /// Output artifact this document was requested as.
    pub output_format: crate::run_config::OutputFormat,
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
pub(crate) fn make_headline_from_observation(observation: &str) -> String {
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
///   layout: Topic, Search Queries, Summary, Findings, Findings Relationship
///   Diagram, In-Project Cross-References, Open Questions, References Index.
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
/// This preserves the original section order: Topic, Search Queries, Summary,
/// Findings, Findings Relationship Diagram, In-Project Cross-References,
/// Open Questions, References Index.
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

    // ── Summary ──────────────────────────────────────────────────────────
    body.push_str("## Summary\n\n");
    if doc.summary.trim().is_empty() {
        body.push_str("(no summary recorded yet — run a gathering pass to populate)\n");
    } else {
        body.push_str(&strip_control_chars(doc.summary.trim()));
        body.push('\n');
    }
    body.push('\n');

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
            body.push_str(&format!("### Finding {n} — {headline}\n\n{remainder}\n\n"));
        }
    }
    // ── Findings Relationship Diagram (FR-001 / FR-002 / FR-012) ────────────
    body.push_str(&crate::diagram::render_findings_diagram(&doc.findings));

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
    body.push_str("### Research Configuration\n\n");
    body.push_str(
        "Evidence was gathered through automated web search and local cross-reference \
                 scanning; the resulting corpus was synthesized into structured findings. \
                 Empty sections below indicate that the corresponding evidence was not yet \
                 produced by the gathering pass.\n\n",
    );

    // -- Results (FR-008) -----------------------------------------------
    body.push_str("## Results\n\n");
    body.push_str("### Summary\n\n");
    if doc.summary.trim().is_empty() {
        body.push_str("_(no summary recorded yet — run a gathering pass to populate)_\n\n");
    } else {
        body.push_str(&strip_control_chars(doc.summary.trim()));
        body.push_str("\n\n");
    }
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
            body.push_str(&format!("### Finding {n} — {headline}\n\n{remainder}\n\n"));
        }
    }
    // ── Findings Relationship Diagram (FR-001 / FR-002 / FR-012). In the
    // IMRaD layout it is a sub-section of Results, so we use a `###` heading
    // and ask the diagram renderer to return only the body.
    body.push_str("### Findings Relationship Diagram\n\n");
    body.push_str(&crate::diagram::render_findings_diagram_body(&doc.findings));

    // ── Discussion (FR-009) ────────────────────────────────────────────────
    body.push_str("## Discussion\n\n");
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
        cross_references: Vec::new(),
        open_questions: Vec::new(),
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
            ..
        } => Some(format!(
            "# Web source\n\n\
             - URL: {url}\n\
             - Title: {title}\n\
             - Published (UTC): {published}\n\
             - Captured (UTC): {captured}\n\
             - Relevance: {relevance}\n\n\
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
            out.push_str(&label);
            if !body.is_empty() {
                out.push('\n');
                out.push_str(body);
            }
        }
    }
    out.trim().to_string()
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
            cross_references: Vec::new(),
            open_questions: Vec::new(),
            template_body: None,
            decomposed_queries: Vec::new(),
            output_format: crate::run_config::OutputFormat::Report,
        }
    }

    #[test]
    fn assemble_document_includes_all_eight_sections() {
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
            .split("### Finding 1 — Observation summary\n\n")
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
            "### Finding 1 — Observation summary\n\n**Observation:**\nfirst observation"
        ));
        assert!(assembled.body.contains(
            "### Finding 2 — Observation summary\n\n**Observation:**\nsecond observation"
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
        let finding = assembled.body.split("### Finding 1").nth(1).unwrap();
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
        });
        let mut doc = sample_doc(item);
        doc.findings = vec!["Mixed [#2] and [#1] and again [#2].".into()];
        let assembled = assemble_document(&doc);
        let finding = assembled.body.split("### Finding 1").nth(1).unwrap();
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
        });
        let mut doc = sample_doc(item);
        doc.findings = vec![
            "**Headline:** Observation summary

**Observation:** spans [#1], [#2], and [#3]."
                .into(),
        ];
        let assembled = assemble_document(&doc);
        let finding = assembled.body.split("### Finding 1").nth(1).unwrap();
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
        });
        let mut doc = sample_doc(item);
        doc.findings = vec![
            "**Headline:** Observation summary

**Observation:** only [#1]."
                .into(),
        ];
        let assembled = assemble_document(&doc);
        let finding = assembled.body.split("### Finding 1").nth(1).unwrap();
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
        let finding = assembled.body.split("### Finding 1").nth(1).unwrap();
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
        let finding = assembled.body.split("### Finding 1").nth(1).unwrap();
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
}
