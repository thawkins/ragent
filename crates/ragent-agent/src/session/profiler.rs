//! Runtime profiler for the agent action loop.
//!
//! Tracks per-operation counters and timing aggregates for the main session
//! processor so the TUI can render a live profiling panel while the profiler is
//! enabled.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant};

/// Aggregated timing statistics for one profiled operation.
#[derive(Debug, Clone, Default)]
pub struct ProfileOperationSnapshot {
    /// Operation label.
    pub name: String,
    /// Number of completed samples.
    pub count: u64,
    /// Total elapsed time across all samples, in milliseconds.
    pub total_ms: f64,
    /// Average elapsed time, in milliseconds.
    pub avg_ms: f64,
    /// Exclusive/self elapsed time across all samples, in milliseconds.
    pub self_total_ms: f64,
    /// Exclusive/self average elapsed time, in milliseconds.
    pub self_avg_ms: f64,
    /// Maximum observed elapsed time, in milliseconds.
    pub max_ms: f64,
    /// Most recent elapsed time, in milliseconds.
    pub last_ms: f64,
}

#[derive(Debug, Clone, Default)]
struct ProfileOperationStats {
    count: u64,
    total_micros: u128,
    max_micros: u128,
    last_micros: u128,
}

/// Snapshot of the current profiler state for UI rendering.
#[derive(Debug, Clone, Default)]
pub struct ProfileSnapshot {
    /// Whether profiling is currently enabled.
    pub enabled: bool,
    /// Elapsed time since profiling was enabled, in milliseconds.
    pub running_for_ms: u64,
    /// Total number of recorded samples across all operations.
    pub total_samples: u64,
    /// Operation snapshots sorted by descending total time.
    pub operations: Vec<ProfileOperationSnapshot>,
}

/// Shared profiler used by the session processor and TUI.
#[derive(Debug, Default)]
pub struct AgentLoopProfiler {
    enabled: AtomicBool,
    started_at: RwLock<Option<Instant>>,
    stats: RwLock<HashMap<String, ProfileOperationStats>>,
}

impl AgentLoopProfiler {
    /// Create a new profiler with profiling disabled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Return whether profiling is currently enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Enable or disable profiling.
    ///
    /// Enabling resets all accumulated statistics so the panel reflects the new
    /// profiling run only.
    pub fn set_enabled(&self, enabled: bool) {
        if enabled {
            self.reset();
            self.enabled.store(true, Ordering::Relaxed);
            if let Ok(mut started_at) = self.started_at.write() {
                *started_at = Some(Instant::now());
            }
        } else {
            self.enabled.store(false, Ordering::Relaxed);
            if let Ok(mut started_at) = self.started_at.write() {
                *started_at = None;
            }
        }
    }

    /// Clear all accumulated samples while preserving the enabled flag.
    pub fn reset(&self) {
        if let Ok(mut stats) = self.stats.write() {
            stats.clear();
        }
    }

    /// Start a profiling scope for a static operation label.
    #[must_use]
    pub fn scope(self: &Arc<Self>, label: &'static str) -> ProfileScope {
        self.scope_owned(label.to_string())
    }

    /// Start a profiling scope for a dynamically generated operation label.
    #[must_use]
    pub fn scope_with<F>(self: &Arc<Self>, label_fn: F) -> ProfileScope
    where
        F: FnOnce() -> String,
    {
        if !self.is_enabled() {
            return ProfileScope::disabled();
        }
        self.scope_owned(label_fn())
    }

    /// Start a profiling scope for an owned operation label.
    #[must_use]
    pub fn scope_owned(self: &Arc<Self>, label: String) -> ProfileScope {
        if !self.is_enabled() {
            return ProfileScope::disabled();
        }
        ProfileScope {
            profiler: Some(self.clone()),
            label,
            started_at: Instant::now(),
        }
    }

    /// Record one completed operation duration.
    pub fn record_duration(&self, label: &str, duration: Duration) {
        if !self.is_enabled() {
            return;
        }

        let elapsed_micros = duration.as_micros();
        if let Ok(mut stats) = self.stats.write() {
            let entry = stats.entry(label.to_string()).or_default();
            entry.count += 1;
            entry.total_micros += elapsed_micros;
            entry.max_micros = entry.max_micros.max(elapsed_micros);
            entry.last_micros = elapsed_micros;
        }
    }

    /// Build a snapshot suitable for the realtime profile panel.
    #[must_use]
    pub fn snapshot(&self) -> ProfileSnapshot {
        let enabled = self.is_enabled();
        let running_for_ms = if enabled {
            self.started_at
                .read()
                .ok()
                .and_then(|started_at| *started_at)
                .map_or(0, |started_at| started_at.elapsed().as_millis() as u64)
        } else {
            0
        };

        let mut operations = self
            .stats
            .read()
            .map(|stats| {
                stats
                    .iter()
                    .map(|(name, stat)| {
                        let total_ms = stat.total_micros as f64 / 1000.0;
                        let avg_ms = if stat.count == 0 {
                            0.0
                        } else {
                            total_ms / stat.count as f64
                        };
                        let child_micros = inferred_child_total_micros(name, &stats);
                        let self_total_micros = stat.total_micros.saturating_sub(child_micros);
                        let self_total_ms = self_total_micros as f64 / 1000.0;
                        let self_avg_ms = if stat.count == 0 {
                            0.0
                        } else {
                            self_total_ms / stat.count as f64
                        };
                        ProfileOperationSnapshot {
                            name: name.clone(),
                            count: stat.count,
                            total_ms,
                            avg_ms,
                            self_total_ms,
                            self_avg_ms,
                            max_ms: stat.max_micros as f64 / 1000.0,
                            last_ms: stat.last_micros as f64 / 1000.0,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        operations.sort_by(|a, b| {
            b.self_total_ms
                .partial_cmp(&a.self_total_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    b.total_ms
                        .partial_cmp(&a.total_ms)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| {
                    b.self_avg_ms
                        .partial_cmp(&a.self_avg_ms)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| {
                    b.avg_ms
                        .partial_cmp(&a.avg_ms)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.name.cmp(&b.name))
        });
        let total_samples = operations.iter().map(|op| op.count).sum();

        ProfileSnapshot {
            enabled,
            running_for_ms,
            total_samples,
            operations,
        }
    }
}

fn total_for_label(stats: &HashMap<String, ProfileOperationStats>, label: &str) -> u128 {
    stats.get(label).map_or(0, |stat| stat.total_micros)
}

fn total_for_prefix(
    stats: &HashMap<String, ProfileOperationStats>,
    prefix: &str,
    exclude: &str,
) -> u128 {
    stats
        .iter()
        .filter(|(name, _)| name.starts_with(prefix) && name.as_str() != exclude)
        .map(|(_, stat)| stat.total_micros)
        .sum()
}

fn inferred_child_total_micros(
    label: &str,
    stats: &HashMap<String, ProfileOperationStats>,
) -> u128 {
    match label {
        "loop.step.total" => {
            total_for_label(stats, "loop.step.setup")
                + total_for_label(stats, "loop.llm.total")
                + total_for_label(stats, "loop.response.process")
                + total_for_label(stats, "loop.no_tool_decision")
                + total_for_label(stats, "loop.tool_phase.total")
                + total_for_label(stats, "loop.background.total")
                + total_for_label(stats, "storage.assistant_interim.update")
        }
        "loop.llm.total" => {
            total_for_label(stats, "loop.llm.backoff_sleep")
                + total_for_label(stats, "loop.llm.create_stream")
                + total_for_label(stats, "loop.llm.stream")
        }
        "loop.llm.stream" => {
            total_for_label(stats, "loop.llm.first_event_wait")
                + total_for_label(stats, "loop.llm.wait_next_event")
                + total_for_prefix(stats, "loop.llm.handle.", label)
        }
        "loop.response.process" => total_for_prefix(stats, "loop.response.", label),
        "loop.no_tool_decision" => total_for_prefix(stats, "loop.no_tool_decision.", label),
        "loop.tool_phase.total" => {
            total_for_prefix(stats, "loop.tool_phase.", label)
                + total_for_prefix(stats, "tool.total:", label)
        }
        "loop.background.total" => total_for_prefix(stats, "loop.background.", label),
        _ if label.starts_with("tool.total:") => {
            let suffix = &label["tool.total:".len()..];
            total_for_label(stats, &format!("tool.pre_hooks:{suffix}"))
                + total_for_label(stats, &format!("tool.permission:{suffix}"))
                + total_for_label(stats, &format!("tool.execute:{suffix}"))
                + total_for_label(stats, &format!("tool.post_hooks:{suffix}"))
        }
        _ => 0,
    }
}

/// RAII profiling scope that records elapsed time on drop.
#[derive(Debug)]
pub struct ProfileScope {
    profiler: Option<Arc<AgentLoopProfiler>>,
    label: String,
    started_at: Instant,
}

impl ProfileScope {
    fn disabled() -> Self {
        Self {
            profiler: None,
            label: String::new(),
            started_at: Instant::now(),
        }
    }
}

impl Drop for ProfileScope {
    fn drop(&mut self) {
        if let Some(profiler) = &self.profiler {
            profiler.record_duration(&self.label, self.started_at.elapsed());
        }
    }
}

static AGENT_LOOP_PROFILER: OnceLock<Arc<AgentLoopProfiler>> = OnceLock::new();

/// Return the process-wide agent-loop profiler instance.
#[must_use]
pub fn agent_loop_profiler() -> Arc<AgentLoopProfiler> {
    AGENT_LOOP_PROFILER
        .get_or_init(|| Arc::new(AgentLoopProfiler::new()))
        .clone()
}
