//! Integration tests for the `newsletter` research output format.
//!
//! A newsletter carries exactly nine elements, in order: the YAML frontmatter,
//! the `# Title:` line, and the Topic, Executive Summary, Top 10 Implications,
//! Open Questions, Concepts, Findings, and Findings Relationship Diagram
//! sections. Every other default-report section is dropped. These tests build a
//! [`ResearchDocument`] directly, so no LLM call or gathering pass is required.

use ragent_research::OutputFormat;
use ragent_research::document::{ResearchDocument, assemble_document};
use ragent_research::item::ResearchItem;
use ragent_research::research_name::ResearchName;
use ragent_research::source::Source;

/// Section headings the newsletter must never emit.
const DROPPED_SECTIONS: &[&str] = &[
    "## Corpus Quality Scoreboard",
    "## Research Brief",
    "## Search Queries",
    "## Search Engine Summary",
    "## Search Provider Requests",
    "## Data Quality & Consistency",
    "## Contradiction Graph",
    "## In-Project Cross-References",
    "## References Index",
    "## Comparison Table",
    "## Entity Profiles",
    "## Abstract",
    "## Introduction",
    "## Methods",
    "## Results",
];

/// Build a fully populated newsletter document: every optional artifact a
/// default report would render is present, so a dropped section in the output
/// is genuinely dropped by the layout rather than merely absent from the data.
fn newsletter_doc() -> ResearchDocument {
    let name = ResearchName::new("newsletter").expect("valid name");
    let mut item = ResearchItem::new(name, "Newsletter Title", "Newsletter topic coverage");
    item.output_format = Some(OutputFormat::Newsletter.as_str().to_string());
    item.add_source(Source::Web {
        url: "https://example.com/a".into(),
        title: "Source A".into(),
        captured_at: chrono::Utc::now(),
        published_at: None,
        body_path: std::path::PathBuf::from("sources/web-01.md"),
        body: "A is fast.".into(),
        relevance: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "openalex".into(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    });
    ResearchDocument {
        item,
        summary: "The sources agree that A is fast [#1].".into(),
        findings: vec![
            "**Headline:** A is fast\n\n**Observation:** A is fast [#1].\n\n\
             **Analysis:** This matters because speed drives adoption [#1].\n\n\
             **Cross-reference / Dependencies:** No direct dependencies.\n\n\
             **Implication:** Prefer A for latency-sensitive work."
                .into(),
        ],
        top_implications: vec!["Adoption grows with speed.".into()],
        cross_references: Vec::new(),
        open_questions: vec!["How fast is A under load?".into()],
        concepts: Some("### Speed\n\nLatency and throughput.".into()),
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
        brief: Some("Investigate newsletter topic coverage.".into()),
        decomposed_queries: vec!["newsletter topic".into()],
        output_format: OutputFormat::Newsletter,
        comparison_table: None,
        evaluation_scorecard: None,
        provider_stats: None,
    }
}

#[test]
fn test_newsletter_renders_the_nine_required_elements_in_order() {
    let assembled = assemble_document(&newsletter_doc());
    let content = &assembled.content;

    // 1. Frontmatter - the assembled content opens with a YAML block.
    assert!(
        content.starts_with("---\n"),
        "newsletter must open with YAML frontmatter:\n{content}"
    );
    let frontmatter_end = content[4..]
        .find("\n---\n")
        .map(|i| i + 4)
        .expect("frontmatter must be closed");
    assert!(
        content[..frontmatter_end].contains("name: newsletter"),
        "frontmatter must carry the research name:\n{content}"
    );

    // 2. Title, then 3-9: the narrative sections, in the specified order.
    let expected = [
        "# Title: Newsletter Title",
        "## Topic",
        "## Executive Summary",
        "## Top 10 Implications",
        "## Open Questions",
        "## Concepts",
        "## Findings",
        "## Findings Relationship Diagram",
    ];
    let mut cursor = frontmatter_end;
    for needle in expected {
        let at = content[cursor..].find(needle).unwrap_or_else(|| {
            panic!("newsletter must contain {needle:?} after the previous element:\n{content}")
        });
        cursor += at + needle.len();
    }
}

#[test]
fn test_newsletter_drops_every_default_section() {
    let assembled = assemble_document(&newsletter_doc());
    for dropped in DROPPED_SECTIONS {
        assert!(
            !assembled.body.contains(dropped),
            "newsletter must not render {dropped:?}:\n{}",
            assembled.body
        );
    }
}

#[test]
fn test_newsletter_carries_the_synthesised_content() {
    let assembled = assemble_document(&newsletter_doc());
    let body = &assembled.body;
    assert!(
        body.contains("The sources agree that A is fast"),
        "executive summary must be rendered:\n{body}"
    );
    assert!(
        body.contains("1. Adoption grows with speed."),
        "top implications must be rendered:\n{body}"
    );
    assert!(
        body.contains("- How fast is A under load?"),
        "open questions must be rendered:\n{body}"
    );
    assert!(
        body.contains("### Speed"),
        "concepts must be rendered:\n{body}"
    );
    assert!(
        body.contains("### **Finding 1**"),
        "findings must be rendered:\n{body}"
    );
}

#[test]
fn test_newsletter_skeleton_is_well_formed() {
    let name = ResearchName::new("newsletter-skeleton").expect("valid name");
    let skeleton = ragent_research::document::render_skeleton(
        &name,
        "Skeleton Title",
        "Skeleton topic",
        OutputFormat::Newsletter,
    );
    for section in [
        "## Topic",
        "## Executive Summary",
        "## Top 10 Implications",
        "## Open Questions",
        "## Findings",
        "## Findings Relationship Diagram",
    ] {
        assert!(
            skeleton.contains(section),
            "newsletter skeleton must contain {section:?}:\n{skeleton}"
        );
    }
    for dropped in DROPPED_SECTIONS {
        assert!(
            !skeleton.contains(dropped),
            "newsletter skeleton must not contain {dropped:?}:\n{skeleton}"
        );
    }
    assert!(
        skeleton.contains("requested_format: newsletter"),
        "the skeleton frontmatter must record the requested format:\n{skeleton}"
    );
}
