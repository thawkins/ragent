//! Consolidated integration-test target: one test binary per crate.
//!
//! Each former top-level `tests/test_*.rs` file is included as a child
//! module via `#[path]`, so it keeps its own file-relative `mod` /
//! `#[path]` resolution and its `test_<file>::<fn>` test name. See
//! `AGENTS.md` ("Test Layout") and
//! `docs/reports/testconsolidation-review.md`.

/// The process-wide lock of this test binary (see AGENTS.md, "Test Layout"):
/// one binary per crate merges every former test file into one process, so a
/// file-local mutex only serialises the children that happen to share that
/// file.
pub fn global_env_mutex() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

/// Acquire [`global_env_mutex`], recovering from a poisoned lock (a panicking
/// test must not wedge every later test).
pub fn global_env_lock() -> std::sync::MutexGuard<'static, ()> {
    global_env_mutex()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[path = "test_attr_session.rs"]
mod test_attr_session;
#[path = "test_batch_export.rs"]
mod test_batch_export;
#[path = "test_cardinality_branches.rs"]
mod test_cardinality_branches;
#[path = "test_counters.rs"]
mod test_counters;
#[path = "test_counters_helpers.rs"]
mod test_counters_helpers;
#[path = "test_custom_resource_attributes.rs"]
mod test_custom_resource_attributes;
#[path = "test_disabled_zero_traffic.rs"]
mod test_disabled_zero_traffic;
#[path = "test_export_health.rs"]
mod test_export_health;
#[path = "test_http_exporter.rs"]
mod test_http_exporter;
#[path = "test_logs_export.rs"]
mod test_logs_export;
#[path = "test_malformed_endpoint.rs"]
mod test_malformed_endpoint;
#[path = "test_metric_toggles.rs"]
mod test_metric_toggles;
#[path = "test_mock_session_metrics.rs"]
mod test_mock_session_metrics;
#[path = "test_nonblocking.rs"]
mod test_nonblocking;
#[path = "test_otlp_http_endpoint.rs"]
mod test_otlp_http_endpoint;
#[path = "test_profile_duration_export.rs"]
mod test_profile_duration_export;
#[path = "test_prometheus_render.rs"]
mod test_prometheus_render;
#[path = "test_recorder_constructors.rs"]
mod test_recorder_constructors;
#[path = "test_resource_attributes.rs"]
mod test_resource_attributes;
#[path = "test_scaffold.rs"]
mod test_scaffold;
#[path = "test_sensitive.rs"]
mod test_sensitive;
#[path = "test_sensitive_data_guard.rs"]
mod test_sensitive_data_guard;
#[path = "test_shutdown.rs"]
mod test_shutdown;
#[path = "test_shutdown_signal.rs"]
mod test_shutdown_signal;
#[path = "test_sigint_flush.rs"]
mod test_sigint_flush;
#[path = "test_subsystem.rs"]
mod test_subsystem;
#[path = "test_subsystem_reconfigure.rs"]
mod test_subsystem_reconfigure;
