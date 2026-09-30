//! Inline tests for `rate_limit.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn check_allows_new_provider() {
    let limiter = RateLimiter::new();
    assert!(limiter.check("yahoo").is_ok());
}

#[test]
fn record_rate_limit_blocks_until_cooldown_passes() {
    let limiter = RateLimiter::new();
    limiter.record_rate_limit("yahoo", Some(30));
    let err = limiter.check("yahoo").expect_err("should be rate limited");
    assert!(err.is_rate_limit());
    assert!(err.to_string().contains("yahoo"));
}

#[test]
fn record_success_clears_rate_limit() {
    let limiter = RateLimiter::new();
    limiter.record_rate_limit("yahoo", Some(30));
    assert!(limiter.check("yahoo").is_err());
    limiter.record_success("yahoo");
    assert!(limiter.check("yahoo").is_ok());
}

#[test]
fn exponential_backoff_capped_at_30s() {
    let limiter = RateLimiter::new();
    for _ in 0..5 {
        limiter.record_rate_limit("yahoo", None);
    }
    let err = limiter.check("yahoo").expect_err("should be rate limited");
    assert!(err.is_rate_limit());
    if let FinanceError::RateLimit { retry_after, .. } = err {
        assert!(retry_after.unwrap() <= MAX_BACKOFF_SECONDS);
    }
}
