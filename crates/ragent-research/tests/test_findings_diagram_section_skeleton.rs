//! rsearchdiag T-014 / FR-013.
//!
//! Integration test verifying that [`render_skeleton`] — the empty
//! `RESEARCH.md` written before any gathering has run — includes the
//! `## Findings Relationship Diagram` section with the zero-findings
//! placeholder, so the section is present from the moment the file lands
//! on disk.
//!
//! Covers FR-013 in conjunction with FR-005 (no Mermaid block when there are
//! zero findings) and FR-012 (every FR-010 section is still present and in
//! canonical order, so adding the diagram did not disturb the skeleton's
//! stable structure).

use ragent_research::document::REQUIRED_SECTIONS;
use ragent_research::render_skeleton;
use ragent_research::research_name::ResearchName;

/// Build the skeleton document for a freshly-created research item (no
/// gathering pass has run, so there are zero findings).
fn skeleton() -> String {
    let name = ResearchName::new("skeleton-diagram").expect("valid name");
    render_skeleton(
        &name,
        "Skeleton Diagram Check",
        "FR-013 skeleton placeholder",
        ragent_research::run_config::OutputFormat::Report,
    )
}

#[test]
fn skeleton_contains_findings_relationship_diagram_section() {
    // FR-013: the `## Findings Relationship Diagram` section is present in
    // the skeleton even though no findings have been gathered yet.
    let skeleton = skeleton();
    assert!(
        skeleton.contains("## Findings Relationship Diagram"),
        "skeleton must contain the Findings Relationship Diagram section (FR-013): {skeleton}"
    );
}

#[test]
fn skeleton_contains_zero_findings_placeholder() {
    // FR-013: the section carries the zero-findings placeholder text.
    let skeleton = skeleton();
    assert!(
        skeleton.contains("_(no findings yet — the gathering pass will populate this section)_"),
        "skeleton diagram section must carry the zero-findings placeholder (FR-013): {skeleton}"
    );
}

#[test]
fn skeleton_does_not_emit_mermaid_block() {
    // FR-005: with zero findings the diagram must NOT emit a Mermaid code
    // block — only the placeholder text.
    let skeleton = skeleton();
    assert!(
        !skeleton.contains("```mermaid"),
        "skeleton with zero findings must not emit a Mermaid fence (FR-005): {skeleton}"
    );
    assert!(
        !skeleton.contains("flowchart TD"),
        "skeleton with zero findings must not emit a Mermaid graph (FR-005): {skeleton}"
    );
}

#[test]
fn skeleton_diagram_section_is_between_findings_and_cross_references() {
    // FR-002 / FR-013: the diagram section sits immediately after
    // `## Findings` and before `## In-Project Cross-References` in the
    // skeleton too.
    let skeleton = skeleton();

    let findings_idx = skeleton
        .find("## Findings\n")
        .expect("## Findings heading must be present in skeleton");
    let diagram_idx = skeleton
        .find("## Findings Relationship Diagram")
        .expect("## Findings Relationship Diagram heading must be present in skeleton");
    let xref_idx = skeleton
        .find("## In-Project Cross-References")
        .expect("## In-Project Cross-References heading must be present in skeleton");

    assert!(
        findings_idx < diagram_idx,
        "diagram section must come after ## Findings in skeleton (FR-002)"
    );
    assert!(
        diagram_idx < xref_idx,
        "diagram section must come before ## In-Project Cross-References in skeleton (FR-002)"
    );

    // No other `## `-level heading sits between Findings and the diagram —
    // the diagram is the immediate next section.
    let between = &skeleton[findings_idx + "## Findings\n".len()..diagram_idx];
    assert!(
        !between.contains("\n## "),
        "no ## section heading may appear between ## Findings and ## Findings Relationship Diagram in skeleton (FR-002): found {between:?}"
    );
}

#[test]
fn skeleton_preserves_all_required_sections_in_order() {
    // FR-012: adding the diagram section did not remove or reorder any of
    // the existing FR-010 sections in the skeleton. Every entry in
    // REQUIRED_SECTIONS must appear, in order, as a `## <name>` heading.
    let skeleton = skeleton();

    let mut cursor = 0usize;
    for section in REQUIRED_SECTIONS {
        let heading = format!("## {section}");
        let found = skeleton[cursor..].find(&heading).map(|p| cursor + p);
        let idx = found.unwrap_or_else(|| {
            panic!("required section `## {section}` missing from skeleton (FR-012): {skeleton}")
        });
        assert!(
            idx >= cursor,
            "section `## {section}` appeared out of order in skeleton (FR-012)"
        );
        cursor = idx + heading.len();
    }
}

#[test]
fn skeleton_is_well_formed_with_frontmatter_and_title() {
    // The skeleton must remain a well-formed RESEARCH.md: it starts with a
    // frontmatter block and carries the document title.
    let skeleton = skeleton();
    assert!(
        skeleton.starts_with("---\n"),
        "skeleton must start with a frontmatter block: {skeleton}"
    );
    assert!(
        skeleton.contains("status: draft"),
        "skeleton frontmatter must mark the item as draft: {skeleton}"
    );
    assert!(
        skeleton.contains("# Title: Skeleton Diagram Check"),
        "skeleton must carry the document title: {skeleton}"
    );
}
