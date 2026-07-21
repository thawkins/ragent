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

use regex::Regex;

/// One directed edge in the findings dependency graph.
///
/// The `from` field is the 1-based finding number that contains the
/// reference; the `to` field is the 1-based finding number being
/// referenced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FindingEdge {
    /// Source finding node (the referrer).
    pub from: usize,
    /// Target finding node (the referenced finding).
    pub to: usize,
}

/// Extract dependency edges from the **Cross-reference / Dependencies:**
/// paragraphs of the supplied findings.
///
/// This is the parser required by T-002. It scans each finding for
/// `Finding N` references, but only inside the dependency paragraph. Edges
/// are returned in the order they are first encountered, and self-loops,
/// out-of-range references, and duplicate edges between the same pair of
/// findings are removed (FR-006, FR-009, FR-010).
///
/// # Arguments
///
/// * `findings` — one string per finding body, in document order.
///
/// # Returns
///
/// A deduplicated, topologically-ordered-by-encounter list of directed
/// edges. The list is empty when there are no valid dependencies.
pub fn extract_finding_edges(findings: &[String]) -> Vec<FindingEdge> {
    if findings.len() <= 1 {
        return Vec::new();
    }
    let finding_re = Regex::new(r"(?i)\bfinding\s+(\d+)\b").expect("valid regex");
    let mut edges: Vec<FindingEdge> = Vec::new();
    for (idx, finding) in findings.iter().enumerate() {
        let from = idx + 1;
        let deps = extract_dependency_paragraph(finding);
        for cap in finding_re.captures_iter(&deps) {
            let to: usize = cap[1].parse().unwrap_or(0);
            if to == 0 || to > findings.len() || to == from {
                continue;
            }
            let candidate = FindingEdge { from, to };
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

/// Render a Mermaid `flowchart TD` block from a slice of finding bodies.
///
/// The returned string is the full fenced Mermaid block, including the
/// surrounding ` ```mermaid ` fence, ready to embed under a
/// `## Findings Relationship Diagram` section. When `findings` is empty, an
/// empty string is returned so the caller can render its own placeholder
/// (FR-005).
///
/// Each finding is rendered as a node labelled `F<n>` where `<n>` is the
/// 1-based finding number. The node text is `Finding <n> — <headline>` where
/// `<headline>` is extracted from the `**Headline:**` paragraph, or a short
/// fallback derived from the `**Observation:**` paragraph (FR-004).
///
/// Edges are extracted from `Finding N` references inside the
/// `**Cross-reference / Dependencies:**` paragraph. Self-loops, duplicate
/// edges, and references outside the range `1..=findings.len()` are silently
/// ignored (FR-006, FR-009, FR-010).
///
/// # Arguments
///
/// * `findings` — one string per finding body, in the order they appear in
///   the document.
///
/// # Returns
///
/// A `String` containing either the fenced Mermaid block or an empty string
/// when there are no findings.
pub fn render_findings_diagram(findings: &[String]) -> String {
    if findings.is_empty() {
        return String::new();
    }

    let mut out = String::from("\n## Findings Relationship Diagram\n\n```mermaid\nflowchart TD\n");

    // Node declarations: one per finding.
    for (idx, finding) in findings.iter().enumerate() {
        let n = idx + 1;
        let headline = extract_headline_for_diagram(finding, n);
        out.push_str(&format!("    F{n}[\"Finding {n} — {headline}\"]\n"));
    }

    // Edge declarations with deterministic ordering.
    let edges = extract_finding_edges(findings);

    if !edges.is_empty() {
        out.push('\n');
        for edge in &edges {
            out.push_str(&format!("    F{} --> F{}\n", edge.from, edge.to));
        }
    }

    out.push_str("```\n");
    out
}

/// Extract the headline for a finding, suitable for a Mermaid node label.
///
/// If the finding contains a `**Headline:**` paragraph, its body is returned.
/// Otherwise a short fallback is derived from the `**Observation:**`
/// paragraph. The result is escaped so it is safe inside a Mermaid quoted
/// node label.
fn extract_headline_for_diagram(finding: &str, finding_number: usize) -> String {
    const LABEL: &str = "**Headline:**";
    let headline = finding.find(LABEL).and_then(|start| {
        let after = &finding[start + LABEL.len()..];
        let end = after.find("\n\n**").unwrap_or(after.len());
        let body = after[..end].trim();
        if body.is_empty() {
            None
        } else {
            Some(body.to_string())
        }
    });

    let headline = headline.unwrap_or_else(|| {
        finding
            .find("**Observation:**")
            .map(|start| {
                let after = &finding[start + "**Observation:**".len()..];
                let end = after.find("\n\n**").unwrap_or(after.len());
                let body = after[..end].trim();
                crate::document::make_headline_from_observation(body)
            })
            .unwrap_or_else(|| format!("Finding {finding_number}"))
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
            let end = after.find("\n\n**").unwrap_or(after.len());
            after[..end].trim().to_string()
        })
        .unwrap_or_default()
}

/// Escape characters that would break a Mermaid quoted node label.
///
/// Replaces characters that Mermaid treats as syntax inside quoted labels
/// (`"`, `|`, `[`, `]`, `#`, `\`) with safe escaped or stripped forms, and
/// collapses `\r\n`, `\r` and `\n` into single spaces. The result is trimmed.
/// This keeps the generated diagram valid without relying on a full Mermaid
/// parser (FR-015, NFR-002).
fn escape_mermaid_label(s: &str) -> String {
    // Double literal backslashes first so the subsequent escapes are not
    // re-escaped (FR-015).
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('|', "\\|")
        .replace('[', "(")
        .replace(']', ")")
        .replace('#', "")
        .replace(['\r', '\n'], " ")
        .trim()
        .to_string()
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
        assert_eq!(edges, vec![FindingEdge { from: 1, to: 2 }]);
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
        assert_eq!(edges, vec![FindingEdge { from: 1, to: 2 }]);
    }

    #[test]
    fn extract_finding_edges_preserves_multiple_distinct_edges() {
        let findings = vec![
            "**Cross-reference / Dependencies:** Depends on Finding 2 and Finding 3.".into(),
            "**Cross-reference / Dependencies:** No direct dependencies.".into(),
            "**Cross-reference / Dependencies:** No direct dependencies.".into(),
        ];
        let edges = extract_finding_edges(&findings);
        assert_eq!(
            edges,
            vec![
                FindingEdge { from: 1, to: 2 },
                FindingEdge { from: 1, to: 3 },
            ]
        );
    }

    #[test]
    fn render_empty_findings_returns_empty_string() {
        let out = render_findings_diagram(&[]);
        assert!(out.is_empty(), "expected empty output for no findings");
    }

    #[test]
    fn render_single_finding_emits_one_node() {
        let findings = vec!["**Headline:** Rust async runtime".into()];
        let out = render_findings_diagram(&findings);
        assert!(out.contains("flowchart TD"));
        assert!(out.contains("F1[\"Finding 1 — Rust async runtime\"]"));
        assert!(!out.contains("-->"), "single finding has no edges");
    }

    #[test]
    fn render_multi_finding_emits_nodes_and_edges() {
        let findings = vec![
            "**Headline:** Child\n\n**Observation:** child observation.\n\n**Analysis:** a.\n\n**Cross-reference / Dependencies:** Builds on Finding 2.\n\n**Implication:** i.".into(),
            "**Headline:** Root\n\n**Observation:** root observation.\n\n**Analysis:** b.\n\n**Cross-reference / Dependencies:** No direct dependencies.\n\n**Implication:** j.".into(),
        ];
        let out = render_findings_diagram(&findings);
        assert!(out.contains("F1[\"Finding 1 — Child\"]"));
        assert!(out.contains("F2[\"Finding 2 — Root\"]"));
        assert!(out.contains("F1 --> F2"));
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
        assert!(out.contains("F1[\"Finding 1 — Tokio dominates the Rust async ecosystem\"]"));
    }

    #[test]
    fn render_escapes_quotes_in_headline() {
        let findings = vec!["**Headline:** He said \"async\" is hard".into()];
        let out = render_findings_diagram(&findings);
        let expected = r#"F1["Finding 1 — He said \"async\" is hard"]"#;
        assert!(out.contains(expected), "escaped output: {out}");
    }

    #[test]
    fn render_escapes_pipe_and_brackets_and_hash() {
        let findings = vec!["**Headline:** C++ | Rust [2024] #async".into()];
        let out = render_findings_diagram(&findings);
        let expected = r#"F1["Finding 1 — C++ \| Rust (2024) async"]"#;
        assert!(out.contains(expected), "escaped output: {out}");
    }

    #[test]
    fn render_escapes_backslash_and_line_breaks() {
        let findings = vec!["**Headline:** path\\is\\ok\nsecond line".into()];
        let out = render_findings_diagram(&findings);
        let expected = "F1[\"Finding 1 — path\\\\is\\\\ok second line\"]";
        assert!(out.contains(expected), "escaped output: {out}");
    }

    #[test]
    fn render_trims_whitespace_after_escape_replacement() {
        let findings = vec!["**Headline:**\nhas leading newline".into()];
        let out = render_findings_diagram(&findings);
        assert!(
            out.contains("F1[\"Finding 1 — has leading newline\"]"),
            "escaped output: {out}"
        );
    }
}
