//! Integration tests for the teammate retry backoff schedule.
//!
//! These tests pin the shape of the exponential-backoff-with-jitter curve
//! used by [`ragent_team::team::manager::teammate_retry_backoff`] so that
//! future refactors don't silently regress to a linear schedule (which was
//! the root cause of synchronised retry storms against cloud providers —
//! see CHANGELOG "Swarm teammate retry backoff").

use std::time::Duration;

use ragent_team::team::manager::teammate_retry_backoff;

const MAX_JITTER_MS: u64 = 500;

#[test]
fn test_teammate_retry_backoff_grows_exponentially() {
    // Sleep just long enough for the monotonic clock to advance between
    // samples so the jitter term varies (the jitter is derived from
    // `Instant::elapsed().as_nanos()`).
    let mut samples: Vec<Duration> = Vec::new();
    for attempt in 1..=4 {
        std::thread::sleep(Duration::from_millis(2));
        samples.push(teammate_retry_backoff(attempt));
    }

    // Strip jitter to assert the base schedule: 1 s, 2 s, 4 s, 8 s.
    let bases: Vec<u64> = samples
        .iter()
        .map(|d| d.as_millis() as u64)
        .map(|ms| (ms / 1_000) * 1_000) // floor to nearest second
        .collect();
    assert_eq!(bases, vec![1_000, 2_000, 4_000, 8_000]);
}

#[test]
fn test_teammate_retry_backoff_jitter_stays_within_bound() {
    for attempt in 1..=4 {
        std::thread::sleep(Duration::from_millis(2));
        let backoff = teammate_retry_backoff(attempt);
        let expected_base = 1_000u64 << (attempt - 1);
        let ms = backoff.as_millis() as u64;
        assert!(
            ms >= expected_base,
            "attempt {attempt}: backoff {ms}ms below base {expected_base}ms"
        );
        assert!(
            ms < expected_base + MAX_JITTER_MS + 50, // small tolerance
            "attempt {attempt}: backoff {ms}ms exceeds base+jitter {expected_base}+{MAX_JITTER_MS}ms"
        );
    }
}

#[test]
fn test_teammate_retry_backoff_caps_at_30_seconds() {
    // Even with an absurd attempt number, the cap holds.
    let backoff = teammate_retry_backoff(60);
    assert!(
        backoff <= Duration::from_secs(30),
        "expected <= 30s, got {backoff:?}"
    );
}

#[test]
fn test_teammate_retry_backoff_attempt_zero_returns_zero() {
    // The retry loop calls us only when `attempt > 0`, but document
    // the behaviour for `attempt == 0` so callers that accidentally pass
    // zero don't get a multi-second sleep they didn't expect.
    let backoff = teammate_retry_backoff(0);
    assert!(backoff < Duration::from_millis(MAX_JITTER_MS + 50));
}
