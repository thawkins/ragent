//! External tests for `cardinality.rs` branches not covered by the inline
//! suite: the `Clone` snapshot impl and the non-string attribute-value
//! sanitisation arm (SEC-ragent-telemetry-005).

#![cfg(feature = "telemetry")]

use opentelemetry::KeyValue;
use ragent_telemetry::cardinality::CardinalityCache;

/// A non-string attribute value is replaced with the `<non-string>` sentinel
/// during sanitisation rather than leaking the raw numeric value.
#[test]
fn test_non_string_attribute_value_is_sanitised() {
    let cache = CardinalityCache::new(1000);
    let attrs = vec![
        KeyValue::new("model", "gpt-4"),
        KeyValue::new("attempt", 3_i64),
    ];
    let resolved = cache.resolve("ragent.llm.requests", &attrs);
    assert_eq!(resolved.len(), 2);
    assert_eq!(resolved[0].value.to_string(), "gpt-4");
    assert_eq!(
        resolved[1].value.to_string(),
        "<non-string>",
        "a non-string attribute value must be replaced with the sentinel"
    );
}

/// `CardinalityCache::clone` produces an independent snapshot with the same
/// configured limit and the same set of seen signatures.
#[test]
fn test_clone_snapshots_state_and_limit() {
    let cache = CardinalityCache::new(7);
    let _ = cache.resolve("ragent.llm.requests", &[KeyValue::new("model", "gpt-4")]);
    assert_eq!(cache.distinct_count("ragent.llm.requests"), 1);

    let cloned = cache.clone();
    assert_eq!(cloned.limit(), 7);
    assert_eq!(cloned.distinct_count("ragent.llm.requests"), 1);

    // The clone is independent: adding to it does not affect the original.
    let _ = cloned.resolve("ragent.llm.requests", &[KeyValue::new("model", "claude")]);
    assert_eq!(cloned.distinct_count("ragent.llm.requests"), 2);
    assert_eq!(cache.distinct_count("ragent.llm.requests"), 1);
}
