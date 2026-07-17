//! Cardinality cap for metric attributes (FR-035).
//!
//! The [`CardinalityCache`] tracks the number of distinct attribute-value
//! combinations seen per metric name. When the number of unique combinations
//! for a single metric exceeds the configured limit (default 1000), excess
//! combinations are collapsed into a single `unknown` bucket by replacing all
//! attribute values with `"unknown"`.
//!
//! # Design
//!
//! The cache is `Arc`-shared across all clones of an [`InstrumentRegistry`]
//! so that cardinality is tracked globally per metric, not per recorder
//! clone. The internal state is protected by an [`RwLock`] — reads (checking
//! if a signature is already seen) take a read lock, writes (inserting a new
//! signature) take a write lock. In the common case (signature already
//! seen), only a read lock is needed.
//!
//! # Attribute signature
//!
//! The signature of an attribute set is the concatenation of the `KeyValue`
//! values in order, separated by `|`. This is deterministic because the
//! recorders always build attribute slices in the same order. The keys are
//! not included in the signature because they are constant per metric — only
//! the values vary.

#[cfg(feature = "telemetry")]
use std::collections::{HashMap, HashSet};
#[cfg(feature = "telemetry")]
use std::sync::RwLock;

#[cfg(feature = "telemetry")]
use opentelemetry::KeyValue;

/// The sentinel value used for attribute values when the cardinality limit
/// is exceeded (FR-035).
pub const UNKNOWN_BUCKET: &str = "unknown";

/// The default cardinality limit per metric (FR-035).
pub const DEFAULT_CARDINALITY_LIMIT: usize = 1000;

/// A thread-safe cache that tracks distinct attribute combinations per metric
/// and collapses excess combinations into an `unknown` bucket (FR-035).
///
/// Held inside an [`Arc`] within [`InstrumentRegistry`](crate::InstrumentRegistry)
/// so that all clones share the same cardinality state.
#[cfg(feature = "telemetry")]
#[derive(Debug)]
pub struct CardinalityCache {
    /// Per-metric sets of seen attribute signatures.
    /// Key: metric name. Value: set of attribute-value signatures.
    seen: RwLock<HashMap<String, HashSet<String>>>,
    /// Maximum number of distinct combinations per metric before overflow.
    max: usize,
}

#[cfg(feature = "telemetry")]
impl CardinalityCache {
    /// Create a new cache with the given per-metric cardinality limit.
    #[must_use]
    pub fn new(max: usize) -> Self {
        Self {
            seen: RwLock::new(HashMap::new()),
            max,
        }
    }

    /// Resolve a set of attributes for a given metric, applying the
    /// cardinality cap (FR-035).
    ///
    /// If the attribute combination is already tracked for this metric,
    /// the original attributes are returned unchanged. If the combination
    /// is new and the per-metric limit has not been reached, the
    /// combination is registered and the original attributes are returned.
    /// If the limit has been reached, all attribute values are replaced
    /// with [`UNKNOWN_BUCKET`] ("unknown") so that excess combinations
    /// collapse into a single bucket.
    ///
    /// When `attrs` is empty (no attributes), this is a no-op — the
    /// metric has no cardinality to cap.
    #[must_use]
    pub fn resolve(&self, metric_name: &str, attrs: &[KeyValue]) -> Vec<KeyValue> {
        // Fast path: no attributes → no cardinality to cap.
        if attrs.is_empty() {
            return Vec::new();
        }

        let signature = make_signature(attrs);

        // Try read lock first — common case is that the signature is already seen.
        {
            if let Ok(seen) = self.seen.read() {
                if let Some(set) = seen.get(metric_name) {
                    if set.contains(&signature) {
                        return attrs.to_vec();
                    }
                }
            }
        }

        // Need to insert — take write lock.
        if let Ok(mut seen) = self.seen.write() {
            let set = seen.entry(metric_name.to_string()).or_default();

            // Double-check after acquiring write lock (another thread may have
            // inserted while we were waiting).
            if set.contains(&signature) {
                return attrs.to_vec();
            }

            if set.len() < self.max {
                set.insert(signature);
                attrs.to_vec()
            } else {
                // Limit exceeded — collapse to unknown bucket.
                attrs
                    .iter()
                    .map(|kv| KeyValue::new(kv.key.clone(), UNKNOWN_BUCKET.to_string()))
                    .collect()
            }
        } else {
            // Lock poisoned — return attrs as-is (fail open, never block).
            attrs.to_vec()
        }
    }

    /// Returns the number of distinct attribute combinations currently
    /// tracked for the given metric name. Primarily useful for testing.
    #[must_use]
    pub fn distinct_count(&self, metric_name: &str) -> usize {
        self.seen
            .read()
            .map(|seen| {
                seen.get(metric_name)
                    .map_or(0, std::collections::HashSet::len)
            })
            .unwrap_or(0)
    }

    /// Returns the configured per-metric cardinality limit.
    #[must_use]
    pub fn limit(&self) -> usize {
        self.max
    }
}

#[cfg(feature = "telemetry")]
impl Default for CardinalityCache {
    fn default() -> Self {
        Self::new(DEFAULT_CARDINALITY_LIMIT)
    }
}

#[cfg(feature = "telemetry")]
impl Clone for CardinalityCache {
    fn clone(&self) -> Self {
        // Clone the inner state so that each InstrumentRegistry clone
        // shares the same cardinality tracking. Actually — we want all
        // clones to share the SAME state, so we use Arc externally.
        // This Clone impl is only used if someone explicitly clones the
        // cache (not the normal path). It creates a snapshot copy.
        let seen = self.seen.read().map(|s| s.clone()).unwrap_or_default();
        Self {
            seen: RwLock::new(seen),
            max: self.max,
        }
    }
}

/// Build a deterministic signature string from an ordered slice of `KeyValue`s.
///
/// Only the values are included, not the keys, because the keys are constant
/// per metric — only the values vary. Values are joined with `|`.
#[cfg(feature = "telemetry")]
fn make_signature(attrs: &[KeyValue]) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(attrs.len());
    for kv in attrs {
        parts.push(kv.value.to_string());
    }
    parts.join("|")
}

// ── No-op stub when `telemetry` feature is off ───────────────────────────

/// No-op cardinality cache used when the `telemetry` Cargo feature is off.
///
/// All methods are zero-cost no-ops — when the feature is off, no
/// `InstrumentRegistry` exists, so this type is never instantiated in
/// practice. It exists only to keep the module compilable.
#[cfg(not(feature = "telemetry"))]
#[derive(Debug, Clone, Default)]
pub struct CardinalityCache;

#[cfg(not(feature = "telemetry"))]
impl CardinalityCache {
    /// Create a no-op cache.
    #[must_use]
    pub fn new(_max: usize) -> Self {
        Self
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(all(test, feature = "telemetry"))]
mod tests {
    use super::*;
    use opentelemetry::KeyValue;

    #[test]
    fn test_empty_attrs_returns_empty() {
        let cache = CardinalityCache::new(1000);
        let result = cache.resolve("ragent.llm.requests", &[]);
        assert!(result.is_empty());
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
}

#[cfg(all(test, not(feature = "telemetry")))]
mod tests {
    use super::*;

    #[test]
    fn test_noop_cardinality_cache() {
        let cache = CardinalityCache::new(1000);
        // No-op — just verify it doesn't panic.
        let _ = cache;
    }
}
