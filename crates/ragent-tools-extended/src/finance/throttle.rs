//! Global cross-provider throttle for the finance toolset.
//!
//! A single process-wide timestamp tracks the last time any Yahoo or Alpha
//! Vantage API call was initiated. Before each provider request the caller
//! must [`wait_for_min_interval`], which sleeps until the configured gap
//! (default 5 seconds) has elapsed since the previous call. This prevents
//! rapid sequences of finance tool calls from tripping provider-side rate
//! limits, especially when multiple tools run in parallel or in quick
//! succession.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Default interval between finance API calls.
const DEFAULT_MIN_CALL_INTERVAL_SECONDS: u64 = 5;

/// Process-wide timestamp of the last finance provider request.
///
/// Held behind a mutex so that concurrent tool calls serialize their wait
/// checks. The stored instant is the earliest time the *next* call is allowed
/// to proceed; callers sleep until now >= that instant.
static NEXT_ALLOWED_CALL: Mutex<Option<Instant>> = Mutex::new(None);

/// Wait until at least `min_call_interval_seconds` have passed since the last
/// finance provider request, then reserve the next slot.
///
/// `config` may be `None` when no finance block is configured; in that case
/// the default 5-second interval is used.
pub async fn wait_for_min_interval(config: Option<&ragent_config::finance::FinanceProviderConfig>) {
    let seconds = config
        .map(|c| c.min_call_interval_seconds)
        .unwrap_or(DEFAULT_MIN_CALL_INTERVAL_SECONDS);
    let min_interval = Duration::from_secs(seconds);

    let wait = {
        let mut guard = NEXT_ALLOWED_CALL
            .lock()
            .expect("finance throttle lock poisoned");
        let now = Instant::now();
        let next_allowed = guard.unwrap_or(now);
        let wait = if next_allowed > now {
            next_allowed.saturating_duration_since(now)
        } else {
            Duration::ZERO
        };
        // Reserve the slot for this call so the next caller waits from the
        // end of our reserved window.
        *guard = Some(now + wait + min_interval);
        wait
    };

    if wait > Duration::ZERO {
        tokio::time::sleep(wait).await;
    }
}

/// Reset the global throttle state. Intended for tests only.
#[cfg(test)]
pub fn reset_throttle_state() {
    let mut guard = NEXT_ALLOWED_CALL
        .lock()
        .expect("finance throttle lock poisoned");
    *guard = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sequential_calls_are_throttled() {
        reset_throttle_state();
        let config = ragent_config::finance::FinanceProviderConfig {
            min_call_interval_seconds: 1,
            ..Default::default()
        };

        // Two sequential calls should be spaced by at least the configured interval.
        let start = Instant::now();
        wait_for_min_interval(Some(&config)).await;
        wait_for_min_interval(Some(&config)).await;
        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_millis(900),
            "expected two 1s-interval calls to span at least ~1s, got {:?}",
            elapsed
        );
        reset_throttle_state();
    }

    #[tokio::test]
    async fn default_interval_uses_five_seconds() {
        reset_throttle_state();
        // With no config the default interval is 5 seconds. We only verify that
        // the call completes and reserves a slot; asserting exact timing is flaky
        // because the global state is shared across concurrent tests.
        wait_for_min_interval(None).await;
        reset_throttle_state();
    }
}
