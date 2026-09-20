//! Multi-engine consensus ranking regression tests (engine-representation).
//!
//! Guards against the merge cap being monopolised by a single engine. Two
//! mechanisms previously buried later engines:
//!
//! 1. Positional rank scores were derived from the flattened cross-engine
//!    list, so an engine returning 75 results pushed every later engine's
//!    unique URL to `rank_score(75+) ~= 0.08`.
//! 2. OpenAlex's normalised relevance saturated at 1.0 for most hits, tying
//!    the whole scholarly block at the top of the merge cap.
//!
//! The merge cap must leave room for other engines' top-ranked results.

use ragent_tools_extended::masterfetch::search::consensus::{
    merge_and_rank, merge_and_rank_with_cap,
};
use ragent_tools_extended::masterfetch::search::engine::{EngineReport, RawResult};

/// Build an OpenAlex-like report: `n` results carrying engine-provided
/// scores spread through the 0.05-0.6 band (post-normalisation, SCALE=100).
fn openalex_report(n: usize) -> EngineReport {
    let results: Vec<RawResult> = (0..n)
        .map(|i| {
            let mut r = RawResult::new(
                format!("Paper {i}"),
                format!("https://doi.org/10.1/{i}"),
                "abstract",
                "openalex",
            );
            // Descending relevance: top hits near 0.6, tail down to ~0.05.
            r.score = Some((i as f64).mul_add(-0.007, 0.6).max(0.05));
            r
        })
        .collect();
    EngineReport::ok("openalex", results)
}

/// Build an unscored (positional-rank) engine report with distinct URLs.
fn unscored_report(name: &str, url_prefix: &str, n: usize) -> EngineReport {
    let results: Vec<RawResult> = (0..n)
        .map(|i| {
            RawResult::new(
                format!("{name} hit {i}"),
                format!("https://{url_prefix}{i}.com"),
                "snippet",
                name,
            )
        })
        .collect();
    EngineReport::ok(name, results)
}

/// With a large saturated OpenAlex block first, the capped top-N must still
/// include later engines' top-ranked (rank 0) results.
#[test]
fn test_merge_cap_includes_later_engines_top_hits() {
    let reports = vec![
        openalex_report(75),
        unscored_report("wikipedia", "en.wikipedia.org/wiki/P", 5),
        unscored_report("langsearch", "web", 5),
    ];
    let out = merge_and_rank_with_cap(&reports, "LLM API pricing comparison 2025", 9);

    let openalex_count = out
        .results
        .iter()
        .filter(|r| r.source == "openalex")
        .count();
    let wikipedia_count = out
        .results
        .iter()
        .filter(|r| r.source == "wikipedia")
        .count();
    let langsearch_count = out
        .results
        .iter()
        .filter(|r| r.source == "langsearch")
        .count();

    assert_eq!(out.results.len(), 9);
    assert!(
        wikipedia_count >= 1,
        "wikipedia rank-0 result must survive the cap: sources {:?}",
        out.results
            .iter()
            .map(|r| r.source.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        langsearch_count >= 1,
        "langsearch rank-0 result must survive the cap: sources {:?}",
        out.results
            .iter()
            .map(|r| r.source.as_str())
            .collect::<Vec<_>>()
    );
    // Not monopolised: OpenAlex cannot take every slot while other engines
    // contributed rank-0 hits.
    assert!(openalex_count < 9);
}

/// A later engine's rank-0 unscored entry must score ~1.0, regardless of how
/// many results earlier engines returned (per-engine rank, not flat index).
#[test]
fn test_per_engine_rank_not_flat_index() {
    let reports = vec![openalex_report(75), unscored_report("brave", "brave", 3)];
    let out = merge_and_rank(&reports, "test");
    let brave_top = out
        .results
        .iter()
        .find(|r| r.url == "https://brave0.com/")
        .expect("brave rank-0 result should be present");
    assert!(
        (brave_top.relevance_score - 1.0).abs() < 1e-9,
        "brave rank-0 should score ~1.0 (per-engine rank), got {}",
        brave_top.relevance_score
    );
}

/// An earlier engine returning many results must not change a later engine's
/// score for its own top hit (rank is per-engine).
#[test]
fn test_later_engine_score_independent_of_earlier_engine_count() {
    let small = vec![openalex_report(1), unscored_report("brave", "brave", 3)];
    let large = vec![openalex_report(75), unscored_report("brave", "brave", 3)];
    let small_top = merged_top_url(&small, "https://brave0.com/");
    let large_top = merged_top_url(&large, "https://brave0.com/");
    assert!(
        (small_top - large_top).abs() < 1e-9,
        "brave rank-0 score must not depend on openalex result count: {small_top} vs {large_top}"
    );
}

fn merged_top_url(reports: &[EngineReport], url: &str) -> f64 {
    let out = merge_and_rank(reports, "test");
    out.results
        .iter()
        .find(|r| r.url == url)
        .expect("url should be present")
        .relevance_score
}

// ---------------------------------------------------------------------------
// Per-engine share limit (diversity-aware cap)
// ---------------------------------------------------------------------------

/// A dominant engine with a large scored block must hold at most half of the
/// capped slots while other engines have candidates to fill the rest.
#[test]
fn test_share_limit_bounds_dominant_engine() {
    let reports = vec![
        openalex_report(75),
        unscored_report("wikipedia", "en.wikipedia.org/wiki/P", 5),
        unscored_report("langsearch", "web", 5),
        unscored_report("serper", "serp", 5),
    ];
    let out = merge_and_rank_with_cap(&reports, "test", 9);

    let count = |name: &str| out.results.iter().filter(|r| r.source == name).count();
    assert_eq!(out.results.len(), 9, "cap must still fill the budget");
    // Share limit for max_results=9 is ceil(9/2)=5.
    assert!(
        count("openalex") <= 5,
        "openalex exceeded share limit: sources {:?}",
        out.results
            .iter()
            .map(|r| r.source.as_str())
            .collect::<Vec<_>>()
    );
    // Every contributing engine that returned results must keep presence.
    assert!(count("wikipedia") >= 1, "wikipedia lost all slots");
    assert!(count("langsearch") >= 1, "langsearch lost all slots");
    assert!(count("serper") >= 1, "serper lost all slots");
}

/// When the other engines have fewer candidates than the dominant engine's
/// share limit, leftover slots are refilled with the dominant engine's
/// highest-scoring skipped entries (full budget preserved).
#[test]
fn test_share_limit_refills_leftover_slots() {
    // 10 openalex results + 1 wikipedia result; cap 6 → openalex share 3,
    // wikipedia contributes 1, remaining 2 slots refill with openalex.
    let reports = vec![
        openalex_report(10),
        unscored_report("wikipedia", "en.wikipedia.org/wiki/P", 1),
    ];
    let out = merge_and_rank_with_cap(&reports, "test", 6);

    assert_eq!(out.results.len(), 6);
    let openalex = out
        .results
        .iter()
        .filter(|r| r.source == "openalex")
        .count();
    let wiki = out
        .results
        .iter()
        .filter(|r| r.source == "wikipedia")
        .count();
    assert_eq!(wiki, 1, "wikipedia candidate must survive the cap");
    assert_eq!(openalex, 5, "leftover slots must refill from openalex");
}

/// A single contributing engine is exempt from the share limit: the capped
/// merge returns the full budget of its best results.
#[test]
fn test_share_limit_not_applied_to_single_engine() {
    let reports = vec![openalex_report(75)];
    let out = merge_and_rank_with_cap(&reports, "test", 9);
    assert_eq!(out.results.len(), 9);
    assert!(
        out.results.iter().all(|r| r.source == "openalex"),
        "single-engine merge must keep only that engine's results"
    );
}

/// The share limit must not demote multi-engine consensus URLs: a URL
/// returned by two engines scores higher and stays in the capped list.
#[test]
fn test_share_limit_keeps_consensus_urls() {
    // openalex + wikipedia both return the same URL (consensus); wikipedia
    // also returns two unique URLs.
    let mut openalex_results: Vec<RawResult> = (0..10)
        .map(|i| {
            let mut r = RawResult::new(
                format!("Paper {i}"),
                format!("https://doi.org/10.1/{i}"),
                "abstract",
                "openalex",
            );
            r.score = Some((i as f64).mul_add(-0.05, 0.6).max(0.05));
            r
        })
        .collect();
    openalex_results.push(RawResult::new(
        "Shared page",
        "https://shared.example.org/page",
        "snippet",
        "openalex",
    ));
    let wiki_results = vec![
        RawResult::new(
            "Shared page",
            "https://shared.example.org/page",
            "snippet",
            "wikipedia",
        ),
        RawResult::new(
            "Wiki A",
            "https://en.wikipedia.org/wiki/A",
            "snippet",
            "wikipedia",
        ),
        RawResult::new(
            "Wiki B",
            "https://en.wikipedia.org/wiki/B",
            "snippet",
            "wikipedia",
        ),
    ];
    let reports = vec![
        EngineReport::ok("openalex", openalex_results),
        EngineReport::ok("wikipedia", wiki_results),
    ];
    let out = merge_and_rank_with_cap(&reports, "test", 4);

    let shared = out
        .results
        .iter()
        .find(|r| r.url == "https://shared.example.org/page")
        .expect("consensus URL must stay in the capped merge");
    assert_eq!(shared.engines_consensus, "2/2");
}
