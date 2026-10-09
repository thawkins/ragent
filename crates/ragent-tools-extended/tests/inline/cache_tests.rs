//! Inline tests for `cache.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

// Fixture quotes use exactly representable decimal values.
#![allow(clippy::float_cmp)]
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
