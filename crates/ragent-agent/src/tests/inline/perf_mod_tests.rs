//! Relocated inline tests for `perf/mod.rs` (ANTIPAT M2 test relocation).
//!
//! The body previously lived in an inline `#[cfg(test)] mod tests` in the
//! source file; it is now compiled from this file via a `#[path]` hook.

use super::*;

/// RAII guard that restores the perf state at the end of a test.
struct StateGuard;
impl Drop for StateGuard {
    fn drop(&mut self) {
        // Reset all atomics to their defaults so each test starts
        // from a clean slate.
        PROFILING_STATE.store(ProfilingState::Unset as u8, Ordering::Relaxed);
        MASTER_ENABLED.store(true, Ordering::Relaxed);
        PROFILING_OVERRIDE.store(false, Ordering::Relaxed);
        PROFILING_OVERRIDE_INSTALLED.store(false, Ordering::Relaxed);
        CONFIG_BACKUP.store(ProfilingState::Unset as u8, Ordering::Relaxed);
    }
}

#[test]
fn default_is_disabled() {
    let _g = StateGuard;
    assert!(!is_profiling_enabled());
    assert!(agent_perf_enabled());
}

#[test]
fn master_enabled_can_be_toggled() {
    let _g = StateGuard;
    set_master_enabled(false);
    assert!(!agent_perf_enabled());
    set_master_enabled(true);
    assert!(agent_perf_enabled());
}

#[test]
fn runtime_override_wins_over_config() {
    let _g = StateGuard;
    set_profiling_from_config(true);
    assert!(is_profiling_enabled());
    set_profiling_override(Some(false));
    assert!(!is_profiling_enabled());
    set_profiling_override(None);
    // After clearing the override, we fall back to the config value.
    assert!(is_profiling_enabled());
}

#[test]
fn env_var_name_is_stable() {
    assert_eq!(env_var_name(), "RAGENT_AGENT_PERF");
}

#[test]
fn config_false_disables_profiling() {
    let _g = StateGuard;
    set_profiling_from_config(false);
    assert!(!is_profiling_enabled());
}

#[test]
fn config_true_enables_profiling() {
    let _g = StateGuard;
    set_profiling_from_config(true);
    assert!(is_profiling_enabled());
}

#[test]
fn profiling_override_round_trip() {
    let _g = StateGuard;
    assert!(!profiling_override_active());
    set_profiling_override(Some(true));
    assert!(profiling_override_active());
    assert!(is_profiling_enabled());
    set_profiling_override(Some(false));
    assert!(profiling_override_active());
    assert!(!is_profiling_enabled());
    set_profiling_override(None);
    assert!(!profiling_override_active());
}
