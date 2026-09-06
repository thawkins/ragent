//! Run-scoped per-provider search-request statistics.
//!
//! [`ProviderCallStats`] counts the logical search calls issued by the
//! research pipeline, keyed on the underlying search tool (e.g. `mf_search`,
//! `websearch`). It is shared by `Arc` across every gather pass in the run —
//! including supervisor/competitive researchers, which clone one
//! [`crate::web_gatherer::WebGatherer`] into N parallel workers — so the
//! totals are per-run rather than per-researcher.
//!
//! The counter is incremented once per logical search (retries included,
//! mirroring [`crate::search_budget::SearchBudget`] semantics) and never for
//! cache hits, budget-skips, or circuit-breaker skips, because those paths
//! issue no provider request.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Per-run counter of search requests issued to each search provider.
#[derive(Debug, Default)]
pub struct ProviderCallStats {
    /// Total logical search calls issued (retries included).
    total: AtomicUsize,
    /// Calls per search-tool name, kept sorted for stable rendering.
    by_tool: Mutex<BTreeMap<String, usize>>,
}

impl ProviderCallStats {
    /// Create an empty counter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one logical search call for `tool` (retries included; cache
    /// hits and budget/circuit skips are never recorded by the gatherer).
    pub fn record(&self, tool: &str) {
        self.total.fetch_add(1, Ordering::Relaxed);
        let mut by_tool = self.by_tool.lock().unwrap_or_else(|p| p.into_inner());
        *by_tool.entry(tool.to_string()).or_insert(0) += 1;
    }

    /// Total number of logical search calls issued this run.
    #[must_use]
    pub fn total(&self) -> usize {
        self.total.load(Ordering::Relaxed)
    }

    /// Snapshot of `(search tool, call count)` pairs, sorted by tool name.
    #[must_use]
    pub fn by_tool(&self) -> Vec<(String, usize)> {
        let by_tool = self.by_tool.lock().unwrap_or_else(|p| p.into_inner());
        by_tool.iter().map(|(k, v)| (k.clone(), *v)).collect()
    }
}
