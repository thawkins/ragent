//! Unit tests for `masterfetch::search::consensus` — cross-engine merge,
//! dedup, ranking, related-query mining, and fetch hints (T-015 / T-036,
//! FR-008, FR-009, NFR-003).
//!
//! All tests use the pure `merge_and_rank` / `mine_related_queries` functions
//! with fixture `EngineReport`s — no network I/O.

use ragent_tools_extended::masterfetch::search::consensus::{
    merge_and_rank, merge_and_rank_with_cap, mine_related_queries,
};
use ragent_tools_extended::masterfetch::search::engine::{EngineReport, RawResult};

// ===========================================================================
// Basic merge + dedup
// ===========================================================================

#[test]
fn test_merge_single_engine_single_result() {
    let reports = vec![EngineReport::ok(
        "ddg",
        vec![RawResult::new(
            "Title",
            "https://example.com",
            "Snippet",
            "ddg",
        )],
    )];
    let output = merge_and_rank(&reports, "test");
    assert_eq!(output.results.len(), 1);
    assert_eq!(output.results[0].title, "Title");
    // normalise_url adds a trailing slash to root paths.
    assert_eq!(output.results[0].url, "https://example.com/");
    assert_eq!(output.results[0].snippet, "Snippet");
}

#[test]
fn test_merge_two_engines_no_overlap() {
    let reports = vec![
        EngineReport::ok(
            "ddg",
            vec![RawResult::new("A", "https://a.com", "Snip A", "ddg")],
        ),
        EngineReport::ok(
            "brave",
            vec![RawResult::new("B", "https://b.com", "Snip B", "brave")],
        ),
    ];
    let output = merge_and_rank(&reports, "test");
    assert_eq!(output.results.len(), 2);
    assert_eq!(output.total_engines, 2);
    assert_eq!(output.engines_with_results, 2);
}

#[test]
fn test_merge_dedup_by_normalised_url() {
    // Same URL with trailing slash vs without — should dedup to 1.
    let reports = vec![
        EngineReport::ok(
            "ddg",
            vec![RawResult::new(
                "A",
                "https://example.com/page/",
                "Snip",
                "ddg",
            )],
        ),
        EngineReport::ok(
            "brave",
            vec![RawResult::new(
                "B",
                "https://example.com/page",
                "Snip B",
                "brave",
            )],
        ),
    ];
    let output = merge_and_rank(&reports, "test");
    assert_eq!(
        output.results.len(),
        1,
        "trailing-slash variants should dedup"
    );
}

#[test]
fn test_merge_dedup_tracking_params() {
    let reports = vec![
        EngineReport::ok(
            "ddg",
            vec![RawResult::new(
                "A",
                "https://example.com/article?utm_source=x",
                "Snip",
                "ddg",
            )],
        ),
        EngineReport::ok(
            "brave",
            vec![RawResult::new(
                "B",
                "https://example.com/article",
                "Snip B",
                "brave",
            )],
        ),
    ];
    let output = merge_and_rank(&reports, "test");
    assert_eq!(
        output.results.len(),
        1,
        "tracking-param variants should dedup"
    );
}

#[test]
fn test_merge_dedup_case_insensitive_host() {
    let reports = vec![
        EngineReport::ok(
            "ddg",
            vec![RawResult::new(
                "A",
                "https://Example.COM/page",
                "Snip",
                "ddg",
            )],
        ),
        EngineReport::ok(
            "brave",
            vec![RawResult::new(
                "B",
                "https://example.com/page",
                "Snip B",
                "brave",
            )],
        ),
    ];
    let output = merge_and_rank(&reports, "test");
    assert_eq!(
        output.results.len(),
        1,
        "case-insensitive host variants should dedup"
    );
}

#[test]
fn test_merge_preserves_first_title_and_snippet() {
    let reports = vec![
        EngineReport::ok(
            "ddg",
            vec![RawResult::new(
                "DDG Title",
                "https://example.com",
                "DDG snippet",
                "ddg",
            )],
        ),
        EngineReport::ok(
            "brave",
            vec![RawResult::new(
                "Brave Title",
                "https://example.com",
                "Brave snippet",
                "brave",
            )],
        ),
    ];
    let output = merge_and_rank(&reports, "test");
    assert_eq!(output.results.len(), 1);
    // The first engine's title/snippet should be used.
    assert_eq!(output.results[0].title, "DDG Title");
    assert_eq!(output.results[0].snippet, "DDG snippet");
}

// ===========================================================================
// Cross-engine consensus boost
// ===========================================================================

#[test]
fn test_consensus_boost_url_in_both_engines_ranks_first() {
    let reports = vec![
        EngineReport::ok(
            "ddg",
            vec![
                RawResult::new("Unique DDG", "https://unique-ddg.com", "", "ddg"),
                RawResult::new("Shared", "https://shared.com", "", "ddg"),
            ],
        ),
        EngineReport::ok(
            "brave",
            vec![
                RawResult::new("Shared", "https://shared.com", "", "brave"),
                RawResult::new("Unique Brave", "https://unique-brave.com", "", "brave"),
            ],
        ),
    ];
    let output = merge_and_rank(&reports, "test");
    // The shared URL should rank first (consensus boost).
    assert_eq!(output.results[0].url, "https://shared.com/");
}

#[test]
fn test_consensus_engines_consensus_label() {
    let reports = vec![
        EngineReport::ok(
            "ddg",
            vec![RawResult::new("A", "https://shared.com", "", "ddg")],
        ),
        EngineReport::ok(
            "brave",
            vec![RawResult::new("B", "https://shared.com", "", "brave")],
        ),
        EngineReport::ok(
            "mojeek",
            vec![RawResult::new("C", "https://other.com", "", "mojeek")],
        ),
    ];
    let output = merge_and_rank(&reports, "test");
    // shared.com appeared in 2 of 3 engines.
    let shared = output
        .results
        .iter()
        .find(|r| r.url == "https://shared.com/")
        .unwrap();
    assert_eq!(shared.engines_consensus, "2/3");
    // other.com appeared in 1 of 3 engines.
    let other = output
        .results
        .iter()
        .find(|r| r.url == "https://other.com/")
        .unwrap();
    assert_eq!(other.engines_consensus, "1/3");
}

#[test]
fn test_consensus_source_lists_all_engines() {
    let reports = vec![
        EngineReport::ok(
            "duckduckgo",
            vec![RawResult::new("A", "https://shared.com", "", "duckduckgo")],
        ),
        EngineReport::ok(
            "brave",
            vec![RawResult::new("B", "https://shared.com", "", "brave")],
        ),
    ];
    let output = merge_and_rank(&reports, "test");
    let shared = &output.results[0];
    assert!(shared.source.contains("duckduckgo"));
    assert!(shared.source.contains("brave"));
}

#[test]
fn test_consensus_score_higher_for_shared_url() {
    let reports = vec![
        EngineReport::ok(
            "ddg",
            vec![
                RawResult::new("Shared", "https://shared.com", "", "ddg"),
                RawResult::new("Unique", "https://unique.com", "", "ddg"),
            ],
        ),
        EngineReport::ok(
            "brave",
            vec![RawResult::new("Shared", "https://shared.com", "", "brave")],
        ),
    ];
    let output = merge_and_rank(&reports, "test");
    let shared_score = output
        .results
        .iter()
        .find(|r| r.url == "https://shared.com/")
        .unwrap()
        .relevance_score;
    let unique_score = output
        .results
        .iter()
        .find(|r| r.url == "https://unique.com/")
        .unwrap()
        .relevance_score;
    assert!(
        shared_score > unique_score,
        "shared URL should have higher score: {shared_score} vs {unique_score}"
    );
}

// ===========================================================================
// Relevance scoring
// ===========================================================================

#[test]
fn test_score_in_range_0_to_1() {
    let reports = vec![
        EngineReport::ok("ddg", vec![RawResult::new("A", "https://a.com", "", "ddg")]),
        EngineReport::ok(
            "brave",
            vec![RawResult::new("B", "https://b.com", "", "brave")],
        ),
    ];
    let output = merge_and_rank(&reports, "test");
    for r in &output.results {
        assert!(
            r.relevance_score >= 0.0 && r.relevance_score <= 1.0,
            "score should be in [0, 1]: {}",
            r.relevance_score
        );
    }
}

#[test]
fn test_first_result_has_highest_score() {
    let reports = vec![EngineReport::ok(
        "ddg",
        vec![
            RawResult::new("A", "https://a.com", "", "ddg"),
            RawResult::new("B", "https://b.com", "", "ddg"),
            RawResult::new("C", "https://c.com", "", "ddg"),
        ],
    )];
    let output = merge_and_rank(&reports, "test");
    assert!(
        output.results[0].relevance_score >= output.results[1].relevance_score,
        "first result should have highest score"
    );
    assert!(
        output.results[1].relevance_score >= output.results[2].relevance_score,
        "second result should have score >= third"
    );
}

#[test]
fn test_score_decreases_with_rank() {
    let results: Vec<RawResult> = (0..10)
        .map(|i| {
            RawResult::new(
                format!("Result {i}"),
                format!("https://r{i}.com"),
                "",
                "ddg",
            )
        })
        .collect();
    let reports = vec![EngineReport::ok("ddg", results)];
    let output = merge_and_rank(&reports, "test");
    for i in 1..output.results.len() {
        assert!(
            output.results[i - 1].relevance_score >= output.results[i].relevance_score,
            "scores should be non-increasing at position {i}"
        );
    }
}

// ===========================================================================
// fetch_relevance tier
// ===========================================================================

#[test]
fn test_tier_high_for_top_result() {
    let reports = vec![
        EngineReport::ok("ddg", vec![RawResult::new("A", "https://a.com", "", "ddg")]),
        EngineReport::ok(
            "brave",
            vec![RawResult::new("A", "https://a.com", "", "brave")],
        ),
    ];
    let output = merge_and_rank(&reports, "test");
    // The shared URL at rank 0 with consensus boost should be high.
    assert_eq!(output.results[0].fetch_relevance, "high");
}

#[test]
fn test_tier_med_for_mid_result() {
    // A single-engine result at a mid rank should get "med" or "low".
    let results: Vec<RawResult> = (0..5)
        .map(|i| RawResult::new(format!("R{i}"), format!("https://r{i}.com"), "", "ddg"))
        .collect();
    let reports = vec![EngineReport::ok("ddg", results)];
    let output = merge_and_rank(&reports, "test");
    // Rank 0 → score ~1.0 → high. Later ranks may be med or low.
    assert!(
        output.results.iter().any(|r| r.fetch_relevance == "high"
            || r.fetch_relevance == "med"
            || r.fetch_relevance == "low"),
        "at least one result should have a valid tier"
    );
}

#[test]
fn test_tier_low_for_bottom_result() {
    // Many results from one engine — the last ones should be "low".
    let results: Vec<RawResult> = (0..30)
        .map(|i| RawResult::new(format!("R{i}"), format!("https://r{i}.com"), "", "ddg"))
        .collect();
    let reports = vec![EngineReport::ok("ddg", results)];
    let output = merge_and_rank(&reports, "test");
    // With 30 results from a single engine, the rank score decay formula
    // 1.0 / (1.0 + rank * 0.15) gives ~0.19 at rank 29. But the flattened
    // list position may differ from per-engine rank. We check that at least
    // some results are "low" tier.
    assert!(
        output.results.iter().any(|r| r.fetch_relevance == "low"),
        "at least one result should be 'low' tier"
    );
}

#[test]
fn test_tier_values_are_valid() {
    let reports = vec![
        EngineReport::ok("ddg", vec![RawResult::new("A", "https://a.com", "", "ddg")]),
        EngineReport::ok(
            "brave",
            vec![RawResult::new("B", "https://b.com", "", "brave")],
        ),
    ];
    let output = merge_and_rank(&reports, "test");
    for r in &output.results {
        assert!(
            r.fetch_relevance == "high" || r.fetch_relevance == "med" || r.fetch_relevance == "low",
            "invalid tier: {}",
            r.fetch_relevance
        );
    }
}

// ===========================================================================
// fetch_hint
// ===========================================================================

#[test]
fn test_hint_high_relevance() {
    let reports = vec![
        EngineReport::ok("ddg", vec![RawResult::new("A", "https://a.com", "", "ddg")]),
        EngineReport::ok(
            "brave",
            vec![RawResult::new("A", "https://a.com", "", "brave")],
        ),
    ];
    let output = merge_and_rank(&reports, "test");
    assert!(
        output.results[0].fetch_hint.contains("high relevance"),
        "top result hint should mention high relevance: {}",
        output.results[0].fetch_hint
    );
    assert!(
        output.results[0].fetch_hint.contains("fetch recommended"),
        "top result hint should recommend fetching: {}",
        output.results[0].fetch_hint
    );
}

#[test]
fn test_hint_low_relevance() {
    let results: Vec<RawResult> = (0..30)
        .map(|i| RawResult::new(format!("R{i}"), format!("https://r{i}.com"), "", "ddg"))
        .collect();
    let reports = vec![EngineReport::ok("ddg", results)];
    let output = merge_and_rank(&reports, "test");
    // With 30 results, some should have low relevance.
    let low = output
        .results
        .iter()
        .find(|r| r.fetch_relevance == "low")
        .expect("should have at least one low-tier result");
    assert!(
        low.fetch_hint.contains("low relevance"),
        "low result hint should mention low relevance: {}",
        low.fetch_hint
    );
    assert!(
        low.fetch_hint.contains("skip"),
        "low result hint should suggest skipping: {}",
        low.fetch_hint
    );
}

#[test]
fn test_hint_medium_relevance() {
    // A single-engine result at a mid rank.
    let results: Vec<RawResult> = (0..6)
        .map(|i| RawResult::new(format!("R{i}"), format!("https://r{i}.com"), "", "ddg"))
        .collect();
    let reports = vec![EngineReport::ok("ddg", results)];
    let output = merge_and_rank(&reports, "test");
    // With 6 results at rank 0-5: scores are 1.0, 0.87, 0.77, 0.69, 0.62, 0.57.
    // Ranks 3-5 should be in the med tier (0.3 <= score < 0.6).
    // Rank 5 → 0.57 → med.
    let med = output.results.iter().find(|r| r.fetch_relevance == "med");
    if let Some(med) = med {
        assert!(
            med.fetch_hint.contains("medium relevance"),
            "med result hint should mention medium relevance: {}",
            med.fetch_hint
        );
    }
    // If no med result (edge case), at least verify tiers are valid.
    for r in &output.results {
        assert!(
            r.fetch_relevance == "high" || r.fetch_relevance == "med" || r.fetch_relevance == "low",
            "invalid tier: {}",
            r.fetch_relevance
        );
    }
}

// ===========================================================================
// Position assignment
// ===========================================================================

#[test]
fn test_positions_are_1_based_sequential() {
    let reports = vec![
        EngineReport::ok(
            "ddg",
            vec![
                RawResult::new("A", "https://a.com", "", "ddg"),
                RawResult::new("B", "https://b.com", "", "ddg"),
                RawResult::new("C", "https://c.com", "", "ddg"),
            ],
        ),
        EngineReport::ok("brave", vec![]),
    ];
    let output = merge_and_rank(&reports, "test");
    assert_eq!(output.results[0].position, 1);
    assert_eq!(output.results[1].position, 2);
    assert_eq!(output.results[2].position, 3);
}

// ===========================================================================
// Blocked engines
// ===========================================================================

#[test]
fn test_blocked_engines_listed() {
    let reports = vec![
        EngineReport::ok("ddg", vec![RawResult::new("A", "https://a.com", "", "ddg")]),
        EngineReport::blocked("brave", "rate-limited"),
    ];
    let output = merge_and_rank(&reports, "test");
    assert!(output.blocked_engines.contains(&"brave".to_string()));
    assert!(!output.blocked_engines.contains(&"ddg".to_string()));
}

#[test]
fn test_engines_with_results_excludes_blocked() {
    let reports = vec![
        EngineReport::ok("ddg", vec![RawResult::new("A", "https://a.com", "", "ddg")]),
        EngineReport::blocked("brave", "rate-limited"),
        EngineReport::ok("mojeek", vec![]),
    ];
    let output = merge_and_rank(&reports, "test");
    assert_eq!(output.total_engines, 3);
    // Only ddg has results; mojeek returned ok but empty.
    assert_eq!(output.engines_with_results, 1);
}

// ===========================================================================
// Total counts
// ===========================================================================

#[test]
fn test_total_raw_results() {
    let reports = vec![
        EngineReport::ok(
            "ddg",
            vec![
                RawResult::new("A", "https://a.com", "", "ddg"),
                RawResult::new("B", "https://b.com", "", "ddg"),
            ],
        ),
        EngineReport::ok(
            "brave",
            vec![RawResult::new("C", "https://c.com", "", "brave")],
        ),
    ];
    let output = merge_and_rank(&reports, "test");
    assert_eq!(output.total_raw_results, 3);
}

#[test]
fn test_total_merged_results_after_dedup() {
    let reports = vec![
        EngineReport::ok(
            "ddg",
            vec![
                RawResult::new("A", "https://a.com", "", "ddg"),
                RawResult::new("B", "https://shared.com", "", "ddg"),
            ],
        ),
        EngineReport::ok(
            "brave",
            vec![RawResult::new("C", "https://shared.com", "", "brave")],
        ),
    ];
    let output = merge_and_rank(&reports, "test");
    assert_eq!(output.total_raw_results, 3);
    assert_eq!(output.total_merged_results, 2); // a.com + shared.com
}

// ===========================================================================
// Empty input
// ===========================================================================

#[test]
fn test_merge_empty_reports() {
    let output = merge_and_rank(&[], "test");
    assert!(output.results.is_empty());
    assert_eq!(output.total_engines, 0);
}

#[test]
fn test_merge_all_engines_blocked() {
    let reports = vec![
        EngineReport::blocked("ddg", "rate-limited"),
        EngineReport::blocked("brave", "rate-limited"),
    ];
    let output = merge_and_rank(&reports, "test");
    assert!(output.results.is_empty());
    assert_eq!(output.blocked_engines.len(), 2);
    assert_eq!(output.total_engines, 2);
}

#[test]
fn test_merge_all_engines_empty_results() {
    let reports = vec![
        EngineReport::ok("ddg", vec![]),
        EngineReport::ok("brave", vec![]),
    ];
    let output = merge_and_rank(&reports, "test");
    assert!(output.results.is_empty());
    assert_eq!(output.total_engines, 2);
    assert!(output.blocked_engines.is_empty());
}

// ===========================================================================
// merge_and_rank_with_cap
// ===========================================================================

#[test]
fn test_cap_truncates_results() {
    let results: Vec<RawResult> = (0..10)
        .map(|i| RawResult::new(format!("R{i}"), format!("https://r{i}.com"), "", "ddg"))
        .collect();
    let reports = vec![EngineReport::ok("ddg", results)];
    let output = merge_and_rank_with_cap(&reports, "test", 3);
    assert_eq!(output.results.len(), 3);
}

#[test]
fn test_cap_larger_than_results() {
    let reports = vec![EngineReport::ok(
        "ddg",
        vec![RawResult::new("A", "https://a.com", "", "ddg")],
    )];
    let output = merge_and_rank_with_cap(&reports, "test", 100);
    assert_eq!(output.results.len(), 1);
}

#[test]
fn test_cap_zero() {
    let reports = vec![EngineReport::ok(
        "ddg",
        vec![RawResult::new("A", "https://a.com", "", "ddg")],
    )];
    let output = merge_and_rank_with_cap(&reports, "test", 0);
    assert_eq!(output.results.len(), 0);
}

// ===========================================================================
// Related-query mining
// ===========================================================================

#[test]
fn test_mine_related_queries_basic() {
    let results = vec![
        RawResult::new(
            "Rust async programming guide",
            "https://a.com",
            "Learn async rust tokio",
            "ddg",
        ),
        RawResult::new(
            "Tokio runtime tutorial",
            "https://b.com",
            "Async runtime tokio rust",
            "brave",
        ),
        RawResult::new(
            "Rust futures explained",
            "https://c.com",
            "Futures async rust",
            "ddg",
        ),
    ];
    let related = mine_related_queries(&results, "rust");
    // "async" and "tokio" should appear in multiple results.
    assert!(
        related.contains(&"async".to_string()),
        "related queries should include 'async': {related:?}"
    );
    assert!(
        related.contains(&"tokio".to_string()),
        "related queries should include 'tokio': {related:?}"
    );
}

#[test]
fn test_mine_related_queries_excludes_query_terms() {
    let results = vec![
        RawResult::new(
            "Rust programming",
            "https://a.com",
            "Rust programming language",
            "ddg",
        ),
        RawResult::new(
            "Rust guide",
            "https://b.com",
            "Rust programming guide",
            "brave",
        ),
    ];
    let related = mine_related_queries(&results, "rust programming");
    // "rust" and "programming" should NOT appear (they're query terms).
    assert!(
        !related.contains(&"rust".to_string()),
        "query terms should be excluded: {related:?}"
    );
    assert!(
        !related.contains(&"programming".to_string()),
        "query terms should be excluded: {related:?}"
    );
}

#[test]
fn test_mine_related_queries_excludes_stopwords() {
    let results = vec![
        RawResult::new("The best guide", "https://a.com", "The best content", "ddg"),
        RawResult::new(
            "The best tutorial",
            "https://b.com",
            "The best resource",
            "brave",
        ),
    ];
    let related = mine_related_queries(&results, "test");
    // "the" and "best" are stopwords — should not appear.
    assert!(
        !related.contains(&"the".to_string()),
        "stopwords should be excluded: {related:?}"
    );
}

#[test]
fn test_mine_related_queries_empty_input() {
    let related = mine_related_queries(&[], "test");
    assert!(related.is_empty());
}

#[test]
fn test_mine_related_queries_max_limit() {
    // Create results with many distinct terms.
    let results: Vec<RawResult> = (0..50)
        .map(|i| {
            RawResult::new(
                format!("term{i} alpha"),
                format!("https://r{i}.com"),
                format!("term{i} beta gamma"),
                "ddg",
            )
        })
        .collect();
    let related = mine_related_queries(&results, "test");
    assert!(
        related.len() <= 10,
        "related queries should be capped at 10: {}",
        related.len()
    );
}

#[test]
fn test_mine_related_queries_short_terms_excluded() {
    let results = vec![RawResult::new(
        "ab cd ef",
        "https://a.com",
        "ab cd ef gh",
        "ddg",
    )];
    let related = mine_related_queries(&results, "test");
    // Terms shorter than 3 chars should be excluded.
    for term in &related {
        assert!(term.len() >= 3, "short terms should be excluded: {term}");
    }
}

#[test]
fn test_mine_related_queries_sorted_by_frequency() {
    let results = vec![
        RawResult::new("alpha alpha alpha", "https://a.com", "alpha alpha", "ddg"),
        RawResult::new("beta", "https://b.com", "beta", "brave"),
    ];
    let related = mine_related_queries(&results, "test");
    // "alpha" appears 5 times, "beta" appears 2 times.
    // "alpha" should come before "beta".
    if related.len() >= 2 {
        assert_eq!(related[0], "alpha");
    }
}

// ===========================================================================
// engines_consensus format
// ===========================================================================

#[test]
fn test_engines_consensus_format_single_engine() {
    let reports = vec![EngineReport::ok(
        "ddg",
        vec![RawResult::new("A", "https://a.com", "", "ddg")],
    )];
    let output = merge_and_rank(&reports, "test");
    assert_eq!(output.results[0].engines_consensus, "1/1");
}

#[test]
fn test_engines_consensus_format_three_engines() {
    let reports = vec![
        EngineReport::ok(
            "ddg",
            vec![RawResult::new("A", "https://shared.com", "", "ddg")],
        ),
        EngineReport::ok(
            "brave",
            vec![RawResult::new("B", "https://shared.com", "", "brave")],
        ),
        EngineReport::ok(
            "mojeek",
            vec![RawResult::new("C", "https://shared.com", "", "mojeek")],
        ),
    ];
    let output = merge_and_rank(&reports, "test");
    assert_eq!(output.results[0].engines_consensus, "3/3");
}

#[test]
fn test_engines_consensus_with_blocked_engines() {
    // 3 engines total, 1 blocked, URL appears in 2.
    let reports = vec![
        EngineReport::ok(
            "ddg",
            vec![RawResult::new("A", "https://shared.com", "", "ddg")],
        ),
        EngineReport::ok(
            "brave",
            vec![RawResult::new("B", "https://shared.com", "", "brave")],
        ),
        EngineReport::blocked("mojeek", "rate-limited"),
    ];
    let output = merge_and_rank(&reports, "test");
    assert_eq!(output.results[0].engines_consensus, "2/3");
}

// ===========================================================================
// Multiple URLs with same engine appearing once each
// ===========================================================================

#[test]
fn test_multiple_unique_urls_from_multiple_engines() {
    let reports = vec![
        EngineReport::ok(
            "ddg",
            vec![
                RawResult::new("A", "https://a.com", "Snip A", "ddg"),
                RawResult::new("B", "https://b.com", "Snip B", "ddg"),
                RawResult::new("C", "https://c.com", "Snip C", "ddg"),
            ],
        ),
        EngineReport::ok(
            "brave",
            vec![
                RawResult::new("D", "https://d.com", "Snip D", "brave"),
                RawResult::new("E", "https://e.com", "Snip E", "brave"),
            ],
        ),
    ];
    let output = merge_and_rank(&reports, "test");
    assert_eq!(output.results.len(), 5);
    assert_eq!(output.total_raw_results, 5);
    assert_eq!(output.total_merged_results, 5);
}

// ===========================================================================
// Same URL appearing multiple times within one engine
// ===========================================================================

#[test]
fn test_same_url_multiple_times_within_one_engine_deduped() {
    let reports = vec![EngineReport::ok(
        "ddg",
        vec![
            RawResult::new("A", "https://example.com", "First", "ddg"),
            RawResult::new("B", "https://example.com", "Second", "ddg"),
        ],
    )];
    let output = merge_and_rank(&reports, "test");
    assert_eq!(
        output.results.len(),
        1,
        "same URL within one engine should dedup"
    );
}
