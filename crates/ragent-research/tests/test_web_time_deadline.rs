//! Integration tests for the `--web-time` / `web_phase_timeout_secs`
//! web-gathering phase deadline (Milestone H-001 extension).
//!
//! When the phase deadline elapses, the gatherer must return everything
//! captured so far as a partial [`GatherResult`] so the session proceeds to
//! analysis/synthesis instead of discarding the phase.

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

/// Fetch that returns instantly for `fast` URLs and sleeps 120 s otherwise.
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
            tokio::time::sleep(std::time::Duration::from_secs(120)).await;
            Ok(WebFetchedPage {
                published_at: None,
                url: url.to_string(),
                title: "Slow page".into(),
                body: "slow body".into(),
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

    // The run must not have waited for the slow fetch (120 s sleep).
    assert!(
        started.elapsed() < std::time::Duration::from_secs(30),
        "run should honour the web phase deadline instead of blocking on slow fetches"
    );

    // Everything captured before the deadline was ingested: the two fast
    // pages, not the slow one.
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
        2,
        "expected the two fast sources, got {web_urls:?}"
    );
    assert!(
        !web_urls.iter().any(|u| u.contains("slow")),
        "the slow source must not have been captured: {web_urls:?}"
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

    // T-004 / FR-002 / FR-003 / FR-005 / NFR-002: the partial corpus must
    // survive all the way to a written RESEARCH.md with citations to only the
    // captured sources.
    let research_md = research_root.join("web-deadline-partial/RESEARCH.md");
    assert!(
        research_md.is_file(),
        "RESEARCH.md must be written after a truncated web phase"
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
        body.contains("[#1]"),
        "RESEARCH.md should contain a citation marker for the partial corpus; got:\n{body}"
    );
    assert!(
        !body.contains("slow.example"),
        "RESEARCH.md must not cite the source that missed the deadline; got:\n{body}"
    );
    assert!(
        body.contains("## References Index"),
        "RESEARCH.md should include the References Index; got:\n{body}"
    );
}

#[tokio::test]
async fn test_default_web_phase_timeout_is_60_seconds() {
    assert_eq!(
        DEFAULT_WEB_PHASE_TIMEOUT_SECS, 60,
        "the web phase deadline default should be 60 seconds"
    );
    let cfg = SessionConfig::default();
    assert_eq!(cfg.web.web_phase_timeout_secs, Some(60));
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
/// The iterative engine path must honour the same web-phase deadline as the
/// overlapped single-pass path (FR-006): every iteration's web-gathering
/// phase is bounded by the configured timeout, and a truncated gather still
/// yields the sources captured so far so the iteration completes.
#[tokio::test]
async fn test_iterative_engine_respects_web_phase_deadline() {
    use ragent_research::{
        EngineConfig, HeuristicPlanner, IterativeEngine, NoopAnalysisEngine, SimpleCritic,
    };

    let web = WebGatherer::new(
        Arc::new(EchoSearch {
            urls: vec![
                "https://fast-one.example".to_string(),
                "https://fast-two.example".to_string(),
                "https://slow.example".to_string(),
            ],
        }),
        Arc::new(MixedFetch {
            fast: vec![
                "https://fast-one.example".to_string(),
                "https://fast-two.example".to_string(),
            ],
        }),
    );

    let engine = IterativeEngine::new(
        Arc::new(HeuristicPlanner::new()),
        Some(web),
        Arc::new(NoopAnalysisEngine),
        Arc::new(SimpleCritic),
        EngineConfig {
            max_iterations: 2,
            max_sources_per_question: 2,
            max_concurrency: 2,
            force_deeper: false,
        },
    )
    .with_phase_deadline(Some(std::time::Duration::from_secs(1)));

    let observer = Arc::new(CaptureEvents::default());
    let started = Instant::now();
    let state = engine
        .run("Rust async runtime", observer.clone())
        .await
        .expect("iterative run should complete despite the deadline");

    // Without the per-iteration deadline the slow fetch (120 s sleep) would
    // stall the web gathering; the deadline must bound it well below that.
    assert!(
        started.elapsed() < std::time::Duration::from_secs(60),
        "iterative web gathering should be bounded by the phase deadline, took {:?}",
        started.elapsed()
    );

    // Partial sources captured before the deadline were kept for synthesis.
    let fast_count = state
        .sources
        .iter()
        .filter(|s| matches!(s, ragent_research::Source::Web { url, .. } if url.contains("fast")))
        .count();
    assert!(
        fast_count >= 2,
        "fast sources captured before the deadline must be ingested, got {fast_count} of {:?}",
        state.sources
    );
    assert!(
        !state
            .sources
            .iter()
            .any(|s| matches!(s, ragent_research::Source::Web { url, .. }
                if url.contains("slow"))),
        "the slow source must not have been captured: {:?}",
        state.sources
    );

    // FR-009 on the iterative path: the engine forwarder emits a
    // `web_phase_start` RunStep carrying the effective deadline. With the 1s
    // budget and fast-only fetches, all sub-questions complete before the
    // deadline expires, so this test verifies the deadline is *attached* and
    // the phase-start event is forwarded; it does not necessarily truncate.
    let events = observer.0.lock().unwrap();
    let starts = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                SessionEvent::RunStep { step, .. } if step == "web_phase_start"
            )
        })
        .count();
    assert!(
        starts >= 1,
        "expected a web_phase_start RunStep event from the engine forwarder, got {events:?}"
    );
    for e in events.iter().filter(|e| {
        matches!(
            e,
            SessionEvent::RunStep { step, .. } if step == "web_phase_start"
        )
    }) {
        if let SessionEvent::RunStep {
            detail: Some(d), ..
        } = e
        {
            assert_eq!(d, "web phase deadline: 1s", "engine forwarder detail");
        }
    }

    // FR-006 / FR-008: the run completed well below the per-fetch sleep time,
    // proving the per-iteration deadline is active even if fast fetches beat it.
    // A separate test exercises the truncation path directly via
    // `gather_with_observer`.
}

/// A search tool that lexically matches whatever query it is given, so the
/// heuristic planner's rephrased sub-questions do not trip the title/snippet
/// relevance pre-filter.
struct EchoSearch {
    urls: Vec<String>,
}

#[async_trait::async_trait]
impl WebSearchTool for EchoSearch {
    async fn search(&self, query: &str, max_results: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        // Embed the query into title and snippet so every term matches.
        Ok(self
            .urls
            .iter()
            .take(max_results)
            .map(|url| WebSearchHit {
                url: url.clone(),
                title: format!("{query} - {url}"),
                snippet: query.to_string(),
                matched_query: query.to_string(),
                search_tool: "test".into(),
                search_engine: "test".into(),
                author: None,
            })
            .collect())
    }
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

/// FR-008 helper: a fetch tool that records the wall-clock time of every call
/// and sleeps `slow_delay` for non-fast URLs, so tests can assert that no
/// fetch is started after the deadline and bound the phase's overshoot.
struct CountingFetch {
    fast: Vec<String>,
    slow_delay: std::time::Duration,
    call_times: Mutex<Vec<Instant>>,
}

impl CountingFetch {
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
impl WebFetchTool for CountingFetch {
    async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
        self.call_times.lock().unwrap().push(Instant::now());
        if self.fast.iter().any(|u| u == url) {
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
            tokio::time::sleep(self.slow_delay).await;
            Ok(WebFetchedPage {
                published_at: None,
                url: url.to_string(),
                title: "Slow page".into(),
                body: "slow body".into(),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
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
        fast: vec!["https://fast-one.example".to_string()],
        slow_delay: std::time::Duration::from_secs(120),
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

/// FR-008 (in-flight overshoot bound): with the deadline expiring mid-fetch,
/// the phase must return promptly instead of waiting for the slow pages, and
/// must not start any fetch after the deadline.
#[tokio::test]
async fn test_overshoot_bounded_by_in_flight_fetch_timeout() {
    let fast_urls = vec!["https://fast-one.example".to_string()];
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
        fast: fast_urls,
        slow_delay: std::time::Duration::from_secs(120),
        call_times: Mutex::new(Vec::new()),
    });
    // 1 s deadline: generous enough that the instant fast fetch is reliably
    // processed before truncation under parallel test scheduling, while the
    // 120 s slow fetches stay far beyond it.
    let deadline = Instant::now() + std::time::Duration::from_secs(1);
    let web = WebGatherer::new(search.clone(), fetch.clone())
        .with_phase_deadline(Some(deadline))
        .with_fetch_timeout(std::time::Duration::from_secs(2));

    let started = Instant::now();
    let result = web
        .gather_with_observer("Rust async runtime", 5, None)
        .await
        .expect("deadline-truncated gather must return a partial result");
    let elapsed = started.elapsed();

    // The slow fetches sleep for 120 s; the phase deadline (1 s) must bound
    // the whole phase far below that instead of waiting for them.
    assert!(
        elapsed < std::time::Duration::from_secs(30),
        "phase overshoot must be bounded, not wait for in-flight slow fetches, took {elapsed:?}"
    );
    // The fast page was captured before the deadline; the slow ones were not.
    assert_eq!(result.sources.len(), 1, "got {:?}", result.sources);
    // All fetches were started before the deadline (the whole batch is
    // polled at once under the default concurrency); none started after.
    assert_eq!(
        fetch.calls(),
        3,
        "every candidate may be fetched once, got {:?}",
        fetch.call_times.lock().unwrap()
    );
    assert_eq!(
        fetch.calls_after(deadline),
        0,
        "no fetch may start after the deadline"
    );
}
