//! Inline tests for `document.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

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
        output_format: crate::run_config::OutputFormat::Report,
        comparison_table: None,
        evaluation_scorecard: None,
        provider_stats: None,
    }
}

#[test]
fn assemble_document_renders_contradiction_graph_section() {
    use crate::contradiction::{ContradictionClaim, ContradictionEdge, ContradictionGraph};
    let sources = [
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
            author: None,
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
            author: None,
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
    // The contradiction graph renders in the CORPA.md companion payload.
    assert!(assembled.corpa.contains("## Contradiction Graph"));
    assert!(!assembled.body.contains("## Contradiction Graph"));
    assert!(assembled.corpa.contains("performance"));
    assert!(assembled.corpa.contains("opposing performance claims"));
    assert!(assembled.corpa.contains("#1"));
    assert!(assembled.corpa.contains("#2"));
}

#[test]
fn assemble_document_contradiction_graph_placeholder_when_empty() {
    let mut doc = sample_doc(sample_item());
    doc.contradiction_graph = Some(crate::contradiction::ContradictionGraph::empty());
    let assembled = assemble_document(&doc);
    assert!(assembled.corpa.contains("## Contradiction Graph"));
    assert!(
        assembled
            .corpa
            .contains("no contradictions detected among the gathered sources")
    );
}

#[test]
fn assemble_document_omits_contradiction_section_when_none() {
    let doc = sample_doc(sample_item());
    let assembled = assemble_document(&doc);
    assert!(!assembled.body.contains("## Contradiction Graph"));
    assert!(!assembled.corpa.contains("## Contradiction Graph"));
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
        assembled.corpa.contains("## Corpus Critic"),
        "CORPA.md should render corpus critic section"
    );
    assert!(
        !assembled.body.contains("## Corpus Critic"),
        "RESEARCH.md must no longer carry the corpus critic section"
    );
    assert!(
        assembled.body.contains("## Gap-Fill Fetch"),
        "report layout should render gap-fill section"
    );
    assert!(assembled.corpa.contains("72/100"));
    assert!(assembled.corpa.contains("Broaden the width sweep"));
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
    assert!(assembled.body.contains("55 -> 70"));
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
        author: None,
    });
    let rendered = render_supporting_file(&item.sources[0], false).expect("web source renders");
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
        body.contains("## Top 10 Implications"),
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
        "Paragraph 1 - **Observation:** observation text. *Paragraph 2 - Analysis:* analysis text. **Cross-reference / Dependencies:** deps. **Implication:** implication text. **Caveat:** caveat text.".into(),
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
        .split("### **Finding 1** - Observation summary\n\n")
        .nth(1)
        .unwrap();
    // After "### Finding N - headline\n\n" the required labels should be separated by blank lines.
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
        "### **Finding 1** - Observation summary\n\n**Observation:**\nfirst observation"
    ));
    assert!(assembled.body.contains(
        "### **Finding 2** - Observation summary\n\n**Observation:**\nsecond observation"
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
        author: None,
    }
}

#[test]
fn render_search_engine_summary_counts_pages_pdfs_videos() {
    let sources = [
        web_source("openalex, wikipedia", "page"),
        web_source("openalex", "page"),
        web_source("wikipedia", "pdf"),
        web_source("exa", "youtube"),
        web_source("exa", "page"),
    ];
    let table = render_search_engine_summary(&sources);
    assert!(table.contains("| Engine | Pages | PDFs | Videos | Total |"));
    // wikipedia: 1 page (from multi-engine source) + 1 pdf = 1 page, 1 pdf, 0 videos, 2 total
    assert!(table.contains("| wikipedia | 1 | 1 | 0 | 2 |"));
    // openalex: 2 pages (one from multi-engine, one single) = 2 pages
    assert!(table.contains("| openalex | 2 | 0 | 0 | 2 |"));
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
    let sources = [web_source("", "page")];
    let table = render_search_engine_summary(&sources);
    assert!(table.is_empty());
}

#[test]
fn assemble_document_renders_search_engine_summary_after_queries() {
    let mut item = sample_item();
    item.sources = vec![
        web_source("openalex", "page"),
        web_source("wikipedia", "pdf"),
    ];
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
    assert!(assembled.body.contains("| openalex | 1 | 0 | 0 | 1 |"));
    assert!(assembled.body.contains("| wikipedia | 0 | 1 | 0 | 1 |"));
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
    assert!(render_supporting_file(&source, false).is_none());
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
        language: Some("English".into()),
        oa_recovery: None,
        author: Some("Alice Writer".into()),
    };
    let out = render_supporting_file(&source, false).expect("web must produce a body");
    assert!(out.contains("# Web source"));
    assert!(out.contains("URL: https://example.com"));
    assert!(out.contains("Author(s): Alice Writer"));
    assert!(out.contains("Language: English"));
    assert!(out.contains("page body content"));
}

#[test]
fn render_supporting_file_web_block_shows_dash_for_missing_author_and_language() {
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
        author: None,
    };
    let out = render_supporting_file(&source, false).expect("web must produce a body");
    assert!(out.contains("Author(s): -"));
    assert!(out.contains("Language: -"));
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
        author: None,
    };
    let out = render_supporting_file(&source, false).expect("web must produce a body");
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
    let out = render_supporting_file(&source, false).expect("local must produce a body");
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
    let out = render_supporting_file(&source, false).expect("local must produce a body");
    assert!(out.contains("no excerpt captured"));
}

#[test]
fn render_bibliography_empty_state() {
    let out = render_bibliography(&[], false);
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
        author: None,
    };
    let out = render_bibliography(&[source], false);
    assert!(out.contains("Example"));
    assert!(out.contains("https://example.com"));
    assert!(out.contains("page body content"));
}

#[test]
fn render_supporting_file_cloaks_web_url_when_requested() {
    // F-13: `--url-cloak` must defang the supporting-file header URL too.
    let source = Source::Web {
        published_at: None,
        url: "https://example.com/path".into(),
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
        author: None,
    };
    let plain = render_supporting_file(&source, false).expect("web must produce a body");
    assert!(plain.contains("URL: https://example.com/path"));
    let cloaked = render_supporting_file(&source, true).expect("web must produce a body");
    assert!(
        !cloaked.contains("https://example.com/path"),
        "cloaked supporting file must not emit the bare URL: {cloaked}"
    );
    assert!(
        cloaked.contains("hxxps://example[.]com/path"),
        "cloaked supporting file must defang the URL: {cloaked}"
    );
}

#[test]
fn render_bibliography_cloaks_web_url_when_requested() {
    // F-13: `--url-cloak` must defang the bibliography Path/URL cell too.
    let source = Source::Web {
        published_at: None,
        url: "https://example.com/path".into(),
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
        author: None,
    };
    let plain = render_bibliography(std::slice::from_ref(&source), false);
    assert!(plain.contains("https://example.com/path"));
    let cloaked = render_bibliography(std::slice::from_ref(&source), true);
    assert!(
        !cloaked.contains("https://example.com/path"),
        "cloaked bibliography must not emit the bare URL: {cloaked}"
    );
    assert!(
        cloaked.contains("hxxps://example[.]com/path"),
        "cloaked bibliography must defang the URL: {cloaked}"
    );
}

#[test]
fn render_bibliography_truncates_preview_to_named_cap() {
    // F-15: the preview cap is a named const, so the truncation boundary is
    // stable and observable.
    let cap = crate::limits::BIBLIOGRAPHY_PREVIEW_CHARS;
    let long_body = "x".repeat(cap + 50);
    let source = Source::Web {
        published_at: None,
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: Utc::now(),
        body_path: PathBuf::from("sources/web-01.md"),
        relevance: String::new(),
        body: long_body,
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    };
    let out = render_bibliography(&[source], false);
    assert!(
        out.contains(&"x".repeat(cap)),
        "preview must retain exactly the cap worth of characters"
    );
    assert!(
        !out.contains(&"x".repeat(cap + 1)),
        "preview must not exceed the cap"
    );
}

#[test]
fn mark_in_progress_only_when_not_archived() {
    let mut item = sample_item();
    mark_in_progress(&mut item);
    assert_eq!(item.status, ResearchStatus::InProgress);

    item.set_status(ResearchStatus::Archived);
    mark_in_progress(&mut item);
    // Archived is a terminal state - gathering cannot restart it.
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
        author: None,
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
        finding.contains("- [1] Example Article - [https://example.com](https://example.com)"),
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
        author: None,
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
        author: None,
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
        author: None,
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
        author: None,
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
        author: None,
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
        author: None,
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
            "**Source date range:** - (cited web sources did not expose a publication date)"
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
        author: None,
    });
    let mut doc = sample_doc(item);
    // The LLM already produced its own Sources paragraph.
    doc.findings = vec![
        "**Headline:** Observation summary

**Observation:** Something important [#1].

**Sources:**
- Article A - https://a"
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
    assert!(assembled.body.contains("open - report may ship"));
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
        assembled.corpa.contains("## Source Tensions"),
        "CORPA.md must contain Source Tensions section"
    );
    assert!(
        !assembled.body.contains("## Source Tensions"),
        "RESEARCH.md must no longer carry the Source Tensions section"
    );
    assert!(assembled.corpa.contains("contradiction"));
    assert!(assembled.corpa.contains("performance"));
    assert!(assembled.corpa.contains("#1, #2"));
    assert!(assembled.corpa.contains("opposing performance claims"));
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
    // IMRaD and report layouts share one CORPA.md companion: the QA
    // sections always render as top-level `##` headings there.
    assert!(
        assembled.corpa.contains("## Source Tensions"),
        "CORPA.md must render Source Tensions"
    );
    assert!(!assembled.body.contains("### Source Tensions"));
    assert!(assembled.corpa.contains("shallow evidence"));
    assert!(assembled.corpa.contains("cost"));
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
    assert!(assembled.body.contains("closed - human approval required"));
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

// ── Data Quality & Consistency summary ────────────────────────────────

/// Build a `ResearchDocument` with all five QA artifacts populated so the
/// Data Quality & Consistency summary has something to synthesize.
fn doc_with_all_qa_artifacts() -> ResearchDocument {
    use crate::contradiction::{ContradictionClaim, ContradictionEdge, ContradictionGraph};
    use crate::corpus_critic::{CorpusCriticReport, GapFetchResult};
    use crate::reconcile::{
        CrossLocusReconcile, ReconcilePair, SourceTensions, TensionKind, TensionRecord,
    };
    use crate::synthesis::{CriticReport, SynthesisAudit};
    use std::path::PathBuf;

    let sources = [
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
            author: None,
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
            author: None,
        },
    ];

    let mut graph = ContradictionGraph::empty();
    graph.add_edge(ContradictionEdge {
        claim_a: ContradictionClaim::from_source("claims better", 1, &sources[0]),
        claim_b: ContradictionClaim::from_source("claims worse", 2, &sources[1]),
        dimension: "performance".into(),
        note: "opposing performance claims".into(),
        strength: 72,
    });

    let tensions = SourceTensions {
        tensions: vec![
            TensionRecord {
                kind: TensionKind::Contradiction,
                label: "performance".into(),
                source_indices: vec![1, 2],
                note: "opposing claims".into(),
            },
            TensionRecord {
                kind: TensionKind::ShallowEvidence,
                label: "cost".into(),
                source_indices: vec![3],
                note: "only one source".into(),
            },
        ],
        sources_scanned: 2,
    };

    let reconcile = CrossLocusReconcile {
        pairs: vec![ReconcilePair {
            locus_a: "performance".into(),
            locus_b: "cost".into(),
            shared_source_indices: vec![1],
            shared_sources: 1,
            conflicting_edges: 1,
            note: "shared source disagrees".into(),
        }],
        sources_scanned: 2,
    };

    let audit = SynthesisAudit {
        summary: "Audit complete.".into(),
        findings: Vec::new(),
        top_implications: Vec::new(),
        cross_references: Vec::new(),
        open_questions: Vec::new(),
        critic_reports: vec![CriticReport {
            name: "coverage".into(),
            score: 60,
            issues: vec!["thin coverage".into()],
            gaps: Vec::new(),
            passed: false,
        }],
        overall_score: 65,
        recommendation: "Proceed with caution - issues: thin coverage.".into(),
        sources_used: 2,
    };

    let corpus_critic = CorpusCriticReport {
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
    };

    let gap_fetch = GapFetchResult {
        queries: vec!["topic cost evidence".into()],
        new_sources: 2,
        failed_queries: 0,
        attempted: true,
        note: String::new(),
    };

    let mut doc = sample_doc(sample_item());
    doc.item.sources = sources.to_vec();
    doc.contradiction_graph = Some(graph);
    doc.source_tensions = Some(tensions);
    doc.cross_locus_reconcile = Some(reconcile);
    doc.synthesis_audit = Some(audit);
    doc.corpus_critic = Some(corpus_critic);
    doc.gap_fetch = Some(gap_fetch);
    doc
}

#[test]
fn render_data_quality_summary_returns_empty_when_no_qa_data() {
    let doc = sample_doc(sample_item());
    let rendered = render_data_quality_summary(&doc);
    assert!(
        rendered.is_empty(),
        "expected empty string when no QA artifacts present, got: {rendered}"
    );
}

#[test]
fn render_data_quality_summary_synthesizes_all_artifacts() {
    let doc = doc_with_all_qa_artifacts();
    let rendered = render_data_quality_summary(&doc);
    // Verdict line uses the synthesis-audit recommendation.
    assert!(
        rendered.contains("**Overall verdict:** Proceed with caution"),
        "verdict should come from synthesis audit: {rendered}"
    );
    // Metrics table includes rows from every populated artifact.
    assert!(rendered.contains("| Metric | Value | Detail |"));
    assert!(
        rendered.contains("Corpus critic"),
        "corpus critic row should be present: {rendered}"
    );
    assert!(
        rendered.contains("Contradictions"),
        "contradictions row should be present: {rendered}"
    );
    assert!(
        rendered.contains("Source tensions"),
        "source tensions row should be present: {rendered}"
    );
    assert!(
        rendered.contains("Cross-locus reconcile"),
        "cross-locus reconcile row should be present: {rendered}"
    );
    assert!(
        rendered.contains("Synthesis audit"),
        "synthesis audit row should be present: {rendered}"
    );
    // Key concerns surface the top issue from each artifact.
    assert!(
        rendered.contains("**Key concerns:**"),
        "key concerns section must be present: {rendered}"
    );
    assert!(
        rendered.contains("Corpus: shallow evidence on Cost"),
        "corpus critic issue should appear: {rendered}"
    );
    assert!(
        rendered.contains("Contradiction: 1 vs 2"),
        "contradiction edge should appear: {rendered}"
    );
    assert!(
        rendered.contains("Tension (contradiction): performance"),
        "source tension should appear: {rendered}"
    );
    assert!(
        rendered.contains("Reconcile: performance <-> cost - 1 conflicting edge(s)"),
        "reconcile conflict should appear: {rendered}"
    );
    assert!(
        rendered.contains("Audit: Audit complete."),
        "audit summary should appear: {rendered}"
    );
}

#[test]
fn assemble_document_report_renders_data_quality_summary_after_implications() {
    let doc = doc_with_all_qa_artifacts();
    let assembled = assemble_document(&doc);
    assert!(
        assembled.body.contains("## Data Quality & Consistency"),
        "report layout must contain Data Quality & Consistency section"
    );
    // The section must appear after Top 10 Implications and before Findings.
    let implications_pos = assembled.body.find("## Top 10 Implications").unwrap();
    let dq_pos = assembled
        .body
        .find("## Data Quality & Consistency")
        .expect("DQ section should be present");
    let findings_pos = assembled.body.find("## Findings").unwrap();
    assert!(
        implications_pos < dq_pos,
        "DQ summary should come after Top 10 Implications"
    );
    assert!(
        dq_pos < findings_pos,
        "DQ summary should come before Findings"
    );
}

#[test]
fn assemble_document_report_omits_data_quality_summary_when_no_qa() {
    let doc = sample_doc(sample_item());
    let assembled = assemble_document(&doc);
    assert!(
        !assembled.body.contains("## Data Quality & Consistency"),
        "skeleton should not contain DQ section: {}",
        assembled.body
    );
}

#[test]
fn assemble_document_imrad_renders_data_quality_summary_in_discussion() {
    let mut doc = doc_with_all_qa_artifacts();
    doc.output_format = crate::run_config::OutputFormat::Imrad;
    let assembled = assemble_document(&doc);
    assert!(
        assembled.body.contains("### Data Quality & Consistency"),
        "IMRaD layout must render DQ summary as a subsection"
    );
    // It must appear inside Discussion, before the Contradiction Graph
    // subsection.
    let discussion_pos = assembled.body.find("## Discussion").unwrap();
    let dq_pos = assembled
        .body
        .find("### Data Quality & Consistency")
        .unwrap();
    let contradiction_pos = assembled
        .body
        .find("### Contradiction Graph")
        .unwrap_or(usize::MAX);
    assert!(
        discussion_pos < dq_pos,
        "DQ summary should come after Discussion heading"
    );
    assert!(
        dq_pos < contradiction_pos,
        "DQ summary should come before Contradiction Graph subsection"
    );
}

#[test]
fn assemble_document_imrad_omits_data_quality_summary_when_no_qa() {
    let mut doc = sample_doc(sample_item());
    doc.output_format = crate::run_config::OutputFormat::Imrad;
    let assembled = assemble_document(&doc);
    assert!(
        !assembled.body.contains("### Data Quality & Consistency"),
        "IMRaD skeleton should not contain DQ section: {}",
        assembled.body
    );
}
