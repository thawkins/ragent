//! Trigger runtime — deduplication and cycle suppression for trigger events.
//!
//! The [`TriggerRuntime`] is the central component that receives trigger
//! envelopes from all sources (dynamic rules, MCP notification hooks) and
//! applies two protective mechanisms before dispatching:
//!
//! 1. **Deduplication** — if an envelope with the same `dedup_hash` has
//!    already been processed within the dedup window, the new envelope is
//!    silently dropped. This prevents repeated notifications from the same
//!    source with identical content from spamming the chat.
//!
//! 2. **Cycle suppression** — if a source fires the same action repeatedly
//!    (detected via `source_id` + `dedup_hash`), the runtime suppresses
//!    further firings after `max_cycles` consecutive duplicates. This
//!    prevents infinite loops where a trigger's output re-triggers itself.
//!
//! See `specs/piegap/SPEC.md` FR-002 and FR-003 for the full specification.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use ragent_types::trigger::{TriggerEnvelope, TriggerFired, TriggerRule, TriggerRuleId};
use tracing::{debug, warn};

/// Configuration for the trigger runtime.
#[derive(Debug, Clone)]
pub struct TriggerRuntimeConfig {
    /// How long to remember a dedup hash before considering it eligible
    /// again. Default: 60 seconds.
    pub dedup_window: Duration,
    /// Maximum consecutive identical firings from the same source before
    /// cycle suppression kicks in. Default: 3.
    pub max_cycles: usize,
}

impl Default for TriggerRuntimeConfig {
    fn default() -> Self {
        Self {
            dedup_window: Duration::from_mins(1),
            max_cycles: 3,
        }
    }
}

/// Entry in the dedup cache.
#[derive(Debug, Clone)]
struct DedupEntry {
    seen_at: Instant,
    count: usize,
}

/// Entry tracking cycle counts per source.
#[derive(Debug, Clone)]
struct CycleEntry {
    last_hash: u64,
    consecutive: usize,
    suppressed: bool,
}

/// The trigger runtime manages trigger rules and processes trigger envelopes
/// with deduplication and cycle suppression.
///
/// Thread-safe via an internal `Mutex`. Cloning a `TriggerRuntime` shares
/// the underlying state (similar to `Arc<Mutex<...>>`).
#[derive(Debug, Clone)]
pub struct TriggerRuntime {
    inner: Arc<Mutex<TriggerRuntimeInner>>,
    /// The runtime configuration (dedup window, max cycles).
    pub config: TriggerRuntimeConfig,
}

#[derive(Debug)]
struct TriggerRuntimeInner {
    /// Registered trigger rules keyed by rule ID.
    rules: HashMap<String, TriggerRule>,
    /// Dedup cache: hash → entry.
    dedup_cache: HashMap<u64, DedupEntry>,
    /// Cycle tracking: source_id → cycle entry.
    cycles: HashMap<String, CycleEntry>,
}

impl TriggerRuntime {
    /// Creates a new trigger runtime with the given configuration.
    pub fn new(config: TriggerRuntimeConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(TriggerRuntimeInner {
                rules: HashMap::new(),
                dedup_cache: HashMap::new(),
                cycles: HashMap::new(),
            })),
            config,
        }
    }

    /// Creates a new trigger runtime with default configuration.
    pub fn default() -> Self {
        Self::new(TriggerRuntimeConfig::default())
    }

    /// Registers a trigger rule with the runtime.
    pub fn add_rule(&self, rule: TriggerRule) -> TriggerRuleId {
        let id = rule.id.clone();
        let id_str = id.as_str().to_string();
        debug!(rule_id = %id_str, "Trigger rule registered");
        self.inner.lock().rules.insert(id_str, rule);
        id
    }

    /// Removes a trigger rule by ID. Returns `true` if the rule was found
    /// and removed.
    pub fn remove_rule(&self, rule_id: &str) -> bool {
        let removed = self.inner.lock().rules.remove(rule_id).is_some();
        if removed {
            debug!(rule_id = %rule_id, "Trigger rule removed");
        }
        removed
    }

    /// Enables a trigger rule. Returns `true` if the rule was found.
    pub fn enable_rule(&self, rule_id: &str) -> bool {
        let mut inner = self.inner.lock();
        if let Some(rule) = inner.rules.get_mut(rule_id) {
            rule.enabled = true;
            debug!(rule_id = %rule_id, "Trigger rule enabled");
            true
        } else {
            false
        }
    }

    /// Disables a trigger rule. Returns `true` if the rule was found.
    pub fn disable_rule(&self, rule_id: &str) -> bool {
        let mut inner = self.inner.lock();
        if let Some(rule) = inner.rules.get_mut(rule_id) {
            rule.enabled = false;
            debug!(rule_id = %rule_id, "Trigger rule disabled");
            true
        } else {
            false
        }
    }

    /// Returns a snapshot of all registered rules.
    pub fn list_rules(&self) -> Vec<TriggerRule> {
        self.inner.lock().rules.values().cloned().collect()
    }

    /// Returns a snapshot of a single rule by ID.
    pub fn get_rule(&self, rule_id: &str) -> Option<TriggerRule> {
        self.inner.lock().rules.get(rule_id).cloned()
    }

    /// Returns the number of registered rules.
    pub fn rule_count(&self) -> usize {
        self.inner.lock().rules.len()
    }

    /// Processes a trigger envelope, applying deduplication and cycle
    /// suppression.
    ///
    /// Returns `Some(TriggerFired)` if the envelope should be dispatched,
    /// or `None` if it was suppressed as a duplicate or cycle.
    pub fn process(&self, envelope: TriggerEnvelope) -> Option<TriggerFired> {
        let now = Instant::now();
        let mut inner = self.inner.lock();

        // ── Step 1: Deduplication ────────────────────────────────────
        // Check if we've seen this exact envelope content recently.
        let dedup = inner.dedup_cache.get(&envelope.dedup_hash).cloned();
        if let Some(entry) = &dedup {
            let age = now.duration_since(entry.seen_at);
            if age < self.config.dedup_window {
                debug!(
                    dedup_hash = envelope.dedup_hash,
                    age_secs = age.as_secs(),
                    "Trigger envelope suppressed as duplicate"
                );
                return None;
            }
        }

        // ── Step 2: Cycle suppression ────────────────────────────────
        // Check if this source is firing the same content repeatedly.
        let cycle = inner.cycles.get(&envelope.source_id).cloned();
        let suppressed = if let Some(c) = &cycle {
            if c.last_hash == envelope.dedup_hash {
                c.consecutive >= self.config.max_cycles
            } else {
                false
            }
        } else {
            false
        };

        if suppressed {
            warn!(
                source_id = %envelope.source_id,
                consecutive = cycle.as_ref().map(|c| c.consecutive).unwrap_or(0),
                "Trigger envelope suppressed due to cycle detection"
            );
            return None;
        }

        // ── Step 3: Update dedup cache ────────────────────────────────
        let count = dedup.as_ref().map(|e| e.count).unwrap_or(0);
        inner.dedup_cache.insert(
            envelope.dedup_hash,
            DedupEntry {
                seen_at: now,
                count: count + 1,
            },
        );

        // ── Step 4: Update cycle tracker ──────────────────────────────
        let cycle_entry = inner.cycles.get(&envelope.source_id).cloned();
        let new_consecutive = if let Some(c) = &cycle_entry {
            if c.last_hash == envelope.dedup_hash {
                c.consecutive + 1
            } else {
                1
            }
        } else {
            1
        };
        let was_suppressed = cycle_entry.as_ref().map(|c| c.suppressed).unwrap_or(false);
        inner.cycles.insert(
            envelope.source_id.clone(),
            CycleEntry {
                last_hash: envelope.dedup_hash,
                consecutive: new_consecutive,
                suppressed: was_suppressed,
            },
        );

        // ── Step 5: Mark rule as fired if applicable ───────────────────
        let rule_id = if envelope.source_kind == ragent_types::trigger::TriggerSourceKind::Dynamic {
            // For dynamic triggers, the source_id is the rule ID.
            if let Some(rule) = inner.rules.get_mut(&envelope.source_id) {
                if rule.fire_once {
                    rule.fired_at = Some(envelope.timestamp);
                }
                Some(rule.id.clone())
            } else {
                None
            }
        } else {
            None
        };

        // ── Step 6: Return the fired result ───────────────────────────
        debug!(
            envelope_id = %envelope.id,
            source_id = %envelope.source_id,
            action_kind = ?envelope.action_kind,
            "Trigger envelope dispatched"
        );

        Some(TriggerFired { envelope, rule_id })
    }

    /// Purges expired dedup entries. Called periodically to prevent
    /// unbounded memory growth.
    pub fn purge_expired(&self) -> usize {
        let now = Instant::now();
        let mut inner = self.inner.lock();
        let before = inner.dedup_cache.len();
        inner
            .dedup_cache
            .retain(|_, entry| now.duration_since(entry.seen_at) < self.config.dedup_window);
        let purged = before - inner.dedup_cache.len();
        if purged > 0 {
            debug!(purged, "Purged expired dedup entries");
        }
        purged
    }

    /// Returns the number of entries currently in the dedup cache.
    pub fn dedup_cache_size(&self) -> usize {
        self.inner.lock().dedup_cache.len()
    }

    /// Returns the number of sources currently being cycle-tracked.
    pub fn cycle_tracker_size(&self) -> usize {
        self.inner.lock().cycles.len()
    }

    /// Clears all rules, dedup cache, and cycle trackers. Used when a
    /// session is closed or reset.
    pub fn clear(&self) {
        let mut inner = self.inner.lock();
        inner.rules.clear();
        inner.dedup_cache.clear();
        inner.cycles.clear();
        debug!("Trigger runtime cleared");
    }
}

impl Default for TriggerRuntime {
    fn default() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ragent_types::trigger::{
        TriggerActionKind, TriggerEnvelope, TriggerRuleStatus, TriggerSourceKind,
    };

    fn make_envelope(source: &str, summary: &str, action: &str) -> TriggerEnvelope {
        TriggerEnvelope::new(
            TriggerSourceKind::McpNotification,
            source,
            summary,
            action,
            TriggerActionKind::InjectSummary,
            false,
        )
    }

    #[test]
    fn test_first_envelope_passes() {
        let rt = TriggerRuntime::default();
        let env = make_envelope("server-1", "build done", "report it");
        assert!(rt.process(env).is_some());
    }

    #[test]
    fn test_duplicate_within_window_suppressed() {
        let rt = TriggerRuntime::default();
        let env1 = make_envelope("server-1", "build done", "report it");
        let env2 = make_envelope("server-1", "build done", "report it");
        assert!(rt.process(env1).is_some());
        assert!(rt.process(env2).is_none()); // suppressed
    }

    #[test]
    fn test_different_content_not_suppressed() {
        let rt = TriggerRuntime::default();
        let env1 = make_envelope("server-1", "build done", "report it");
        let env2 = make_envelope("server-1", "build failed", "report it");
        assert!(rt.process(env1).is_some());
        assert!(rt.process(env2).is_some()); // different content
    }

    #[test]
    fn test_different_source_not_suppressed() {
        let rt = TriggerRuntime::default();
        let env1 = make_envelope("server-1", "build done", "report it");
        let env2 = make_envelope("server-2", "build done", "report it");
        assert!(rt.process(env1).is_some());
        assert!(rt.process(env2).is_some());
    }

    #[test]
    fn test_cycle_suppression_after_max_cycles() {
        let config = TriggerRuntimeConfig {
            dedup_window: Duration::from_secs(0), // no dedup window so only cycle matters
            max_cycles: 3,
        };
        let rt = TriggerRuntime::new(config);

        // First 3 firings of the same source+content pass (cycle count 1,2,3)
        for _ in 0..3 {
            let env = make_envelope("server-1", "build done", "report it");
            assert!(
                rt.process(env).is_some(),
                "Should pass within max_cycles boundary"
            );
        }

        // 4th firing is suppressed (consecutive > max_cycles)
        let env = make_envelope("server-1", "build done", "report it");
        assert!(rt.process(env).is_none(), "Should be cycle-suppressed");
    }

    #[test]
    fn test_cycle_resets_on_different_content() {
        let config = TriggerRuntimeConfig {
            dedup_window: Duration::from_secs(0),
            max_cycles: 2,
        };
        let rt = TriggerRuntime::new(config);

        // Fire same content twice
        let env = make_envelope("server-1", "build done", "report it");
        assert!(rt.process(env).is_some());
        let env = make_envelope("server-1", "build done", "report it");
        assert!(rt.process(env).is_some());

        // Different content resets cycle
        let env = make_envelope("server-1", "build failed", "report it");
        assert!(rt.process(env).is_some());

        // Same content as before the reset — should pass (cycle reset)
        let env = make_envelope("server-1", "build done", "report it");
        assert!(rt.process(env).is_some());
    }

    #[test]
    fn test_rule_add_remove_enable_disable() {
        let rt = TriggerRuntime::default();
        let rule = TriggerRule::new("file exists", "print it");
        let id = rt.add_rule(rule);
        assert_eq!(rt.rule_count(), 1);

        assert!(rt.disable_rule(id.as_str()));
        let r = rt.get_rule(id.as_str()).unwrap();
        assert!(!r.enabled);
        assert_eq!(r.status(), TriggerRuleStatus::Disabled);

        assert!(rt.enable_rule(id.as_str()));
        let r = rt.get_rule(id.as_str()).unwrap();
        assert!(r.enabled);
        assert_eq!(r.status(), TriggerRuleStatus::Active);

        assert!(rt.remove_rule(id.as_str()));
        assert_eq!(rt.rule_count(), 0);
        assert!(!rt.remove_rule(id.as_str())); // already gone
    }

    #[test]
    fn test_dynamic_trigger_marks_rule_fired() {
        let rt = TriggerRuntime::default();
        let mut rule = TriggerRule::new("file exists", "print it");
        rule.id = TriggerRuleId::from("rule-test-1");
        rt.add_rule(rule);

        let env = TriggerEnvelope::new(
            TriggerSourceKind::Dynamic,
            "rule-test-1",
            "file exists",
            "print it",
            TriggerActionKind::SubAgent,
            false,
        );
        let fired = rt.process(env).unwrap();
        assert_eq!(fired.rule_id.as_ref().unwrap().as_str(), "rule-test-1");

        let r = rt.get_rule("rule-test-1").unwrap();
        assert!(r.fired_at.is_some());
        assert_eq!(r.status(), TriggerRuleStatus::Fired);
    }

    #[test]
    fn test_purge_expired() {
        let config = TriggerRuntimeConfig {
            dedup_window: Duration::from_millis(10),
            max_cycles: 100,
        };
        let rt = TriggerRuntime::new(config);

        let env = make_envelope("s1", "msg", "act");
        rt.process(env);
        assert_eq!(rt.dedup_cache_size(), 1);

        std::thread::sleep(Duration::from_millis(50));
        let purged = rt.purge_expired();
        assert_eq!(purged, 1);
        assert_eq!(rt.dedup_cache_size(), 0);
    }

    #[test]
    fn test_clear() {
        let rt = TriggerRuntime::default();
        rt.add_rule(TriggerRule::new("cond", "act"));
        let env = make_envelope("s1", "msg", "act");
        rt.process(env);
        assert_eq!(rt.rule_count(), 1);
        assert_eq!(rt.dedup_cache_size(), 1);

        rt.clear();
        assert_eq!(rt.rule_count(), 0);
        assert_eq!(rt.dedup_cache_size(), 0);
        assert_eq!(rt.cycle_tracker_size(), 0);
    }

    #[test]
    fn test_shared_state_via_clone() {
        let rt = TriggerRuntime::default();
        let rt2 = rt.clone();
        let id = rt.add_rule(TriggerRule::new("cond", "act"));
        // The clone shares the same state.
        assert_eq!(rt2.rule_count(), 1);
        assert!(rt2.get_rule(id.as_str()).is_some());
    }
}
