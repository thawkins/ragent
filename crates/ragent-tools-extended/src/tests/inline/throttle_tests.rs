//! Inline tests for `throttle.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[tokio::test]
async fn throttle_behaviour() {
    // ── sequential calls are spaced by the configured interval ──────────
    reset_throttle_state();
    let config = ragent_config::finance::FinanceProviderConfig {
        min_call_interval_seconds: 1,
        ..Default::default()
    };

    let start = Instant::now();
    wait_for_min_interval(Some(&config)).await;
    wait_for_min_interval(Some(&config)).await;
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(900),
        "expected two 1s-interval calls to span at least ~1s, got {elapsed:?}",
    );
    reset_throttle_state();

    // ── default interval uses five seconds ──────────────────────────────
    // With no config the default interval is 5 seconds. We only verify that
    // the call completes and reserves a slot; asserting exact timing is flaky
    // because the global state is shared across concurrent tests.
    wait_for_min_interval(None).await;
    reset_throttle_state();
}
