//! Inline tests for `diagram.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

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
        "**Cross-reference / Dependencies:** Depends on Finding 2 and relies on Finding 3.".into(),
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
        out.contains("_(no findings yet - the gathering pass will populate this section)_"),
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
    assert!(out.contains("F1[\"1 - Rust async runtime\"]"));
    assert!(!out.contains("-->"), "single finding has no edges");
}

#[test]
fn render_multi_finding_emits_nodes_edges_and_linkstyle() {
    let findings = vec![
        "**Headline:** Child\n\n**Observation:** child observation.\n\n**Analysis:** a.\n\n**Cross-reference / Dependencies:** Builds on Finding 2.\n\n**Implication:** i.".into(),
        "**Headline:** Root\n\n**Observation:** root observation.\n\n**Analysis:** b.\n\n**Cross-reference / Dependencies:** No direct dependencies.\n\n**Implication:** j.".into(),
    ];
    let out = render_findings_diagram(&findings);
    assert!(out.contains("F1[\"1 - Child\"]"));
    assert!(out.contains("F2[\"2 - Root\"]"));
    assert!(out.contains("F1 --> F2"));
    assert!(out.contains("linkStyle 0 stroke-width:4px"));
}

#[test]
fn render_assigns_central_class_for_high_indegree() {
    let findings = vec![
        "**Headline:** Hub\n\n**Cross-reference / Dependencies:** No direct dependencies.".into(),
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
    assert!(out.contains("F1[\"1 - 'Root' node\"]"));
    assert!(out.contains(r#"F2["2 - First child \| branch"]"#));
    assert!(out.contains(r#"F3["3 - Second child (2024) tag"]"#));

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
    assert!(out.contains("F1[\"1 - Tokio dominates the Rust async ecosystem\"]"));
}

#[test]
fn render_escapes_quotes_and_backticks_in_headline() {
    let findings =
        vec!["**Headline:** He said `async` is \"hard\" and `rlms` is pip-installable".into()];
    let out = render_findings_diagram(&findings);
    let expected = r#"F1["1 - He said 'async' is 'hard' and 'rlms' is pip-installable"]"#;
    assert!(out.contains(expected), "escaped output: {out}");
}

#[test]
fn render_escapes_pipe_and_brackets_and_hash() {
    let findings = vec!["**Headline:** C++ | Rust [2024] #async".into()];
    let out = render_findings_diagram(&findings);
    let expected = r#"F1["1 - C++ \| Rust (2024) async"]"#;
    assert!(out.contains(expected), "escaped output: {out}");
}

#[test]
fn render_escapes_backslash_and_line_breaks() {
    let findings = vec!["**Headline:** path\\is\\ok\nsecond line".into()];
    let out = render_findings_diagram(&findings);
    let expected = "F1[\"1 - path\\\\is\\\\ok second line\"]";
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
            "F1[\"1 - The 'rlms' library is pip-installable and supports 'fast' memory\"]"
        ),
        "backticks must become single quotes: {out}"
    );
}

#[test]
fn render_trims_whitespace_after_escape_replacement() {
    let findings = vec!["**Headline:**\nhas leading newline".into()];
    let out = render_findings_diagram(&findings);
    assert!(
        out.contains("F1[\"1 - has leading newline\"]"),
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
        out.contains("F1[\"1 - Compact child\"]"),
        "headline must stop at next label: {out}"
    );
    assert!(
        out.contains("F2[\"2 - Compact root\"]"),
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
