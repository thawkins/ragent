//! Tests for the `--age` web-source publication filter
//! ([`ragent_research::WebGatherer::with_max_age`]).
//!
//! A span-bearing gatherer must reject a fetched page whose publication date
//! is older than the span, and must also reject a page that carries no
//! publication date at all. Without a span every dated and undated page is
//! kept, so the filter is strictly opt-in.

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use ragent_research::web_gatherer::GatherObserver;
use ragent_research::{
    AgeSpan, GatherEvent, WebFetchTool, WebFetchedPage, WebGatherer, WebSearchHit, WebSearchTool,
};

/// Topic chosen so the crafted hits clear the gatherer's lexical relevance
/// filter (the snippet repeats the query's terms).
const TOPIC: &str = "rust async runtime";

/// One page the fake fetcher can serve, keyed by URL.
struct PageSpec {
    url: &'static str,
    published_at: Option<DateTime<Utc>>,
}

/// A search double that returns one hit per served page.
struct FixedSearch {
    hits: Vec<WebSearchHit>,
}

#[async_trait]
impl WebSearchTool for FixedSearch {
    async fn search(&self, _query: &str, _max: usize) -> Result<Vec<WebSearchHit>> {
        Ok(self.hits.clone())
    }
}

/// A fetch double that serves a fixed body and per-URL publication date.
struct DatedFetch {
    pages: Vec<PageSpec>,
}

#[async_trait]
impl WebFetchTool for DatedFetch {
    async fn fetch(&self, url: &str) -> Result<WebFetchedPage> {
        let spec = self
            .pages
            .iter()
            .find(|p| p.url == url)
            .ok_or_else(|| anyhow::anyhow!("unexpected fetch of {url}"))?;
        Ok(WebFetchedPage {
            url: spec.url.to_string(),
            title: format!("{TOPIC} page"),
            // Longer than `MIN_EXTRACTABLE_CONTENT_CHARS` (256) so only the
            // age filter can reject it.
            body: Arc::from(
                "Rust async runtime scheduling throughput improvement across engines, \
                 with a reusable executor that reduces contention in production \
                 workloads and keeps tail latency flat under sustained load. The \
                 captured page body is deliberately long enough to clear the \
                 gatherer's minimum-extractable-content guard, so a rejected page \
                 can only have been dropped by the publication-age filter under \
                 test and never for being too short.",
            ),
            published_at: spec.published_at,
            content_type: Some("text/html; charset=utf-8".to_string()),
            page_type: None,
            language: None,
            author: None,
        })
    }

    async fn fetch_with_limit(&self, url: &str, max_bytes: usize) -> Result<WebFetchedPage> {
        let mut page = self.fetch(url).await?;
        if page.body.len() > max_bytes {
            let truncated = ragent_research::truncate_body_to_bytes(page.body.as_ref(), max_bytes);
            page.body = Arc::from(truncated);
        }
        Ok(page)
    }
}

/// Collects the gatherer's exclusion events so a test can assert on the
/// reported reasons.
#[derive(Default)]
struct ExclusionRecorder {
    excluded: std::sync::Mutex<Vec<(String, String)>>,
}

impl ExclusionRecorder {
    fn excluded(&self) -> Vec<(String, String)> {
        self.excluded.lock().map(|v| v.clone()).unwrap_or_default()
    }
}

impl GatherObserver for ExclusionRecorder {
    fn on_event(&self, event: GatherEvent) {
        if let GatherEvent::SourceExcluded { url, reason } = event
            && let Ok(mut guard) = self.excluded.lock()
        {
            guard.push((url, reason));
        }
    }
}

/// Build a hit whose snippet repeats the topic, so the lexical pre-filter
/// keeps it and only the age filter can drop it.
fn hit(url: &'static str) -> WebSearchHit {
    WebSearchHit {
        url: url.to_string(),
        title: format!("{TOPIC} reference"),
        // The snippet carries every query term, so the lexical relevance
        // pre-filter keeps the hit and only the age filter can drop it.
        snippet: format!("{TOPIC}: scheduling throughput improvements for rust async runtime code"),
        matched_query: TOPIC.to_string(),
        search_tool: "mf_search".to_string(),
        search_engine: "langsearch".to_string(),
        author: None,
    }
}

/// Build a gatherer serving `pages`, with an optional publication span.
fn gatherer_for(pages: Vec<PageSpec>, age: Option<&str>) -> WebGatherer {
    let hits = pages.iter().map(|p| hit(p.url)).collect();
    let gatherer = WebGatherer::new(
        Arc::new(FixedSearch { hits }),
        Arc::new(DatedFetch { pages }),
    );
    gatherer.with_max_age(age.map(|a| AgeSpan::parse(a).expect("valid span")))
}

/// URLs of the captured sources, in capture order.
fn captured_urls(sources: &[ragent_research::Source]) -> Vec<String> {
    sources
        .iter()
        .map(|s| s.path_or_url().to_string())
        .collect()
}

#[tokio::test]
async fn without_a_span_recent_old_and_undated_pages_are_all_kept() {
    let now = Utc::now();
    let pages = vec![
        PageSpec {
            url: "https://example.com/recent",
            published_at: Some(now - Duration::days(1)),
        },
        PageSpec {
            url: "https://example.com/old",
            published_at: Some(now - Duration::days(900)),
        },
        PageSpec {
            url: "https://example.com/undated",
            published_at: None,
        },
    ];
    let recorder = ExclusionRecorder::default();
    let sources = gatherer_for(pages, None)
        .gather_with_observer(TOPIC, 10, Some(&recorder))
        .await
        .expect("gather should succeed")
        .sources;

    assert_eq!(sources.len(), 3, "no page is filtered: {sources:?}");
    assert!(
        recorder.excluded().is_empty(),
        "nothing may be excluded without --age: {:?}",
        recorder.excluded()
    );
}

#[tokio::test]
async fn a_span_rejects_pages_older_than_the_span() {
    let now = Utc::now();
    let pages = vec![
        PageSpec {
            url: "https://example.com/recent",
            published_at: Some(now - Duration::days(2)),
        },
        PageSpec {
            url: "https://example.com/old",
            published_at: Some(now - Duration::days(60)),
        },
    ];
    let recorder = ExclusionRecorder::default();
    let sources = gatherer_for(pages, Some("30d"))
        .gather_with_observer(TOPIC, 10, Some(&recorder))
        .await
        .expect("gather should succeed")
        .sources;

    assert_eq!(
        captured_urls(&sources),
        vec!["https://example.com/recent".to_string()],
        "only the page inside the span may be captured"
    );
    let excluded = recorder.excluded();
    assert_eq!(excluded.len(), 1, "got: {excluded:?}");
    assert_eq!(excluded[0].0, "https://example.com/old");
    assert!(
        excluded[0].1.contains("older than --age 30d"),
        "got: {}",
        excluded[0].1
    );
}

#[tokio::test]
async fn a_span_rejects_undated_pages() {
    let pages = vec![PageSpec {
        url: "https://example.com/undated",
        published_at: None,
    }];
    let recorder = ExclusionRecorder::default();
    let sources = gatherer_for(pages, Some("1y"))
        .gather_with_observer(TOPIC, 10, Some(&recorder))
        .await
        .expect("gather should succeed")
        .sources;

    assert!(
        sources.is_empty(),
        "an undated page is rejected while --age is set: {sources:?}"
    );
    let excluded = recorder.excluded();
    assert_eq!(excluded.len(), 1, "got: {excluded:?}");
    assert!(
        excluded[0].1.contains("no publication date"),
        "got: {}",
        excluded[0].1
    );
}

#[tokio::test]
async fn months_and_years_extend_the_window() {
    let now = Utc::now();
    // 100 days old: inside `6m` (180 days) and inside `1y`, outside `3m` (90 days).
    let pages = vec![PageSpec {
        url: "https://example.com/hundred-days",
        published_at: Some(now - Duration::days(100)),
    }];

    let months = gatherer_for(
        vec![PageSpec {
            url: "https://example.com/hundred-days",
            published_at: Some(now - Duration::days(100)),
        }],
        Some("6m"),
    )
    .gather(TOPIC, 10)
    .await
    .expect("gather should succeed")
    .len();
    assert_eq!(months, 1, "a 100-day-old page is inside a 6-month span");

    let short_months = gatherer_for(pages, Some("3m"))
        .gather(TOPIC, 10)
        .await
        .expect("gather should succeed")
        .len();
    assert_eq!(
        short_months, 0,
        "a 100-day-old page is outside a 3-month span"
    );
}

/// A search double that records every query it is asked, so a test can assert
/// on the exact text sent to the engines.
struct RecordingQuerySearch {
    hits: Vec<WebSearchHit>,
    seen: std::sync::Mutex<Vec<String>>,
}

#[async_trait]
impl WebSearchTool for RecordingQuerySearch {
    async fn search(&self, query: &str, _max: usize) -> Result<Vec<WebSearchHit>> {
        if let Ok(mut guard) = self.seen.lock() {
            guard.push(query.to_string());
        }
        Ok(self.hits.clone())
    }
}

impl RecordingQuerySearch {
    fn queries(&self) -> Vec<String> {
        self.seen.lock().map(|v| v.clone()).unwrap_or_default()
    }
}

/// The formatter: the span's start date is appended as an `after:` operator so
/// a search backend reads a publication window rather than a stray date token.
#[test]
fn age_hint_appends_the_window_start_date() {
    let now = DateTime::parse_from_rfc3339("2025-06-15T10:30:00Z")
        .expect("valid instant")
        .with_timezone(&Utc);
    let span = AgeSpan::parse("30d").expect("valid span");

    assert_eq!(
        ragent_research::search_query_age_hint("rust async runtime", span, now),
        "rust async runtime after:2025-05-16"
    );

    // Surrounding whitespace is not carried into the hint, and an identical
    // hint already appended is not duplicated.
    assert_eq!(
        ragent_research::search_query_age_hint("rust async runtime\n", span, now),
        "rust async runtime after:2025-05-16"
    );
    assert_eq!(
        ragent_research::search_query_age_hint("rust async runtime after:2025-05-16", span, now),
        "rust async runtime after:2025-05-16"
    );

    // The operator prefix is part of the hint, so a query that merely ends with
    // the bare date still picks up the operator.
    assert_eq!(
        ragent_research::search_query_age_hint("rust async runtime 2025-05-16", span, now),
        "rust async runtime 2025-05-16 after:2025-05-16"
    );
}

/// `--age` dates *every* decomposed sub-query, not only the first: each query
/// the gatherer sends to the search tool ends with the same `after:` hint.
#[tokio::test]
async fn with_a_span_every_decomposed_query_ends_with_the_iso_date() {
    let now = Utc::now();
    let span = AgeSpan::parse("6m").expect("valid span");
    let expected_hint = format!("after:{}", span.oldest_allowed_date(now));

    let search = Arc::new(RecordingQuerySearch {
        hits: vec![hit("https://example.com/recent")],
        seen: std::sync::Mutex::new(Vec::new()),
    });
    let fetch = Arc::new(DatedFetch {
        pages: vec![PageSpec {
            url: "https://example.com/recent",
            published_at: Some(now - Duration::days(1)),
        }],
    });

    let sources = WebGatherer::new(search.clone(), fetch)
        .with_decomposer(Arc::new(ragent_research::HeuristicQueryDecomposer))
        .with_max_age(Some(span))
        .gather_with_observer("rust async runtime, tokio scheduling", 10, None)
        .await
        .expect("gather should succeed")
        .sources;

    let queries = search.queries();
    assert!(
        queries.len() > 1,
        "the topic decomposes into several queries: {queries:?}"
    );
    for query in &queries {
        assert!(
            query.ends_with(&expected_hint),
            "query {query:?} should end with the window hint {expected_hint}"
        );
    }
    assert_eq!(sources.len(), 1, "the recent page is kept: {sources:?}");
}

/// Without `--age` the queries reach the search tool exactly as decomposed -
/// no date is appended.
#[tokio::test]
async fn without_a_span_decomposed_queries_are_unchanged() {
    let now = Utc::now();
    let search = Arc::new(RecordingQuerySearch {
        hits: vec![hit("https://example.com/recent")],
        seen: std::sync::Mutex::new(Vec::new()),
    });
    let fetch = Arc::new(DatedFetch {
        pages: vec![PageSpec {
            url: "https://example.com/recent",
            published_at: Some(now - Duration::days(1)),
        }],
    });

    WebGatherer::new(search.clone(), fetch)
        .with_decomposer(Arc::new(ragent_research::HeuristicQueryDecomposer))
        .gather_with_observer("rust async runtime, tokio scheduling", 10, None)
        .await
        .expect("gather should succeed");

    let queries = search.queries();
    assert!(!queries.is_empty(), "at least one query is issued");
    for query in &queries {
        assert!(
            !query.contains("20"),
            "no date hint should be appended without --age: {query:?}"
        );
    }
}

/// Records the `QueriesDecomposed` event so a test can assert what the
/// progress panel and `RESEARCH.md` will report.
#[derive(Default)]
struct QueryRecorder {
    queries: std::sync::Mutex<Vec<String>>,
}

impl QueryRecorder {
    fn queries(&self) -> Vec<String> {
        self.queries.lock().map(|v| v.clone()).unwrap_or_default()
    }
}

impl GatherObserver for QueryRecorder {
    fn on_event(&self, event: GatherEvent) {
        if let GatherEvent::QueriesDecomposed { queries } = event
            && let Ok(mut guard) = self.queries.lock()
        {
            guard.extend(queries);
        }
    }
}

/// The reported query list carries the `--age` hint: the progress panel and the
/// generated report show what is actually sent to the search backend, not the
/// bare decomposed text.
#[tokio::test]
async fn a_span_reports_the_hinted_queries_to_the_observer() {
    let now = Utc::now();
    let search = Arc::new(RecordingQuerySearch {
        hits: vec![hit("https://example.com/recent")],
        seen: std::sync::Mutex::new(Vec::new()),
    });
    let fetch = Arc::new(DatedFetch {
        pages: vec![PageSpec {
            url: "https://example.com/recent",
            published_at: Some(now - Duration::days(1)),
        }],
    });
    let observer = QueryRecorder::default();
    let span = AgeSpan::parse("30d").expect("valid span");
    let expected_hint = format!("after:{}", span.oldest_allowed_date(now));

    WebGatherer::new(search.clone(), fetch)
        .with_decomposer(Arc::new(ragent_research::HeuristicQueryDecomposer))
        .with_max_age(Some(span))
        .gather_with_observer("rust async runtime, tokio scheduling", 10, Some(&observer))
        .await
        .expect("gather should succeed");

    let reported = observer.queries();
    let sent = search.queries();
    assert!(!reported.is_empty(), "the observer sees the queries");
    for query in &reported {
        assert!(
            query.ends_with(&expected_hint),
            "reported query {query:?} should end with {expected_hint}"
        );
    }
    assert_eq!(
        reported, sent,
        "the reported list must match the queries sent to the search tool"
    );
}

/// Records the structured date bound the gatherer hands the search tool.
struct DateBoundRecorder {
    hits: Vec<WebSearchHit>,
    seen: std::sync::Mutex<Vec<Option<String>>>,
}

#[async_trait]
impl WebSearchTool for DateBoundRecorder {
    async fn search(&self, _query: &str, _max: usize) -> Result<Vec<WebSearchHit>> {
        Ok(self.hits.clone())
    }

    async fn search_with_date_bound(
        &self,
        _query: &str,
        _max: usize,
        _exclude_engines: &[&str],
        published_after: Option<&str>,
    ) -> Result<Vec<WebSearchHit>> {
        if let Ok(mut guard) = self.seen.lock() {
            guard.push(published_after.map(str::to_string));
        }
        Ok(self.hits.clone())
    }
}

/// With `--age` the window start reaches the search tool as a structured
/// `YYYY-MM-DD` bound - the form every registered engine honours - rather than
/// only as an `after:` token inside the query text.
#[tokio::test]
async fn a_span_hands_the_search_tool_a_structured_date_bound() {
    let now = Utc::now();
    let search = Arc::new(DateBoundRecorder {
        hits: vec![hit("https://example.com/recent")],
        seen: std::sync::Mutex::new(Vec::new()),
    });
    let fetch = Arc::new(DatedFetch {
        pages: vec![PageSpec {
            url: "https://example.com/recent",
            published_at: Some(now - Duration::days(1)),
        }],
    });
    let span = AgeSpan::parse("30d").expect("valid span");
    let expected = span.oldest_allowed_date(now);

    WebGatherer::new(search.clone(), fetch)
        .with_decomposer(Arc::new(ragent_research::HeuristicQueryDecomposer))
        .with_max_age(Some(span))
        .gather("rust async runtime, tokio scheduling", 10)
        .await
        .expect("gather should succeed");

    let seen = search.seen.lock().map(|v| v.clone()).unwrap_or_default();
    assert!(!seen.is_empty(), "at least one search is issued");
    for bound in &seen {
        assert_eq!(
            bound.as_deref(),
            Some(expected.as_str()),
            "every sub-query must carry the window start as a date bound"
        );
    }
}

/// Without `--age` no date bound is passed, so an engine is never constrained.
#[tokio::test]
async fn without_a_span_no_date_bound_is_passed() {
    let now = Utc::now();
    let search = Arc::new(DateBoundRecorder {
        hits: vec![hit("https://example.com/recent")],
        seen: std::sync::Mutex::new(Vec::new()),
    });
    let fetch = Arc::new(DatedFetch {
        pages: vec![PageSpec {
            url: "https://example.com/recent",
            published_at: Some(now - Duration::days(1)),
        }],
    });

    WebGatherer::new(search.clone(), fetch)
        .with_decomposer(Arc::new(ragent_research::HeuristicQueryDecomposer))
        .gather("rust async runtime, tokio scheduling", 10)
        .await
        .expect("gather should succeed");

    let seen = search.seen.lock().map(|v| v.clone()).unwrap_or_default();
    assert!(!seen.is_empty(), "at least one search is issued");
    assert!(
        seen.iter().all(Option::is_none),
        "no date bound without --age: {seen:?}"
    );
}
