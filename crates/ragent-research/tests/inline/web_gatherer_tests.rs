//! Inline tests for `web_gatherer.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::source_vault::{NewVaultSource, SourceVault};
use std::sync::Mutex;

/// Generate a body string of at least [`MIN_EXTRACTABLE_CONTENT_CHARS`]
/// characters so fake fetched pages pass the minimum-content-length guard
/// in [`WebGatherer::gather_with_observer`]. The `prefix` is repeated and
/// padded so callers can still recognise their test content in assertions.
fn body256(prefix: &str) -> String {
    let mut s = String::new();
    while s.chars().count() < MIN_EXTRACTABLE_CONTENT_CHARS {
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(prefix);
    }
    s
}

/// In-memory `WebSearchTool` for tests.
#[derive(Default)]
struct FakeSearch {
    hits: Vec<WebSearchHit>,
    calls: Mutex<Vec<String>>,
}

#[async_trait]
impl WebSearchTool for FakeSearch {
    async fn search(&self, query: &str, _max_results: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        self.calls.lock().unwrap().push(query.to_string());
        // Tag each returned hit with the query that produced it so the
        // gatherer's relevance computation has realistic metadata.
        let mut out = self.hits.clone();
        for hit in &mut out {
            hit.matched_query = query.to_string();
        }
        Ok(out)
    }
}

/// In-memory `WebFetchTool` for tests. Each URL maps to an optional
/// `WebFetchedPage`; missing URLs produce an error. URLs listed in
/// `fail_urls` fail with an untyped (network-classified) error; URLs in
/// `fail_kinds` fail with a typed [`FetchFailure`] carrying the given cause.
#[derive(Default)]
struct FakeFetch {
    pages: std::collections::HashMap<String, WebFetchedPage>,
    fail_urls: Vec<String>,
    fail_kinds: std::collections::HashMap<String, FetchFailureKind>,
    calls: Mutex<Vec<String>>,
}

#[async_trait]
impl WebFetchTool for FakeFetch {
    async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
        self.calls.lock().unwrap().push(url.to_string());
        if let Some(kind) = self.fail_kinds.get(url) {
            return Err(
                FetchFailure::new(*kind, format!("simulated fetch failure for {url}")).into(),
            );
        }
        if self.fail_urls.iter().any(|u| u == url) {
            anyhow::bail!("simulated fetch failure for {url}");
        }
        self.pages
            .get(url)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no fake page registered for {url}"))
    }
}

#[test]
fn relevance_exact_title_match_unchanged() {
    let (label, retained) = PreparedQuery::new("Rust async runtime").label(
        "Rust async runtime",
        "some unrelated snippet",
        "https://example.com/foo",
    );
    assert_eq!(label, "Very high - exact title match");
    assert!(retained);
}

#[test]
fn relevance_low_when_no_terms_match() {
    let (label, retained) = PreparedQuery::new("quantum computing").label(
        "Rust async runtime",
        "tokio and futures",
        "https://example.com/rust",
    );
    assert!(label.starts_with("Very low"));
    assert!(!retained);
}

fn gatherer_with(
    hits: Vec<WebSearchHit>,
    pages: std::collections::HashMap<String, WebFetchedPage>,
    fail_urls: Vec<String>,
) -> (WebGatherer, Arc<FakeSearch>, Arc<FakeFetch>) {
    let search = Arc::new(FakeSearch {
        hits,
        calls: Mutex::new(Vec::new()),
    });
    let fetch = Arc::new(FakeFetch {
        pages,
        fail_urls,
        calls: Mutex::new(Vec::new()),
        ..Default::default()
    });
    let g = WebGatherer::new(search.clone(), fetch.clone());
    (g, search, fetch)
}

#[tokio::test]
async fn gather_prefilters_low_relevance_hits_before_fetch() {
    // Both hits will be fetched if we don't pre-filter, but the second has
    // a title/snippet that does not match the query at all.
    let hits = vec![
        WebSearchHit {
            url: "https://good.example".into(),
            title: "Rust async runtime guide".into(),
            snippet: " Tokio and async Rust performance".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
            author: None,
        },
        WebSearchHit {
            url: "https://bad.example".into(),
            title: "completely unrelated shopping page".into(),
            snippet: "buy shoes and gadgets here".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
            author: None,
        },
    ];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://good.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://good.example".into(),
            title: "Rust async runtime guide".into(),
            body: Arc::from(body256("body good")),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    pages.insert(
        "https://bad.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://bad.example".into(),
            title: "completely unrelated shopping page".into(),
            body: Arc::from(body256("body bad")),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    let (g, _, fetch) = gatherer_with(hits, pages, Vec::new());
    let sources = g.gather("Rust async runtime", 5).await.unwrap();
    assert_eq!(
        sources.len(),
        1,
        "low-relevance hit should be pre-filtered before fetch"
    );
    if let Source::Web { url, .. } = &sources[0] {
        assert_eq!(url, "https://good.example");
    }
    let calls = fetch.calls.lock().unwrap();
    assert!(
        !calls.contains(&"https://bad.example".to_string()),
        "bad URL must not be fetched"
    );
}

#[tokio::test]
async fn gather_captures_scholarly_hit_without_fetch() {
    // An OpenAlex hit carries a reconstructed abstract in the snippet and
    // is captured as a self-contained source. The DOI/landing-page URL is
    // never fetched (it would be a paywalled redirect readability cannot
    // extract), and the lexical relevance pre-filter is bypassed because
    // OpenAlex already ranked the result by its own relevance score.
    let abstract_text = "We present a novel approach to async runtime \
         scheduling in Rust using a work-stealing executor that improves \
         throughput by thirty percent on benchmarks.";
    let hits = vec![WebSearchHit {
        url: "https://doi.org/10.1000/rust-async".into(),
        title: "Work-Stealing Async Scheduling in Rust".into(),
        snippet: format!("{abstract_text} (Year: 2024 | Cited: 17 | OA: yes | Source: ACM)"),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "openalex".into(),
        author: Some("Ada Lovelace, Alan Turing".into()),
    }];
    let (g, _, fetch) = gatherer_with(hits, std::collections::HashMap::new(), Vec::new());
    let sources = g.gather("free keyless open search APIs", 5).await.unwrap();
    assert_eq!(sources.len(), 1, "scholarly hit should be captured");
    match &sources[0] {
        Source::Web {
            url,
            title,
            relevance,
            page_type,
            search_engine,
            language,
            author,
            ..
        } => {
            assert_eq!(url, "https://doi.org/10.1000/rust-async");
            assert_eq!(
                author.as_deref(),
                Some("Ada Lovelace, Alan Turing"),
                "scholarly hit author should propagate to the captured source"
            );
            assert_eq!(title, "Work-Stealing Async Scheduling in Rust");
            assert_eq!(page_type.as_deref(), Some("scholarly"));
            assert_eq!(search_engine, "openalex");
            assert!(
                relevance.contains("Scholarly"),
                "scholarly sources use the engine-ranked relevance label"
            );
            assert_eq!(
                language.as_deref(),
                Some("English"),
                "OpenAlex snippet should have its language detected"
            );
        }
        other => panic!("expected Source::Web, got {other:?}"),
    }
    let calls = fetch.calls.lock().unwrap();
    assert!(
        calls.is_empty(),
        "scholarly hit must not trigger a URL fetch (no network), got {calls:?}"
    );
}

#[tokio::test]
async fn gather_filters_scholarly_hit_when_papers_not_set() {
    // With --papers unset, OpenAlex hits should be filtered out before
    // any fetch or capture.
    let hits = vec![WebSearchHit {
        url: "https://doi.org/10.1000/rust-async".into(),
        title: "Work-Stealing Async Scheduling in Rust".into(),
        snippet: format!(
            "We present a novel approach to async runtime \
             scheduling in Rust using a work-stealing executor that improves \
             throughput by thirty percent on benchmarks. \
             (Year: 2024 | Cited: 17 | OA: yes | Source: ACM)"
        ),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "openalex".into(),
        author: None,
    }];
    let (g, _, fetch) = gatherer_with(hits, std::collections::HashMap::new(), Vec::new());
    let g = g.with_disable_scholarly(true);
    let sources = g.gather("free keyless open search APIs", 5).await.unwrap();
    assert_eq!(
        sources.len(),
        0,
        "scholarly hit should be filtered out when --papers is not set"
    );
    let calls = fetch.calls.lock().unwrap();
    assert!(
        calls.is_empty(),
        "no fetch should be attempted for filtered scholarly hit"
    );
}

#[tokio::test]
async fn gather_rejects_scholarly_hit_with_no_abstract() {
    // An OpenAlex work with no abstract produces a snippet shorter than
    // `MIN_SCHOLARLY_CONTENT_CHARS`; it should be rejected rather than
    // admitted as near-empty noise.
    let hits = vec![WebSearchHit {
        url: "https://doi.org/10.1000/no-abstract".into(),
        title: "Untitled Work With No Abstract".into(),
        snippet: "(Year: 2024 | Cited: 0 | OA: no)".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "openalex".into(),
        author: None,
    }];
    let (g, _, fetch) = gatherer_with(hits, std::collections::HashMap::new(), Vec::new());
    let sources = g.gather("anything at all", 5).await.unwrap();
    assert!(
        sources.is_empty(),
        "abstract-less scholarly hit should be rejected"
    );
    let calls = fetch.calls.lock().unwrap();
    assert!(calls.is_empty(), "no fetch should be attempted");
}

#[tokio::test]
async fn gather_fetches_shared_engine_url_instead_of_treating_as_scholarly() {
    // When the same URL is returned by OpenAlex *and* a general web engine,
    // the page is a fetchable HTML page. The gatherer must take the normal
    // fetch path (capturing the richer page body) rather than treating it
    // as a scholarly snippet-only source.
    let hits = vec![WebSearchHit {
        url: "https://shared.example/paper".into(),
        title: "Rust async runtime".into(),
        snippet: "Rust async runtime tokio".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "langsearch, openalex".into(),
        author: None,
    }];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://shared.example/paper".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://shared.example/paper".into(),
            title: "Rust async runtime".into(),
            body: Arc::from(body256("full page body for the shared URL")),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    let (g, _, fetch) = gatherer_with(hits, pages, Vec::new());
    let sources = g.gather("Rust async runtime", 5).await.unwrap();
    assert_eq!(sources.len(), 1, "shared-engine hit should be captured");
    match &sources[0] {
        Source::Web { page_type, .. } => {
            assert_ne!(
                page_type.as_deref(),
                Some("scholarly"),
                "shared-engine URL must not be treated as scholarly"
            );
        }
        other => panic!("expected Source::Web, got {other:?}"),
    }
    let calls = fetch.calls.lock().unwrap();
    assert!(
        calls.contains(&"https://shared.example/paper".to_string()),
        "shared-engine URL must be fetched normally, got {calls:?}"
    );
}

#[tokio::test]
async fn gather_captures_encyclopedia_hit_without_fetch() {
    // A Wikipedia hit carries a page summary in the snippet and is captured
    // as a self-contained source. The article URL is never fetched (the
    // full Wikipedia HTML is large and readability extraction on it can
    // fail), and the lexical relevance pre-filter is bypassed because
    // Wikipedia's search API already ranked the result by its own
    // relevance score.
    let summary = "DuckDuckGo is an internet search engine that emphasizes \
         protecting searchers' privacy and avoiding the filter bubble of \
         personalized search results.";
    let hits = vec![WebSearchHit {
        url: "https://en.wikipedia.org/wiki/DuckDuckGo".into(),
        title: "DuckDuckGo".into(),
        snippet: summary.to_string(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "wikipedia".into(),
        author: None,
    }];
    let (g, _, fetch) = gatherer_with(hits, std::collections::HashMap::new(), Vec::new());
    // Query terms that do NOT lexically overlap with "DuckDuckGo" - this
    // would be rejected by the pre-filter if the encyclopedia bypass were
    // not in place.
    let sources = g
        .gather("free keyless web search APIs no API key required", 5)
        .await
        .unwrap();
    assert_eq!(sources.len(), 1, "encyclopedia hit should be captured");
    match &sources[0] {
        Source::Web {
            url,
            title,
            relevance,
            page_type,
            search_engine,
            language,
            ..
        } => {
            assert_eq!(url, "https://en.wikipedia.org/wiki/DuckDuckGo");
            assert_eq!(title, "DuckDuckGo");
            assert_eq!(page_type.as_deref(), Some("encyclopedia"));
            assert_eq!(search_engine, "wikipedia");
            assert!(
                relevance.contains("Encyclopedia"),
                "encyclopedia sources use the engine-ranked relevance label"
            );
            assert_eq!(
                language.as_deref(),
                Some("English"),
                "Wikipedia snippet should have its language detected"
            );
        }
        other => panic!("expected Source::Web, got {other:?}"),
    }
    let calls = fetch.calls.lock().unwrap();
    assert!(
        calls.is_empty(),
        "encyclopedia hit must not trigger a URL fetch (no network), got {calls:?}"
    );
}

#[tokio::test]
async fn gather_rejects_encyclopedia_hit_with_no_summary() {
    // A Wikipedia article with no extract produces a snippet shorter than
    // `MIN_ENCYCLOPEDIA_CONTENT_CHARS`; it should be rejected rather than
    // admitted as near-empty noise.
    let hits = vec![WebSearchHit {
        url: "https://en.wikipedia.org/wiki/No_Summary".into(),
        title: "No Summary".into(),
        snippet: "[thumbnail: https://example.com/thumb.png]".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "wikipedia".into(),
        author: None,
    }];
    let (g, _, fetch) = gatherer_with(hits, std::collections::HashMap::new(), Vec::new());
    let sources = g.gather("anything at all", 5).await.unwrap();
    assert!(
        sources.is_empty(),
        "summary-less encyclopedia hit should be rejected"
    );
    let calls = fetch.calls.lock().unwrap();
    assert!(calls.is_empty(), "no fetch should be attempted");
}

#[tokio::test]
async fn gather_fetches_shared_wikipedia_engine_url_instead_of_treating_as_encyclopedia() {
    // When the same URL is returned by Wikipedia *and* a general web engine,
    // the page is a fetchable HTML page. The gatherer must take the normal
    // fetch path (capturing the richer page body) rather than treating it
    // as an encyclopedia snippet-only source.
    let hits = vec![WebSearchHit {
        url: "https://en.wikipedia.org/wiki/Rust_(programming_language)".into(),
        title: "Rust (programming language)".into(),
        snippet: "Rust programming language".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "langsearch, wikipedia".into(),
        author: None,
    }];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://en.wikipedia.org/wiki/Rust_(programming_language)".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://en.wikipedia.org/wiki/Rust_(programming_language)".into(),
            title: "Rust (programming language)".into(),
            body: Arc::from(body256("full page body for the shared Wikipedia URL")),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    let (g, _, fetch) = gatherer_with(hits, pages, Vec::new());
    let sources = g.gather("Rust programming language", 5).await.unwrap();
    assert_eq!(sources.len(), 1, "shared-engine hit should be captured");
    match &sources[0] {
        Source::Web { page_type, .. } => {
            assert_ne!(
                page_type.as_deref(),
                Some("encyclopedia"),
                "shared-engine URL must not be treated as encyclopedia"
            );
        }
        other => panic!("expected Source::Web, got {other:?}"),
    }
    let calls = fetch.calls.lock().unwrap();
    assert!(
        calls.contains(&"https://en.wikipedia.org/wiki/Rust_(programming_language)".to_string()),
        "shared-engine URL must be fetched normally, got {calls:?}"
    );
}

#[tokio::test]
async fn gather_keep_low_relevance_disables_prefilter() {
    let hits = vec![WebSearchHit {
        url: "https://bad.example".into(),
        title: "completely unrelated page".into(),
        snippet: "buy shoes".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "test".into(),
        author: None,
    }];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://bad.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://bad.example".into(),
            title: "completely unrelated page".into(),
            body: Arc::from(body256("body")),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    let (g, _, _) = gatherer_with(hits, pages, Vec::new());
    let g = g.with_keep_low_relevance(true);
    let sources = g.gather("Rust async runtime", 5).await.unwrap();
    assert_eq!(sources.len(), 1, "--use-low-relevance keeps the hit");
}

#[tokio::test]
async fn gather_caps_huge_body_at_max_source_body_bytes() {
    use crate::document::MAX_SOURCE_BODY_BYTES;
    let hits = vec![WebSearchHit {
        url: "https://huge.example".into(),
        title: "Huge page".into(),
        snippet: "Rust async runtime".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "test".into(),
        author: None,
    }];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://huge.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://huge.example".into(),
            title: "Huge page".into(),
            body: Arc::from("x".repeat(MAX_SOURCE_BODY_BYTES + 1024)),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    let (g, _, _) = gatherer_with(hits, pages, Vec::new());
    let sources = g.gather("Rust async runtime", 5).await.unwrap();
    assert_eq!(sources.len(), 1);
    if let Source::Web { body, .. } = &sources[0] {
        assert!(
            body.len() <= MAX_SOURCE_BODY_BYTES + 128,
            "body should be capped near MAX_SOURCE_BODY_BYTES, got {} bytes",
            body.len()
        );
        assert!(body.contains("truncated"));
    }
}

#[tokio::test]
async fn gather_times_out_slow_fetch() {
    struct SlowFetch;
    #[async_trait]
    impl WebFetchTool for SlowFetch {
        async fn fetch(&self, _url: &str) -> anyhow::Result<WebFetchedPage> {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            Ok(WebFetchedPage {
                published_at: None,
                url: _url.to_string(),
                title: "slow".into(),
                body: "slow body".into(),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
    }
    let hits = vec![WebSearchHit {
        url: "https://slow.example".into(),
        title: "Slow".into(),
        snippet: "Rust async runtime".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "test".into(),
        author: None,
    }];
    let g = WebGatherer::new(
        Arc::new(FakeSearch {
            hits,
            calls: Mutex::new(Vec::new()),
        }),
        Arc::new(SlowFetch),
    )
    .with_fetch_timeout(std::time::Duration::from_millis(100));
    #[derive(Default)]
    struct CollectEvents(Mutex<Vec<GatherEvent>>);
    impl GatherObserver for CollectEvents {
        fn on_event(&self, event: GatherEvent) {
            self.0.lock().unwrap().push(event);
        }
    }
    let obs = CollectEvents::default();
    let result = g
        .gather_with_observer("Rust async runtime", 5, Some(&obs))
        .await
        .unwrap();
    assert!(result.sources.is_empty(), "slow fetch should time out");
    let events = obs.0.lock().unwrap();
    assert!(
        events.iter().any(|e| matches!(
            e,
            GatherEvent::FetchFailed { url, error } if url == "https://slow.example" && error.contains("timed out")
        )),
        "expected timeout FetchFailed event, got {events:?}"
    );
}

#[tokio::test]
async fn gather_preserves_search_ranking_order_with_prefilter_gap() {
    // Three hits: the middle one is low relevance and should be dropped.
    // The remaining two must still be numbered web-01 and web-02 in
    // search-ranking order.
    let hits = vec![
        WebSearchHit {
            url: "https://first.example".into(),
            title: "First Rust async page".into(),
            snippet: "Rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
            author: None,
        },
        WebSearchHit {
            url: "https://low.example".into(),
            title: "unrelated shopping".into(),
            snippet: "buy shoes".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
            author: None,
        },
        WebSearchHit {
            url: "https://second.example".into(),
            title: "Second Rust async page".into(),
            snippet: "Rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
            author: None,
        },
    ];
    let mut pages = std::collections::HashMap::new();
    for url in ["https://first.example", "https://second.example"] {
        pages.insert(
            url.into(),
            WebFetchedPage {
                published_at: None,
                url: url.into(),
                title: format!("Title {url}"),
                body: Arc::from(body256("body")),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            },
        );
    }
    let (g, _, _) = gatherer_with(hits, pages, Vec::new());
    let sources = g.gather("Rust async runtime", 5).await.unwrap();
    assert_eq!(sources.len(), 2);
    assert_eq!(
        sources[0].body_path(),
        Some(PathBuf::from("sources/web-01.md").as_path())
    );
    assert_eq!(
        sources[1].body_path(),
        Some(PathBuf::from("sources/web-02.md").as_path())
    );
    if let Source::Web { url, .. } = &sources[0] {
        assert_eq!(url, "https://first.example");
    }
    if let Source::Web { url, .. } = &sources[1] {
        assert_eq!(url, "https://second.example");
    }
}

#[tokio::test]
async fn gather_returns_empty_vec_when_search_returns_no_hits() {
    let (g, _, _) = gatherer_with(Vec::new(), std::collections::HashMap::new(), Vec::new());
    let sources = g.gather("rust async", 5).await.unwrap();
    assert!(sources.is_empty());
}

#[tokio::test]
async fn gather_returns_empty_vec_when_search_tool_errors() {
    struct AlwaysFailSearch;
    #[async_trait]
    impl WebSearchTool for AlwaysFailSearch {
        async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
            anyhow::bail!("network down")
        }
    }
    struct OkFetch;
    #[async_trait]
    impl WebFetchTool for OkFetch {
        async fn fetch(&self, _: &str) -> anyhow::Result<WebFetchedPage> {
            Ok(WebFetchedPage {
                published_at: None,
                url: "u".into(),
                title: "t".into(),
                body: Arc::from(body256("b")),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
    }
    let g = WebGatherer::new(Arc::new(AlwaysFailSearch), Arc::new(OkFetch));
    let sources = g.gather("topic", 5).await.unwrap();
    assert!(
        sources.is_empty(),
        "search failure must not surface as an error"
    );
}

#[tokio::test]
async fn gather_creates_web_source_per_hit_with_sequential_body_paths() {
    let hits = vec![
        WebSearchHit {
            url: "https://a.example".into(),
            title: "A".into(),
            snippet: "topic Rust async Tokio runtime".into(),
            matched_query: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
            author: None,
        },
        WebSearchHit {
            url: "https://b.example".into(),
            title: "B".into(),
            snippet: "topic Rust async Tokio runtime".into(),
            matched_query: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
            author: None,
        },
        WebSearchHit {
            url: "https://c.example".into(),
            title: "C".into(),
            snippet: "topic Rust async Tokio runtime".into(),
            matched_query: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
            author: None,
        },
    ];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://a.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://a.example".into(),
            title: "A - resolved".into(),
            body: Arc::from(body256("body a")),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    pages.insert(
        "https://b.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://b.example".into(),
            title: "B - resolved".into(),
            body: Arc::from(body256("body b")),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    pages.insert(
        "https://c.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://c.example".into(),
            title: String::new(), // empty title should fall back to search hit title
            body: Arc::from(body256("body c")),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    let (g, _, _) = gatherer_with(hits, pages, Vec::new());
    let sources = g.gather("topic", 5).await.unwrap();
    assert_eq!(sources.len(), 3);
    for (i, src) in sources.iter().enumerate() {
        let Source::Web {
            published_at: None,
            url,
            title,
            body_path,
            ..
        } = src
        else {
            panic!("expected Source::Web, got {src:?}");
        };
        assert_eq!(
            body_path.as_path(),
            PathBuf::from(format!("sources/web-{:02}.md", i + 1)).as_path()
        );
        assert!(!url.is_empty());
        assert!(!title.is_empty());
    }
    // The third source had an empty page title, so it should have
    // fallen back to the search-hit title "C".
    if let Source::Web { title, .. } = &sources[2] {
        assert_eq!(title, "C");
    }
}
#[tokio::test]
async fn gather_skips_individual_fetch_failures() {
    let hits = vec![
        WebSearchHit {
            url: "https://ok".into(),
            title: "OK".into(),
            snippet: "topic Rust async Tokio runtime".into(),
            matched_query: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
            author: None,
        },
        WebSearchHit {
            url: "https://bad".into(),
            title: "Bad".into(),
            snippet: "topic Rust async Tokio runtime".into(),
            matched_query: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
            author: None,
        },
    ];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://ok".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://ok".into(),
            title: "OK".into(),
            body: Arc::from(body256("b")),

            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    let (g, _, _) = gatherer_with(hits, pages, vec!["https://bad".into()]);
    let sources = g.gather("topic", 5).await.unwrap();
    assert_eq!(
        sources.len(),
        1,
        "failed fetch should be skipped, not abort"
    );
    if let Source::Web { url, .. } = &sources[0] {
        assert_eq!(url, "https://ok");
    }
}

#[tokio::test]
async fn gather_suppresses_failed_youtube_fetch_with_reason() {
    // A YouTube hit whose fetch adapter errors out (e.g. transcript
    // extraction failed because no caption tracks are available) must not
    // produce a source: it is suppressed with the adapter's error message
    // surfaced in the FetchFailed event, the video never enters the
    // research corpus, and `youtube_count` stays at zero.
    let hits = vec![WebSearchHit {
        url: "https://www.youtube.com/watch?v=abc".into(),
        title: "Some Video".into(),
        snippet: "topic Rust async Tokio runtime".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "test".into(),
        author: None,
    }];
    // No page is registered, so the URL must be in `fail_urls`; the real
    // adapter bails instead of returning a placeholder body.
    let (g, _, _) = gatherer_with(
        hits,
        std::collections::HashMap::new(),
        vec!["https://www.youtube.com/watch?v=abc".into()],
    );

    #[derive(Default)]
    struct CollectEvents(std::sync::Mutex<Vec<GatherEvent>>);
    impl GatherObserver for CollectEvents {
        fn on_event(&self, event: GatherEvent) {
            self.0.lock().unwrap().push(event);
        }
    }
    let obs = CollectEvents::default();
    g.gather_with_observer("topic", 5, Some(&obs))
        .await
        .unwrap();

    let events = obs.0.lock().unwrap();
    assert!(
        events.iter().any(|e| matches!(
            e,
            GatherEvent::FetchFailed { url, error }
                if url == "https://www.youtube.com/watch?v=abc"
                    && error.contains("simulated fetch failure")
        )),
        "expected FetchFailed with the adapter error, got {events:?}"
    );
    assert!(
        !events.iter().any(|e| matches!(
            e,
            GatherEvent::SourceCaptured { url, .. }
                if url == "https://www.youtube.com/watch?v=abc"
        )),
        "failed youtube fetch must not be captured as a source, got {events:?}"
    );
}
/// On a capped run the fetch budget stops at `max_results` for a single
/// query: the third candidate is capped (not fetched).
#[tokio::test]
async fn gather_respects_max_results() {
    let hits = vec![
        WebSearchHit {
            url: "https://1".into(),
            title: "1".into(),
            snippet: "topic Rust async Tokio runtime".into(),
            matched_query: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
            author: None,
        },
        WebSearchHit {
            url: "https://2".into(),
            title: "2".into(),
            snippet: "topic Rust async Tokio runtime".into(),
            matched_query: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
            author: None,
        },
        WebSearchHit {
            url: "https://3".into(),
            title: "3".into(),
            snippet: "topic Rust async Tokio runtime".into(),
            matched_query: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
            author: None,
        },
    ];
    let mut pages = std::collections::HashMap::new();
    for u in ["https://1", "https://2", "https://3"] {
        pages.insert(
            u.into(),
            WebFetchedPage {
                published_at: None,
                url: u.into(),
                title: u.into(),
                body: Arc::from(body256("b")),

                content_type: None,
                page_type: None,
                language: None,
                author: None,
            },
        );
    }
    let (g, _, _) = gatherer_with(hits, pages, Vec::new());
    let g = g.with_volume_cap(Some(2));
    let sources = g.gather("topic", 2).await.unwrap();
    assert_eq!(sources.len(), 2, "capped run must not exceed max_results");
}
#[tokio::test]
async fn gather_rejects_zero_max_results() {
    let (g, _, _) = gatherer_with(Vec::new(), std::collections::HashMap::new(), Vec::new());
    let err = g.gather("topic", 0).await.unwrap_err();
    assert!(matches!(err, WebGatherError::ZeroLimit));
}

#[tokio::test]
async fn gather_rejects_empty_topic() {
    let (g, _, _) = gatherer_with(Vec::new(), std::collections::HashMap::new(), Vec::new());
    let err = g.gather(" ", 5).await.unwrap_err();
    assert!(matches!(err, WebGatherError::EmptyTopic));
}

#[tokio::test]
async fn gather_records_search_call() {
    let (g, search, _) = gatherer_with(Vec::new(), std::collections::HashMap::new(), Vec::new());
    let _ = g.gather("rust async", 5).await.unwrap();
    let calls = search.calls.lock().unwrap();
    assert_eq!(calls.as_slice(), &["rust async".to_string()]);
}
#[test]
fn web_body_path_zero_pads_and_uses_one_based_index() {
    assert_eq!(web_body_path(0), PathBuf::from("sources/web-01.md"));
    assert_eq!(web_body_path(8), PathBuf::from("sources/web-09.md"));
    assert_eq!(web_body_path(9), PathBuf::from("sources/web-10.md"));
}

#[tokio::test]
async fn gather_with_observer_emits_search_failed_on_search_error() {
    struct FailSearch;
    #[async_trait]
    impl WebSearchTool for FailSearch {
        async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
            anyhow::bail!("api key missing")
        }
    }
    struct OkFetch;
    #[async_trait]
    impl WebFetchTool for OkFetch {
        async fn fetch(&self, _: &str) -> anyhow::Result<WebFetchedPage> {
            Ok(WebFetchedPage {
                published_at: None,
                url: "u".into(),
                title: "t".into(),
                body: Arc::from(body256("b")),

                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
    }
    #[derive(Default)]
    struct CollectEvents(Mutex<Vec<GatherEvent>>);
    impl GatherObserver for CollectEvents {
        fn on_event(&self, event: GatherEvent) {
            self.0.lock().unwrap().push(event);
        }
    }
    let g = WebGatherer::new(Arc::new(FailSearch), Arc::new(OkFetch)).with_search_max_retries(0);
    let obs = CollectEvents::default();
    let result = g
        .gather_with_observer("topic", 5, Some(&obs))
        .await
        .unwrap();
    assert!(result.sources.is_empty());
    assert_eq!(result.queries, vec!["topic".to_string()]);
    let events = obs.0.lock().unwrap();
    assert_eq!(events.len(), 3);
    assert!(
        matches!(&events[0], GatherEvent::QueriesDecomposed { queries } if queries == &["topic".to_string()])
    );
    assert!(
        matches!(&events[1],
            GatherEvent::SearchFailed { error } if error.contains("api key missing")
        ),
        "got {:?}",
        events[1]
    );
}
#[tokio::test]
async fn gather_with_observer_emits_no_hits_when_search_is_empty() {
    #[derive(Default)]
    struct CollectEvents(Mutex<Vec<GatherEvent>>);
    impl GatherObserver for CollectEvents {
        fn on_event(&self, event: GatherEvent) {
            self.0.lock().unwrap().push(event);
        }
    }
    let (g, _, _) = gatherer_with(Vec::new(), std::collections::HashMap::new(), Vec::new());
    let obs = CollectEvents::default();
    let result = g
        .gather_with_observer("rust async", 5, Some(&obs))
        .await
        .unwrap();
    assert!(result.sources.is_empty());
    assert_eq!(result.queries, vec!["rust async".to_string()]);
    let events = obs.0.lock().unwrap();
    assert_eq!(events.len(), 3);
    assert!(
        matches!(&events[0], GatherEvent::QueriesDecomposed { queries } if queries == &["rust async".to_string()])
    );
    assert!(matches!(events[1], GatherEvent::SearchReturnedNoHits));
}
#[tokio::test]
async fn gather_with_observer_emits_fetch_failed_for_each_bad_url() {
    let hits = vec![
        WebSearchHit {
            url: "https://ok".into(),
            title: "OK".into(),
            snippet: "topic Rust async Tokio runtime".into(),
            matched_query: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
            author: None,
        },
        WebSearchHit {
            url: "https://bad".into(),
            title: "Bad".into(),
            snippet: "topic Rust async Tokio runtime".into(),
            matched_query: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
            author: None,
        },
    ];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://ok".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://ok".into(),
            title: "OK".into(),
            body: Arc::from(body256("b")),

            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    let (g, _, _) = gatherer_with(hits, pages, vec!["https://bad".into()]);
    #[derive(Default)]
    struct CollectEvents(Mutex<Vec<GatherEvent>>);
    impl GatherObserver for CollectEvents {
        fn on_event(&self, event: GatherEvent) {
            self.0.lock().unwrap().push(event);
        }
    }
    let obs = CollectEvents::default();
    let result = g
        .gather_with_observer("topic", 5, Some(&obs))
        .await
        .unwrap();
    assert_eq!(result.sources.len(), 1);
    let events = obs.0.lock().unwrap();
    assert!(
            events.iter().any(|e| matches!(
                e,
                GatherEvent::FetchFailed { url, error } if url == "https://bad" && error.contains("simulated fetch failure")
            )),
            "got {:?}",
              *events
          );
}

#[tokio::test]
async fn gather_with_decomposer_runs_parallel_sub_queries_and_dedupes() {
    struct RecordingSearch {
        responses: std::collections::HashMap<String, Vec<WebSearchHit>>,
        calls: Mutex<Vec<String>>,
    }
    #[async_trait]
    impl WebSearchTool for RecordingSearch {
        async fn search(
            &self,
            query: &str,
            _max_results: usize,
        ) -> anyhow::Result<Vec<WebSearchHit>> {
            self.calls.lock().unwrap().push(query.to_string());
            Ok(self.responses.get(query).cloned().unwrap_or_default())
        }
    }
    struct OkFetch;
    #[async_trait]
    impl WebFetchTool for OkFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            Ok(WebFetchedPage {
                published_at: None,
                url: url.to_string(),
                title: format!("title-{url}"),
                body: Arc::from(body256(&format!("body-{url}"))),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
    }

    let responses = std::collections::HashMap::from([
        (
            "Rust async".to_string(),
            vec![WebSearchHit {
                url: "https://a.example".into(),
                title: "A".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
                author: None,
            }],
        ),
        (
            "Tokio runtime".to_string(),
            vec![
                WebSearchHit {
                    url: "https://a.example".into(), // duplicate URL
                    title: "A2".into(),
                    snippet: "topic Rust async Tokio runtime".into(),
                    matched_query: String::new(),
                    search_tool: String::new(),
                    search_engine: String::new(),
                    author: None,
                },
                WebSearchHit {
                    url: "https://b.example".into(),
                    title: "B".into(),
                    snippet: "topic Rust async Tokio runtime".into(),
                    matched_query: String::new(),
                    search_tool: String::new(),
                    search_engine: String::new(),
                    author: None,
                },
            ],
        ),
    ]);
    let search = Arc::new(RecordingSearch {
        responses,
        calls: Mutex::new(Vec::new()),
    });
    let gatherer = WebGatherer::new(search.clone(), Arc::new(OkFetch))
        .with_decomposer(Arc::new(HeuristicQueryDecomposer));

    let result = gatherer
        .gather_with_observer("Rust async and Tokio runtime", 5, None)
        .await
        .unwrap();

    // Both sub-queries plus the catch-all full topic were issued.
    let calls = search.calls.lock().unwrap();
    assert!(calls.contains(&"Rust async".to_string()));
    assert!(calls.contains(&"Tokio runtime".to_string()));
    assert!(calls.contains(&"Rust async and Tokio runtime".to_string()));

    // The duplicate https://a.example URL is fetched only once.
    assert_eq!(
        result.sources.len(),
        2,
        "dedup should leave two unique URLs"
    );
    assert_eq!(result.queries.len(), 3);
}

#[tokio::test]
async fn llm_decomposer_parses_json_queries() {
    use ragent_llm::llm::{ChatRequest, LlmClient, StreamEvent};
    use ragent_llm::providers::ProviderRegistry;
    use std::pin::Pin;

    struct JsonReplyClient {
        text: String,
    }

    #[async_trait]
    impl LlmClient for JsonReplyClient {
        async fn chat(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
            let events = vec![
                StreamEvent::TextDelta {
                    text: self.text.clone(),
                },
                StreamEvent::Finish {
                    reason: ragent_llm::llm::LlmFinishReason::Stop,
                },
            ];
            Ok(Box::pin(futures::stream::iter(events)))
        }
    }

    struct JsonProvider;

    #[async_trait]
    impl ragent_llm::provider::Provider for JsonProvider {
        fn id(&self) -> &'static str {
            "json"
        }

        fn name(&self) -> &'static str {
            "JSON"
        }

        fn default_models(&self) -> Vec<ragent_llm::provider::ModelInfo> {
            Vec::new()
        }

        async fn create_client(
            &self,
            _api_key: &str,
            _base_url: Option<&str>,
            _options: &std::collections::HashMap<String, serde_json::Value>,
        ) -> anyhow::Result<Box<dyn LlmClient>> {
            Ok(Box::new(JsonReplyClient {
                        text: r#"{"queries":["Rust async internals","Tokio runtime","Rust async and Tokio runtime"]}"#.into(),
                    }))
        }

        fn set_event_bus(&self, _event_bus: Option<Arc<ragent_types::event::EventBus>>) {}

        fn as_any_static(&self) -> &dyn std::any::Any {
            self
        }
    }

    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(JsonProvider));
    let decomposer = LlmQueryDecomposer::new(Arc::new(registry), "json", "json-model");
    let queries = decomposer
        .decompose("Rust async and Tokio runtime")
        .await
        .unwrap();
    assert_eq!(
        queries,
        vec![
            "Rust async internals".to_string(),
            "Tokio runtime".to_string(),
            "Rust async and Tokio runtime".to_string(),
        ]
    );
}
#[tokio::test]
async fn llm_decomposer_falls_back_to_heuristic_on_bad_json() {
    use ragent_llm::llm::{ChatRequest, LlmClient, StreamEvent};
    use ragent_llm::providers::ProviderRegistry;
    use std::pin::Pin;

    struct BadJsonClient;

    #[async_trait]
    impl LlmClient for BadJsonClient {
        async fn chat(
            &self,
            _request: ChatRequest,
        ) -> anyhow::Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
            let events = vec![
                StreamEvent::TextDelta {
                    text: "not json".into(),
                },
                StreamEvent::Finish {
                    reason: ragent_llm::llm::LlmFinishReason::Stop,
                },
            ];
            Ok(Box::pin(futures::stream::iter(events)))
        }
    }

    struct BadJsonProvider;

    #[async_trait]
    impl ragent_llm::provider::Provider for BadJsonProvider {
        fn id(&self) -> &'static str {
            "badjson"
        }

        fn name(&self) -> &'static str {
            "Bad JSON"
        }

        fn default_models(&self) -> Vec<ragent_llm::provider::ModelInfo> {
            Vec::new()
        }

        async fn create_client(
            &self,
            _api_key: &str,
            _base_url: Option<&str>,
            _options: &std::collections::HashMap<String, serde_json::Value>,
        ) -> anyhow::Result<Box<dyn LlmClient>> {
            Ok(Box::new(BadJsonClient))
        }

        fn set_event_bus(&self, _event_bus: Option<Arc<ragent_types::event::EventBus>>) {}

        fn as_any_static(&self) -> &dyn std::any::Any {
            self
        }
    }

    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(BadJsonProvider));
    let decomposer = LlmQueryDecomposer::new(Arc::new(registry), "badjson", "badjson-model");
    let queries = decomposer
        .decompose("Rust async and Tokio runtime")
        .await
        .unwrap();
    assert!(queries.contains(&"Rust async".to_string()));
    assert!(queries.contains(&"Tokio runtime".to_string()));
    assert!(queries.contains(&"Rust async and Tokio runtime".to_string()));
}

/// A fetch tool that sleeps for a fixed duration before returning, and
/// tracks the maximum number of concurrently in-flight `fetch` calls via
/// an [`AtomicUsize`].
struct ConcurrencyTrackingFetch {
    delay: std::time::Duration,
    in_flight: Arc<std::sync::atomic::AtomicUsize>,
    max_in_flight: Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait]
impl WebFetchTool for ConcurrencyTrackingFetch {
    async fn fetch(&self, _url: &str) -> anyhow::Result<WebFetchedPage> {
        let prev = self
            .in_flight
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // Track the high-water mark of concurrent in-flight fetches.
        let now = prev + 1;
        let mut max = self.max_in_flight.load(std::sync::atomic::Ordering::SeqCst);
        while now > max {
            match self.max_in_flight.compare_exchange(
                max,
                now,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(actual) => max = actual,
            }
        }
        tokio::time::sleep(self.delay).await;
        self.in_flight
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        Ok(WebFetchedPage {
            published_at: None,
            url: _url.to_string(),
            title: format!("title-{_url}"),
            body: Arc::from(body256(&format!("body-{_url}"))),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        })
    }
}

/// `with_fetch_concurrency(0)` is clamped up to `1` so the stream always
/// makes progress; the field reflects the clamped value.
#[test]
fn with_fetch_concurrency_clamps_zero_to_one() {
    let search: Arc<dyn WebSearchTool> = Arc::new(FakeSearch::default());
    let fetch: Arc<dyn WebFetchTool> = Arc::new(ConcurrencyTrackingFetch {
        delay: std::time::Duration::ZERO,
        in_flight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        max_in_flight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });
    let g = WebGatherer::new(search, fetch).with_fetch_concurrency(0);
    assert_eq!(g.fetch_concurrency, 1);
    let g = g.with_fetch_concurrency(7);
    assert_eq!(g.fetch_concurrency, 7);
}

/// The fetch phase of [`WebGatherer::gather_with_observer`] issues up to
/// `fetch_concurrency` page fetches concurrently. With 6 candidate URLs
/// and `fetch_concurrency = 6`, all six fetches should be in flight at
/// once (high-water mark == 6); with `fetch_concurrency = 2` the
/// high-water mark is capped at 2.
#[tokio::test]
async fn gather_fetches_pages_concurrently_up_to_fetch_concurrency() {
    let hits: Vec<WebSearchHit> = (0..6)
        .map(|i| WebSearchHit {
            url: format!("https://h{i}.example"),
            title: format!("H{i}"),
            snippet: "topic Rust async Tokio runtime".into(),
            matched_query: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
            author: None,
        })
        .collect();

    // fetch_concurrency = 6 -> high-water mark should reach 6.
    let in_flight = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let max_in_flight = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let search: Arc<dyn WebSearchTool> = Arc::new(FakeSearch {
        hits: hits.clone(),
        calls: Mutex::new(Vec::new()),
    });
    let fetch: Arc<dyn WebFetchTool> = Arc::new(ConcurrencyTrackingFetch {
        delay: std::time::Duration::from_millis(40),
        in_flight,
        max_in_flight: max_in_flight.clone(),
    });
    let g = WebGatherer::new(search, fetch).with_fetch_concurrency(6);
    let sources = g.gather("topic", 6).await.unwrap();
    assert_eq!(sources.len(), 6, "all six hits should be captured");
    assert_eq!(
        max_in_flight.load(std::sync::atomic::Ordering::SeqCst),
        6,
        "all 6 fetches should have been in flight simultaneously"
    );

    // fetch_concurrency = 2 -> high-water mark should be capped at 2.
    let in_flight2 = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let max_in_flight2 = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let search2: Arc<dyn WebSearchTool> = Arc::new(FakeSearch {
        hits,
        calls: Mutex::new(Vec::new()),
    });
    let fetch2: Arc<dyn WebFetchTool> = Arc::new(ConcurrencyTrackingFetch {
        delay: std::time::Duration::from_millis(40),
        in_flight: in_flight2,
        max_in_flight: max_in_flight2.clone(),
    });
    let g2 = WebGatherer::new(search2, fetch2).with_fetch_concurrency(2);
    let sources2 = g2.gather("topic", 6).await.unwrap();
    assert_eq!(sources2.len(), 6);
    let max2 = max_in_flight2.load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        max2 <= 2,
        "fetch_concurrency=2 should cap in-flight at 2, got {max2}"
    );
    assert_eq!(
        max2, 2,
        "with 6 hits and concurrency 2 the high-water mark should reach 2"
    );
}

/// The default `fetch_concurrency` on a freshly-constructed
/// [`WebGatherer`] is [`DEFAULT_FETCH_CONCURRENCY`].
#[test]
fn default_fetch_concurrency_is_ten() {
    let search: Arc<dyn WebSearchTool> = Arc::new(FakeSearch::default());
    let fetch: Arc<dyn WebFetchTool> = Arc::new(ConcurrencyTrackingFetch {
        delay: std::time::Duration::ZERO,
        in_flight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        max_in_flight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });
    let g = WebGatherer::new(search, fetch);
    assert_eq!(g.fetch_concurrency, DEFAULT_FETCH_CONCURRENCY);
    assert_eq!(DEFAULT_FETCH_CONCURRENCY, 10);
}

/// `fetch_url_as_source` classifies the media type from the fetched page's
/// content type so PDF and YouTube seed URLs are reported correctly.
#[tokio::test]
async fn fetch_url_as_source_classifies_pdf_and_youtube_media_types() {
    struct TypedFetch {
        pages: std::collections::HashMap<String, WebFetchedPage>,
    }
    #[async_trait]
    impl WebFetchTool for TypedFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            self.pages
                .get(url)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no fake page for {url}"))
        }
    }

    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://example.com/paper.pdf".into(),
        WebFetchedPage {
            url: "https://example.com/paper.pdf".into(),
            title: "Paper".into(),
            body: "extracted pdf text".into(),
            content_type: Some("application/pdf".into()),
            page_type: Some("pdf".into()),
            published_at: None,
            language: None,
            author: None,
        },
    );
    pages.insert(
        "https://www.youtube.com/watch?v=abc123".into(),
        WebFetchedPage {
            url: "https://www.youtube.com/watch?v=abc123".into(),
            title: "Video".into(),
            body: "transcript text".into(),
            content_type: Some("text/html; charset=utf-8".into()),
            page_type: Some("youtube".into()),
            published_at: None,
            language: None,
            author: None,
        },
    );

    let g = WebGatherer::new(
        Arc::new(FakeSearch::default()),
        Arc::new(TypedFetch { pages }),
    )
    .with_allow_pdf_web_sources(true);

    let (pdf_source, _) = g
        .fetch_url_as_source("https://example.com/paper.pdf")
        .await
        .unwrap();
    if let Source::Web { media_type, .. } = &pdf_source {
        assert_eq!(media_type, "pdf");
    } else {
        panic!("expected Source::Web for PDF");
    }

    let (yt_source, _) = g
        .fetch_url_as_source("https://www.youtube.com/watch?v=abc123")
        .await
        .unwrap();
    if let Source::Web { media_type, .. } = &yt_source {
        assert_eq!(media_type, "youtube");
    } else {
        panic!("expected Source::Web for YouTube");
    }
}

/// `gather` copies the detected language from the fetched page into the
/// web source so the References Index can render it.
#[tokio::test]
async fn gather_propagates_detected_language_to_source() {
    let hits = vec![WebSearchHit {
        url: "https://fr.example".into(),
        title: "Article".into(),
        snippet: "topic Rust async".into(),
        matched_query: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        author: None,
    }];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://fr.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://fr.example".into(),
            title: "Article".into(),
            body: Arc::from(body256("corps de texte")),
            content_type: None,
            page_type: None,
            language: Some("French".into()),
            author: None,
        },
    );
    let (g, _, _) = gatherer_with(hits, pages, Vec::new());
    let sources = g.gather("topic", 5).await.unwrap();
    assert_eq!(sources.len(), 1);
    if let Source::Web { language, .. } = &sources[0] {
        assert_eq!(language.as_deref(), Some("French"));
    } else {
        panic!("expected Source::Web");
    }
}

/// `fetch_url_as_source` copies the detected language from the fetched page
/// into the returned web source.
#[tokio::test]
async fn fetch_url_as_source_propagates_detected_language() {
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://es.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://es.example".into(),
            title: "Pagina".into(),
            body: Arc::from(body256("cuerpo")),
            content_type: None,
            page_type: None,
            language: Some("Spanish".into()),
            author: None,
        },
    );
    let g = WebGatherer::new(
        Arc::new(FakeSearch::default()),
        Arc::new(FakeFetch {
            pages,
            ..Default::default()
        }),
    );
    let (source, _) = g.fetch_url_as_source("https://es.example").await.unwrap();
    if let Source::Web { language, .. } = &source {
        assert_eq!(language.as_deref(), Some("Spanish"));
    } else {
        panic!("expected Source::Web");
    }
}
#[tokio::test]
async fn gather_counts_pdf_and_youtube_sources() {
    let hits = vec![
        WebSearchHit {
            url: "https://example.com/paper.pdf".into(),
            title: "PDF".into(),
            snippet: "topic Rust async Tokio runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
            author: None,
        },
        WebSearchHit {
            url: "https://www.youtube.com/watch?v=abc123".into(),
            title: "YouTube".into(),
            snippet: "topic Rust async Tokio runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
            author: None,
        },
    ];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://example.com/paper.pdf".into(),
        WebFetchedPage {
            url: "https://example.com/paper.pdf".into(),
            title: "PDF".into(),
            body: Arc::from(body256("pdf body")),
            content_type: Some("application/pdf".into()),
            page_type: Some("pdf".into()),
            published_at: None,
            language: None,
            author: None,
        },
    );
    pages.insert(
        "https://www.youtube.com/watch?v=abc123".into(),
        WebFetchedPage {
            url: "https://www.youtube.com/watch?v=abc123".into(),
            title: "YouTube".into(),
            body: Arc::from(body256("youtube transcript")),
            content_type: Some("text/html".into()),
            page_type: Some("youtube".into()),
            published_at: None,
            language: None,
            author: None,
        },
    );
    let (g, _, _) = gatherer_with(hits, pages, Vec::new());
    let result = g
        .with_allow_pdf_web_sources(true)
        .gather_with_observer("topic", 5, None)
        .await
        .unwrap();
    assert_eq!(result.pdf_count, 1);
    assert_eq!(result.youtube_count, 1);
    assert_eq!(result.sources.len(), 2);
}

// -- Milestone H-002: search retry tests ---------------------------

/// Search tool that fails the first N calls then succeeds.
struct FailNTimes {
    fail_count: std::sync::atomic::AtomicU32,
    n: u32,
    hits: Vec<WebSearchHit>,
}

#[async_trait]
impl WebSearchTool for FailNTimes {
    async fn search(&self, _query: &str, _max: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        let count = self
            .fail_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if count < self.n {
            anyhow::bail!("transient failure #{count}");
        }
        Ok(self.hits.clone())
    }
}

#[tokio::test]
async fn h002_search_retries_then_succeeds() {
    let hits = vec![WebSearchHit {
        url: "https://retry.example".into(),
        title: "Rust async runtime".into(),
        snippet: "Tokio and async Rust".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "test".into(),
        author: None,
    }];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://retry.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://retry.example".into(),
            title: "Rust async runtime".into(),
            body: Arc::from(body256("body")),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    let search = Arc::new(FailNTimes {
        fail_count: std::sync::atomic::AtomicU32::new(0),
        n: 2,
        hits: hits.clone(),
    });
    let fetch = Arc::new(FakeFetch {
        pages,
        fail_urls: Vec::new(),
        calls: Mutex::new(Vec::new()),
        ..Default::default()
    });
    let g = WebGatherer::new(search, fetch)
        .with_search_max_retries(3)
        .with_search_retry_base_delay_ms(0);
    let result = g.gather("Rust async runtime", 5).await.unwrap();
    assert_eq!(result.len(), 1, "search should succeed after retries");
}

#[tokio::test]
async fn h002_search_retries_exhausted_emits_search_failed() {
    struct AlwaysFail;
    #[async_trait]
    impl WebSearchTool for AlwaysFail {
        async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
            anyhow::bail!("persistent failure")
        }
    }
    struct OkFetch;
    #[async_trait]
    impl WebFetchTool for OkFetch {
        async fn fetch(&self, _: &str) -> anyhow::Result<WebFetchedPage> {
            Ok(WebFetchedPage {
                published_at: None,
                url: "u".into(),
                title: "t".into(),
                body: Arc::from(body256("b")),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
    }
    #[derive(Default)]
    struct CollectEvents(Mutex<Vec<GatherEvent>>);
    impl GatherObserver for CollectEvents {
        fn on_event(&self, event: GatherEvent) {
            self.0.lock().unwrap().push(event);
        }
    }
    let g = WebGatherer::new(Arc::new(AlwaysFail), Arc::new(OkFetch))
        .with_search_max_retries(2)
        .with_search_retry_base_delay_ms(0);
    let obs = CollectEvents::default();
    let result = g
        .gather_with_observer("topic", 5, Some(&obs))
        .await
        .unwrap();
    assert!(result.sources.is_empty());
    let events = obs.0.lock().unwrap();
    // Should have: QueriesDecomposed, SearchRetrying x2, SearchFailed.
    let retry_count = events
        .iter()
        .filter(|e| matches!(e, GatherEvent::SearchRetrying { .. }))
        .count();
    assert_eq!(retry_count, 2, "expected 2 retry events");
    assert!(
        events.iter().any(|e| matches!(
            e,
            GatherEvent::SearchFailed { error } if error.contains("persistent failure")
        )),
        "expected SearchFailed event"
    );
}

#[tokio::test]
async fn gather_rejects_pages_below_min_extractable_content_chars() {
    let hits = vec![WebSearchHit {
        url: "https://short.example".into(),
        title: "Short page".into(),
        snippet: "Rust async runtime".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "test".into(),
        author: None,
    }];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://short.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://short.example".into(),
            title: "Short page".into(),
            // 100 chars - below the 256-char minimum.
            body: Arc::from("x".repeat(100)),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    let (g, _, _) = gatherer_with(hits, pages, Vec::new());
    let sources = g.gather("Rust async runtime", 5).await.unwrap();
    assert!(
        sources.is_empty(),
        "page with < MIN_EXTRACTABLE_CONTENT_CHARS should be rejected"
    );
}

#[tokio::test]
async fn gather_accepts_pages_at_min_extractable_content_chars() {
    let hits = vec![WebSearchHit {
        url: "https://exact.example".into(),
        title: "Exact page".into(),
        snippet: "Rust async runtime".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "test".into(),
        author: None,
    }];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://exact.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://exact.example".into(),
            title: "Exact page".into(),
            // Exactly 256 chars - at the minimum.
            body: Arc::from("x".repeat(MIN_EXTRACTABLE_CONTENT_CHARS)),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    let (g, _, _) = gatherer_with(hits, pages, Vec::new());
    let sources = g.gather("Rust async runtime", 5).await.unwrap();
    assert_eq!(
        sources.len(),
        1,
        "page with exactly MIN_EXTRACTABLE_CONTENT_CHARS should be accepted"
    );
}

#[tokio::test]
async fn gather_source_captured_event_carries_body_preview() {
    let hits = vec![WebSearchHit {
        url: "https://preview.example".into(),
        title: "Preview page".into(),
        snippet: "Rust async runtime".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "test".into(),
        author: None,
    }];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://preview.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://preview.example".into(),
            title: "Preview page".into(),
            body: Arc::from("Rust async runtime programming guide with Tokio tasks, futures, channels, and executors. ".repeat(20)),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    let (g, _, _) = gatherer_with(hits, pages, Vec::new());
    #[derive(Default)]
    struct CollectEvents(Mutex<Vec<GatherEvent>>);
    impl GatherObserver for CollectEvents {
        fn on_event(&self, event: GatherEvent) {
            self.0.lock().unwrap().push(event);
        }
    }
    let obs = CollectEvents::default();
    g.gather_with_observer("Rust async runtime", 5, Some(&obs))
        .await
        .unwrap();
    let events = obs.0.lock().unwrap();
    let captured = events.iter().find(|e| {
        matches!(
            e,
            GatherEvent::SourceCaptured { url, .. }
                if url == "https://preview.example"
        )
    });
    assert!(captured.is_some(), "expected SourceCaptured event");
    if let Some(GatherEvent::SourceCaptured {
        body_preview,
        language,
        ..
    }) = captured
    {
        assert_eq!(
            body_preview.chars().count(),
            MIN_EXTRACTABLE_CONTENT_CHARS,
            "body_preview should be exactly MIN_EXTRACTABLE_CONTENT_CHARS chars"
        );
        assert!(
            !body_preview.is_empty(),
            "body_preview should carry content"
        ); // When the fetcher does not report a language, the research layer
        // now runs an aggressive best-guess detector on the body. The
        // fake body is real English prose, so the detector must succeed.
        assert_ne!(
            language, "UNKNOWN",
            "language should be guessed when page.language is None"
        );
    }
}

#[tokio::test]
async fn gather_emits_fetch_failed_for_short_content() {
    let hits = vec![WebSearchHit {
        url: "https://tiny.example".into(),
        title: "Tiny page".into(),
        snippet: "Rust async runtime".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "test".into(),
        author: None,
    }];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://tiny.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://tiny.example".into(),
            title: "Tiny page".into(),
            body: "tiny".into(),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    let (g, _, _) = gatherer_with(hits, pages, Vec::new());
    #[derive(Default)]
    struct CollectEvents(Mutex<Vec<GatherEvent>>);
    impl GatherObserver for CollectEvents {
        fn on_event(&self, event: GatherEvent) {
            self.0.lock().unwrap().push(event);
        }
    }
    let obs = CollectEvents::default();
    let result = g
        .gather_with_observer("Rust async runtime", 5, Some(&obs))
        .await
        .unwrap();
    assert!(result.sources.is_empty());
    assert_eq!(result.excluded_count, 1);
    let events = obs.0.lock().unwrap();
    assert!(
        events.iter().any(|e| matches!(
            e,
            GatherEvent::SourceExcluded { url, reason }
                if url == "https://tiny.example"
                    && reason.contains("too short")
        )),
        "expected SourceExcluded with 'too short' message, got {:?}",
        *events
    );
}

#[tokio::test]
async fn gather_uses_vault_sources_when_available() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vault_root = tmp.path().join("vault");
    let vault = SourceVault::open_with_root(&vault_root, "run-2026-001").unwrap();
    vault
        .store(&NewVaultSource {
            url: "https://vaulted.example/page".into(),
            title: "Vaulted Rust Async Page".into(),
            fetch_timestamp: Some(Utc::now()),
            search_tool: "mf_search".into(),
            search_engine: "langsearch".into(),
            media_type: "page".into(),
            content_type: None,
            body_text: body256("vaulted rust async runtime content"),
            summary_text: None,
        })
        .unwrap();

    // Search tool that would fail if called - proves vault short-circuits
    // the web-search phase.
    struct PanicSearch;
    #[async_trait]
    impl WebSearchTool for PanicSearch {
        async fn search(
            &self,
            _query: &str,
            _max_results: usize,
        ) -> anyhow::Result<Vec<WebSearchHit>> {
            panic!("search should not be called when vault has matches")
        }
    }
    struct PanicFetch;
    #[async_trait]
    impl WebFetchTool for PanicFetch {
        async fn fetch(&self, _url: &str) -> anyhow::Result<WebFetchedPage> {
            panic!("fetch should not be called when vault has matches")
        }
    }

    let g =
        WebGatherer::new(Arc::new(PanicSearch), Arc::new(PanicFetch)).with_vault(Arc::new(vault));
    let result = g
        .gather_with_observer("rust async runtime", 5, None)
        .await
        .unwrap();
    assert_eq!(
        result.sources.len(),
        1,
        "vaulted source should be returned directly"
    );
    if let Source::Web { url, body, .. } = &result.sources[0] {
        assert_eq!(url, "https://vaulted.example/page");
        assert!(
            body.contains("vaulted rust async runtime content"),
            "vault body should be present"
        );
    } else {
        panic!("expected Source::Web")
    }
}

#[tokio::test]
async fn gather_falls_back_to_search_when_vault_has_no_matches() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vault_root = tmp.path().join("vault");
    let vault = SourceVault::open_with_root(&vault_root, "run-2026-empty").unwrap();

    let hits = vec![WebSearchHit {
        url: "https://web.example".into(),
        title: "Web page".into(),
        snippet: "rust async runtime".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "test".into(),
        author: None,
    }];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://web.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://web.example".into(),
            title: "Web page".into(),
            body: Arc::from(body256("fresh web content")),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    let (search, fetch) = (
        Arc::new(FakeSearch {
            hits,
            calls: Mutex::new(Vec::new()),
        }),
        Arc::new(FakeFetch {
            pages,
            fail_urls: Vec::new(),
            calls: Mutex::new(Vec::new()),
            ..Default::default()
        }),
    );
    let g = WebGatherer::new(search.clone(), fetch.clone()).with_vault(Arc::new(vault));
    let result = g.gather("rust async runtime", 5).await.unwrap();
    assert_eq!(
        result.len(),
        1,
        "should fall back to web search when vault has no matches"
    );
    assert_eq!(
        search.calls.lock().unwrap().len(),
        1,
        "search should be called once"
    );
    assert_eq!(
        fetch.calls.lock().unwrap().len(),
        1,
        "fetch should be called once"
    );
}

#[tokio::test]
async fn gather_skips_search_when_vault_has_sufficient_sources() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vault_root = tmp.path().join("vault");
    let vault = SourceVault::open_with_root(&vault_root, "run-2026-sufficient").unwrap();

    for i in 0..3 {
        vault
            .store(&NewVaultSource {
                url: format!("https://vaulted.example/page-{i}"),
                title: format!("Vaulted Rust Async Page {i}"),
                fetch_timestamp: Some(Utc::now()),
                search_tool: "mf_search".into(),
                search_engine: "langsearch".into(),
                media_type: "page".into(),
                content_type: None,
                body_text: body256(&format!("vaulted rust async runtime content {i}")),
                summary_text: None,
            })
            .unwrap();
    }

    struct PanicSearch;
    #[async_trait]
    impl WebSearchTool for PanicSearch {
        async fn search(
            &self,
            _query: &str,
            _max_results: usize,
        ) -> anyhow::Result<Vec<WebSearchHit>> {
            panic!("search should not be called when vault has sufficient sources")
        }
    }
    struct PanicFetch;
    #[async_trait]
    impl WebFetchTool for PanicFetch {
        async fn fetch(&self, _url: &str) -> anyhow::Result<WebFetchedPage> {
            panic!("fetch should not be called when vault has sufficient sources")
        }
    }

    let g = WebGatherer::new(Arc::new(PanicSearch), Arc::new(PanicFetch))
        .with_vault(Arc::new(vault))
        .with_sufficient_sources(3);
    let result = g
        .gather_with_observer("rust async runtime", 5, None)
        .await
        .unwrap();
    assert_eq!(
        result.sources.len(),
        3,
        "all vaulted sources should be returned without new fetches"
    );
}

#[tokio::test]
async fn gather_falls_back_to_search_when_vault_is_below_threshold() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vault_root = tmp.path().join("vault");
    let vault = SourceVault::open_with_root(&vault_root, "run-2026-below").unwrap();
    vault
        .store(&NewVaultSource {
            url: "https://vaulted.example/page".into(),
            title: "Vaulted Rust Async Page".into(),
            fetch_timestamp: Some(Utc::now()),
            search_tool: "mf_search".into(),
            search_engine: "langsearch".into(),
            media_type: "page".into(),
            content_type: None,
            body_text: body256("vaulted rust async runtime content"),
            summary_text: None,
        })
        .unwrap();

    let hits = vec![WebSearchHit {
        url: "https://web.example".into(),
        title: "Web page".into(),
        snippet: "rust async runtime".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "test".into(),
        author: None,
    }];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://web.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://web.example".into(),
            title: "Web page".into(),
            body: Arc::from(body256("fresh web content")),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    let (search, fetch) = (
        Arc::new(FakeSearch {
            hits,
            calls: Mutex::new(Vec::new()),
        }),
        Arc::new(FakeFetch {
            pages,
            fail_urls: Vec::new(),
            calls: Mutex::new(Vec::new()),
            ..Default::default()
        }),
    );
    let g = WebGatherer::new(search.clone(), fetch.clone())
        .with_vault(Arc::new(vault))
        .with_sufficient_sources(3);
    let result = g.gather("rust async runtime", 5).await.unwrap();
    assert_eq!(
        result.len(),
        1,
        "should fall back to web search when vault is below threshold"
    );
    assert_eq!(
        search.calls.lock().unwrap().len(),
        1,
        "search should be called once"
    );
    assert_eq!(
        fetch.calls.lock().unwrap().len(),
        1,
        "fetch should be called once"
    );
}

#[tokio::test]
async fn gather_emits_vault_sufficient_event() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vault_root = tmp.path().join("vault");
    let vault = SourceVault::open_with_root(&vault_root, "run-2026-event").unwrap();

    for i in 0..3 {
        vault
            .store(&NewVaultSource {
                url: format!("https://vaulted.example/page-{i}"),
                title: format!("Vaulted Rust Async Page {i}"),
                fetch_timestamp: Some(Utc::now()),
                search_tool: "mf_search".into(),
                search_engine: "langsearch".into(),
                media_type: "page".into(),
                content_type: None,
                body_text: body256(&format!("vaulted rust async runtime content {i}")),
                summary_text: None,
            })
            .unwrap();
    }

    struct PanicSearch;
    #[async_trait]
    impl WebSearchTool for PanicSearch {
        async fn search(
            &self,
            _query: &str,
            _max_results: usize,
        ) -> anyhow::Result<Vec<WebSearchHit>> {
            panic!("search should not be called when vault is sufficient")
        }
    }
    struct PanicFetch;
    #[async_trait]
    impl WebFetchTool for PanicFetch {
        async fn fetch(&self, _url: &str) -> anyhow::Result<WebFetchedPage> {
            panic!("fetch should not be called when vault is sufficient")
        }
    }

    #[derive(Default)]
    struct CollectEvents(Mutex<Vec<GatherEvent>>);
    impl GatherObserver for CollectEvents {
        fn on_event(&self, event: GatherEvent) {
            self.0.lock().unwrap().push(event);
        }
    }
    let obs = CollectEvents::default();
    let g = WebGatherer::new(Arc::new(PanicSearch), Arc::new(PanicFetch))
        .with_vault(Arc::new(vault))
        .with_sufficient_sources(3);
    g.gather_with_observer("rust async runtime", 5, Some(&obs))
        .await
        .unwrap();

    let events = obs.0.lock().unwrap();
    let sufficient = events
        .iter()
        .find(|e| matches!(e, GatherEvent::VaultSufficient { required: 3, .. }));
    assert!(
        sufficient.is_some(),
        "expected VaultSufficient event, got {events:?}"
    );
}

/// A fake summarizer that returns deterministic text so we can assert the
/// vault stores the summary alongside the original URL and timestamp (T-013).
struct FakeSummarizer;

#[async_trait]
impl crate::page_summarizer::PageSummarizer for FakeSummarizer {
    async fn summarize_page(
        &self,
        url: &str,
        _body: &str,
    ) -> anyhow::Result<crate::page_summarizer::PageSummary> {
        Ok(crate::page_summarizer::PageSummary {
            url: url.to_string(),
            summary: "Summarized.".to_string(),
            summarized_at: chrono::Utc::now(),
        })
    }
}

#[tokio::test]
async fn gather_stores_summarized_source_in_vault_with_url_and_timestamp() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vault_root = tmp.path().join("vault");
    let vault = SourceVault::open_with_root(&vault_root, "run-2026-summary").unwrap();

    let hits = vec![WebSearchHit {
        url: "https://web.example".into(),
        title: "Web page".into(),
        snippet: "rust async runtime".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "test".into(),
        author: None,
    }];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://web.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://web.example".into(),
            title: "Web page".into(),
            body: Arc::from(body256("fresh web content with many details")),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    let (search, fetch) = (
        Arc::new(FakeSearch {
            hits,
            calls: Mutex::new(Vec::new()),
        }),
        Arc::new(FakeFetch {
            pages,
            fail_urls: Vec::new(),
            calls: Mutex::new(Vec::new()),
            ..Default::default()
        }),
    );
    let summarizer: Arc<dyn crate::page_summarizer::PageSummarizer> = Arc::new(FakeSummarizer);
    let g = WebGatherer::new(search, fetch)
        .with_vault(Arc::new(vault.clone()))
        .with_summarizer(summarizer);
    let result = g.gather("rust async runtime", 5).await.unwrap();
    assert_eq!(result.len(), 1, "one source should be captured");
    assert_eq!(
        result[0].path_or_url(),
        "https://web.example",
        "source should carry the original URL"
    );

    let stored = vault.list(5).unwrap();
    assert_eq!(stored.len(), 1, "vault should contain one stored source");
    assert_eq!(stored[0].url, "https://web.example");
    assert!(
        stored[0]
            .body_text
            .contains("fresh web content with many details"),
        "vault should keep the original body text"
    );
    assert_eq!(
        stored[0].summary_text.as_deref(),
        Some("Summarized."),
        "vault should store the generated summary"
    );
    assert!(
        stored[0].fetch_timestamp > chrono::DateTime::UNIX_EPOCH,
        "vault should store a post-epoch fetch timestamp"
    );
}

/// Fake OA client that mimics Unpaywall/Europe PMC responses for a
/// configurable recovered URL.
struct FakeOaClient {
    recovered_url: String,
}

#[async_trait]
impl OpenAccessClient for FakeOaClient {
    async fn fetch_text(&self, _url: &str) -> crate::open_access::Result<String> {
        Ok(String::new())
    }

    async fn fetch_json(&self, url: &str) -> crate::open_access::Result<serde_json::Value> {
        if url.contains("unpaywall.org") {
            Ok(serde_json::json!({
                "is_oa": true,
                "oa_status": "gold",
                "best_oa_location": {
                    "url_for_pdf": self.recovered_url,
                    "url": self.recovered_url,
                    "license": "cc-by"
                }
            }))
        } else {
            Ok(serde_json::json!({ "resultList": { "result": [] } }))
        }
    }
}

#[tokio::test]
async fn gather_recovers_open_access_copy_for_short_scholarly_source() {
    let hits = vec![WebSearchHit {
        url: "https://doi.org/10.1234/example".into(),
        title: "Paywalled Paper".into(),
        snippet: "short abstract".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "langsearch".into(),
        author: None,
    }];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://doi.org/10.1234/example".into(),
        WebFetchedPage {
            url: "https://doi.org/10.1234/example".into(),
            title: "Paywalled Paper".into(),
            body: "short abstract".into(),
            published_at: None,
            content_type: None,
            page_type: Some("scholarly".into()),
            language: None,
            author: None,
        },
    );
    pages.insert(
        "https://oa.example.com/full.pdf".into(),
        WebFetchedPage {
            url: "https://oa.example.com/full.pdf".into(),
            title: "Recovered Full Text".into(),
            body: Arc::from(body256("open access full text content here")),
            published_at: None,
            content_type: Some("application/pdf".into()),
            page_type: None,
            language: Some("English".into()),
            author: None,
        },
    );
    let fetch = Arc::new(FakeFetch {
        pages,
        fail_urls: Vec::new(),
        calls: Mutex::new(Vec::new()),
        ..Default::default()
    });
    let oa_client = Arc::new(FakeOaClient {
        recovered_url: "https://oa.example.com/full.pdf".into(),
    });
    let gatherer = WebGatherer::new(
        Arc::new(FakeSearch {
            hits,
            calls: Mutex::new(Vec::new()),
        }),
        fetch,
    )
    .with_keep_low_relevance(true)
    .with_open_access_recovery(true, Some("oa@example.com".into()))
    .with_oa_min_full_text_chars(500)
    .with_oa_client(oa_client);
    let sources = gatherer.gather("some scholarly topic", 5).await.unwrap();
    assert_eq!(
        sources.len(),
        1,
        "short scholarly source should be recovered"
    );
    let src = &sources[0];
    assert_eq!(src.title(), "Recovered Full Text");
    assert_eq!(src.path_or_url(), "https://doi.org/10.1234/example");
    let recovery = src
        .oa_recovery()
        .expect("OA recovery metadata should be present");
    assert_eq!(recovery.url, "https://oa.example.com/full.pdf");
    assert_eq!(recovery.source.to_string(), "unpaywall");
}

#[tokio::test]
async fn gather_does_not_recover_when_body_is_long_enough() {
    let hits = vec![WebSearchHit {
        url: "https://doi.org/10.1234/example".into(),
        title: "Open Paper".into(),
        snippet: "already full text".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "langsearch".into(),
        author: None,
    }];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://doi.org/10.1234/example".into(),
        WebFetchedPage {
            url: "https://doi.org/10.1234/example".into(),
            title: "Open Paper".into(),
            body: Arc::from(body256("already long full text")),
            published_at: None,
            content_type: None,
            page_type: Some("scholarly".into()),
            language: None,
            author: None,
        },
    );
    let fetch = Arc::new(FakeFetch {
        pages,
        fail_urls: Vec::new(),
        calls: Mutex::new(Vec::new()),
        ..Default::default()
    });
    let oa_client = Arc::new(FakeOaClient {
        recovered_url: "https://oa.example.com/full.pdf".into(),
    });
    let gatherer = WebGatherer::new(
        Arc::new(FakeSearch {
            hits,
            calls: Mutex::new(Vec::new()),
        }),
        fetch,
    )
    .with_keep_low_relevance(true)
    .with_open_access_recovery(true, Some("oa@example.com".into()))
    .with_oa_min_full_text_chars(200)
    .with_oa_client(oa_client);
    let sources = gatherer.gather("some scholarly topic", 5).await.unwrap();
    assert_eq!(sources.len(), 1);
    assert!(
        sources[0].oa_recovery().is_none(),
        "long source should not be recovered"
    );
}

#[tokio::test]
async fn gather_does_not_recover_when_disabled() {
    let hits = vec![WebSearchHit {
        url: "https://doi.org/10.1234/example".into(),
        title: "Paywalled Paper".into(),
        snippet: "short".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "openalex".into(),
        author: None,
    }];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://doi.org/10.1234/example".into(),
        WebFetchedPage {
            url: "https://doi.org/10.1234/example".into(),
            title: "Paywalled Paper".into(),
            body: "short".into(),
            published_at: None,
            content_type: None,
            page_type: Some("scholarly".into()),
            language: None,
            author: None,
        },
    );
    let fetch = Arc::new(FakeFetch {
        pages,
        fail_urls: Vec::new(),
        calls: Mutex::new(Vec::new()),
        ..Default::default()
    });
    let oa_client = Arc::new(FakeOaClient {
        recovered_url: "https://oa.example.com/full.pdf".into(),
    });
    let gatherer = WebGatherer::new(
        Arc::new(FakeSearch {
            hits,
            calls: Mutex::new(Vec::new()),
        }),
        fetch,
    )
    .with_open_access_recovery(false, None)
    .with_oa_client(oa_client);
    let sources = gatherer.gather("some scholarly topic", 5).await.unwrap();
    assert!(
        sources.is_empty() || sources[0].oa_recovery().is_none(),
        "recovery disabled"
    );
}

/// T-006: the width sweep records which mf_search parallel backends
/// contributed hits, and the aggregate `GatherResult` carries the
/// deduplicated engine list plus considered/captured/excluded counts.
#[tokio::test]
async fn width_sweep_tracks_parallel_engines_and_considered_count() {
    let hits = vec![
        WebSearchHit {
            url: "https://langsearch.example".into(),
            title: "LangSearch result".into(),
            snippet: "topic Rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "langsearch".into(),
            author: None,
        },
        WebSearchHit {
            url: "https://tavily.example".into(),
            title: "Tavily result".into(),
            snippet: "topic Rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "tavily".into(),
            author: None,
        },
        WebSearchHit {
            url: "https://wikipedia.example".into(),
            title: "Wikipedia summary".into(),
            snippet: "Rust is a multi-paradigm, general-purpose programming language emphasizing performance and safety, especially safe concurrency.".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "wikipedia".into(),
            author: None,
        },
        WebSearchHit {
            url: "https://openalex.example/paper".into(),
            title: "OpenAlex paper".into(),
            snippet: "We evaluate asynchronous runtimes in Rust across a range of benchmark workloads and report detailed performance comparisons. (Year: 2024 | Cited: 5 | OA: yes)".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "openalex".into(),
            author: None,
        },
    ];
    let mut pages = std::collections::HashMap::new();
    for url in ["https://langsearch.example", "https://tavily.example"] {
        pages.insert(
            url.into(),
            WebFetchedPage {
                published_at: None,
                url: url.into(),
                title: format!("title {url}"),
                body: Arc::from(body256("body")),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            },
        );
    }
    let (g, _, _) = gatherer_with(hits, pages, Vec::new());
    let result = g
        .gather_with_observer("Rust async runtime", 10, None)
        .await
        .unwrap();
    // LangSearch and Tavily pages are fetched; Wikipedia and OpenAlex are
    // captured as snippet-only sources.
    assert_eq!(
        result.sources.len(),
        4,
        "all four backend hits should be captured"
    );
    assert_eq!(
        result.engines,
        vec!["langsearch", "openalex", "tavily", "wikipedia"],
        "engines should be sorted and deduplicated"
    );
    assert_eq!(
        result.considered_count, 4,
        "all four unique URLs were considered"
    );
    assert_eq!(result.excluded_count, 0, "no sources were excluded");
}

/// T-006: when the same URL is returned by multiple parallel backends,
/// the engine list on the source should reflect every contributing
/// engine and the result engines should still be deduplicated.
#[tokio::test]
async fn width_sweep_merges_engines_for_shared_url() {
    let hits = vec![WebSearchHit {
        url: "https://shared.example".into(),
        title: "Shared result".into(),
        snippet: "topic Rust async runtime".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "langsearch, tavily".into(),
        author: None,
    }];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://shared.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://shared.example".into(),
            title: "Shared result".into(),
            body: Arc::from(body256("shared body")),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    let (g, _, _) = gatherer_with(hits, pages, Vec::new());
    let result = g
        .gather_with_observer("Rust async runtime", 5, None)
        .await
        .unwrap();
    assert_eq!(result.sources.len(), 1);
    assert_eq!(result.engines, vec!["langsearch", "tavily"]);
}

/// T-006: the width-sweep summary event is emitted after parallel
/// searches resolve, carrying the contributing engine list and counts.
#[tokio::test]
async fn width_sweep_emits_summary_event() {
    let hits = vec![
        WebSearchHit {
            url: "https://langsearch.example".into(),
            title: "LangSearch result".into(),
            snippet: "topic Rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "langsearch".into(),
            author: None,
        },
        WebSearchHit {
            url: "https://tavily.example".into(),
            title: "Tavily result".into(),
            snippet: "topic Rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "tavily".into(),
            author: None,
        },
    ];
    let mut pages = std::collections::HashMap::new();
    for url in ["https://langsearch.example", "https://tavily.example"] {
        pages.insert(
            url.into(),
            WebFetchedPage {
                published_at: None,
                url: url.into(),
                title: format!("title {url}"),
                body: Arc::from(body256("body")),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            },
        );
    }
    let (g, _, _) = gatherer_with(hits, pages, Vec::new());

    #[derive(Default)]
    struct CollectEvents(Mutex<Vec<GatherEvent>>);
    impl GatherObserver for CollectEvents {
        fn on_event(&self, event: GatherEvent) {
            self.0.lock().unwrap().push(event);
        }
    }
    let obs = CollectEvents::default();
    let result = g
        .gather_with_observer("Rust async runtime", 5, Some(&obs))
        .await
        .unwrap();

    let events = obs.0.lock().unwrap();
    let summary = events.iter().find(|e| {
        matches!(
            e, GatherEvent::WidthSweepSummary { engines, .. } if engines.contains(&"langsearch".to_string())
        )
    });
    assert!(
        summary.is_some(),
        "expected WidthSweepSummary event, got {events:?}"
    );
    if let Some(GatherEvent::WidthSweepSummary {
        queries,
        engines,
        considered,
        captured,
        excluded,
        per_engine,
        capped,
        cancelled,
        ..
    }) = summary
    {
        assert_eq!(queries, &["Rust async runtime".to_string()]);
        assert_eq!(engines, &["langsearch", "tavily"]);
        assert_eq!(*considered, 2);
        assert_eq!(*captured, result.sources.len());
        assert_eq!(*excluded, 0);
        assert_eq!(*capped, 0);
        assert_eq!(*cancelled, 0);
        assert_eq!(
            per_engine
                .iter()
                .map(|s| (s.engine.as_str(), s.considered, s.captured, s.excluded))
                .collect::<Vec<_>>(),
            vec![("langsearch", 1, 1, 0), ("tavily", 1, 1, 0)],
        );
    }
}

/// T-006: the per-engine summary table balances exactly -
/// considered == captured + excluded + capped + cancelled - and credits
/// every engine in a consensus CSV. Covers a PDF exclusion, a pre-filter
/// exclusion, and scholarly/encyclopedia snippet-only captures.
#[tokio::test]
async fn width_sweep_summary_per_engine_accounting_balances() {
    let hits = vec![
        WebSearchHit {
            url: "https://example.com/paper.pdf".into(),
            title: "PDF paper".into(),
            snippet: "topic Rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "langsearch".into(),
            author: None,
        },
        WebSearchHit {
            url: "https://openalex.example/paper".into(),
            title: "OpenAlex paper".into(),
            snippet: "We evaluate asynchronous runtimes in Rust across a range of benchmark workloads and report detailed performance comparisons. (Year: 2024 | Cited: 5 | OA: yes)".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "openalex".into(),
            author: None,
        },
        WebSearchHit {
            url: "https://wikipedia.example".into(),
            title: "Wikipedia summary".into(),
            snippet: "Rust is a multi-paradigm, general-purpose programming language emphasizing performance and safety, especially safe concurrency.".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "wikipedia".into(),
            author: None,
        },
        // Consensus hit credited to both engines in the CSV.
        WebSearchHit {
            url: "https://shared.example".into(),
            title: "Shared result".into(),
            snippet: "topic Rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "langsearch, tavily".into(),
            author: None,
        },
        // Unrelated hit rejected by the title/snippet pre-filter.
        WebSearchHit {
            url: "https://unrelated.example".into(),
            title: "Cooking recipes for the weekend".into(),
            snippet: "delicious pancakes and waffles with maple syrup".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "langsearch".into(),
            author: None,
        },
    ];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://shared.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://shared.example".into(),
            title: "Shared result".into(),
            body: Arc::from(body256("shared body")),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    let (g, _, _) = gatherer_with(hits, pages, Vec::new());

    #[derive(Default)]
    struct CollectEvents(Mutex<Vec<GatherEvent>>);
    impl GatherObserver for CollectEvents {
        fn on_event(&self, event: GatherEvent) {
            self.0.lock().unwrap().push(event);
        }
    }
    let obs = CollectEvents::default();
    let result = g
        .gather_with_observer("Rust async runtime", 10, Some(&obs))
        .await
        .unwrap();

    // PDF excluded, pre-filter excluded, openalex + wikipedia synthesized,
    // shared page fetched: 5 considered = 2 captured + 2 excluded + 0 + 0.
    assert_eq!(result.considered_count, 5);
    assert_eq!(result.sources.len(), 3);
    assert_eq!(result.excluded_count, 2);

    let events = obs.0.lock().unwrap();
    let summary = events.iter().find(
        |e| matches!(e, GatherEvent::WidthSweepSummary { engines, .. } if !engines.is_empty()),
    );
    let Some(GatherEvent::WidthSweepSummary {
        considered,
        captured,
        excluded,
        per_engine,
        excluded_by_reason,
        capped,
        cancelled,
        ..
    }) = summary
    else {
        panic!("expected WidthSweepSummary event, got {events:?}");
    };
    assert_eq!(*considered, 5);
    assert_eq!(*captured, 3);
    assert_eq!(*excluded, 2);
    assert_eq!(*capped, 0);
    assert_eq!(*cancelled, 0);
    assert_eq!(
        considered,
        &(captured + excluded + capped + cancelled),
        "summary must balance"
    );
    let by_engine: std::collections::HashMap<&str, (usize, usize, usize)> = per_engine
        .iter()
        .map(|s| (s.engine.as_str(), (s.considered, s.captured, s.excluded)))
        .collect();
    // langsearch: PDF excluded + unrelated excluded + shared page
    // (captured - the shared URL is a normal page credited to both
    // engines in the consensus CSV).
    assert_eq!(by_engine.get("langsearch"), Some(&(3, 1, 2)));
    // tavily: consensus CSV credit for the shared captured page.
    assert_eq!(by_engine.get("tavily"), Some(&(1, 1, 0)));
    assert_eq!(by_engine.get("openalex"), Some(&(1, 1, 0)));
    // Reason breakdown: one PDF-disabled and one low-relevance exclusion,
    // both credited to langsearch, each also tallied globally.
    let langsearch = per_engine
        .iter()
        .find(|s| s.engine == "langsearch")
        .expect("langsearch row");
    assert_eq!(
        langsearch
            .excluded_by_reason
            .get(&ExclusionReason::PdfDisabled)
            .copied(),
        Some(1)
    );
    assert_eq!(
        langsearch
            .excluded_by_reason
            .get(&ExclusionReason::LowRelevance)
            .copied(),
        Some(1)
    );
    assert_eq!(
        excluded_by_reason
            .get(&ExclusionReason::PdfDisabled)
            .copied(),
        Some(1)
    );
    assert_eq!(
        excluded_by_reason
            .get(&ExclusionReason::LowRelevance)
            .copied(),
        Some(1)
    );
    assert_eq!(
        excluded_by_reason.values().sum::<usize>(),
        *excluded,
        "global reason tally must sum to excluded"
    );
    assert_eq!(by_engine.get("wikipedia"), Some(&(1, 1, 0)));
}

/// T-006: the per-engine summary breaks the `fetch` exclusion count down by
/// fine-grained failure cause. A fetch that returns a typed [`FetchFailure`]
/// lands in its exact kind column; an untyped error falls back to `net`.
#[tokio::test]
async fn width_sweep_summary_breaks_fetch_failures_down_by_cause() {
    let hits = vec![
        WebSearchHit {
            url: "https://blocked.example".into(),
            title: "Blocked".into(),
            snippet: "topic Rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "langsearch".into(),
            author: None,
        },
        WebSearchHit {
            url: "https://plain-error.example".into(),
            title: "Plain error".into(),
            snippet: "topic Rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "langsearch".into(),
            author: None,
        },
    ];
    let mut fetch = FakeFetch {
        fail_kinds: std::iter::once((
            "https://blocked.example".to_string(),
            FetchFailureKind::SecurityBlocked,
        ))
        .collect(),
        ..Default::default()
    };
    // The second URL fails with an untyped error, so it is classified as a
    // generic network failure.
    fetch
        .fail_urls
        .push("https://plain-error.example".to_string());
    let search = Arc::new(FakeSearch {
        hits,
        calls: Mutex::new(Vec::new()),
    });
    let g = WebGatherer::new(search, Arc::new(fetch));

    #[derive(Default)]
    struct CollectEvents(Mutex<Vec<GatherEvent>>);
    impl GatherObserver for CollectEvents {
        fn on_event(&self, event: GatherEvent) {
            self.0.lock().unwrap().push(event);
        }
    }
    let obs = CollectEvents::default();
    let result = g
        .gather_with_observer("Rust async runtime", 10, Some(&obs))
        .await
        .unwrap();
    assert_eq!(result.sources.len(), 0);
    assert_eq!(result.excluded_count, 2);

    let events = obs.0.lock().unwrap();
    let summary = events.iter().find(
        |e| matches!(e, GatherEvent::WidthSweepSummary { engines, .. } if !engines.is_empty()),
    );
    let Some(GatherEvent::WidthSweepSummary {
        per_engine,
        excluded_by_reason,
        failed_by_kind,
        ..
    }) = summary
    else {
        panic!("expected WidthSweepSummary event, got {events:?}");
    };
    // Both failures are counted as `fetch` exclusions.
    assert_eq!(
        excluded_by_reason
            .get(&ExclusionReason::FetchFailed)
            .copied(),
        Some(2)
    );
    assert_eq!(
        failed_by_kind
            .get(&FetchFailureKind::SecurityBlocked)
            .copied(),
        Some(1)
    );
    assert_eq!(
        failed_by_kind.get(&FetchFailureKind::Network).copied(),
        Some(1)
    );
    assert_eq!(
        failed_by_kind.values().sum::<usize>(),
        2,
        "fetch-failure kinds must sum to the global fetch exclusion count"
    );
    let langsearch = per_engine
        .iter()
        .find(|s| s.engine == "langsearch")
        .expect("langsearch row");
    assert_eq!(
        langsearch
            .failed_by_kind
            .get(&FetchFailureKind::SecurityBlocked)
            .copied(),
        Some(1)
    );
    assert_eq!(
        langsearch
            .failed_by_kind
            .get(&FetchFailureKind::Network)
            .copied(),
        Some(1)
    );
    assert_eq!(
        langsearch.failed_by_kind.values().sum::<usize>(),
        langsearch
            .excluded_by_reason
            .get(&ExclusionReason::FetchFailed)
            .copied()
            .unwrap_or(0),
        "per-engine fetch-failure kinds must sum to that engine's fetch count"
    );
}

/// T-006: on a capped (competitive) run, hits beyond the fetch budget
/// are reported as `capped` so the summary still balances instead of
/// silently vanishing.
#[tokio::test]
async fn width_sweep_summary_reports_fetch_capped_hits() {
    let hits = vec![
        WebSearchHit {
            url: "https://openalex.example/paper".into(),
            title: "OpenAlex paper".into(),
            snippet: "We evaluate asynchronous runtimes in Rust across a range of benchmark workloads and report detailed performance comparisons. (Year: 2024 | Cited: 5 | OA: yes)".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "openalex".into(),
            author: None,
        },
        WebSearchHit {
            url: "https://wikipedia.example".into(),
            title: "Wikipedia summary".into(),
            snippet: "Rust is a multi-paradigm, general-purpose programming language emphasizing performance and safety, especially safe concurrency.".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "wikipedia".into(),
            author: None,
        },
        WebSearchHit {
            url: "https://langsearch.example".into(),
            title: "LangSearch result".into(),
            snippet: "topic Rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "langsearch".into(),
            author: None,
        },
    ];
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        "https://langsearch.example".into(),
        WebFetchedPage {
            published_at: None,
            url: "https://langsearch.example".into(),
            title: "title".into(),
            body: Arc::from(body256("body")),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        },
    );
    let (g, _, _) = gatherer_with(hits, pages, Vec::new());
    // Capped (competitive-style) run: budget 2 keeps the two snippet-only
    // hits and caps the third.
    let g = g.with_volume_cap(Some(2));
    let result = g
        .gather_with_observer("Rust async runtime", 2, None)
        .await
        .unwrap();
    // 3 considered = 2 captured + 0 excluded + 1 capped.
    assert_eq!(result.considered_count, 3);
    assert_eq!(result.sources.len(), 2);
    assert_eq!(result.excluded_count, 0);
}

/// Uncapped runs (the default) fetch every retained candidate regardless
/// of the caller's `max_results`; the same run with a volume cap trims to
/// the capped budget. Uses one query so the two policies differ only in
/// the cap.
#[tokio::test]
async fn uncapped_run_fetches_every_retained_candidate() {
    let hits: Vec<WebSearchHit> = (0..3)
        .map(|i| WebSearchHit {
            url: format!("https://{i}.example"),
            title: format!("Rust async runtime result {i}"),
            snippet: "topic Rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "test".into(),
            search_engine: "test".into(),
            author: None,
        })
        .collect();
    let mut pages = std::collections::HashMap::new();
    for i in 0..3 {
        pages.insert(
            format!("https://{i}.example"),
            WebFetchedPage {
                published_at: None,
                url: format!("https://{i}.example"),
                title: format!("title-{i}"),
                body: Arc::from(body256("body")),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            },
        );
    }
    // Uncapped: all 3 candidates are fetched and captured even though
    // `max_results` is 2.
    let (g, _, _) = gatherer_with(hits.clone(), pages.clone(), Vec::new());
    let result = g.gather_with_observer("topic", 2, None).await.unwrap();
    assert_eq!(result.considered_count, 3);
    assert_eq!(result.sources.len(), 3, "uncapped runs fetch everything");

    // Capped: budget 2 keeps 2 candidates and caps the third.
    let (g, _, _) = gatherer_with(hits, pages, Vec::new());
    let g = g.with_volume_cap(Some(2));
    let result = g.gather_with_observer("topic", 2, None).await.unwrap();
    assert_eq!(result.sources.len(), 2, "capped run stops at the budget");
}

/// PDF search hits are skipped by default, so they do not consume the
/// fetch budget or trigger expensive PDF extraction.
#[tokio::test]
async fn gather_skips_pdf_web_sources_by_default() {
    let hits = vec![WebSearchHit {
        url: "https://example.com/paper.pdf".into(),
        title: "PDF".into(),
        snippet: "topic Rust async".into(),
        matched_query: String::new(),
        search_tool: "mf_search".into(),
        search_engine: "test".into(),
        author: None,
    }];
    let (g, _, _) = gatherer_with(hits, std::collections::HashMap::new(), Vec::new());
    let result = g.gather_with_observer("topic", 5, None).await.unwrap();
    assert_eq!(result.pdf_count, 0);
    assert!(result.sources.is_empty());
}

/// On a capped run the volume cap is also the per-query search allowance
/// passed to the search tool, scaled per sub-query for the fetch budget
/// (original T-006 semantics, now restricted to competitive runs).
#[tokio::test]
async fn volume_cap_sets_per_query_search_allowance() {
    #[derive(Default)]
    struct RecordingSearch {
        allowances: Mutex<Vec<usize>>,
    }
    #[async_trait]
    impl WebSearchTool for RecordingSearch {
        async fn search(
            &self,
            _query: &str,
            max_results: usize,
        ) -> anyhow::Result<Vec<WebSearchHit>> {
            self.allowances.lock().unwrap().push(max_results);
            Ok((0..4)
                .map(|i| WebSearchHit {
                    url: format!("https://{i}.example"),
                    title: format!("Rust async runtime result {i}"),
                    snippet: "topic Rust async runtime".into(),
                    matched_query: String::new(),
                    search_tool: "test".into(),
                    search_engine: "test".into(),
                    author: None,
                })
                .collect())
        }
    }
    struct OkFetch;
    #[async_trait]
    impl WebFetchTool for OkFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            Ok(WebFetchedPage {
                published_at: None,
                url: url.to_string(),
                title: format!("title-{url}"),
                body: Arc::from(body256(&format!("body-{url}"))),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
    }
    // Capped run: the search tool sees the cap (2), not the caller's
    // `max_results` (10). 4 hits with a per-query allowance of 2 and a
    // fetch budget of 2 x 1 query = 2 -> 4 considered = 2 captured + 2
    // capped.
    let search = Arc::new(RecordingSearch::default());
    let g = WebGatherer::new(search.clone(), Arc::new(OkFetch)).with_volume_cap(Some(2));
    let result = g.gather_with_observer("topic", 10, None).await.unwrap();
    assert_eq!(
        search.allowances.lock().unwrap().as_slice(),
        &[2],
        "capped run passes the cap as the per-query allowance"
    );
    assert_eq!(result.sources.len(), 2);
    assert_eq!(result.excluded_count, 0);

    // Uncapped run: the search tool sees the large uncapped allowance.
    let search = Arc::new(RecordingSearch::default());
    let g = WebGatherer::new(search.clone(), Arc::new(OkFetch));
    let result = g.gather_with_observer("topic", 10, None).await.unwrap();
    assert_eq!(
        search.allowances.lock().unwrap().as_slice(),
        &[UNCAPTED_MAX_RESULTS],
        "uncapped run asks for engine-max results"
    );
    assert_eq!(result.sources.len(), 4, "uncapped run fetches everything");
}

/// Uncapped multi-query sweeps fetch every retained candidate: 3
/// sub-queries x 2 hits are all captured with no capping, even though the
/// caller's `max_results` is 2.
#[tokio::test]
async fn fetch_budget_scales_with_sub_query_count() {
    struct MultiQuerySearch;
    #[async_trait]
    impl WebSearchTool for MultiQuerySearch {
        async fn search(
            &self,
            query: &str,
            _max_results: usize,
        ) -> anyhow::Result<Vec<WebSearchHit>> {
            // Each sub-query returns 2 unique hits well above any single
            // global cap of 2.
            Ok((0..2)
                .map(|i| WebSearchHit {
                    url: format!("https://{i}.{query}.example"),
                    title: format!("Result {i} for {query}"),
                    snippet: format!("topic about {query} number {i}"),
                    matched_query: String::new(),
                    search_tool: "test".into(),
                    search_engine: "test".into(),
                    author: None,
                })
                .collect())
        }
    }
    struct OkFetch;
    #[async_trait]
    impl WebFetchTool for OkFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            Ok(WebFetchedPage {
                published_at: None,
                url: url.to_string(),
                title: format!("title-{url}"),
                body: Arc::from(body256(&format!("body-{url}"))),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
    }
    struct ThreeQueries;
    #[async_trait]
    impl QueryDecomposer for ThreeQueries {
        async fn decompose(&self, _topic: &str) -> anyhow::Result<Vec<String>> {
            Ok((0..3).map(|i| format!("q{i}")).collect())
        }
    }
    let g = WebGatherer::new(Arc::new(MultiQuerySearch), Arc::new(OkFetch))
        .with_decomposer(Arc::new(ThreeQueries));
    let result = g.gather_with_observer("topic", 2, None).await.unwrap();
    // 3 sub-queries x 2 hits = 6 unique candidates; the run is uncapped,
    // so nothing is capped and everything is captured.
    assert_eq!(result.considered_count, 6);
    assert_eq!(result.sources.len(), 6, "no candidate should be capped");
}

/// A capped multi-query sweep scales the fetch budget by the sub-query
/// count: with cap 2 and 3 sub-queries the budget is 6, so all 6 unique
/// candidates are captured even though the cap is smaller than the
/// candidate total.
#[tokio::test]
async fn volume_cap_scales_fetch_budget_with_sub_query_count() {
    struct MultiQuerySearch;
    #[async_trait]
    impl WebSearchTool for MultiQuerySearch {
        async fn search(
            &self,
            query: &str,
            _max_results: usize,
        ) -> anyhow::Result<Vec<WebSearchHit>> {
            Ok((0..2)
                .map(|i| WebSearchHit {
                    url: format!("https://{i}.{query}.example"),
                    title: format!("Result {i} for {query}"),
                    snippet: format!("topic about {query} number {i}"),
                    matched_query: String::new(),
                    search_tool: "test".into(),
                    search_engine: "test".into(),
                    author: None,
                })
                .collect())
        }
    }
    struct OkFetch;
    #[async_trait]
    impl WebFetchTool for OkFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            Ok(WebFetchedPage {
                published_at: None,
                url: url.to_string(),
                title: format!("title-{url}"),
                body: Arc::from(body256(&format!("body-{url}"))),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
    }
    struct ThreeQueries;
    #[async_trait]
    impl QueryDecomposer for ThreeQueries {
        async fn decompose(&self, _topic: &str) -> anyhow::Result<Vec<String>> {
            Ok((0..3).map(|i| format!("q{i}")).collect())
        }
    }
    let g = WebGatherer::new(Arc::new(MultiQuerySearch), Arc::new(OkFetch))
        .with_decomposer(Arc::new(ThreeQueries))
        .with_volume_cap(Some(2));
    let result = g.gather_with_observer("topic", 2, None).await.unwrap();
    // Budget = cap 2 x 3 sub-queries = 6 = candidate total: no capping.
    assert_eq!(result.considered_count, 6);
    assert_eq!(result.sources.len(), 6, "scaled budget covers the sweep");
}
