//! Inline tests for `model.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::{
    BENCH_TRANSIENT_MAX_BACKOFF_SECS, BENCH_TRANSIENT_MAX_ELAPSED_SECS, benchmark_retry_delay,
    extract_benchmark_error_status_code, format_retry_exhausted_error,
    is_permanent_benchmark_api_error,
};
use std::time::{Duration, Instant};

#[test]
fn test_extract_benchmark_error_status_code_reads_http_style_messages() {
    assert_eq!(
        extract_benchmark_error_status_code("Ollama Cloud API error (503 Service Unavailable)"),
        Some(503)
    );
    assert_eq!(
        extract_benchmark_error_status_code("HTTP 429 Too Many Requests"),
        Some(429)
    );
}

#[test]
fn test_is_permanent_benchmark_api_error_ignores_retryable_statuses() {
    assert!(!is_permanent_benchmark_api_error(
        "OpenAI API error (429 Too Many Requests): rate limited"
    ));
    assert!(!is_permanent_benchmark_api_error(
        "Ollama Cloud API error (503 Service Unavailable): server overloaded"
    ));
}

#[test]
fn test_benchmark_retry_delay_grows_and_caps_for_retryable_errors() {
    let started = Instant::now();
    assert_eq!(
        benchmark_retry_delay("HTTP 503 Service Unavailable", 0, started),
        Some(Duration::from_secs(2))
    );
    assert_eq!(
        benchmark_retry_delay("HTTP 503 Service Unavailable", 3, started),
        Some(Duration::from_secs(16))
    );
    assert_eq!(
        benchmark_retry_delay("HTTP 503 Service Unavailable", 6, started),
        Some(Duration::from_secs(BENCH_TRANSIENT_MAX_BACKOFF_SECS))
    );
}

#[test]
fn test_benchmark_retry_delay_stops_after_budget_is_spent() {
    let started = Instant::now()
        .checked_sub(Duration::from_secs(BENCH_TRANSIENT_MAX_ELAPSED_SECS))
        .unwrap();
    assert_eq!(
        benchmark_retry_delay("HTTP 503 Service Unavailable", 0, started),
        None
    );
    assert_eq!(
        benchmark_retry_delay("HTTP 400 Bad Request", 0, Instant::now()),
        None
    );
}

#[test]
fn test_format_retry_exhausted_error_reports_attempts_and_elapsed_time() {
    let started = Instant::now().checked_sub(Duration::from_secs(9)).unwrap();
    let message = format_retry_exhausted_error("HTTP 503 Service Unavailable", 5, started);
    assert!(message.contains("503"));
    assert!(message.contains("5 attempt(s)"));
    assert!(message.contains("over 9s") || message.contains("over 10s"));
}
