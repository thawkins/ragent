//! Inline tests for `mod.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

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

struct BlockedMockEngine;

#[async_trait::async_trait]
impl SearchEngine for BlockedMockEngine {
    fn name(&self) -> &'static str {
        "blocked"
    }
    async fn search(&self, _query: &str, _opts: &SearchOptions) -> EngineReport {
        EngineReport::blocked("blocked", "rate-limited")
    }
}

#[test]
fn test_build_cache_key_includes_query() {
    let opts = SearchOptions::default();
    let key1 = build_cache_key("rust", &opts);
    let key2 = build_cache_key("python", &opts);
    assert_ne!(key1, key2);
}

#[test]
fn test_build_cache_key_case_insensitive_query() {
    let opts = SearchOptions::default();
    let key1 = build_cache_key("Rust", &opts);
    let key2 = build_cache_key("rust", &opts);
    assert_eq!(key1, key2);
}

#[test]
fn test_build_cache_key_includes_max_results() {
    let opts1 = SearchOptions::new(5);
    let opts2 = SearchOptions::new(10);
    let key1 = build_cache_key("rust", &opts1);
    let key2 = build_cache_key("rust", &opts2);
    assert_ne!(key1, key2);
}

#[test]
fn test_build_cache_key_includes_per_engine_results() {
    let opts1 = SearchOptions::new(10).with_per_engine_results(50);
    let opts2 = SearchOptions::new(10).with_per_engine_results(75);
    let key1 = build_cache_key("rust", &opts1);
    let key2 = build_cache_key("rust", &opts2);
    assert_ne!(key1, key2);
}

#[test]
fn test_build_cache_key_includes_site() {
    let opts1 = SearchOptions::default().with_site("github.com");
    let opts2 = SearchOptions::default().with_site("stackoverflow.com");
    let key1 = build_cache_key("rust", &opts1);
    let key2 = build_cache_key("rust", &opts2);
    assert_ne!(key1, key2);
}

#[test]
fn test_build_cache_key_includes_freshness() {
    let opts1 = SearchOptions::default().with_freshness(Freshness::Day);
    let opts2 = SearchOptions::default().with_freshness(Freshness::Week);
    let key1 = build_cache_key("rust", &opts1);
    let key2 = build_cache_key("rust", &opts2);
    assert_ne!(key1, key2);
}

#[test]
fn test_build_cache_key_includes_page() {
    let opts1 = SearchOptions::default().with_page(0);
    let opts2 = SearchOptions::default().with_page(1);
    let key1 = build_cache_key("rust", &opts1);
    let key2 = build_cache_key("rust", &opts2);
    assert_ne!(key1, key2);
}

#[test]
fn test_build_cache_key_trims_query() {
    let opts = SearchOptions::default();
    let key1 = build_cache_key("  rust  ", &opts);
    let key2 = build_cache_key("rust", &opts);
    assert_eq!(key1, key2);
}

#[tokio::test]
async fn test_orchestrator_with_mock_engines() {
    let engines: Vec<Arc<dyn SearchEngine>> = vec![
        Arc::new(MockEngine {
            name: "ddg",
            results: vec![RawResult::new("A", "https://a.com", "Snip A", "ddg")],
        }),
        Arc::new(MockEngine {
            name: "brave",
            results: vec![RawResult::new("B", "https://b.com", "Snip B", "brave")],
        }),
    ];
    let orchestrator = SearchOrchestrator::with_engines(engines);

    let output = orchestrator.search("test", &SearchOptions::default()).await;

    assert_eq!(output.query, "test");
    assert!(!output.cached);
    assert_eq!(output.merge.total_engines, 2);
    assert_eq!(output.merge.results.len(), 2);
    assert!(output.engines_used.contains(&"ddg".to_string()));
    assert!(output.engines_used.contains(&"brave".to_string()));
}

#[tokio::test]
async fn test_orchestrator_empty_query_returns_empty() {
    let orchestrator = SearchOrchestrator::with_engines(vec![]);
    let output = orchestrator.search("", &SearchOptions::default()).await;

    assert_eq!(output.query, "");
    assert!(
        output.merge.results.is_empty(),
        "merged results should be empty"
    );
    assert!(!output.cached);
}

#[tokio::test]
async fn test_orchestrator_blocked_engine_reported() {
    let engines: Vec<Arc<dyn SearchEngine>> = vec![
        Arc::new(MockEngine {
            name: "ddg",
            results: vec![RawResult::new("A", "https://a.com", "", "ddg")],
        }),
        Arc::new(BlockedMockEngine),
    ];
    let orchestrator = SearchOrchestrator::with_engines(engines);

    let output = orchestrator.search("test", &SearchOptions::default()).await;

    // T-016: blocked entries are "name: reason" so callers see the cause.
    assert!(
        output
            .merge
            .blocked_engines
            .iter()
            .any(|e| e.starts_with("blocked")),
        "expected an entry starting with 'blocked', got {:?}",
        output.merge.blocked_engines
    );
    assert_eq!(output.merge.results.len(), 1);
}

#[tokio::test]
async fn test_orchestrator_cache_hit() {
    let engines: Vec<Arc<dyn SearchEngine>> = vec![Arc::new(MockEngine {
        name: "ddg",
        results: vec![RawResult::new("A", "https://a.com", "", "ddg")],
    })];
    let orchestrator = SearchOrchestrator::with_engines(engines);

    // First call - cache miss.
    let output1 = orchestrator.search("rust", &SearchOptions::default()).await;
    assert!(!output1.cached);
    assert_eq!(orchestrator.cache_size(), 1);

    // Second call - cache hit.
    let output2 = orchestrator.search("rust", &SearchOptions::default()).await;
    assert!(output2.cached);
    assert_eq!(output2.merge.results.len(), output1.merge.results.len());
}

#[tokio::test]
async fn test_orchestrator_cache_miss_different_query() {
    let engines: Vec<Arc<dyn SearchEngine>> = vec![Arc::new(MockEngine {
        name: "ddg",
        results: vec![RawResult::new("A", "https://a.com", "", "ddg")],
    })];
    let orchestrator = SearchOrchestrator::with_engines(engines);

    let _ = orchestrator.search("rust", &SearchOptions::default()).await;
    let output = orchestrator
        .search("python", &SearchOptions::default())
        .await;
    assert!(!output.cached);
    assert_eq!(orchestrator.cache_size(), 2);
}

#[tokio::test]
async fn test_orchestrator_cache_miss_different_options() {
    let engines: Vec<Arc<dyn SearchEngine>> = vec![Arc::new(MockEngine {
        name: "ddg",
        results: vec![RawResult::new("A", "https://a.com", "", "ddg")],
    })];
    let orchestrator = SearchOrchestrator::with_engines(engines);

    let _ = orchestrator.search("rust", &SearchOptions::new(5)).await;
    let output = orchestrator.search("rust", &SearchOptions::new(10)).await;
    assert!(!output.cached);
}

#[tokio::test]
async fn test_orchestrator_clear_cache() {
    let engines: Vec<Arc<dyn SearchEngine>> = vec![Arc::new(MockEngine {
        name: "ddg",
        results: vec![RawResult::new("A", "https://a.com", "", "ddg")],
    })];
    let orchestrator = SearchOrchestrator::with_engines(engines);

    let _ = orchestrator.search("rust", &SearchOptions::default()).await;
    assert_eq!(orchestrator.cache_size(), 1);

    orchestrator.clear_cache();
    assert_eq!(orchestrator.cache_size(), 0);

    // Next search should be a cache miss.
    let output = orchestrator.search("rust", &SearchOptions::default()).await;
    assert!(!output.cached);
}

#[tokio::test]
async fn test_orchestrator_max_results_cap() {
    let engines: Vec<Arc<dyn SearchEngine>> = vec![Arc::new(MockEngine {
        name: "ddg",
        results: (0..20)
            .map(|i| RawResult::new(format!("R{i}"), format!("https://r{i}.com"), "", "ddg"))
            .collect(),
    })];
    let orchestrator = SearchOrchestrator::with_engines(engines);

    let output = orchestrator.search("test", &SearchOptions::new(5)).await;

    assert!(output.merge.results.len() <= 5);
}

#[test]
fn test_orchestrator_engine_count() {
    let engines: Vec<Arc<dyn SearchEngine>> = vec![
        Arc::new(MockEngine {
            name: "a",
            results: vec![],
        }),
        Arc::new(MockEngine {
            name: "b",
            results: vec![],
        }),
    ];
    let orchestrator = SearchOrchestrator::with_engines(engines);
    assert_eq!(orchestrator.engine_count(), 2);
}

#[test]
fn test_orchestrator_engine_names() {
    let engines: Vec<Arc<dyn SearchEngine>> = vec![
        Arc::new(MockEngine {
            name: "alpha",
            results: vec![],
        }),
        Arc::new(MockEngine {
            name: "beta",
            results: vec![],
        }),
    ];
    let orchestrator = SearchOrchestrator::with_engines(engines);
    let names = orchestrator.engine_names();
    assert!(names.contains(&"alpha"));
    assert!(names.contains(&"beta"));
}

#[test]
fn test_orchestrator_select_engine_found() {
    let engines: Vec<Arc<dyn SearchEngine>> = vec![
        Arc::new(MockEngine {
            name: "alpha",
            results: vec![RawResult::new("A", "https://a.com", "", "alpha")],
        }),
        Arc::new(MockEngine {
            name: "beta",
            results: vec![RawResult::new("B", "https://b.com", "", "beta")],
        }),
    ];
    let orchestrator = SearchOrchestrator::with_engines(engines);

    let filtered = orchestrator
        .select_engine("beta")
        .expect("beta should be found");
    assert_eq!(filtered.engine_count(), 1);
    assert_eq!(filtered.engine_names(), vec!["beta"]);
}

#[test]
fn test_orchestrator_select_engine_not_found() {
    let engines: Vec<Arc<dyn SearchEngine>> = vec![Arc::new(MockEngine {
        name: "alpha",
        results: vec![],
    })];
    let orchestrator = SearchOrchestrator::with_engines(engines);
    assert!(orchestrator.select_engine("gamma").is_none());
}

#[test]
fn test_orchestrator_exclude_engines_removes_named() {
    let engines: Vec<Arc<dyn SearchEngine>> = vec![
        Arc::new(MockEngine {
            name: "openalex",
            results: vec![],
        }),
        Arc::new(MockEngine {
            name: "wikipedia",
            results: vec![],
        }),
        Arc::new(MockEngine {
            name: "langsearch",
            results: vec![],
        }),
    ];
    let orchestrator = SearchOrchestrator::with_engines(engines);

    let filtered = orchestrator.exclude_engines(&["openalex"]);

    assert_eq!(filtered.engine_count(), 2);
    assert_eq!(filtered.engine_names(), vec!["wikipedia", "langsearch"]);
    // The receiver is unchanged.
    assert_eq!(orchestrator.engine_count(), 3);
}

#[test]
fn test_orchestrator_exclude_engines_empty_is_identity() {
    let engines: Vec<Arc<dyn SearchEngine>> = vec![
        Arc::new(MockEngine {
            name: "openalex",
            results: vec![],
        }),
        Arc::new(MockEngine {
            name: "wikipedia",
            results: vec![],
        }),
    ];
    let orchestrator = SearchOrchestrator::with_engines(engines);

    let filtered = orchestrator.exclude_engines::<&str>(&[]);

    assert_eq!(filtered.engine_names(), vec!["openalex", "wikipedia"]);
}

#[test]
fn test_orchestrator_exclude_engines_ignores_unknown_names() {
    let engines: Vec<Arc<dyn SearchEngine>> = vec![
        Arc::new(MockEngine {
            name: "openalex",
            results: vec![],
        }),
        Arc::new(MockEngine {
            name: "wikipedia",
            results: vec![],
        }),
    ];
    let orchestrator = SearchOrchestrator::with_engines(engines);

    let filtered = orchestrator.exclude_engines(&["openalex", "not_a_real_engine"]);

    assert_eq!(filtered.engine_names(), vec!["wikipedia"]);
}

#[test]
fn test_orchestrator_exclude_engines_all_leaves_none() {
    let engines: Vec<Arc<dyn SearchEngine>> = vec![
        Arc::new(MockEngine {
            name: "openalex",
            results: vec![],
        }),
        Arc::new(MockEngine {
            name: "wikipedia",
            results: vec![],
        }),
    ];
    let orchestrator = SearchOrchestrator::with_engines(engines);

    let filtered = orchestrator.exclude_engines(&["openalex", "wikipedia"]);

    assert_eq!(filtered.engine_count(), 0);
}

#[test]
fn test_orchestrator_exclude_engines_accepts_string_slices() {
    let engines: Vec<Arc<dyn SearchEngine>> = vec![
        Arc::new(MockEngine {
            name: "openalex",
            results: vec![],
        }),
        Arc::new(MockEngine {
            name: "wikipedia",
            results: vec![],
        }),
    ];
    let orchestrator = SearchOrchestrator::with_engines(engines);
    let names = vec!["openalex".to_string()];

    let filtered = orchestrator.exclude_engines(&names);

    assert_eq!(filtered.engine_names(), vec!["wikipedia"]);
}

#[tokio::test]
async fn test_orchestrator_select_engine_returns_only_that_engine() {
    let engines: Vec<Arc<dyn SearchEngine>> = vec![
        Arc::new(MockEngine {
            name: "alpha",
            results: vec![RawResult::new("A", "https://a.com", "", "alpha")],
        }),
        Arc::new(MockEngine {
            name: "beta",
            results: vec![RawResult::new("B", "https://b.com", "", "beta")],
        }),
    ];
    let orchestrator = SearchOrchestrator::with_engines(engines);

    let filtered = orchestrator
        .select_engine("alpha")
        .expect("alpha should be found");
    let output = filtered.search("test", &SearchOptions::default()).await;

    assert_eq!(output.merge.results.len(), 1);
    assert_eq!(output.merge.results[0].title, "A");
    assert_eq!(output.merge.total_engines, 1);
}

#[test]
fn test_search_cache_ttl_is_5_minutes() {
    assert_eq!(SEARCH_CACHE_TTL, Duration::from_mins(5));
}

#[test]
fn test_engine_timeout_is_30_seconds() {
    assert_eq!(ENGINE_TIMEOUT, Duration::from_secs(30));
}

/// An engine that sleeps longer than `ENGINE_TIMEOUT`.
struct SlowMockEngine {
    name: &'static str,
    results: Vec<RawResult>,
    delay: Duration,
}

#[async_trait::async_trait]
impl SearchEngine for SlowMockEngine {
    fn name(&self) -> &str {
        self.name
    }
    async fn search(&self, _query: &str, _opts: &SearchOptions) -> EngineReport {
        tokio::time::sleep(self.delay).await;
        EngineReport::ok(self.name, self.results.clone())
    }
}

#[tokio::test]
async fn test_orchestrator_slow_engine_timed_out() {
    // One fast engine returns a result; one slow engine exceeds the
    // per-engine timeout and is dropped from the merge.
    let engines: Vec<Arc<dyn SearchEngine>> = vec![
        Arc::new(MockEngine {
            name: "fast",
            results: vec![RawResult::new("A", "https://a.com", "", "fast")],
        }),
        Arc::new(SlowMockEngine {
            name: "slow",
            results: vec![RawResult::new("B", "https://b.com", "", "slow")],
            delay: ENGINE_TIMEOUT + Duration::from_millis(500),
        }),
    ];
    let orchestrator = SearchOrchestrator::with_engines(engines);

    let output = orchestrator.search("test", &SearchOptions::default()).await;

    // Fast engine's result is present.
    assert_eq!(output.merge.results.len(), 1);
    assert_eq!(output.merge.results[0].title, "A");

    // Slow engine is reported as blocked/errored (it contributed no
    // results). T-016: entries carry `name: reason`.
    assert!(
        output
            .merge
            .blocked_engines
            .iter()
            .any(|e| e.starts_with("slow")),
        "expected 'slow' in blocked_engines, got {:?}",
        output.merge.blocked_engines
    );
}

#[tokio::test]
async fn test_orchestrator_all_engines_timed_out() {
    // All engines exceed the timeout - merge should be empty.
    let engines: Vec<Arc<dyn SearchEngine>> = vec![
        Arc::new(SlowMockEngine {
            name: "slow1",
            results: vec![RawResult::new("A", "https://a.com", "", "slow1")],
            delay: ENGINE_TIMEOUT + Duration::from_secs(1),
        }),
        Arc::new(SlowMockEngine {
            name: "slow2",
            results: vec![RawResult::new("B", "https://b.com", "", "slow2")],
            delay: ENGINE_TIMEOUT + Duration::from_secs(2),
        }),
    ];
    let orchestrator = SearchOrchestrator::with_engines(engines);

    let start = Instant::now();
    let output = orchestrator.search("test", &SearchOptions::default()).await;
    let elapsed = start.elapsed();

    // Should complete in roughly ENGINE_TIMEOUT, not ENGINE_TIMEOUT + 2s.
    assert!(
        elapsed < ENGINE_TIMEOUT + Duration::from_secs(1),
        "search took {elapsed:?}, expected ~{ENGINE_TIMEOUT:?}"
    );
    assert!(
        output.merge.results.is_empty(),
        "merged results should be empty"
    );
}
