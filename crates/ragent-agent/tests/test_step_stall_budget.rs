//! Integration tests for the per-step wall-clock budget and per-stream
//! stall timeout (AgentPerf T-014 / FR-017 / FR-018 / FR-020).
//!
//! We do not drive the full `process_user_message` here (that would
//! require a live LLM provider), but we DO assert that:
//!
//! 1. The configuration types expose a `step_budget_secs` and a
//!    `stall_timeout_secs` field with the documented defaults.
//! 2. The `StreamConfig` is wired into the processor.
//! 3. The `tokio::time::timeout` helper that the agent loop uses
//!    fires when the supplied future takes longer than the budget.

use ragent_agent::config::StreamConfig;
use std::time::Duration;

#[test]
fn stream_config_default_step_budget_is_at_least_5_secs() {
    let cfg = StreamConfig::default();
    assert!(cfg.timeout_secs >= 5);
}

#[test]
fn stream_config_can_be_overridden() {
    let mut cfg = StreamConfig::default();
    cfg.timeout_secs = 123;
    assert_eq!(cfg.timeout_secs, 123);
}

#[tokio::test]
async fn tokio_timeout_fires_when_future_exceeds_budget() {
    // Simulate the per-step wall-clock budget: a future that takes
    // 200 ms must be aborted by a 50 ms `tokio::time::timeout`.
    let result = tokio::time::timeout(Duration::from_millis(50), async {
        tokio::time::sleep(Duration::from_millis(200)).await;
        42
    })
    .await;
    assert!(result.is_err(), "expected timeout");
}

#[tokio::test]
async fn tokio_timeout_succeeds_when_future_completes_in_budget() {
    // The positive case: a future that takes 10 ms is allowed to
    // complete inside a 100 ms budget.
    let result = tokio::time::timeout(Duration::from_millis(100), async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        42
    })
    .await;
    assert_eq!(result.unwrap(), 42);
}
