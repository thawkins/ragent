//! Engine-level resilience tests for the `mf_search` pipeline (T-016).
//!
//! These tests pin the engine-layer behaviour that previously surfaced as
//! "only LangSearch results" in `/websearch search`: transient engine
//! failures (HTTP 429, 5xx, transport timeouts) must be retried at the engine
//! level — not patched per caller — and the orchestrator must stagger engine
//! starts so parallel fan-out does not burst keyless backends past their
//! rate limiters.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use ragent_tools_extended::masterfetch::search::engine::{
    DEFAULT_SEARCH_MAX_RETRIES, DEFAULT_SEARCH_RETRY_DELAY, EngineReport, report_is_transient,
    search_with_retry, strip_disallowed_quotes,
};
use ragent_tools_extended::masterfetch::search::{
    ENGINE_STAGGER, ENGINE_TIMEOUT, RawResult, SearchEngine, SearchOptions, SearchOrchestrator,
};

// ---------------------------------------------------------------------------
// Test engines
// ---------------------------------------------------------------------------

/// Engine that fails transiently `fails_before_ok` times, then succeeds.
struct TransientThenOk {
    calls: Arc<AtomicUsize>,
    fails_before_ok: usize,
}

#[async_trait::async_trait]
impl SearchEngine for TransientThenOk {
    fn name(&self) -> &'static str {
        "transient-then-ok"
    }
    async fn search(&self, _q: &str, _o: &SearchOptions) -> EngineReport {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if n < self.fails_before_ok {
            EngineReport::blocked("transient-then-ok", "rate-limited (429)")
        } else {
            EngineReport::ok(
                "transient-then-ok",
                vec![RawResult::new(
                    "ok",
                    "https://example.com/ok",
                    "snippet",
                    "transient-then-ok",
                )],
            )
        }
    }
}

/// Engine that always fails with a non-transient (quota) block.
struct QuotaBlocked;

#[async_trait::async_trait]
impl SearchEngine for QuotaBlocked {
    fn name(&self) -> &'static str {
        "quota-blocked"
    }
    async fn search(&self, _q: &str, _o: &SearchOptions) -> EngineReport {
        EngineReport::blocked("quota-blocked", "Serper API returned HTTP 403 Forbidden")
    }
}

/// Engine that always fails with a transient HTTP 5xx.
struct TransientFiveXx;

#[async_trait::async_trait]
impl SearchEngine for TransientFiveXx {
    fn name(&self) -> &'static str {
        "five-xx"
    }
    async fn search(&self, _q: &str, _o: &SearchOptions) -> EngineReport {
        EngineReport::blocked(
            "five-xx",
            "Wikipedia API returned HTTP 503 Service Unavailable",
        )
    }
}

/// Engine whose report has an empty error and no results — not transient.
struct EmptyOk;

#[async_trait::async_trait]
impl SearchEngine for EmptyOk {
    fn name(&self) -> &'static str {
        "empty-ok"
    }
    async fn search(&self, _q: &str, _o: &SearchOptions) -> EngineReport {
        EngineReport::ok("empty-ok", Vec::new())
    }
}

/// Engine that records the instant it was first polled.
struct TimedEngine {
    name: &'static str,
    started: Arc<std::sync::Mutex<Vec<(&'static str, Instant)>>>,
}

#[async_trait::async_trait]
impl SearchEngine for TimedEngine {
    fn name(&self) -> &str {
        self.name
    }
    async fn search(&self, _q: &str, _o: &SearchOptions) -> EngineReport {
        self.started
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push((self.name, Instant::now()));
        EngineReport::ok(self.name, Vec::new())
    }
}

// ---------------------------------------------------------------------------
// report_is_transient classification
// ---------------------------------------------------------------------------

#[test]
fn test_report_is_transient_429_and_rate_limit() {
    assert!(report_is_transient(&EngineReport::blocked(
        "x",
        "Serper API returned HTTP 429 Too Many Requests"
    )));
    assert!(report_is_transient(&EngineReport::blocked(
        "x",
        "rate-limited"
    )));
    assert!(report_is_transient(&EngineReport::blocked(
        "x",
        "rate-limited: Insufficient budget. Resets at midnight UTC."
    )));
}

#[test]
fn test_report_is_transient_5xx() {
    assert!(report_is_transient(&EngineReport::blocked(
        "x",
        "Wikipedia API returned HTTP 503 Service Unavailable"
    )));
    assert!(report_is_transient(&EngineReport::blocked(
        "x",
        "OpenAlex API returned HTTP 500 Internal Server Error"
    )));
}

#[test]
fn test_report_is_transient_transport_error_reports() {
    assert!(report_is_transient(&EngineReport::error(
        "x",
        "HTTP request failed: error sending request: operation timed out"
    )));
    assert!(report_is_transient(&EngineReport::error(
        "x",
        "HTTP request failed: connect error"
    )));
}

#[test]
fn test_report_is_transient_quota_is_not_transient() {
    assert!(!report_is_transient(&EngineReport::blocked(
        "x",
        "Exa API returned HTTP 402 Payment Required"
    )));
    assert!(!report_is_transient(&EngineReport::blocked(
        "x",
        "Serper API auth failed: HTTP 403 Forbidden"
    )));
    assert!(!report_is_transient(&EngineReport::blocked(
        "x",
        "Tavily API returned HTTP 432"
    )));
    assert!(!report_is_transient(&EngineReport::blocked(
        "x",
        "missing Serper API key"
    )));
}

#[test]
fn test_report_is_transient_success_or_empty_is_not_transient() {
    assert!(!report_is_transient(&EngineReport::ok("x", Vec::new())));
    assert!(!report_is_transient(&EngineReport::ok(
        "x",
        vec![RawResult::new("t", "https://e.com", "s", "x")]
    )));
}

// ---------------------------------------------------------------------------
// search_with_retry
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_with_retry_recovers_transient_failure() {
    let calls = Arc::new(AtomicUsize::new(0));
    let engine: Arc<dyn SearchEngine> = Arc::new(TransientThenOk {
        calls: Arc::clone(&calls),
        fails_before_ok: 1,
    });
    let opts = SearchOptions::new(5);
    let report = search_with_retry(
        &engine,
        "q",
        &opts,
        DEFAULT_SEARCH_MAX_RETRIES,
        Duration::from_millis(5),
    )
    .await;
    assert!(
        report.has_results(),
        "expected retry to recover: {report:?}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_search_with_retry_does_not_retry_quota_blocks() {
    let engine: Arc<dyn SearchEngine> = Arc::new(QuotaBlocked);
    let t0 = Instant::now();
    let report = search_with_retry(
        &engine,
        "q",
        &SearchOptions::new(5),
        DEFAULT_SEARCH_MAX_RETRIES,
        Duration::from_millis(5),
    )
    .await;
    assert!(!report.has_results());
    assert!(report.engine_blocked);
    // No retry: call returns immediately (well under one retry delay).
    assert!(
        t0.elapsed() < Duration::from_secs(1),
        "quota block must not be retried, took {:?}",
        t0.elapsed()
    );
}

#[tokio::test]
async fn test_search_with_retry_gives_up_after_retries_on_persistent_transient() {
    let calls = Arc::new(AtomicUsize::new(0));
    let engine: Arc<dyn SearchEngine> = Arc::new(TransientThenOk {
        calls: Arc::clone(&calls),
        fails_before_ok: 99,
    });
    let report = search_with_retry(
        &engine,
        "q",
        &SearchOptions::new(5),
        2,
        Duration::from_millis(5),
    )
    .await;
    assert!(!report.has_results());
    assert_eq!(calls.load(Ordering::SeqCst), 3); // 1 initial + 2 retries
}

// ---------------------------------------------------------------------------
// Orchestrator-level: stagger + retry wiring
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_orchestrator_staggers_engine_starts() {
    let started = Arc::new(std::sync::Mutex::new(Vec::new()));
    let engines: Vec<Arc<dyn SearchEngine>> = (0..3)
        .map(|i| {
            let name: &'static str = match i {
                0 => "e0",
                1 => "e1",
                _ => "e2",
            };
            Arc::new(TimedEngine {
                name,
                started: Arc::clone(&started),
            }) as Arc<dyn SearchEngine>
        })
        .collect();
    let orchestrator = SearchOrchestrator::with_engines(engines);
    let _ = orchestrator.search("q", &SearchOptions::new(5)).await;

    let observed = started.lock().unwrap_or_else(|p| p.into_inner());
    assert_eq!(observed.len(), 3);
    let base = observed[0].1;
    let deltas: Vec<Duration> = observed.iter().map(|(_, t)| *t - base).collect();
    // e0 unstaggered; e1/e2 each start at least one ENGINE_STAGGER apart.
    // Allow generous slack for scheduler jitter, assert only lower bound.
    let staggered = deltas
        .iter()
        .filter(|d| **d >= ENGINE_STAGGER.mul_f64(0.9))
        .count();
    assert!(
        staggered >= 1,
        "expected at least one engine start staggered by >= {ENGINE_STAGGER:?}, deltas={deltas:?}"
    );
}

#[tokio::test]
async fn test_orchestrator_recovers_transient_engine_via_retry() {
    // Transient engine fails once on 429; orchestrator must still return its
    // results because the engine-level retry recovered it.
    let orchestrator = SearchOrchestrator::with_engines(vec![Arc::new(TransientThenOk {
        calls: Arc::new(AtomicUsize::new(0)),
        fails_before_ok: 1,
    })]);
    let out = orchestrator.search("q", &SearchOptions::new(5)).await;
    assert_eq!(
        out.merge.total_merged_results, 1,
        "retry should recover the transient engine, blocked={:?}",
        out.merge.blocked_engines
    );
    assert!(out.merge.blocked_engines.is_empty());
}

#[tokio::test]
async fn test_orchestrator_does_not_retry_quota_blocked_engine() {
    let orchestrator = SearchOrchestrator::with_engines(vec![Arc::new(QuotaBlocked)]);
    let t0 = Instant::now();
    let out = orchestrator.search("q", &SearchOptions::new(5)).await;
    assert!(t0.elapsed() < ENGINE_TIMEOUT);
    assert_eq!(out.merge.blocked_engines.len(), 1);
    assert!(out.merge.blocked_engines[0].contains("quota-blocked"));
    assert!(out.merge.blocked_engines[0].contains("403"));
}

#[tokio::test]
async fn test_orchestrator_blocked_engines_carry_reason() {
    let orchestrator = SearchOrchestrator::with_engines(vec![
        Arc::new(QuotaBlocked) as Arc<dyn SearchEngine>,
        Arc::new(TransientFiveXx) as Arc<dyn SearchEngine>,
    ]);
    let out = orchestrator.search("q", &SearchOptions::new(5)).await;
    assert_eq!(out.merge.blocked_engines.len(), 2);
    let joined = out.merge.blocked_engines.join(" | ");
    assert!(joined.contains("403"), "expected 403 in {joined}");
    assert!(joined.contains("503"), "expected 503 in {joined}");
}

#[tokio::test]
async fn test_orchestrator_empty_still_has_no_blocked_engines() {
    let orchestrator = SearchOrchestrator::with_engines(vec![Arc::new(EmptyOk)]);
    let out = orchestrator.search("q", &SearchOptions::new(5)).await;
    assert!(out.merge.blocked_engines.is_empty());
}

// ---------------------------------------------------------------------------
// Constants pinned so regressions are loud
// ---------------------------------------------------------------------------

#[test]
fn test_engine_level_retry_defaults() {
    // T-016: one retry fires into the same rate-limit window the first
    // attempt hit (observed against live Wikipedia); two retries with
    // exponential backoff are the engine-level default.
    assert_eq!(DEFAULT_SEARCH_MAX_RETRIES, 2);
    assert_eq!(DEFAULT_SEARCH_RETRY_DELAY, Duration::from_secs(1));
    assert!(ENGINE_STAGGER > Duration::ZERO);
    assert!(ENGINE_STAGGER < Duration::from_secs(1));
    assert!(ENGINE_TIMEOUT >= Duration::from_secs(25));
}

#[test]
fn test_openalex_daily_quota_429_is_not_retried() {
    // OpenAlex's 429 carries the provider's own "Insufficient budget" daily
    // quota message; retrying it wastes three extra paid calls.
    assert!(!report_is_transient(&EngineReport::blocked(
        "openalex",
        "rate-limited: Insufficient budget. This request costs $0.001 but you only have $0 remaining. Resets at midnight UTC."
    )));
    // …but Wikipedia's plain per-IP limiter IS transient.
    assert!(report_is_transient(&EngineReport::blocked(
        "wikipedia",
        "rate-limited"
    )));
}

// ---------------------------------------------------------------------------
// strip_disallowed_quotes (shared helper)
// ---------------------------------------------------------------------------

#[test]
fn test_strip_disallowed_quotes_removes_all_quote_styles() {
    // Shared helper used by every engine's request builder: ASCII quotes,
    // smart/typographic quotes, and guillemets are all removed; phrase
    // boundaries survive as plain whitespace-separated terms.
    assert_eq!(
        strip_disallowed_quotes("\"agentic loop\" 'architecture' components"),
        "agentic loop architecture components"
    );
}

#[test]
fn test_strip_disallowed_quotes_removes_smart_quotes() {
    assert_eq!(
        strip_disallowed_quotes("\u{201C}agentic loop\u{201D} \u{2018}LLM agents\u{2019}"),
        "agentic loop LLM agents"
    );
}

#[test]
fn test_strip_disallowed_quotes_collapses_whitespace() {
    assert_eq!(strip_disallowed_quotes("  a   \"b\"  "), "a b");
}

#[test]
fn test_strip_disallowed_quotes_unquoted_passthrough() {
    assert_eq!(
        strip_disallowed_quotes("rust async traits"),
        "rust async traits"
    );
}
