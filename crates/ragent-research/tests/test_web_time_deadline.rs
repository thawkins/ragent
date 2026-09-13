//! Integration tests for the `--web-time` / `web_phase_timeout_secs`
//! web-gathering phase deadline (Milestone H-001 extension).
//!
//! The deadline bounds the *search stage*: when it elapses, no new searches
//! are issued and the pass proceeds with whatever was found. The fetch stage
//! is never deadline-bounded — fetches are never gated on or cancelled by the
//! deadline; each fetch runs to completion or its own `fetch_timeout`.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use tempfile::TempDir;

use ragent_research::{
    DEFAULT_WEB_PHASE_TIMEOUT_SECS, InputConfig, LocalConfig, ResearchManager, ResearchSession,
    SessionConfig, SessionEvent, SessionObserver, WebConfig, WebFetchTool, WebFetchedPage,
    WebGatherer, WebSearchHit, WebSearchTool,
};

/// Search returning `hits` immediately.
struct FakeSearch {
    hits: Vec<WebSearchHit>,
}

#[async_trait::async_trait]
impl WebSearchTool for FakeSearch {
    async fn search(&self, _query: &str, _max: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        Ok(self.hits.clone())
    }
}

/// Fetch that returns instantly for `fast` URLs and otherwise returns a
/// slow-marker page after a moderate sleep (1.5 s). The sleep is long enough
/// to outlive the test deadlines but short enough to keep the
/// non-deadline-bounded fetch stage fast.
struct MixedFetch {
    fast: Vec<String>,
}

impl MixedFetch {
    fn is_fast(&self, url: &str) -> bool {
        self.fast.iter().any(|u| u == url)
    }
}

#[async_trait::async_trait]
impl WebFetchTool for MixedFetch {
    async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
        if self.is_fast(url) {
            Ok(WebFetchedPage {
                published_at: None,
                url: url.to_string(),
                title: format!("Fast page {url}"),
                body: "Rust async runtime details. ".repeat(30),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        } else {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            Ok(WebFetchedPage {
                published_at: None,
                url: url.to_string(),
                title: "Slow page".into(),
                body: "slow body about Rust async runtime. ".repeat(12),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
    }
}

fn hit(url: &str) -> WebSearchHit {
    WebSearchHit {
        url: url.to_string(),
        title: format!("Rust async {url}"),
        snippet: "Tokio runtime async await".into(),
        matched_query: String::new(),
        search_tool: "test".into(),
        search_engine: "test".into(),
        author: None,
    }
}

/// Records every observed session event.
#[derive(Debug, Default)]
struct CaptureEvents(Mutex<Vec<SessionEvent>>);

impl SessionObserver for CaptureEvents {
    fn on_event(&self, event: SessionEvent) {
        self.0.lock().unwrap().push(event);
    }
}

/// Minimal session config: web-only, no local/spec gathering, no decomposition
/// (no LLM wired), so the fakes fully control timing.
fn web_only_cfg(deadline_secs: Option<u64>, topic: &str) -> SessionConfig {
    SessionConfig {
        input: InputConfig {
            topic: topic.into(),
            ..InputConfig::default()
        },
        web: WebConfig {
            max_web_results: 10,
            fetch_timeout_secs: 150,
            web_phase_timeout_secs: deadline_secs,
            ..WebConfig::default()
        },
        local: LocalConfig {
            disable_local: true,
            disable_specs: true,
            ..LocalConfig::default()
        },
        clarify: false,
        ..SessionConfig::default()
    }
}

#[tokio::test]
async fn test_web_deadline_returns_partial_sources_and_proceeds() {
    let fast_urls = vec![
        "https://fast-one.example".to_string(),
        "https://fast-two.example".to_string(),
    ];
    let web = WebGatherer::new(
        Arc::new(FakeSearch {
            hits: vec![
                hit("https://fast-one.example"),
                hit("https://fast-two.example"),
                hit("https://slow.example"),
            ],
        }),
        Arc::new(MixedFetch { fast: fast_urls }),
    );

    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();
    let manager = ResearchManager::new(&research_root);
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(ragent_research::NoopAnalysisEngine),
    );
    let observer = Arc::new(CaptureEvents::default());

    let started = Instant::now();
    let outcome = session
        .run(
            "web-deadline-partial",
            "Partial",
            &web_only_cfg(Some(1), "Rust async runtime"),
            observer.clone(),
        )
        .await
        .expect("run should complete despite the deadline");

    // The fetch stage is not deadline-bounded, but the "slow" fetch only
    // sleeps 1.5 s, so the whole run must stay well under that order of
    // magnitude (no artificial stall).
    assert!(
        started.elapsed() < std::time::Duration::from_secs(30),
        "run should stay prompt with moderate fetches, took {:?}",
        started.elapsed()
    );

    // Fetches are never cancelled on the phase deadline: every searched
    // candidate is fetched and ingested, fast AND slow.
    let web_urls: Vec<&str> = outcome
        .sources
        .iter()
        .filter_map(|s| match s {
            ragent_research::Source::Web { url, .. } => Some(url.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        web_urls.len(),
        3,
        "all three searched sources must be captured now that fetches are never cancelled, got {web_urls:?}"
    );
    assert!(
        web_urls.iter().any(|u| u.contains("slow")),
        "the slow source must be captured too (its fetch ran to completion): {web_urls:?}"
    );

    // The deadline was surfaced as a `web_deadline` RunStep diagnostic.
    {
        let events = observer.0.lock().unwrap();
        let deadline_events: Vec<_> = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    SessionEvent::RunStep { step, .. } if step == "web_deadline"
                )
            })
            .collect();
        assert_eq!(
            deadline_events.len(),
            1,
            "exactly one web_deadline event must be emitted per gather phase (FR-004), got {deadline_events:?}"
        );
    }

    // The complete corpus (deadline only truncates the *search stage*) must
    // survive all the way to a written RESEARCH.md citing every source.
    let research_md = research_root.join("web-deadline-partial/RESEARCH.md");
    assert!(
        research_md.is_file(),
        "RESEARCH.md must be written after the gather phase"
    );
    let body = tokio::fs::read_to_string(&research_md).await.unwrap();
    assert!(
        body.contains("Fast page https://fast-one.example"),
        "RESEARCH.md should cite the first captured source title; got:\n{body}"
    );
    assert!(
        body.contains("Fast page https://fast-two.example"),
        "RESEARCH.md should cite the second captured source title; got:\n{body}"
    );
    assert!(
        body.contains("slow.example"),
        "RESEARCH.md must cite the slow source too — its fetch was never cancelled; got:\n{body}"
    );
    assert!(
        body.contains("## References Index"),
        "RESEARCH.md should include the References Index; got:\n{body}"
    );
}

#[tokio::test]
async fn test_default_web_phase_timeout_is_180_seconds() {
    assert_eq!(
        DEFAULT_WEB_PHASE_TIMEOUT_SECS, 180,
        "the web phase deadline default should be 180 seconds"
    );
    let cfg = SessionConfig::default();
    assert_eq!(cfg.web.web_phase_timeout_secs, Some(180));
}

#[tokio::test]
async fn test_web_time_zero_disables_the_deadline() {
    // `--web-time 0` maps to `Some(0)`, which disables the deadline: the
    // gather is free to use its per-page fetch timeouts only. With fast
    // fetches the run completes normally and no web_deadline event fires.
    let fast_urls = vec!["https://fast-one.example".to_string()];
    let web = WebGatherer::new(
        Arc::new(FakeSearch {
            hits: vec![hit("https://fast-one.example")],
        }),
        Arc::new(MixedFetch { fast: fast_urls }),
    );

    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();
    let manager = ResearchManager::new(&research_root);
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(ragent_research::NoopAnalysisEngine),
    );
    let observer = Arc::new(CaptureEvents::default());

    let outcome = session
        .run(
            "web-time-zero",
            "Zero",
            &web_only_cfg(Some(0), "Rust async runtime"),
            observer.clone(),
        )
        .await
        .expect("run should complete");
    assert_eq!(
        outcome
            .sources
            .iter()
            .filter(|s| matches!(s, ragent_research::Source::Web { .. }))
            .count(),
        1
    );
    let events = observer.0.lock().unwrap();
    assert!(
        !events.iter().any(|e| matches!(
            e,
            SessionEvent::RunStep { step, .. } if step == "web_deadline"
        )),
        "no web_deadline event should fire when the deadline is disabled"
    );
}

#[test]
fn test_web_time_flag_parses_to_web_phase_timeout() {
    let cmd =
        ragent_research::ResearchCliCommand::parse("create my-topic \"A topic\" --web-time 90");
    match cmd {
        ragent_research::ResearchCliCommand::Create {
            web_phase_timeout_secs,
            ..
        } => {
            assert_eq!(web_phase_timeout_secs, Some(90));
        }
        other => panic!("expected Create, got {other:?}"),
    }

    // The legacy long form still parses.
    let cmd = ragent_research::ResearchCliCommand::parse(
        "create my-topic \"A topic\" --web-phase-timeout-secs 120",
    );
    match cmd {
        ragent_research::ResearchCliCommand::Create {
            web_phase_timeout_secs,
            ..
        } => {
            assert_eq!(web_phase_timeout_secs, Some(120));
        }
        other => panic!("expected Create, got {other:?}"),
    }

    // `--web-time 0` is preserved so the deadline can be disabled.
    let cmd =
        ragent_research::ResearchCliCommand::parse("create my-topic \"A topic\" --web-time 0");
    match cmd {
        ragent_research::ResearchCliCommand::Create {
            web_phase_timeout_secs,
            ..
        } => {
            assert_eq!(web_phase_timeout_secs, Some(0));
        }
        other => panic!("expected Create, got {other:?}"),
    }
}

/// FR-009: when the web-gather phase begins with an active deadline, a
/// `web_phase_start` RunStep notification carrying the effective deadline
/// seconds must reach the session event stream before any other deadline
/// diagnostic fires.
#[tokio::test]
async fn test_phase_start_notification_emitted_once_with_deadline() {
    let web = WebGatherer::new(
        Arc::new(FakeSearch {
            hits: vec![hit("https://fast-one.example")],
        }),
        Arc::new(MixedFetch {
            fast: vec!["https://fast-one.example".to_string()],
        }),
    );

    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();
    let manager = ResearchManager::new(&research_root);
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(ragent_research::NoopAnalysisEngine),
    );
    let observer = Arc::new(CaptureEvents::default());

    session
        .run(
            "web-phase-start",
            "Start",
            &web_only_cfg(Some(45), "Rust async runtime"),
            observer.clone(),
        )
        .await
        .expect("run should complete");

    let events = observer.0.lock().unwrap();
    let starts: Vec<_> = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                SessionEvent::RunStep { step, .. } if step == "web_phase_start"
            )
        })
        .collect();
    assert_eq!(
        starts.len(),
        1,
        "exactly one web_phase_start event expected, got {starts:?}"
    );
    match starts[0] {
        SessionEvent::RunStep {
            step,
            status,
            detail,
        } => {
            assert_eq!(step, "web_phase_start");
            assert_eq!(status, "in_progress");
            let detail = detail.as_deref().unwrap_or_default();
            // The event carries the *remaining* effective deadline at phase
            // start, so a 45s budget observed after session setup may report
            // 44s. Parse and range-check instead of exact-matching.
            let secs: u64 = detail
                .strip_prefix("web phase deadline: ")
                .and_then(|rest| rest.strip_suffix('s'))
                .and_then(|num| num.parse().ok())
                .unwrap_or_else(|| {
                    panic!("phase-start detail must carry deadline seconds, got {detail:?}")
                });
            assert!(
                (1..=45).contains(&secs),
                "effective deadline must be within the configured budget, got {secs}s ({detail:?})"
            );
        }
        other => panic!("expected RunStep, got {other:?}"),
    }
}

/// FR-009: with the deadline disabled (`--web-time 0`), no phase-start
/// notification is emitted — the UI must not render a countdown.
#[tokio::test]
async fn test_phase_start_notification_absent_when_deadline_disabled() {
    let web = WebGatherer::new(
        Arc::new(FakeSearch {
            hits: vec![hit("https://fast-one.example")],
        }),
        Arc::new(MixedFetch {
            fast: vec!["https://fast-one.example".to_string()],
        }),
    );

    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();
    let manager = ResearchManager::new(&research_root);
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(ragent_research::NoopAnalysisEngine),
    );
    let observer = Arc::new(CaptureEvents::default());

    session
        .run(
            "web-phase-start-zero",
            "Zero",
            &web_only_cfg(Some(0), "Rust async runtime"),
            observer.clone(),
        )
        .await
        .expect("run should complete");

    let events = observer.0.lock().unwrap();
    assert!(
        !events.iter().any(|e| matches!(
            e,
            SessionEvent::RunStep { step, .. } if step == "web_phase_start"
        )),
        "no web_phase_start event should fire when the deadline is disabled, got {events:?}"
    );
}

/// FR-008 helper: a search tool that records the wall-clock time of every
/// call so tests can assert that no search was issued after the deadline.
struct CountingSearch {
    hits: Vec<WebSearchHit>,
    delay: std::time::Duration,
    call_times: Mutex<Vec<Instant>>,
}

impl CountingSearch {
    fn calls(&self) -> usize {
        self.call_times.lock().unwrap().len()
    }

    fn calls_after(&self, deadline: Instant) -> usize {
        self.call_times
            .lock()
            .unwrap()
            .iter()
            .filter(|t| **t > deadline)
            .count()
    }
}

#[async_trait::async_trait]
impl WebSearchTool for CountingSearch {
    async fn search(&self, _query: &str, _max: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        self.call_times.lock().unwrap().push(Instant::now());
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        Ok(self.hits.clone())
    }
}

/// FR-008 helper: a decomposer that fans the topic out into `n` sub-queries
/// so the search stage has more work queued than the deadline allows.
struct MultiQueries(usize);

#[async_trait::async_trait]
impl ragent_research::QueryDecomposer for MultiQueries {
    async fn decompose(&self, _topic: &str) -> anyhow::Result<Vec<String>> {
        Ok((0..self.0).map(|i| format!("sub-query {i}")).collect())
    }
}

/// Helper: a fetch tool that records the wall-clock time of every call and
/// completes every page after a configurable delay. The fetch stage is never
/// deadline-bounded, so fetches that start are always allowed to finish.
struct CountingFetch {
    delay: std::time::Duration,
    call_times: Mutex<Vec<Instant>>,
}

impl CountingFetch {
    fn calls(&self) -> usize {
        self.call_times.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
impl WebFetchTool for CountingFetch {
    async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
        self.call_times.lock().unwrap().push(Instant::now());
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        Ok(WebFetchedPage {
            published_at: None,
            url: url.to_string(),
            title: format!("Page {url}"),
            body: "Rust async runtime details. ".repeat(30),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        })
    }
}

/// FR-008 (no new searches after the deadline): the search stage polls at
/// most `buffer_unordered(4)` sub-queries at once. With 8 sub-queries whose
/// searches each take 2 s and a 500 ms deadline, the deadline elapses while
/// the first batch is still in flight, so the loop must break before any of
/// the remaining four searches is started.
#[tokio::test]
async fn test_no_search_issued_after_deadline() {
    let search = Arc::new(CountingSearch {
        hits: vec![hit("https://fast-one.example")],
        delay: std::time::Duration::from_secs(2),
        call_times: Mutex::new(Vec::new()),
    });
    let fetch = Arc::new(CountingFetch {
        delay: std::time::Duration::ZERO,
        call_times: Mutex::new(Vec::new()),
    });
    let deadline = Instant::now() + std::time::Duration::from_millis(500);
    let web = WebGatherer::new(search.clone(), fetch.clone())
        .with_decomposer(Arc::new(MultiQueries(8)))
        .with_phase_deadline(Some(deadline));

    let started = Instant::now();
    let result = web
        .gather_with_observer("Rust async runtime", 8, None)
        .await
        .expect("deadline-truncated gather must return a partial result");
    let elapsed = started.elapsed();

    // The first search batch sleeps 2 s; the deadline (500 ms) must break
    // the loop long before any second batch could start.
    assert!(
        elapsed < std::time::Duration::from_secs(30),
        "the truncated search loop must not wait for in-flight searches, took {elapsed:?}"
    );
    // Only the initial buffer_unordered(4) batch was ever started: every
    // search call happened before the deadline, none after.
    assert_eq!(
        search.calls(),
        4,
        "only the first in-flight batch may be issued, got {} calls at {:?}",
        search.calls(),
        search.call_times.lock().unwrap()
    );
    assert_eq!(
        search.calls_after(deadline),
        0,
        "no search may be issued after the deadline"
    );
    // The truncated search stage yields no hits, so the fetch stage — which
    // only starts from search hits — must never run at all.
    assert_eq!(
        fetch.calls(),
        0,
        "no fetch may be issued when the search stage was truncated before any hit"
    );
    assert!(
        result.sources.is_empty(),
        "no sources can be captured after the deadline: {:?}",
        result.sources
    );
}

/// Fetch-stage deadline neutrality: fetches are never gated on or cancelled
/// by the phase deadline. With the deadline expiring mid-fetch, every
/// candidate produced by the search stage is still started and every
/// in-flight fetch runs to completion (or its own `--fetch-timeout-secs`).
#[tokio::test]
async fn test_fetches_run_to_completion_past_deadline() {
    let search = Arc::new(CountingSearch {
        hits: vec![
            hit("https://fast-one.example"),
            hit("https://slow.example"),
            hit("https://slow-two.example"),
        ],
        delay: std::time::Duration::ZERO,
        call_times: Mutex::new(Vec::new()),
    });
    let fetch = Arc::new(CountingFetch {
        delay: std::time::Duration::from_millis(1500),
        call_times: Mutex::new(Vec::new()),
    });
    // 500 ms deadline: the searches have already resolved, but every fetch
    // (1500 ms each) will still be in flight when the deadline passes.
    let deadline = Instant::now() + std::time::Duration::from_millis(500);
    let web = WebGatherer::new(search.clone(), fetch.clone())
        .with_phase_deadline(Some(deadline))
        .with_fetch_timeout(std::time::Duration::from_secs(10));

    let started = Instant::now();
    let result = web
        .gather_with_observer("Rust async runtime", 5, None)
        .await
        .expect("gather must return every fetched source");
    let elapsed = started.elapsed();

    // The phase now waits for all fetches: 3 x 1500 ms fetches run
    // concurrently under the default concurrency, so the phase takes at
    // least ~1.5 s — well past the 500 ms deadline — and never truncates
    // them.
    assert!(
        elapsed >= std::time::Duration::from_millis(1400),
        "the fetch stage must wait for in-flight fetches past the deadline, took {elapsed:?}"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(30),
        "but must not stall unboundedly, took {elapsed:?}"
    );
    assert_eq!(
        result.sources.len(),
        3,
        "every searched candidate must be captured — none cancelled, got {:?}",
        result.sources
    );
    assert_eq!(
        fetch.calls(),
        3,
        "every candidate may be fetched once, got {:?}",
        fetch.call_times.lock().unwrap()
    );
}
