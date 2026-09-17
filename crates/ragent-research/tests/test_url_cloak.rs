//! Tests for the `/research create --url-cloak` source-URL defanging.
//!
//! Covers the three surfaces the flag touches:
//!
//! - the `cloak_url` primitive (scheme rewrite, dot bracketing, non-URL
//!   passthrough, code-span wrapping);
//! - the `References Index` table and the `**Sources:**` finding bullets, both
//!   of which must carry the URL as plain text when the flag is on and be
//!   byte-identical to the uncloaked output when it is off;
//! - the `--url-cloak` flag round-trip through the shared CLI parser and
//!   `ResearchRunRequest::from_invocation` replay.

use ragent_research::ResearchRunRequest;
use ragent_research::cli::ResearchCliCommand;
use ragent_research::io::{ResearchIo, cloak_url};
use ragent_research::source::Source;
use ragent_research::{ResearchDocument, ResearchItem, ResearchName, assemble_document};
use std::path::PathBuf;

fn web_source(url: &str) -> Source {
    Source::Web {
        url: url.to_string(),
        title: "Example Page".to_string(),
        captured_at: chrono::Utc::now(),
        published_at: None,
        body_path: PathBuf::new(),
        body: String::new(),
        relevance: "High".to_string(),
        search_tool: "mf_search".to_string(),
        search_engine: "wikipedia".to_string(),
        content_type: None,
        page_type: None,
        media_type: "page".to_string(),
        language: None,
        oa_recovery: None,
        author: None,
    }
}

fn doc_with(sources: Vec<Source>, findings: Vec<String>, cloak: bool) -> ResearchDocument {
    let name = ResearchName::new("cloak-test").expect("name must validate");
    let mut item = ResearchItem::new(name, "Cloak Test", "topic");
    for s in sources {
        item.add_source(s);
    }
    item.url_cloak = cloak;
    ResearchDocument {
        item,
        summary: "Summary text.".to_string(),
        findings,
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
        output_format: ragent_research::run_config::OutputFormat::Report,
        comparison_table: None,
        evaluation_scorecard: None,
        provider_stats: None,
    }
}

// ── cloak_url primitive ──────────────────────────────────────────────────

#[test]
fn cloak_url_rewrites_scheme_and_brackets_dots() {
    assert_eq!(
        cloak_url("https://example.com/path?a=b"),
        "`hxxps://example[.]com/path?a=b`"
    );
    assert_eq!(cloak_url("http://example.com"), "`hxxp://example[.]com`");
}

#[test]
fn cloak_url_preserves_scheme_case_position_and_non_urls() {
    // The scheme may be upper-case; only the prefix is rewritten.
    assert_eq!(cloak_url("HTTPS://Example.COM"), "`hxxps://Example[.]COM`");
    // Local paths, spec ids, and labels are not URLs and pass through.
    assert_eq!(cloak_url("src/lib.rs"), "src/lib.rs");
    assert_eq!(cloak_url("auth-refactor"), "auth-refactor");
}

#[test]
fn cloak_url_escapes_table_separators() {
    assert_eq!(cloak_url("https://x.com/a|b"), "`hxxps://x[.]com/a\\|b`");
}

// ── References Index ─────────────────────────────────────────────────────

#[test]
fn references_index_cloaks_web_urls_when_requested() {
    let table = ResearchIo::render_references_index_table(
        &[web_source("https://example.com")],
        chrono::Utc::now(),
        true,
    );
    assert!(
        table.contains("hxxps://example[.]com"),
        "cloaked table must carry the defanged URL: {table}"
    );
    assert!(
        !table.contains("https://example.com"),
        "the live URL must not survive cloaking: {table}"
    );
}

#[test]
fn references_index_matches_uncloaked_output_when_disabled() {
    let sources = vec![web_source("https://example.com")];
    let plain = ResearchIo::render_references_index_table(&sources, chrono::Utc::now(), false);
    assert!(plain.contains("https://example.com"));
    assert!(!plain.contains("hxxps"));
    assert!(!plain.contains("[.]"));
}

#[test]
fn references_index_leaves_non_web_paths_unchanged_under_cloak() {
    let sources = vec![Source::Local {
        path: "src/lib.rs".to_string(),
        kind: ragent_research::LocalSourceKind::InProject,
        captured_at: chrono::Utc::now(),
        body_path: PathBuf::new(),
        relevance: "anchor".to_string(),
        body: String::new(),
    }];
    let table = ResearchIo::render_references_index_table(&sources, chrono::Utc::now(), true);
    assert!(
        table.contains("src/lib.rs"),
        "local paths must not be clad in a code span: {table}"
    );
}

// ── Sources bullets ──────────────────────────────────────────────────────

#[test]
fn finding_sources_bullets_cloak_cited_web_urls() {
    let doc = doc_with(
        vec![web_source("https://example.com/page")],
        vec!["**Observation:** claim [#1]".to_string()],
        true,
    );
    let assembled = assemble_document(&doc);
    assert!(
        assembled.body.contains("hxxps://example[.]com/page"),
        "finding Sources bullet must carry the defanged URL: {}",
        assembled.body
    );
    assert!(
        !assembled.body.contains("https://example.com/page"),
        "the live URL must not survive in the body: {}",
        assembled.body
    );
}

#[test]
fn finding_sources_bullets_unchanged_without_cloak() {
    let doc = doc_with(
        vec![web_source("https://example.com/page")],
        vec!["**Observation:** claim [#1]".to_string()],
        false,
    );
    let assembled = assemble_document(&doc);
    assert!(assembled.body.contains("https://example.com/page"));
    assert!(!assembled.body.contains("hxxps"));
}

#[test]
fn corpa_sources_reference_cloaks_urls() {
    let doc = doc_with(
        vec![web_source("https://example.com/page")],
        vec!["**Observation:** claim [#1]".to_string()],
        true,
    );
    let assembled = assemble_document(&doc);
    assert!(
        assembled.corpa.contains("hxxps://example[.]com/page"),
        "CORPA Sources Reference must carry the defanged URL"
    );
}

// ── Flag round-trip ──────────────────────────────────────────────────────

#[test]
fn from_invocation_round_trips_url_cloak() {
    let req =
        ResearchRunRequest::from_invocation("ragent research create cloak \"topic\" --url-cloak")
            .expect("must parse");
    assert!(req.url_cloak);
}

#[test]
fn from_invocation_leaves_url_cloak_off_when_absent() {
    let req = ResearchRunRequest::from_invocation("ragent research create plain \"topic\"")
        .expect("must parse");
    assert!(!req.url_cloak);
}

#[test]
fn help_message_lists_url_cloak() {
    let help = ResearchCliCommand::build_help_message();
    assert!(
        help.contains("--url-cloak"),
        "help text must advertise the flag"
    );
}

#[test]
fn build_session_config_copies_url_cloak() {
    let req = ResearchRunRequest {
        url_cloak: true,
        ..ResearchRunRequest::new("cloak", "topic")
    };
    let cfg = ragent_research::build_session_config(&req, None);
    assert!(cfg.output.url_cloak);
}

// ── Frontmatter persistence (replay support) ─────────────────────────────

#[test]
fn frontmatter_records_url_cloak_and_round_trips() {
    let name = ResearchName::new("cloak-test").expect("valid name");
    let mut item = ResearchItem::new(name, "Cloak Test", "topic");
    item.url_cloak = true;
    let fm = item.render_frontmatter();
    assert!(
        fm.contains("url_cloak: true"),
        "frontmatter must record the flag; got:\n{fm}"
    );
    let parsed = ResearchItem::from_frontmatter(&fm).expect("must parse");
    assert!(parsed.url_cloak);
}

#[test]
fn frontmatter_omits_url_cloak_when_off() {
    let name = ResearchName::new("cloak-test").expect("valid name");
    let item = ResearchItem::new(name, "Cloak Test", "topic");
    let fm = item.render_frontmatter();
    assert!(
        !fm.contains("url_cloak"),
        "frontmatter must omit the line when off; got:\n{fm}"
    );
    let parsed = ResearchItem::from_frontmatter(&fm).expect("must parse");
    assert!(!parsed.url_cloak);
}
