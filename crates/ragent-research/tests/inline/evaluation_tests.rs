//! Inline tests for `evaluation.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

fn web_source(url: &str) -> Source {
    Source::Web {
        url: url.into(),
        title: "Source".into(),
        captured_at: chrono::Utc::now(),
        published_at: None,
        body_path: std::path::PathBuf::from("sources/web-01.md"),
        body: "body".into(),
        relevance: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "openalex".into(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    }
}

#[test]
fn failed_when_no_topic_or_brief() {
    let card = evaluate_report("", None, "", &[], &[], &OutputFormat::Report);
    assert!(card.is_failed());
    assert!(card.error.as_deref().unwrap().contains("no topic"));
}

#[test]
fn groundedness_rewards_citations_and_sources() {
    let finding = "Rust async is useful [#1].".to_string();
    let sources = vec![web_source("https://example.com")];
    let card = evaluate_report(
        "Rust async",
        None,
        "Summary.",
        &[finding],
        &sources,
        &OutputFormat::Report,
    );
    assert!(card.groundedness >= 50);
    assert!(card.overall > 0);
}

#[test]
fn structure_rewards_finding_labels() {
    let finding = "**Headline:** Important\n\n**Observation:** it works.".to_string();
    let card = evaluate_report(
        "Rust async",
        None,
        "Summary.",
        &[finding],
        &[],
        &OutputFormat::Report,
    );
    assert!(card.structure > 20);
}

#[test]
fn render_scorecard_includes_all_dimensions() {
    let card = EvaluationScorecard::new(80, 70, 90, 60, 75, "Looks good.");
    let rendered = render_scorecard(&card);
    assert!(rendered.contains("## Self-Evaluation Scorecard"));
    assert!(rendered.contains("| Quality | 80/100 |"));
    assert!(rendered.contains("| **Overall** | **75/100** |"));
    assert!(rendered.contains("Looks good."));
}

#[test]
fn render_failed_scorecard_surfaces_error() {
    let card = EvaluationScorecard::failed("something went wrong");
    let rendered = render_scorecard(&card);
    assert!(rendered.contains("something went wrong"));
}
