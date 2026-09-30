//! Inline tests for `cardinality.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use opentelemetry::KeyValue;

#[test]
fn test_empty_attrs_returns_empty() {
    let cache = CardinalityCache::new(1000);
    let result = cache.resolve("ragent.llm.requests", &[]);
    assert_eq!(result, Vec::<opentelemetry::KeyValue>::new());
}

#[test]
fn test_new_combination_registered() {
    let cache = CardinalityCache::new(1000);
    let attrs = vec![
        KeyValue::new("model", "gpt-4"),
        KeyValue::new("provider", "openai"),
    ];
    let result = cache.resolve("ragent.llm.requests", &attrs);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].value.to_string(), "gpt-4");
    assert_eq!(result[1].value.to_string(), "openai");
    assert_eq!(cache.distinct_count("ragent.llm.requests"), 1);
}

#[test]
fn test_seen_combination_returned_unchanged() {
    let cache = CardinalityCache::new(1000);
    let attrs = vec![
        KeyValue::new("model", "gpt-4"),
        KeyValue::new("provider", "openai"),
    ];

    // First call registers the combination.
    let r1 = cache.resolve("ragent.llm.requests", &attrs);
    // Second call should return the same attrs (already seen).
    let r2 = cache.resolve("ragent.llm.requests", &attrs);

    assert_eq!(r1, r2);
    assert_eq!(cache.distinct_count("ragent.llm.requests"), 1);
}

#[test]
fn test_different_combinations_tracked_separately() {
    let cache = CardinalityCache::new(1000);

    let attrs1 = vec![
        KeyValue::new("model", "gpt-4"),
        KeyValue::new("provider", "openai"),
    ];
    let attrs2 = vec![
        KeyValue::new("model", "claude-3"),
        KeyValue::new("provider", "anthropic"),
    ];

    let _ = cache.resolve("ragent.llm.requests", &attrs1);
    let _ = cache.resolve("ragent.llm.requests", &attrs2);

    assert_eq!(cache.distinct_count("ragent.llm.requests"), 2);
}

#[test]
fn test_overflow_collapses_to_unknown() {
    let cache = CardinalityCache::new(3);

    // Register 3 distinct combinations (fills the limit).
    for i in 0..3 {
        let attrs = vec![KeyValue::new("model", format!("model-{i}"))];
        let result = cache.resolve("ragent.llm.requests", &attrs);
        assert_eq!(
            result[0].value.to_string(),
            format!("model-{i}"),
            "combination {i} should be registered, not collapsed"
        );
    }

    assert_eq!(cache.distinct_count("ragent.llm.requests"), 3);

    // 4th combination should collapse to "unknown".
    let attrs4 = vec![KeyValue::new("model", "model-overflow")];
    let result = cache.resolve("ragent.llm.requests", &attrs4);
    assert_eq!(
        result[0].value.to_string(),
        "unknown",
        "overflow combination should collapse to unknown"
    );

    // The distinct count should NOT increase (unknown is not tracked as new).
    assert_eq!(
        cache.distinct_count("ragent.llm.requests"),
        3,
        "unknown bucket should not increase distinct count"
    );
}

#[test]
fn test_existing_combination_after_overflow_still_returned_unchanged() {
    let cache = CardinalityCache::new(2);

    let attrs1 = vec![KeyValue::new("model", "model-a")];
    let attrs2 = vec![KeyValue::new("model", "model-b")];
    let attrs_overflow = vec![KeyValue::new("model", "model-c")];

    let _ = cache.resolve("ragent.llm.requests", &attrs1);
    let _ = cache.resolve("ragent.llm.requests", &attrs2);

    // This one overflows.
    let _ = cache.resolve("ragent.llm.requests", &attrs_overflow);

    // Going back to an already-seen combination should still return it
    // unchanged (not collapsed to unknown).
    let result = cache.resolve("ragent.llm.requests", &attrs1);
    assert_eq!(
        result[0].value.to_string(),
        "model-a",
        "previously-seen combination should not be collapsed"
    );
}

#[test]
fn test_different_metrics_tracked_independently() {
    let cache = CardinalityCache::new(2);

    let attrs = vec![KeyValue::new("model", "gpt-4")];

    let _ = cache.resolve("ragent.llm.requests", &attrs);
    let _ = cache.resolve("ragent.tool.invocations", &attrs);

    // Each metric has its own count.
    assert_eq!(cache.distinct_count("ragent.llm.requests"), 1);
    assert_eq!(cache.distinct_count("ragent.tool.invocations"), 1);
    assert_eq!(cache.distinct_count("ragent.sessions.total"), 0);
}

#[test]
fn test_multi_attribute_overflow_replaces_all_values() {
    let cache = CardinalityCache::new(1);

    let attrs1 = vec![
        KeyValue::new("model", "gpt-4"),
        KeyValue::new("provider", "openai"),
    ];
    let _ = cache.resolve("ragent.llm.requests", &attrs1);

    // Second combination with 2 attributes should overflow both.
    let attrs2 = vec![
        KeyValue::new("model", "claude-3"),
        KeyValue::new("provider", "anthropic"),
    ];
    let result = cache.resolve("ragent.llm.requests", &attrs2);

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].value.to_string(), "unknown");
    assert_eq!(result[1].value.to_string(), "unknown");
}

#[test]
fn test_default_limit_is_1000() {
    let cache = CardinalityCache::default();
    assert_eq!(cache.limit(), 1000);
}

#[test]
fn test_limit_zero_collapses_immediately() {
    // A limit of 0 means even the first combination overflows.
    let cache = CardinalityCache::new(0);
    let attrs = vec![KeyValue::new("model", "gpt-4")];
    let result = cache.resolve("ragent.llm.requests", &attrs);
    assert_eq!(result[0].value.to_string(), "unknown");
    assert_eq!(cache.distinct_count("ragent.llm.requests"), 0);
}
