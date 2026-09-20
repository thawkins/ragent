//! Mermaid findings relationship diagram generator (rsearchdiag).
//!
//! This module renders a [Mermaid.js](https://mermaid.js.org/) `flowchart TD`
//! diagram from the **Cross-reference / Dependencies** paragraphs of a research
//! document's findings. Each finding becomes a node and each `Finding N`
//! reference becomes a directed edge, enabling readers to see how the
//! evidence builds on itself before they reach the prose sections.
//!
//! The generation is deterministic, uses only in-memory string operations, and
//! performs no I/O or LLM calls (NFR-001, NFR-003).

/// Classified dependency strength for a finding-to-finding edge (FR-007).
///
/// The strength is derived from keywords in the **Cross-reference /
/// Dependencies:** paragraph. Strong relationships use a thicker stroke;
/// weak relationships use a thinner stroke; all other references use the
/// default stroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeStrength {
    /// Strong relationship keywords such as "builds on" or "depends on".
    Strong,
    /// Weak relationship keywords such as "contradicts" or "see also".
    Weak,
    /// No strong/weak keyword detected.
    #[default]
    Default,
}

impl EdgeStrength {
    /// Return the Mermaid `stroke-width` value for this strength.
    #[must_use]
    pub const fn stroke_width(self) -> &'static str {
        match self {
            Self::Strong => "4px",
            Self::Weak => "1.5px",
            Self::Default => "2px",
        }
    }
}

/// One directed edge in the findings dependency graph.
///
/// The `from` field is the 1-based finding number that contains the
/// reference; the `to` field is the 1-based finding number being
/// referenced. The `strength` field captures the semantic weight of the
/// dependency phrasing (FR-007).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FindingEdge {
    /// Source finding node (the referrer).
    pub from: usize,
    /// Target finding node (the referenced finding).
    pub to: usize,
    /// Classified dependency strength derived from the surrounding prose.
    pub strength: EdgeStrength,
}

use regex::Regex;
use std::sync::OnceLock;

static FINDING_RE: OnceLock<Regex> = OnceLock::new();

fn finding_re() -> &'static Regex {
    FINDING_RE.get_or_init(|| Regex::new(r"(?i)\bfinding\s+(\d+)\b").expect("valid finding regex"))
}

/// Extract dependency edges from the **Cross-reference / Dependencies:**
/// paragraphs of the supplied findings.
///
/// This is the parser required by T-002. It scans each finding for
/// `Finding N` references, but only inside the dependency paragraph. Each
/// edge is classified as strong, weak, or default based on the wording
/// in the clause that contains the reference (FR-007). Edges are returned
/// in the order they are first encountered, and self-loops, out-of-range
/// references, and duplicate edges between the same pair of findings are
/// removed (FR-006, FR-009, FR-010). When duplicate edges with differing
/// strengths are encountered, the first encountered strength wins.
///
/// # Arguments
///
/// * `findings` — one string per finding body, in document order.
///
/// # Returns
///
/// A deduplicated, topologically-ordered-by-encounter list of directed
/// edges. The list is empty when there are no valid dependencies.
#[must_use]
pub fn extract_finding_edges(findings: &[String]) -> Vec<FindingEdge> {
    if findings.len() <= 1 {
        return Vec::new();
    }
    let finding_re = finding_re();
    let mut edges: Vec<FindingEdge> = Vec::new();
    for (idx, finding) in findings.iter().enumerate() {
        let from = idx + 1;
        let deps = extract_dependency_paragraph(finding);
        for cap in finding_re.captures_iter(&deps) {
            let to: usize = cap[1].parse().unwrap_or(0);
            if to == 0 || to > findings.len() || to == from {
                continue;
            }
            let start = cap.get(0).map_or(0, |m| m.start());
            let strength = classify_edge_strength(&deps, start);
            let candidate = FindingEdge { from, to, strength };
            if !edges
                .iter()
                .any(|e| e.from == candidate.from && e.to == candidate.to)
            {
                edges.push(candidate);
            }
        }
    }
    edges
}

/// Render just the body of the findings relationship diagram, without a
/// section heading.
///
/// This helper is used when the caller already supplies its own heading (for
/// example, `### Findings Relationship Diagram` inside the `IMRaD` `## Results`
/// section). It emits the same Mermaid code or empty-state placeholder as
/// [`render_findings_diagram`], but omits the leading `##` heading line.
#[must_use]
pub fn render_findings_diagram_body(findings: &[String]) -> String {
    let mut out = String::new();
    if findings.is_empty() {
        out.push_str("_(no findings yet — the gathering pass will populate this section)_\n");
        return out;
    }

    out.push_str("```mermaid\nflowchart TD\n");

    // Node declarations: one per finding.
    for (idx, finding) in findings.iter().enumerate() {
        let n = idx + 1;
        let headline = extract_headline_for_diagram(finding, n);
        out.push_str(&format!("    F{n}[\"{n} — {headline}\"]\n"));
    }

    // Edge declarations with deterministic ordering.
    let edges = extract_finding_edges(findings);

    if !edges.is_empty() {
        out.push('\n');
        for (idx, edge) in edges.iter().enumerate() {
            out.push_str(&format!("    F{} --> F{}\n", edge.from, edge.to));
            out.push_str(&format!(
                "    linkStyle {idx} stroke-width:{width}\n",
                idx = idx,
                width = edge.strength.stroke_width()
            ));
        }
    }

    // Node font sizing based on in-degree (FR-008).
    let mut in_degree = vec![0usize; findings.len()];
    for edge in &edges {
        if let Some(counter) = in_degree.get_mut(edge.to - 1) {
            *counter += 1;
        }
    }
    let central_nodes: Vec<usize> = in_degree
        .iter()
        .enumerate()
        .filter(|(_, count)| **count >= 2)
        .map(|(idx, _)| idx + 1)
        .collect();
    out.push('\n');
    out.push_str("    classDef central font-size:15px;\n");
    out.push_str("    classDef normal font-size:12px;\n");
    for n in 1..=findings.len() {
        if central_nodes.contains(&n) {
            out.push_str(&format!("    class F{n} central;\n"));
        } else {
            out.push_str(&format!("    class F{n} normal;\n"));
        }
    }

    out.push_str("```\n");
    out
}

/// Render a complete `## Findings Relationship Diagram` section.
///
/// This is the public entry point used by callers that need a self-contained
/// top-level section. The generation is deterministic, uses only in-memory
/// string operations, and performs no I/O or LLM calls (NFR-001, NFR-003).
/// The output starts with a blank line, the `##` heading, a blank line, and
/// then the diagram body produced by [`render_findings_diagram_body`].
#[must_use]
pub fn render_findings_diagram(findings: &[String]) -> String {
    let body = render_findings_diagram_body(findings);
    format!("\n## Findings Relationship Diagram\n\n{body}")
}

/// Find the end of a labeled paragraph, stopping at the next bold label.
///
/// Labels may be separated by either a single newline (`\n**Label:**`) or
/// the canonical blank line (`\n\n**Label:**`). The earliest match wins.
/// This prevents one label's body from leaking into the next label's
/// extraction when the LLM emits compact single-newline findings.
fn find_label_boundary(text: &str) -> usize {
    let double = text.find("\n\n**");
    let single = text.find("\n**");
    match (double, single) {
        (Some(d), Some(s)) => d.min(s),
        (Some(d), None) => d,
        (None, Some(s)) => s,
        (None, None) => text.len(),
    }
}

/// Extract the headline for a finding, suitable for a Mermaid node label.
///
/// If the finding contains a `**Headline:**` paragraph, its body is returned.
/// Otherwise a short fallback is derived from the `**Observation:**`
/// paragraph. The result is escaped so it is safe inside a Mermaid quoted
/// node label.
///
/// Uses [`find_label_boundary`] so the extraction stops at the next bold
/// label whether the LLM separated it with one newline or the canonical
/// blank line.
fn extract_headline_for_diagram(finding: &str, finding_number: usize) -> String {
    const LABEL: &str = "**Headline:**";
    let headline = finding.find(LABEL).and_then(|start| {
        let after = &finding[start + LABEL.len()..];
        let end = find_label_boundary(after);
        let body = after[..end].trim();
        if body.is_empty() {
            None
        } else {
            Some(body.to_string())
        }
    });

    let headline = headline.unwrap_or_else(|| {
        finding.find("**Observation:**").map_or_else(
            || format!("Finding {finding_number}"),
            |start| {
                let after = &finding[start + "**Observation:**".len()..];
                let end = find_label_boundary(after);
                let body = after[..end].trim();
                crate::document::make_headline_from_observation(body)
            },
        )
    });

    escape_mermaid_label(&headline)
}

/// Return just the body of the **Cross-reference / Dependencies:** paragraph.
///
/// If the paragraph cannot be located, an empty string is returned so no
/// edges are extracted for this finding.
fn extract_dependency_paragraph(finding: &str) -> String {
    const LABEL: &str = "**Cross-reference / Dependencies:**";
    finding
        .find(LABEL)
        .map(|start| {
            let after = &finding[start + LABEL.len()..];
            let end = find_label_boundary(after);
            after[..end].trim().to_string()
        })
        .unwrap_or_default()
}

/// Escape characters that would break a Mermaid quoted node label.
///
/// Replaces characters that Mermaid treats as syntax inside quoted labels
/// (`|`, `[`, `]`, `#`, `\`) with safe escaped or stripped forms, turns `"`
/// and `` ` `` into single quotes (Mermaid does not support escaping either
/// character inside a double-quoted label), and collapses `\r\n`, `\r` and
/// `\n` into single spaces. The result is trimmed.
/// This keeps the generated diagram valid without relying on a full Mermaid
/// parser (FR-015, NFR-002).
fn escape_mermaid_label(s: &str) -> String {
    // Double literal backslashes first so the subsequent escapes are not
    // re-escaped (FR-015).
    s.replace('\\', "\\\\")
        .replace(['"', '`'], "'")
        .replace('|', "\\|")
        .replace('[', "(")
        .replace(']', ")")
        .replace('#', "")
        .replace(['\r', '\n'], " ")
        .trim()
        .to_string()
}

/// Classify the dependency strength of a `Finding N` reference based on the
/// wording of the clause that contains it.
///
/// The classification is deliberately simple and keyword-driven:
///
/// * Strong: "builds on", "depends on", "prerequisite to",
///   "relies on", "requires".
/// * Weak: "contradicts", "relates to", "see also", "related to",
///   "compares to", "contrary to".
/// * Default: anything else.
///
/// Only the clause containing the reference is inspected, and within that
/// clause only the coordinate segment that contains the reference is used.
/// This lets adjacent references in the same dependency paragraph have
/// different strengths.
fn classify_edge_strength(paragraph: &str, match_start: usize) -> EdgeStrength {
    let lower = paragraph.to_lowercase();
    let clause = clause_around(&lower, match_start);
    let segment = segment_containing(clause, match_start);
    if is_strong_clause(segment) {
        EdgeStrength::Strong
    } else if is_weak_clause(segment) {
        EdgeStrength::Weak
    } else {
        EdgeStrength::Default
    }
}

/// Extract the clause surrounding `match_start`, delimited by sentence
/// boundaries (`.`, `!`, `?`) or paragraph ends.
fn clause_around(paragraph: &str, match_start: usize) -> &str {
    let bytes = paragraph.as_bytes();
    if match_start >= bytes.len() {
        return paragraph;
    }
    let start = bytes[..match_start]
        .iter()
        .rposition(|&b| matches!(b, b'.' | b'!' | b'?' | b'\n' | b'\r'))
        .map_or(0, |pos| pos + 1);
    let end = bytes[match_start..]
        .iter()
        .position(|&b| matches!(b, b'.' | b'!' | b'?' | b'\n' | b'\r'))
        .map_or(paragraph.len(), |pos| match_start + pos);
    &paragraph[start..end]
}

/// Return the sub-segment of `clause` that contains `match_start`.
///
/// Segments are split at coordinating conjunctions (`and`, `but`, `while`,
/// `whereas`, `yet`, `or`) and commas, so each `Finding N` reference in a
/// compound sentence can have its own strength.
fn segment_containing(clause: &str, match_start: usize) -> &str {
    if match_start >= clause.len() {
        return clause;
    }
    static SEGMENT_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = SEGMENT_RE.get_or_init(|| {
        regex::Regex::new(r"(?:,\s+|\s+(?:and|but|while|whereas|yet|or)\s+)").expect("valid regex")
    });
    let mut seg_start = 0;
    for m in re.find_iter(clause) {
        let seg_end = m.start();
        if match_start < seg_end {
            return &clause[seg_start..seg_end];
        }
        seg_start = m.end();
    }
    &clause[seg_start..]
}

fn is_strong_clause(clause: &str) -> bool {
    [
        "builds on",
        "depends on",
        "prerequisite to",
        "relies on",
        "requires",
    ]
    .iter()
    .any(|phrase| clause.contains(phrase))
}

fn is_weak_clause(clause: &str) -> bool {
    [
        "contradicts",
        "relates to",
        "see also",
        "related to",
        "compares to",
        "contrary to",
    ]
    .iter()
    .any(|phrase| clause.contains(phrase))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_finding_edges_empty_and_single() {
        assert!(extract_finding_edges(&[]).is_empty());
        let one = vec!["**Cross-reference / Dependencies:** Builds on Finding 1.".into()];
        assert!(
            extract_finding_edges(&one).is_empty(),
            "single finding has no valid edges"
        );
    }

    #[test]
    fn extract_finding_edges_parses_dependency_paragraph_only() {
        let findings = vec![
            "**Observation:** child. **Analysis:** a. **Cross-reference / Dependencies:** Builds on Finding 2. **Implication:** i.".into(),
            "**Observation:** root. **Analysis:** b. **Cross-reference / Dependencies:** No direct dependencies. **Implication:** j.".into(),
        ];
        let edges = extract_finding_edges(&findings);
        assert_eq!(
            edges,
            vec![FindingEdge {
                from: 1,
                to: 2,
                strength: EdgeStrength::Strong,
            }]
        );
    }

    #[test]
    fn extract_finding_edges_ignores_finding_n_outside_dependency_paragraph() {
        let findings = vec![
            "**Observation:** mentions Finding 2. **Analysis:** a. **Cross-reference / Dependencies:** No direct dependencies. **Implication:** i.".into(),
            "**Observation:** root. **Analysis:** b. **Cross-reference / Dependencies:** No direct dependencies. **Implication:** j.".into(),
        ];
        let edges = extract_finding_edges(&findings);
        assert!(
            edges.is_empty(),
            "references outside dependency paragraph must not become edges"
        );
    }

    #[test]
    fn extract_finding_edges_dedupes_self_loops_and_out_of_range() {
        let findings = vec![
            "**Cross-reference / Dependencies:** See Finding 1, Finding 2, finding 2, Finding 0, Finding 99.".into(),
            "**Cross-reference / Dependencies:** No direct dependencies.".into(),
        ];
        let edges = extract_finding_edges(&findings);
        assert_eq!(
            edges,
            vec![FindingEdge {
                from: 1,
                to: 2,
                strength: EdgeStrength::Default,
            }]
        );
    }

    #[test]
    fn extract_finding_edges_preserves_multiple_distinct_edges() {
        let findings = vec![
            "**Cross-reference / Dependencies:** Depends on Finding 2 and relies on Finding 3."
                .into(),
            "**Cross-reference / Dependencies:** No direct dependencies.".into(),
            "**Cross-reference / Dependencies:** No direct dependencies.".into(),
        ];
        let edges = extract_finding_edges(&findings);
        assert_eq!(
            edges,
            vec![
                FindingEdge {
                    from: 1,
                    to: 2,
                    strength: EdgeStrength::Strong,
                },
                FindingEdge {
                    from: 1,
                    to: 3,
                    strength: EdgeStrength::Strong,
                },
            ]
        );
    }

    #[test]
    fn extract_finding_edges_classifies_strength_by_clause() {
        let findings = vec![
            "**Cross-reference / Dependencies:** Depends on Finding 2. See also Finding 3.".into(),
            "**Cross-reference / Dependencies:** No direct dependencies.".into(),
            "**Cross-reference / Dependencies:** No direct dependencies.".into(),
        ];
        let edges = extract_finding_edges(&findings);
        assert_eq!(
            edges,
            vec![
                FindingEdge {
                    from: 1,
                    to: 2,
                    strength: EdgeStrength::Strong,
                },
                FindingEdge {
                    from: 1,
                    to: 3,
                    strength: EdgeStrength::Weak,
                },
            ]
        );
    }

    #[test]
    fn render_empty_findings_returns_section_with_placeholder() {
        let out = render_findings_diagram(&[]);
        assert!(
            out.contains("## Findings Relationship Diagram"),
            "section heading must still be present"
        );
        assert!(
            out.contains("_(no findings yet — the gathering pass will populate this section)_"),
            "placeholder text must be present: {out}"
        );
        assert!(
            !out.contains("```mermaid"),
            "no Mermaid block for zero findings"
        );
        assert!(
            !out.contains("flowchart TD"),
            "no Mermaid graph for zero findings"
        );
    }

    #[test]
    fn render_single_finding_emits_one_node_and_no_edges() {
        let findings = vec!["**Headline:** Rust async runtime".into()];
        let out = render_findings_diagram(&findings);
        assert!(out.contains("## Findings Relationship Diagram"));
        assert!(out.contains("flowchart TD"));
        assert!(out.contains("F1[\"1 — Rust async runtime\"]"));
        assert!(!out.contains("-->"), "single finding has no edges");
    }

    #[test]
    fn render_multi_finding_emits_nodes_edges_and_linkstyle() {
        let findings = vec![
            "**Headline:** Child\n\n**Observation:** child observation.\n\n**Analysis:** a.\n\n**Cross-reference / Dependencies:** Builds on Finding 2.\n\n**Implication:** i.".into(),
            "**Headline:** Root\n\n**Observation:** root observation.\n\n**Analysis:** b.\n\n**Cross-reference / Dependencies:** No direct dependencies.\n\n**Implication:** j.".into(),
        ];
        let out = render_findings_diagram(&findings);
        assert!(out.contains("F1[\"1 — Child\"]"));
        assert!(out.contains("F2[\"2 — Root\"]"));
        assert!(out.contains("F1 --> F2"));
        assert!(out.contains("linkStyle 0 stroke-width:4px"));
    }

    #[test]
    fn render_assigns_central_class_for_high_indegree() {
        let findings = vec![
            "**Headline:** Hub\n\n**Cross-reference / Dependencies:** No direct dependencies."
                .into(),
            "**Headline:** A\n\n**Cross-reference / Dependencies:** Depends on Finding 1.".into(),
            "**Headline:** B\n\n**Cross-reference / Dependencies:** Builds on Finding 1.".into(),
        ];
        let out = render_findings_diagram(&findings);
        assert!(out.contains("classDef central font-size:15px;"));
        assert!(out.contains("classDef normal font-size:12px;"));
        assert!(out.contains("class F1 central;"));
        assert!(out.contains("class F2 normal;"));
        assert!(out.contains("class F3 normal;"));
    }

    #[test]
    fn render_assigns_normal_class_for_low_indegree() {
        let findings = vec![
            "**Headline:** A\n\n**Cross-reference / Dependencies:** Depends on Finding 2.".into(),
            "**Headline:** B\n\n**Cross-reference / Dependencies:** No direct dependencies.".into(),
        ];
        let out = render_findings_diagram(&findings);
        assert!(out.contains("classDef normal font-size:12px;"));
        assert!(out.contains("class F1 normal;"));
        assert!(out.contains("class F2 normal;"));
        // The `central` classDef is still declared in case later nodes need it,
        // but no node is assigned to it when no node has in-degree >= 2.
        assert!(!out.contains("central;"));
    }

    #[test]
    fn render_comprehensive_diagram_covers_edges_linkstyle_classdef_and_escaping() {
        let findings = vec![
            "**Headline:** \"Root\" node\n\n**Observation:** root observation.\n\n**Analysis:** a.\n\n**Cross-reference / Dependencies:** No direct dependencies.\n\n**Implication:** i.".into(),
            "**Headline:** First child | branch\n\n**Observation:** child observation.\n\n**Analysis:** b.\n\n**Cross-reference / Dependencies:** Builds on Finding 1.\n\n**Implication:** j.".into(),
            "**Headline:** Second child [2024] #tag\n\n**Observation:** another child.\n\n**Analysis:** c.\n\n**Cross-reference / Dependencies:** Relates to Finding 1, depends on Finding 1, see Finding 0.\n\n**Implication:** k.".into(),
        ];
        let out = render_findings_diagram(&findings);

        // Section and fence.
        assert!(out.contains("## Findings Relationship Diagram"));
        assert!(out.contains("```mermaid"));
        assert!(out.contains("flowchart TD"));

        // Nodes with escaped labels (FR-004, FR-015).
        assert!(out.contains("F1[\"1 — 'Root' node\"]"));
        assert!(out.contains(r#"F2["2 — First child \| branch"]"#));
        assert!(out.contains(r#"F3["3 — Second child (2024) tag"]"#));

        // Edges and linkStyle per strength (FR-006, FR-007).
        assert!(out.contains("F2 --> F1"));
        assert!(out.contains("F3 --> F1"));
        assert!(!out.contains("F1 --> F1"), "self-loops omitted");
        assert!(!out.contains("F3 --> F0"), "zero-reference omitted");
        assert!(out.contains("linkStyle 0 stroke-width:4px"));
        assert!(out.contains("linkStyle 1 stroke-width:1.5px"));

        // Node class assignment based on in-degree (FR-008).
        assert!(out.contains("classDef central font-size:15px;"));
        assert!(out.contains("classDef normal font-size:12px;"));
        assert!(out.contains("class F1 central;"));
        assert!(out.contains("class F2 normal;"));
        assert!(out.contains("class F3 normal;"));
    }

    #[test]
    fn render_ignores_self_loops_and_out_of_range_and_duplicates() {
        let findings = vec![
            "**Headline:** A\n\n**Cross-reference / Dependencies:** See Finding 1, Finding 0, Finding 99, finding 1.".into(),
            "**Headline:** B\n\n**Cross-reference / Dependencies:** No direct dependencies.".into(),
        ];
        let out = render_findings_diagram(&findings);
        let count = out.matches("F1 --> F1").count();
        assert_eq!(count, 0, "self-loops should be omitted");
        assert!(!out.contains("F1 --> F99"), "out-of-range edges omitted");
        assert!(!out.contains("F1 --> F0"), "zero references omitted");
    }

    #[test]
    fn render_uses_observation_fallback_when_headline_missing() {
        let findings = vec![
            "**Observation:** Tokio dominates the Rust async ecosystem.\n\n**Analysis:** a.\n\n**Cross-reference / Dependencies:** No direct dependencies.\n\n**Implication:** i.".into(),
        ];
        let out = render_findings_diagram(&findings);
        // make_headline_from_observation takes up to 15 words; the text has 7.
        assert!(out.contains("F1[\"1 — Tokio dominates the Rust async ecosystem\"]"));
    }

    #[test]
    fn render_escapes_quotes_and_backticks_in_headline() {
        let findings =
            vec!["**Headline:** He said `async` is \"hard\" and `rlms` is pip-installable".into()];
        let out = render_findings_diagram(&findings);
        let expected = r#"F1["1 — He said 'async' is 'hard' and 'rlms' is pip-installable"]"#;
        assert!(out.contains(expected), "escaped output: {out}");
    }

    #[test]
    fn render_escapes_pipe_and_brackets_and_hash() {
        let findings = vec!["**Headline:** C++ | Rust [2024] #async".into()];
        let out = render_findings_diagram(&findings);
        let expected = r#"F1["1 — C++ \| Rust (2024) async"]"#;
        assert!(out.contains(expected), "escaped output: {out}");
    }

    #[test]
    fn render_escapes_backslash_and_line_breaks() {
        let findings = vec!["**Headline:** path\\is\\ok\nsecond line".into()];
        let out = render_findings_diagram(&findings);
        let expected = "F1[\"1 — path\\\\is\\\\ok second line\"]";
        assert!(out.contains(expected), "escaped output: {out}");
    }

    #[test]
    fn render_escapes_backticks_that_break_mermaid_quoted_labels() {
        let findings = vec![
            "**Headline:** The `rlms` library is pip-installable and supports 'fast' memory".into(),
        ];
        let out = render_findings_diagram(&findings);
        assert!(
            out.contains(
                "F1[\"1 — The 'rlms' library is pip-installable and supports 'fast' memory\"]"
            ),
            "backticks must become single quotes: {out}"
        );
    }

    #[test]
    fn render_trims_whitespace_after_escape_replacement() {
        let findings = vec!["**Headline:**\nhas leading newline".into()];
        let out = render_findings_diagram(&findings);
        assert!(
            out.contains("F1[\"1 — has leading newline\"]"),
            "escaped output: {out}"
        );
    }

    #[test]
    fn render_handles_single_newline_between_labels() {
        let findings = vec![
            "**Headline:** Compact child\n**Observation:** child observation.\n**Analysis:** a.\n**Cross-reference / Dependencies:** Builds on Finding 2.\n**Implication:** i.".into(),
            "**Headline:** Compact root\n**Observation:** root observation.\n**Analysis:** b.\n**Cross-reference / Dependencies:** No direct dependencies.\n**Implication:** j.".into(),
        ];
        let out = render_findings_diagram(&findings);
        assert!(
            out.contains("F1[\"1 — Compact child\"]"),
            "headline must stop at next label: {out}"
        );
        assert!(
            out.contains("F2[\"2 — Compact root\"]"),
            "headline must stop at next label: {out}"
        );
        assert!(out.contains("F1 --> F2"));
        assert!(out.contains("linkStyle 0 stroke-width:4px"));
        assert!(
            !out.contains("**Observation:**"),
            "diagram must not contain raw label markers: {out}"
        );
    }

    #[test]
    fn extract_dependency_paragraph_stops_at_single_newline_label() {
        let finding = "**Observation:** some text.\n**Cross-reference / Dependencies:** Builds on Finding 2.\n**Implication:** i.";
        let deps = extract_dependency_paragraph(finding);
        assert_eq!(deps, "Builds on Finding 2.");
    }
}
