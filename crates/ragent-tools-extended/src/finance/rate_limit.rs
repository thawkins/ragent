//! Per-provider rate limiting for the finance toolset.
//!
//! Tracks consecutive 429 responses per provider and applies a capped,
//! jittered exponential backoff. Additional requests while a provider is in
//! cooldown are rejected immediately without touching the network.

use crate::finance::{FinanceError, FinanceResult};
use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use std::collections::HashMap;
use std::sync::Mutex;

/// Maximum total backoff time after a 429 (30 seconds), per FR-012.
pub const MAX_BACKOFF_SECONDS: u64 = 30;

/// Base delay before the first retry attempt.
const BASE_BACKOFF_SECONDS: u64 = 1;

/// Maximum random jitter added to each backoff interval.
const MAX_JITTER_SECONDS: u64 = 1;

#[derive(Debug, Clone, Default)]
struct ProviderState {
    cooldown_until: Option<DateTime<Utc>>,
    consecutive_429s: u32,
}

/// Thread-safe rate limiter keyed by provider name.
#[derive(Debug, Default)]
pub struct RateLimiter {
    state: Mutex<HashMap<String, ProviderState>>,
}

impl RateLimiter {
    /// Create a new, empty rate limiter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check whether a request is allowed for `provider` right now.
    ///
    /// Returns `Ok(())` if the provider is not in cooldown. Returns a
    /// [`FinanceError::RateLimit`] with the remaining cooldown seconds otherwise.
    pub fn check(&self, provider: &str) -> FinanceResult<()> {
        let now = Utc::now();
        let state = self
            .state
            .lock()
            .map_err(|_| FinanceError::ProviderFailure {
                provider: provider.to_string(),
                message: "rate limiter lock poisoned".to_string(),
            })?;

        if let Some(entry) = state.get(provider)
            && let Some(until) = entry.cooldown_until
        {
            let remaining = until.signed_duration_since(now);
            if remaining > Duration::zero() {
                return Err(FinanceError::RateLimit {
                    provider: provider.to_string(),
                    retry_after: Some(remaining.num_seconds().max(0) as u64),
                });
            }
        }
        Ok(())
    }

    /// Record a successful response from `provider`, resetting the backoff state.
    pub fn record_success(&self, provider: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.insert(
                provider.to_string(),
                ProviderState {
                    cooldown_until: None,
                    consecutive_429s: 0,
                },
            );
        }
    }

    /// Record a 429 response from `provider` and put it into cooldown.
    ///
    /// `retry_after` is used when the provider explicitly reports one;
    /// otherwise an exponential backoff (1s * 2^n, capped at 30s) with jitter
    /// is applied.
    pub fn record_rate_limit(&self, provider: &str, retry_after: Option<u64>) {
        let now = Utc::now();
        if let Ok(mut state) = self.state.lock() {
            let entry = state.entry(provider.to_string()).or_default();
            entry.consecutive_429s += 1;

            let explicit = retry_after.map(|s| s.min(MAX_BACKOFF_SECONDS)).unwrap_or(0);
            let exponential = BASE_BACKOFF_SECONDS
                .saturating_mul(1_u64 << entry.consecutive_429s.saturating_sub(1))
                .min(MAX_BACKOFF_SECONDS);
            let base = if explicit > 0 { explicit } else { exponential };
            let jitter = rand::rng().random_range(0..=MAX_JITTER_SECONDS);
            let delay_seconds = base.saturating_add(jitter).min(MAX_BACKOFF_SECONDS);

            entry.cooldown_until = Some(now + Duration::seconds(delay_seconds as i64));
        }
    }

    /// Reset a provider's backoff state manually (mainly for tests).
    #[cfg(test)]
    pub fn reset(&self, provider: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.remove(provider);
        }
    }
}

#[cfg(test)]
mod tests {
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
}
