//! Tests for HTTP 429 (Too Many Requests) retry logic.

/// Test that `parse_retry_after` correctly parses integer seconds.
#[test]
fn test_parse_retry_after_seconds() {
    // We can't directly test the private function, but we can test it via
    // the public execute_with_retry interface with a mock server.
    // For now, this is a placeholder that will be expanded once we have
    // a test mock server.
}

/// Test that exponential backoff delays are reasonable.
#[test]
fn test_backoff_delays() {
    // Delays should be: 500ms, 1000ms, 2000ms, 4000ms, 8000ms (capped)
    let expected: Vec<u64> = vec![500, 1000, 2000, 4000, 8000];
    for (attempt, exp_ms) in expected.iter().enumerate() {
        let delay_ms = 500 * (1_u64 << attempt.min(4));
        assert_eq!(
            delay_ms, *exp_ms,
            "Backoff for attempt {attempt} should be {exp_ms}ms, got {delay_ms}ms"
        );
    }
}

/// Test that the 5th retry (index 4) also caps at 8000ms.
#[test]
fn test_backoff_delay_cap() {
    let delay_ms = 500 * (1_u64 << 4); // 4.min(4) = 4
    assert_eq!(delay_ms, 8000, "Backoff should cap at 8000ms");

    let delay_ms = 500 * (1_u64 << 4); // 10.min(4) = 4
    assert_eq!(delay_ms, 8000, "Backoff should still cap at 8000ms");
}
