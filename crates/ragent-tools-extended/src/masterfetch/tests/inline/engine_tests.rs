//! Inline tests for `engine.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

// --- Freshness ---

#[test]
fn test_freshness_as_str() {
    assert_eq!(Freshness::Day.as_str(), "day");
    assert_eq!(Freshness::Week.as_str(), "week");
    assert_eq!(Freshness::Month.as_str(), "month");
    assert_eq!(Freshness::Year.as_str(), "year");
    assert_eq!(Freshness::Any.as_str(), "any");
}

#[test]
fn test_freshness_display() {
    assert_eq!(Freshness::Day.to_string(), "day");
    assert_eq!(Freshness::Any.to_string(), "any");
}

#[test]
fn test_freshness_from_str() {
    assert_eq!("day".parse::<Freshness>().unwrap(), Freshness::Day);
    assert_eq!("WEEK".parse::<Freshness>().unwrap(), Freshness::Week);
    assert_eq!("Month".parse::<Freshness>().unwrap(), Freshness::Month);
    assert_eq!("year".parse::<Freshness>().unwrap(), Freshness::Year);
    assert_eq!("any".parse::<Freshness>().unwrap(), Freshness::Any);
    assert_eq!("".parse::<Freshness>().unwrap(), Freshness::Any);
}

#[test]
fn test_freshness_from_str_invalid() {
    assert!("hour".parse::<Freshness>().is_err());
    assert!("invalid".parse::<Freshness>().is_err());
}

#[test]
fn test_freshness_default_is_any() {
    assert_eq!(Freshness::default(), Freshness::Any);
}

// --- SearchOptions ---

#[test]
fn test_search_options_default() {
    let opts = SearchOptions::default();
    assert_eq!(opts.max_results, DEFAULT_MAX_RESULTS);
    assert_eq!(opts.per_engine_results, DEFAULT_PER_ENGINE_RESULTS);
    assert_eq!(opts.site, String::new());
    assert_eq!(opts.exclude_sites, Vec::<String>::new());
    assert_eq!(opts.freshness, Freshness::Any);
    assert_eq!(opts.page, DEFAULT_PAGE);
}

#[test]
fn test_search_options_new_clamps_max_results() {
    let opts = SearchOptions::new(0);
    assert_eq!(opts.max_results, 1);

    let opts = SearchOptions::new(1000);
    assert_eq!(opts.max_results, MAX_MAX_RESULTS);

    let opts = SearchOptions::new(6);
    assert_eq!(opts.max_results, 6);
}

#[test]
fn test_search_options_per_engine_results_default() {
    let opts = SearchOptions::new(10);
    assert_eq!(opts.per_engine_results, DEFAULT_PER_ENGINE_RESULTS);
}

#[test]
fn test_search_options_with_per_engine_results_clamps() {
    let opts = SearchOptions::new(10).with_per_engine_results(0);
    assert_eq!(opts.per_engine_results, 1);

    let opts = SearchOptions::new(10).with_per_engine_results(300);
    assert_eq!(opts.per_engine_results, MAX_PER_ENGINE_RESULTS);

    let opts = SearchOptions::new(10).with_per_engine_results(50);
    assert_eq!(opts.per_engine_results, 50);
}

#[test]
fn test_search_options_builder() {
    let opts = SearchOptions::new(5)
        .with_site("example.com")
        .with_exclude_sites(vec!["spam.com".into()])
        .with_freshness(Freshness::Week)
        .with_page(2)
        .with_per_engine_results(50);
    assert_eq!(opts.max_results, 5);
    assert_eq!(opts.per_engine_results, 50);
    assert_eq!(opts.site, "example.com");
    assert_eq!(opts.exclude_sites, vec!["spam.com"]);
    assert_eq!(opts.freshness, Freshness::Week);
    assert_eq!(opts.page, 2);
}

// --- RawResult ---

#[test]
fn test_raw_result_new() {
    let r = RawResult::new("Title", "https://example.com", "Snippet", "ddg");
    assert_eq!(r.title, "Title");
    assert_eq!(r.url, "https://example.com");
    assert_eq!(r.snippet, "Snippet");
    assert_eq!(r.source, "ddg");
    assert!(r.score.is_none());
}

#[test]
fn test_raw_result_default() {
    let r = RawResult::default();
    assert_eq!(r.title, String::new());
    assert_eq!(r.url, String::new());
    assert_eq!(r.snippet, String::new());
    assert_eq!(r.source, String::new());
    assert!(r.score.is_none());
}

#[test]
fn test_raw_result_normalised_url() {
    let r = RawResult::new("T", "https://Example.com/page/", "", "ddg");
    assert_eq!(r.normalised_url(), "https://example.com/page");
}

#[test]
fn test_raw_result_normalised_url_strips_tracking() {
    let r = RawResult::new("T", "https://example.com/a?utm_source=x&keep=1", "", "ddg");
    assert_eq!(r.normalised_url(), "https://example.com/a?keep=1");
}

#[test]
fn test_raw_result_normalised_url_invalid_falls_back() {
    let r = RawResult::new("T", "not a url", "", "ddg");
    // Falls back to raw URL.
    assert_eq!(r.normalised_url(), "not a url");
}

// --- EngineReport ---

#[test]
fn test_engine_report_ok() {
    let results = vec![RawResult::new("A", "https://a.com", "", "ddg")];
    let report = EngineReport::ok("ddg", results);
    assert_eq!(report.engine, "ddg");
    assert_eq!(report.result_count, 1);
    assert!(!report.engine_blocked);
    assert_eq!(report.error, String::new());
    assert!(report.has_results());
    assert!(report.is_success());
}

#[test]
fn test_engine_report_ok_empty_results() {
    let report = EngineReport::ok("ddg", vec![]);
    assert_eq!(report.result_count, 0);
    assert!(!report.has_results());
    assert!(report.is_success()); // success even with 0 results
}

#[test]
fn test_engine_report_blocked() {
    let report = EngineReport::blocked("brave", "rate limited (429)");
    assert_eq!(report.engine, "brave");
    assert!(report.engine_blocked);
    assert_eq!(report.error, "rate limited (429)");
    assert!(!report.has_results());
    assert!(!report.is_success());
}

#[test]
fn test_engine_report_error() {
    let report = EngineReport::error("mojeek", "timeout");
    assert_eq!(report.engine, "mojeek");
    assert!(!report.engine_blocked);
    assert_eq!(report.error, "timeout");
    assert!(!report.has_results());
    assert!(!report.is_success());
}

#[test]
fn test_engine_report_default() {
    let report = EngineReport::default();
    assert_eq!(report.engine, String::new());
    assert!(report.results.is_empty(), "results should be empty");
    assert!(!report.engine_blocked);
    assert_eq!(report.result_count, 0);
}

// --- normalise_result_url ---

#[test]
fn test_normalise_result_url_basic() {
    assert_eq!(
        normalise_result_url("https://Example.com/page/"),
        "https://example.com/page"
    );
}

#[test]
fn test_normalise_result_url_strips_tracking() {
    assert_eq!(
        normalise_result_url("https://example.com/a?utm_source=x&keep=1"),
        "https://example.com/a?keep=1"
    );
}

#[test]
fn test_normalise_result_url_idempotent() {
    let once = normalise_result_url("https://example.com/page/");
    let twice = normalise_result_url(&once);
    assert_eq!(once, twice);
}

#[test]
fn test_normalise_result_url_invalid_falls_back() {
    assert_eq!(normalise_result_url("not a url"), "not a url");
    assert_eq!(normalise_result_url(""), "");
}

#[test]
fn test_normalise_result_url_strips_default_port() {
    assert_eq!(
        normalise_result_url("https://example.com:443/page"),
        "https://example.com/page"
    );
}

// --- dedup_results_by_url ---

#[test]
fn test_dedup_removes_duplicates() {
    let results = vec![
        RawResult::new("A", "https://example.com/page/", "", "ddg"),
        RawResult::new("B", "https://example.com/page", "", "brave"),
        RawResult::new("C", "https://other.com", "", "ddg"),
    ];
    let deduped = dedup_results_by_url(&results);
    assert_eq!(deduped.len(), 2);
    assert_eq!(deduped[0].title, "A"); // first kept
    assert_eq!(deduped[1].title, "C");
}

#[test]
fn test_dedup_preserves_first_occurrence() {
    let results = vec![
        RawResult::new("Brave", "https://example.com/page", "", "brave"),
        RawResult::new("DDG", "https://example.com/page/", "", "ddg"),
    ];
    let deduped = dedup_results_by_url(&results);
    assert_eq!(deduped.len(), 1);
    assert_eq!(deduped[0].title, "Brave"); // first kept
}

#[test]
fn test_dedup_empty_input() {
    let deduped = dedup_results_by_url(&[]);
    assert!(deduped.is_empty(), "deduped should be empty");
}

#[test]
fn test_dedup_no_duplicates() {
    let results = vec![
        RawResult::new("A", "https://a.com", "", "ddg"),
        RawResult::new("B", "https://b.com", "", "ddg"),
        RawResult::new("C", "https://c.com", "", "ddg"),
    ];
    let deduped = dedup_results_by_url(&results);
    assert_eq!(deduped.len(), 3);
}

#[test]
fn test_dedup_tracking_params_ignored() {
    let results = vec![
        RawResult::new("A", "https://example.com/page?utm_source=x", "", "ddg"),
        RawResult::new("B", "https://example.com/page", "", "brave"),
    ];
    let deduped = dedup_results_by_url(&results);
    assert_eq!(deduped.len(), 1);
}

#[test]
fn test_dedup_case_insensitive_host() {
    let results = vec![
        RawResult::new("A", "https://Example.COM/page", "", "ddg"),
        RawResult::new("B", "https://example.com/page", "", "brave"),
    ];
    let deduped = dedup_results_by_url(&results);
    assert_eq!(deduped.len(), 1);
}

#[test]
fn test_dedup_default_port_normalized() {
    let results = vec![
        RawResult::new("A", "https://example.com:443/page", "", "ddg"),
        RawResult::new("B", "https://example.com/page", "", "brave"),
    ];
    let deduped = dedup_results_by_url(&results);
    assert_eq!(deduped.len(), 1);
}

#[test]
fn test_dedup_different_paths_not_duplicates() {
    let results = vec![
        RawResult::new("A", "https://example.com/page1", "", "ddg"),
        RawResult::new("B", "https://example.com/page2", "", "brave"),
    ];
    let deduped = dedup_results_by_url(&results);
    assert_eq!(deduped.len(), 2);
}

#[test]
fn test_dedup_different_queries_not_duplicates() {
    let results = vec![
        RawResult::new("A", "https://example.com/search?q=1", "", "ddg"),
        RawResult::new("B", "https://example.com/search?q=2", "", "brave"),
    ];
    let deduped = dedup_results_by_url(&results);
    assert_eq!(deduped.len(), 2);
}

// --- collect_all_results ---

#[test]
fn test_collect_all_results_flattens() {
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
    let all = collect_all_results(&reports);
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].title, "A");
    assert_eq!(all[2].title, "C");
}

#[test]
fn test_collect_all_results_empty_reports() {
    let all = collect_all_results(&[]);
    assert!(all.is_empty(), "all should be empty");
}

#[test]
fn test_collect_all_results_blocked_reports_contribute_nothing() {
    let reports = vec![
        EngineReport::ok("ddg", vec![RawResult::new("A", "https://a.com", "", "ddg")]),
        EngineReport::blocked("brave", "rate limited"),
    ];
    let all = collect_all_results(&reports);
    assert_eq!(all.len(), 1);
}

// --- count_engines_with_results ---

#[test]
fn test_count_engines_with_results() {
    let reports = vec![
        EngineReport::ok("ddg", vec![RawResult::new("A", "https://a.com", "", "ddg")]),
        EngineReport::blocked("brave", "rate limited"),
        EngineReport::ok("mojeek", vec![]), // 0 results but not blocked
    ];
    assert_eq!(count_engines_with_results(&reports), 1);
}

#[test]
fn test_count_engines_with_results_all_blocked() {
    let reports = vec![
        EngineReport::blocked("ddg", "blocked"),
        EngineReport::blocked("brave", "blocked"),
    ];
    assert_eq!(count_engines_with_results(&reports), 0);
}

#[test]
fn test_count_engines_with_results_all_success() {
    let reports = vec![
        EngineReport::ok("ddg", vec![RawResult::new("A", "https://a.com", "", "ddg")]),
        EngineReport::ok(
            "brave",
            vec![RawResult::new("B", "https://b.com", "", "brave")],
        ),
    ];
    assert_eq!(count_engines_with_results(&reports), 2);
}

// --- count_total_results ---

#[test]
fn test_count_total_results() {
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
    assert_eq!(count_total_results(&reports), 3);
}

#[test]
fn test_count_total_results_empty() {
    assert_eq!(count_total_results(&[]), 0);
}

// --- blocked_engine_names ---

#[test]
fn test_blocked_engine_names() {
    let reports = vec![
        EngineReport::ok("ddg", vec![]),
        EngineReport::blocked("brave", "rate limited"),
        EngineReport::error("mojeek", "timeout"),
    ];
    let blocked = blocked_engine_names(&reports);
    assert!(blocked.contains(&"brave"));
    assert!(blocked.contains(&"mojeek"));
    assert!(!blocked.contains(&"ddg"));
}

#[test]
fn test_blocked_engine_names_all_ok() {
    let reports = vec![EngineReport::ok(
        "ddg",
        vec![RawResult::new("A", "https://a.com", "", "ddg")],
    )];
    let blocked = blocked_engine_names(&reports);
    assert!(blocked.is_empty(), "blocked should be empty");
}

// --- SearchEngineError ---

#[test]
fn test_search_engine_error_display() {
    assert_eq!(
        SearchEngineError::EmptyQuery.to_string(),
        "search query must not be empty"
    );
    assert_eq!(
        SearchEngineError::RateLimited(429).to_string(),
        "engine rate-limited (HTTP 429)"
    );
}

// --- SearchEngine trait (mock implementation) ---

struct MockEngine {
    name: &'static str,
    results: Vec<RawResult>,
}

#[async_trait::async_trait]
impl SearchEngine for MockEngine {
    fn name(&self) -> &str {
        self.name
    }

    async fn search(&self, _query: &str, _opts: &SearchOptions) -> EngineReport {
        EngineReport::ok(self.name, self.results.clone())
    }
}

struct BlockedEngine;

#[async_trait::async_trait]
impl SearchEngine for BlockedEngine {
    fn name(&self) -> &'static str {
        "blocked-engine"
    }

    async fn search(&self, _query: &str, _opts: &SearchOptions) -> EngineReport {
        EngineReport::blocked("blocked-engine", "rate limited (429)")
    }
}

#[tokio::test]
async fn test_mock_engine_returns_results() {
    let engine = MockEngine {
        name: "mock",
        results: vec![
            RawResult::new("A", "https://a.com", "Snippet A", "mock"),
            RawResult::new("B", "https://b.com", "Snippet B", "mock"),
        ],
    };
    let report = engine.search("test", &SearchOptions::default()).await;
    assert_eq!(report.engine, "mock");
    assert_eq!(report.result_count, 2);
    assert!(report.is_success());
    assert!(!report.engine_blocked);
}

#[tokio::test]
async fn test_blocked_engine_returns_blocked_report() {
    let engine = BlockedEngine;
    let report = engine.search("test", &SearchOptions::default()).await;
    assert_eq!(report.engine, "blocked-engine");
    assert!(report.engine_blocked);
    assert!(!report.is_success());
    assert!(!report.has_results());
}

#[tokio::test]
async fn test_multiple_engines_in_parallel() {
    let engines: Vec<Box<dyn SearchEngine>> = vec![
        Box::new(MockEngine {
            name: "ddg",
            results: vec![RawResult::new("A", "https://a.com", "", "ddg")],
        }),
        Box::new(MockEngine {
            name: "brave",
            results: vec![RawResult::new("B", "https://b.com", "", "brave")],
        }),
        Box::new(BlockedEngine),
    ];

    let query = "test query";
    let opts = SearchOptions::default();

    // Run all engines concurrently.
    let mut handles = Vec::new();
    for _engine in &engines {
        let query = query.to_string();
        handles.push(tokio::spawn(async move {
            // Can't move Box<dyn SearchEngine> across spawn, so we
            // call directly - this test just verifies the pattern works.
            let _ = query;
        }));
    }
    // For the test, call sequentially (the parallel pattern is tested
    // by the consensus merger in T-015).
    let mut reports = Vec::new();
    for engine in &engines {
        reports.push(engine.search(query, &opts).await);
    }
    assert_eq!(reports.len(), 3);
    assert_eq!(count_engines_with_results(&reports), 2);
    assert_eq!(count_total_results(&reports), 2);
    assert!(blocked_engine_names(&reports).contains(&"blocked-engine"));
}

#[tokio::test]
async fn test_trait_object_dyn_compatibility() {
    // Verify the trait is dyn-compatible (object-safe).
    let engine: Box<dyn SearchEngine> = Box::new(MockEngine {
        name: "mock",
        results: vec![RawResult::new("A", "https://a.com", "", "mock")],
    });
    assert_eq!(engine.name(), "mock");
    let report = engine.search("test", &SearchOptions::default()).await;
    assert!(report.is_success());
}
