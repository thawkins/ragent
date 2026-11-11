//! A regression guard over the real `aisurvey` report pair: the deck must group
//! its findings under the report's own concepts.
//!
//! The fixture is read from the checked-in `research/aisurvey/` item, so the
//! test pins the shape of a real, LLM-written report rather than a hand-built
//! one. Nothing is written: the pair is parsed and rendered in memory only.

use std::path::Path;

use ragent_research::deck::{
    DeckElement, DeckRenderOptions, ParsedReport, ReportSeries, diff_reports, parse_report,
    render_deck,
};

/// Read `research/aisurvey/RESEARCH-<stamp>.md` from the repository root.
fn report(stamp: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../research/aisurvey")
        .join(format!("RESEARCH-{stamp}.md"));
    std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "the fixture report {} must be readable: {error}",
            path.display()
        )
    })
}

/// Parse a report that is expected to succeed.
fn parse_ok(text: &str) -> ParsedReport {
    match parse_report(text) {
        Ok(parsed) => parsed,
        Err(error) => panic!("the fixture report must parse: {error}"),
    }
}

#[test]
fn test_real_report_pair_groups_findings_under_its_concepts() {
    let older = parse_ok(&report("2026-11-02"));
    let newer = parse_ok(&report("2026-11-03"));

    let concepts = newer.items(DeckElement::Concepts);
    assert!(
        concepts.len() >= 3,
        "the fixture report declares its concepts: {concepts:?}"
    );
    assert!(
        concepts.iter().all(|concept| !concept.detail.is_empty()),
        "every concept carries its definition prose"
    );

    let diff = diff_reports(&older, &newer);
    let pair = ReportSeries::classify(&[
        "RESEARCH-2026-11-02.md".to_string(),
        "RESEARCH-2026-11-03.md".to_string(),
    ])
    .select_pair("aisurvey", None, None)
    .expect("two dated reports select a pair");
    let options = DeckRenderOptions::new(
        "aisurvey",
        chrono::NaiveDate::from_ymd_opt(2026, 11, 4).expect("a real date"),
        &pair,
        &older,
        &newer,
    );
    let deck = render_deck(&diff, &options);

    // One slide per concept, each headed by the concept's own name.
    for concept in concepts {
        assert!(
            deck.markdown.contains(&format!("\n## {}\n", concept.text)),
            "the deck must carry a slide for concept {:?}",
            concept.text
        );
    }
    // The reader-facing narrative the deck no longer renders.
    assert!(
        !deck.markdown.contains("## Top Implications"),
        "no implication slide"
    );
    assert!(
        !deck.markdown.contains("## Open Questions"),
        "no question slide"
    );
}
