//! In-memory quote cache with a configurable TTL.
//!
//! Quotes are keyed by `(provider, symbol)` and evicted lazily on read when
//! their TTL expires. This reduces repeated provider calls for the same
//! popular symbol without requiring any persistent storage.

use crate::finance::model::Quote;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::RwLock;

/// Default TTL for cached quotes (60 seconds).
pub const DEFAULT_QUOTE_TTL: Duration = Duration::seconds(60);

#[derive(Debug, Clone)]
struct CacheEntry {
    quote: Quote,
    inserted_at: DateTime<Utc>,
}

/// Thread-safe in-memory cache keyed by `(provider, symbol)`.
#[derive(Debug)]
pub struct QuoteCache {
    entries: RwLock<HashMap<(String, String), CacheEntry>>,
    ttl: Duration,
}

impl QuoteCache {
    /// Create a cache with the given TTL.
    #[must_use]
    pub fn new(ttl: Duration) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            ttl,
        }
    }

    /// Create a cache with the default 60-second TTL.
    #[must_use]
    pub fn default_cache() -> Self {
        Self::new(DEFAULT_QUOTE_TTL)
    }

    /// Retrieve a non-expired quote for the given provider and symbol.
    ///
    /// Returns `None` if no entry exists or if the entry has expired. Expired
    /// entries are removed lazily during the lookup.
    pub fn get(&self, provider: &str, symbol: &str) -> Option<Quote> {
        let key = normalized_key(provider, symbol);
        let now = Utc::now();

        {
            let entries = self.entries.read().ok()?;
            if let Some(entry) = entries.get(&key) {
                let age = now.signed_duration_since(entry.inserted_at);
                if age < Duration::zero() || age >= self.ttl {
                    return None;
                }
                return Some(entry.quote.clone());
            }
        }

        // Entry is missing or expired; try to remove a stale entry.
        if let Ok(mut entries) = self.entries.write()
            && let Some(entry) = entries.get(&key)
        {
            let age = now.signed_duration_since(entry.inserted_at);
            if age >= Duration::zero() && age < self.ttl {
                return Some(entry.quote.clone());
            }
            entries.remove(&key);
        }

        None
    }

    /// Insert or replace a quote for the given provider and symbol.
    pub fn set(&self, provider: &str, symbol: &str, quote: Quote) {
        let key = normalized_key(provider, symbol);
        let entry = CacheEntry {
            quote,
            inserted_at: Utc::now(),
        };
        if let Ok(mut entries) = self.entries.write() {
            entries.insert(key, entry);
        }
    }

    /// Remove all cached entries.
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.write() {
            entries.clear();
        }
    }
}

fn normalized_key(provider: &str, symbol: &str) -> (String, String) {
    (provider.to_ascii_lowercase(), symbol.to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_quote(symbol: &str) -> Quote {
        Quote {
            symbol: symbol.to_string(),
            price: 150.0,
            open: 149.0,
            high: 151.0,
            low: 148.0,
            close: 150.0,
            volume: 1_000_000,
            change: 1.0,
            change_percent: 0.67,
            currency: "USD".to_string(),
            market_state: "REGULAR".to_string(),
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn returns_none_when_empty() {
        let cache = QuoteCache::default_cache();
        assert!(cache.get("yahoo", "AAPL").is_none());
    }

    #[test]
    fn stores_and_returns_quote() {
        let cache = QuoteCache::default_cache();
        let quote = sample_quote("AAPL");
        cache.set("yahoo", "AAPL", quote.clone());
        let cached = cache.get("yahoo", "AAPL").expect("quote should be cached");
        assert_eq!(cached.symbol, "AAPL");
        assert_eq!(cached.price, 150.0);
    }

    #[test]
    fn key_is_case_normalized() {
        let cache = QuoteCache::default_cache();
        let quote = sample_quote("AAPL");
        cache.set("Yahoo", "aapl", quote.clone());
        let cached = cache
            .get("YAHOO", "AAPL")
            .expect("lookup should be case-insensitive");
        assert_eq!(cached.symbol, "AAPL");
    }

    #[test]
    fn expired_entries_are_not_returned() {
        let cache = QuoteCache::new(Duration::zero());
        let quote = sample_quote("AAPL");
        cache.set("yahoo", "AAPL", quote);
        assert!(cache.get("yahoo", "AAPL").is_none());
    }

    #[test]
    fn clear_removes_all_entries() {
        let cache = QuoteCache::default_cache();
        cache.set("yahoo", "AAPL", sample_quote("AAPL"));
        cache.set("yahoo", "MSFT", sample_quote("MSFT"));
        cache.clear();
        assert!(cache.get("yahoo", "AAPL").is_none());
        assert!(cache.get("yahoo", "MSFT").is_none());
    }
}
