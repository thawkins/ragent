//! Run-scoped search budget and shared query-result cache.
//!
//! These two primitives bound and deduplicate the web-search calls issued by
//! the research pipeline so a single run cannot exhaust the quota of the
//! configured search providers.
//!
//! - [`SearchBudget`] is an atomic counter that caps the number of paid
//!   search calls a run may issue. It is shared by `Arc` across every
//!   researcher and gather pass in the run, so supervisor/competitive modes
//!   (which clone one [`crate::web_gatherer::WebGatherer`] into N parallel
//!   researchers) draw from a single pool.
//! - [`SharedQueryCache`] memoises successful search results keyed on the
//!   normalized query text. Competitive researchers routinely decompose their
//!   entity-scoped sub-topics into near-identical dimension queries ("X
//!   pricing", "Y pricing"); the cache turns the second and subsequent
//!   identical query into a free lookup instead of another provider call.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

// PERF-080: FxHash for the short, non-adversarial normalised-query cache key.
use rustc_hash::FxHashMap as HashMap;

/// Hard cap on the number of web-search calls a research run may issue.
///
/// `limit` of `None` means unlimited (the historic behaviour). All clones of
/// a budget share the same counter via `Arc`, so handing the same
/// `Arc<SearchBudget>` to every researcher in a supervisor/competitive run
/// enforces a per-run (not per-researcher) ceiling.
#[derive(Debug)]
pub struct SearchBudget {
    limit: Option<usize>,
    used: AtomicUsize,
}

impl SearchBudget {
    /// Create a budget allowing at most `limit` search calls (`None` = unlimited).
    #[must_use]
    pub fn new(limit: Option<usize>) -> Self {
        Self {
            limit,
            used: AtomicUsize::new(0),
        }
    }

    /// Try to reserve one search call. Returns `false` when the budget is
    /// exhausted and the caller must skip the search.
    ///
    /// A reservation covers one logical search including its retries; the
    /// gatherer acquires once per sub-query before entering its retry loop.
    pub fn try_acquire(&self) -> bool {
        match self.limit {
            None => true,
            Some(limit) => {
                let n = self.used.fetch_add(1, Ordering::Relaxed);
                n < limit
            }
        }
    }

    /// Number of search calls consumed by successful acquisitions. Failed
    /// (rejected) attempts do not count, so this never exceeds the limit.
    #[must_use]
    pub fn used(&self) -> usize {
        let used = self.used.load(Ordering::Relaxed);
        used.min(self.limit.unwrap_or(usize::MAX))
    }

    /// Configured limit, if any.
    #[must_use]
    pub fn limit(&self) -> Option<usize> {
        self.limit
    }

    /// Whether the budget is exhausted (no further calls may be reserved).
    #[must_use]
    pub fn exhausted(&self) -> bool {
        match self.limit {
            None => false,
            Some(limit) => self.used.load(Ordering::Relaxed) >= limit,
        }
    }
}

/// Cache of successful search results shared across the gather passes of a
/// single research run.
///
/// Keyed on the normalized (lowercased, whitespace-collapsed) query text.
/// A concurrent miss does not block or reserve — two researchers decomposing
/// to the same query at the same instant may both pay for it once; any later
/// identical query hits the cache.
#[derive(Debug)]
pub struct SharedQueryCache {
    /// PERF-077: each cached hit list is shared behind an `Arc<[WebSearchHit]>`
    /// so `insert` stores the caller's `Arc` (no deep clone) and `get` returns
    /// a refcount bump rather than a full vector copy.
    entries: Mutex<HashMap<String, std::sync::Arc<[crate::web_gatherer::WebSearchHit]>>>,
}

impl SharedQueryCache {
    /// Create an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::default()),
        }
    }

    /// Normalize a query for cache-key purposes: lowercase and collapse all
    /// whitespace runs to a single space.
    #[must_use]
    pub fn normalize(query: &str) -> String {
        query
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    }

    /// Look up cached hits for `query`. Returns a shared handle so callers
    /// cannot mutate the shared entry and no deep copy is made (PERF-077).
    pub fn get(&self, query: &str) -> Option<std::sync::Arc<[crate::web_gatherer::WebSearchHit]>> {
        let key = Self::normalize(query);
        let map = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.get(&key).cloned()
    }

    /// Store successful hits for `query`, replacing any previous entry.
    ///
    /// PERF-077: takes the caller's `Arc<[WebSearchHit]>` so this is a refcount
    /// hand-off, not a deep clone of every hit.
    pub fn insert(&self, query: &str, hits: std::sync::Arc<[crate::web_gatherer::WebSearchHit]>) {
        if hits.is_empty() {
            // An empty result is not worth caching: a later identical query
            // may succeed against a recovered engine.
            return;
        }
        let key = Self::normalize(query);
        let mut map = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.insert(key, hits);
    }
}

impl Default for SharedQueryCache {
    fn default() -> Self {
        Self::new()
    }
}
