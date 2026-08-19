//! Unit tests for `finance::cache` TTL and eviction behavior.
//!
//! These tests exercise the public `QuoteCache` API through the crate boundary
//! so they verify the same surface that tools and providers use.

use chrono::Duration;
use ragent_tools_extended::finance::{Quote, QuoteCache};

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
        timestamp: chrono::Utc::now(),
    }
}

#[test]
fn test_cache_returns_quote_within_ttl() {
    let cache = QuoteCache::default_cache();
    let quote = sample_quote("AAPL");
    cache.set("yahoo", "AAPL", quote.clone());

    let cached = cache.get("yahoo", "AAPL").expect("quote should be cached");
    assert_eq!(cached.symbol, "AAPL");
    assert_eq!(cached.price, 150.0);
}

#[test]
fn test_cache_returns_none_for_expired_entry() {
    let cache = QuoteCache::new(Duration::zero());
    let quote = sample_quote("AAPL");
    cache.set("yahoo", "AAPL", quote);

    assert!(
        cache.get("yahoo", "AAPL").is_none(),
        "expired entries should not be returned"
    );
}

#[test]
fn test_cache_replacement_refreshes_ttl() {
    let cache = QuoteCache::new(Duration::milliseconds(100));
    let original = sample_quote("AAPL");
    cache.set("yahoo", "AAPL", original.clone());

    // Sleep until the first entry is on the edge of expiry, then replace it.
    std::thread::sleep(Duration::milliseconds(80).to_std().unwrap());
    let replacement = {
        let mut q = sample_quote("AAPL");
        q.price = 155.0;
        q
    };
    cache.set("yahoo", "AAPL", replacement.clone());

    // Without the replacement, this would have expired.
    std::thread::sleep(Duration::milliseconds(50).to_std().unwrap());
    let cached = cache
        .get("yahoo", "AAPL")
        .expect("replacement should refresh TTL");
    assert_eq!(cached.price, 155.0);
}

#[test]
fn test_cache_keys_have_independent_expiry() {
    let cache = QuoteCache::new(Duration::milliseconds(400));
    cache.set("yahoo", "AAPL", sample_quote("AAPL"));
    std::thread::sleep(Duration::milliseconds(50).to_std().unwrap());
    cache.set("yahoo", "MSFT", sample_quote("MSFT"));

    // Sleep until AAPL is expired but MSFT still lives.
    std::thread::sleep(Duration::milliseconds(370).to_std().unwrap());

    assert!(
        cache.get("yahoo", "AAPL").is_none(),
        "AAPL should have expired"
    );
    let cached = cache
        .get("yahoo", "MSFT")
        .expect("MSFT should still be within TTL");
    assert_eq!(cached.symbol, "MSFT");
}

#[test]
fn test_cache_negative_ttl_never_returns() {
    let cache = QuoteCache::new(Duration::seconds(-1));
    let quote = sample_quote("AAPL");
    cache.set("yahoo", "AAPL", quote);

    assert!(
        cache.get("yahoo", "AAPL").is_none(),
        "negative TTL should treat every entry as expired"
    );
}

#[test]
fn test_cache_selectively_evicts_stale_entry() {
    let cache = QuoteCache::new(Duration::milliseconds(50));
    cache.set("yahoo", "AAPL", sample_quote("AAPL"));
    cache.set("yahoo", "MSFT", sample_quote("MSFT"));

    std::thread::sleep(Duration::milliseconds(80).to_std().unwrap());

    // AAPL lookup evicts the stale entry.
    assert!(
        cache.get("yahoo", "AAPL").is_none(),
        "AAPL should be evicted"
    );

    // MSFT lookup also evicts.
    assert!(
        cache.get("yahoo", "MSFT").is_none(),
        "MSFT should be evicted"
    );
}

#[test]
fn test_cache_clear_removes_all_entries() {
    let cache = QuoteCache::default_cache();
    cache.set("yahoo", "AAPL", sample_quote("AAPL"));
    cache.set("yahoo", "MSFT", sample_quote("MSFT"));

    cache.clear();

    assert!(cache.get("yahoo", "AAPL").is_none());
    assert!(cache.get("yahoo", "MSFT").is_none());
}

#[test]
fn test_cache_keys_are_case_normalized() {
    let cache = QuoteCache::default_cache();
    let quote = sample_quote("AAPL");
    cache.set("Yahoo", "aapl", quote.clone());

    let cached = cache
        .get("yahoo", "AAPL")
        .expect("lookup should be case-insensitive");
    assert_eq!(cached.symbol, "AAPL");
}

#[test]
fn test_cache_distinguishes_provider_symbol_pairs() {
    let cache = QuoteCache::default_cache();
    let mut yahoo_quote = sample_quote("AAPL");
    yahoo_quote.price = 150.0;
    let mut paid_quote = sample_quote("AAPL");
    paid_quote.price = 151.0;

    cache.set("yahoo", "AAPL", yahoo_quote);
    cache.set("paid", "AAPL", paid_quote);

    assert_eq!(cache.get("yahoo", "AAPL").unwrap().price, 150.0);
    assert_eq!(cache.get("paid", "AAPL").unwrap().price, 151.0);
}
