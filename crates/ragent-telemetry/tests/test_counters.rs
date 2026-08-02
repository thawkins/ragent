//! External tests for `tests` from `crates/ragent-telemetry/src/counters.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_telemetry::counters::{
    AtomicF64, add_sessions_active, current_values, increment_llm_requests, set_llm_duration_last,
    set_rate_limit_requests_pct,
};

#[test]
fn test_atomic_f64_store_load() {
    let a = AtomicF64::default();
    a.store(std::f64::consts::PI);
    assert!((a.load() - std::f64::consts::PI).abs() < 1e-9);
}

#[test]
fn test_atomic_f64_fetch_add() {
    let a = AtomicF64::default();
    a.fetch_add(1.5);
    a.fetch_add(2.5);
    assert!((a.load() - 4.0).abs() < 1e-9);
}

#[test]
fn test_counter_helpers_update_snapshot() {
    increment_llm_requests(2);
    add_sessions_active(1);
    add_sessions_active(-1);
    set_rate_limit_requests_pct(42.0);
    set_llm_duration_last(123.4);

    let values = current_values();
    assert_eq!(values.llm_requests, 2);
    assert_eq!(values.sessions_active, 0);
    assert!((values.rate_limit_requests_pct - 42.0).abs() < 1e-9);
    assert!((values.llm_duration_last - 123.4).abs() < 1e-9);
}
