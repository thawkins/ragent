//! specs/imradreport T-006 / NFR-001.
//!
//! Unit tests verifying that [`assemble_document`] emits the `IMRaD` section order
//! when [`OutputFormat::Imrad`] is selected. The tests construct a
//! [`ResearchDocument`] directly so no LLM call or gathering pass is required.

use ragent_research::OutputFormat;
use ragent_research::document::{CrossReference, ResearchDocument, assemble_document};
use ragent_research::item::ResearchItem;
use ragent_research::research_name::ResearchName;

/// Build a minimal [`ResearchItem`] for tests.
fn sample_item() -> ResearchItem {
    let name = ResearchName::new("imrad-sections").expect("valid name");
    ResearchItem::new(name, "IMRaD Section Check", "IMRaD layout verification")
}

/// Build a [`ResearchDocument`] with controlled fields.
const fn sample_doc(item: ResearchItem) -> ResearchDocument {
    ResearchDocument {
        item,
        summary: String::new(),
        findings: Vec::new(),
        cross_references: Vec::new(),
        open_questions: Vec::new(),
        template_body: None,
        decomposed_queries: Vec::new(),
        output_format: OutputFormat::Imrad,
    }
}

/// Return the 1-based positions of each top-level `## ` heading in `body`,
/// in the order they appear. Only H2 headings are captured.
fn section_positions(body: &str) -> Vec<(&str, usize)> {
    body.lines()
        .enumerate()
        .filter(|(_, line)| line.starts_with("## "))
        .map(|(idx, line)| (line.trim(), idx + 1))
        .collect()
}

/// Return the exact set of H2 heading lines in `body`.
fn h2_headings(body: &str) -> Vec<&str> {
    body.lines()
        .filter(|line| line.starts_with("## "))
        .map(str::trim)
        .collect()
}

#[test]
fn imrad_empty_sections_appear_in_order() {
    let doc = sample_doc(sample_item());
    let assembled = assemble_document(&doc);
    let headings = h2_headings(&assembled.body);

    assert!(
        assembled.body.contains("# Title:"),
        "Title heading must still be rendered: {}",
        assembled.body
    );

    let expected = vec![
        "## Abstract",
        "## Introduction",
        "## Methods",
        "## Results",
        "## Discussion",
        "## References Index",
    ];
    assert_eq!(
        headings, expected,
        "IMRaD sections must appear in canonical order:\ngot: {headings:?}\nbody:\n{}",
        assembled.body
    );
}

#[test]
fn imrad_legacy_top_level_headings_absent() {
    let doc = sample_doc(sample_item());
    let assembled = assemble_document(&doc);
    let headings = h2_headings(&assembled.body);
    let forbidden = ["## Topic", "## Search Queries", "## Summary", "## Findings"];
    for heading in forbidden {
        assert!(
            !headings.contains(&heading),
            "legacy top-level heading `{heading}` must not appear in IMRaD H2 headings: {headings:?}\nbody:\n{}",
            assembled.body
        );
    }
}

#[test]
fn imrad_populated_fields_render_in_sections() {
    let mut doc = sample_doc(sample_item());
    doc.summary = "A concise abstract/summary of the research.".to_string();
    doc.decomposed_queries = vec!["query one".into(), "query two".into()];
    doc.findings = vec![
        "**Headline:** Finding one headline\n\n\
         **Observation:** Observation text.\n\n\
         **Analysis:** Analysis text.\n\n\
         **Cross-reference / Dependencies:** No direct dependencies.\n\n\
         **Implication:** Implication text."
            .to_string(),
    ];
    doc.cross_references.push(CrossReference {
        path: "src/lib.rs".into(),
        relevance: "contains the core implementation".into(),
    });
    doc.open_questions
        .push("What is the migration path?".into());

    let assembled = assemble_document(&doc);
    let body = &assembled.body;

    // Section order and presence.
    let positions = section_positions(body);
    let headings: Vec<&str> = positions.iter().map(|(h, _)| *h).collect();
    assert_eq!(
        headings,
        vec![
            "## Abstract",
            "## Introduction",
            "## Methods",
            "## Results",
            "## Discussion",
            "## References Index",
        ]
    );

    // Abstract contains the summary.
    let abstract_idx = body.find("## Abstract").unwrap();
    let intro_idx = body.find("## Introduction").unwrap();
    let abstract_body = &body[abstract_idx..intro_idx];
    assert!(
        abstract_body.contains("A concise abstract/summary of the research."),
        "Abstract must contain the summary text: {abstract_body}"
    );

    // Introduction contains the topic.
    let methods_idx = body.find("## Methods").unwrap();
    let intro_body = &body[intro_idx..methods_idx];
    assert!(
        intro_body.contains("IMRaD layout verification"),
        "Introduction must contain the topic: {intro_body}"
    );
    assert!(
        intro_body.contains("research item investigates the topic above"),
        "Introduction must contain the objective framing paragraph: {intro_body}"
    );

    // Methods contains the decomposed queries and a configuration sub-section.
    let results_idx = body.find("## Results").unwrap();
    let methods_body = &body[methods_idx..results_idx];
    assert!(
        methods_body.contains("### Search Queries"),
        "Methods must contain Search Queries sub-section: {methods_body}"
    );
    assert!(
        methods_body.contains("- query one"),
        "Search Queries must list decomposed queries: {methods_body}"
    );
    assert!(
        methods_body.contains("### Research Configuration"),
        "Methods must contain Research Configuration sub-section: {methods_body}"
    );

    // Results contains the summary sub-section and findings sub-section.
    let discussion_idx = body.find("## Discussion").unwrap();
    let results_body = &body[results_idx..discussion_idx];
    assert!(
        results_body.contains("### Summary"),
        "Results must contain Summary sub-section: {results_body}"
    );
    assert!(
        results_body.contains("### Findings"),
        "Results must contain Findings sub-section: {results_body}"
    );
    assert!(
        results_body.contains("### Finding 1 — Finding one headline"),
        "Results must render numbered findings: {results_body}"
    );
    assert!(
        results_body.contains("### Findings Relationship Diagram"),
        "Results must contain the diagram as a sub-section: {results_body}"
    );

    // Discussion contains cross-references and open questions sub-sections.
    let references_idx = body.find("## References Index").unwrap();
    let discussion_body = &body[discussion_idx..references_idx];
    assert!(
        discussion_body.contains("### In-Project Cross-References"),
        "Discussion must contain In-Project Cross-References sub-section: {discussion_body}"
    );
    assert!(
        discussion_body.contains("### Open Questions"),
        "Discussion must contain Open Questions sub-section: {discussion_body}"
    );
    assert!(
        discussion_body.contains("`src/lib.rs`"),
        "Cross-references table must include the project path: {discussion_body}"
    );
    assert!(
        discussion_body.contains("What is the migration path?"),
        "Open Questions must include the question: {discussion_body}"
    );
}

#[test]
fn imrad_references_index_unchanged() {
    let mut doc = sample_doc(sample_item());
    doc.item.add_source(ragent_research::Source::Web {
        url: "https://example.com".into(),
        title: "Example Article".into(),
        captured_at: chrono::Utc::now(),
        published_at: None,
        body_path: std::path::PathBuf::from("sources/web-01.md"),
        relevance: String::new(),
        body: "body".into(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
    });

    let assembled = assemble_document(&doc);
    assert!(
        assembled.body.contains("## References Index"),
        "References Index must be present"
    );
    assert!(
        assembled
            .body
            .contains("| 1 | web | [https://example.com](https://example.com) |"),
        "References Index must linkify the source URL: {}",
        assembled.body
    );
}
#[test]
fn imrad_format_does_not_change_document_fields() {
    let mut doc = sample_doc(sample_item());
    doc.summary = "summary".to_string();
    doc.findings = vec!["**Headline:** h\n\n**Observation:** o.\n\n**Analysis:** a.\n\n**Cross-reference / Dependencies:** No direct dependencies.\n\n**Implication:** i.".into()];

    let assembled = assemble_document(&doc);

    // The fields themselves must be unchanged; only the layout differs.
    assert_eq!(doc.summary, "summary");
    assert_eq!(doc.findings.len(), 1);
    assert!(assembled.body.contains("## Abstract"));
    assert!(assembled.body.contains("## Results"));
}

#[test]
fn imrad_skeleton_renders_imrad_sections() {
    let name = ResearchName::new("imrad-skeleton").expect("valid name");
    let skeleton = ragent_research::render_skeleton(
        &name,
        "IMRaD Skeleton",
        "skeleton topic",
        OutputFormat::Imrad,
    );

    let headings = h2_headings(&skeleton);
    assert_eq!(
        headings,
        vec![
            "## Abstract",
            "## Introduction",
            "## Methods",
            "## Results",
            "## Discussion",
            "## References Index",
        ],
        "IMRaD skeleton must use canonical section order:\n{skeleton}"
    );
    assert!(
        skeleton.contains("requested_format: imrad"),
        "IMRaD skeleton must persist requested format in frontmatter:\n{skeleton}"
    );
}

#[test]
fn imrad_skeleton_does_not_contain_legacy_top_level_headings() {
    let name = ResearchName::new("imrad-skeleton").expect("valid name");
    let skeleton = ragent_research::render_skeleton(
        &name,
        "IMRaD Skeleton",
        "skeleton topic",
        OutputFormat::Imrad,
    );
    let headings = h2_headings(&skeleton);
    let forbidden = ["## Topic", "## Search Queries", "## Summary", "## Findings"];
    for heading in forbidden {
        assert!(
            !headings.contains(&heading),
            "IMRaD skeleton must not contain legacy H2 heading `{heading}`: {headings:?}\n{skeleton}"
        );
    }
}

#[test]
fn report_skeleton_unchanged_after_signature_update() {
    let name = ResearchName::new("report-skeleton").expect("valid name");
    let skeleton = ragent_research::render_skeleton(
        &name,
        "Report Skeleton",
        "skeleton topic",
        OutputFormat::Report,
    );
    let headings = h2_headings(&skeleton);
    assert!(
        headings.contains(&"## Topic"),
        "Report skeleton must still contain Topic section: {headings:?}\n{skeleton}"
    );
    assert!(
        headings.contains(&"## Findings"),
        "Report skeleton must still contain Findings section: {headings:?}\n{skeleton}"
    );
    assert!(
        !skeleton.contains("requested_format:"),
        "Report skeleton must not persist a default format:\n{skeleton}"
    );
}
