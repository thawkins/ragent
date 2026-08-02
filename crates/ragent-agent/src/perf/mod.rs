//! Agent-loop performance subsystem.
//!
//! Centralises the runtime configuration and helpers used by the
//! `AgentPerf` specification (`specs/AgentPerf/SPEC.md`):
//!
//! * [`is_profiling_enabled`] — resolves the `RAGENT_AGENT_PERF` environment
//!   variable (and, when present, the `agent_perf.profiling` config field) and
//!   caches the result for cheap repeated lookups.
//! * [`agent_perf_enabled`] — master switch consulted by all performance
//!   optimisations; allows the entire subsystem to be turned off without
//!   recompiling.
//!
//! All of these helpers are deliberately tiny and side-effect-free: the
//! agent action loop consults them on every iteration, so they must be
//! branch-prediction friendly and lock-free.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// State of the resolved profiling flag.
///
/// Stored in an `AtomicU8` so that the lookup is a single relaxed load on
/// every read, and so the three states (unset/env/config) can be observed
/// in a single memory location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum ProfilingState {
    /// No environment variable, no config, no runtime override.
    Unset = 0,
    /// Profiling is enabled (env var, config, or runtime override).
    Enabled = 1,
    /// Profiling is explicitly disabled.
    Disabled = 2,
}

impl ProfilingState {
    fn to_bool(self) -> Option<bool> {
        match self {
            Self::Unset => None,
            Self::Enabled => Some(true),
            Self::Disabled => Some(false),
        }
    }
}

static PROFILING_STATE: AtomicU8 = AtomicU8::new(ProfilingState::Unset as u8);
static MASTER_ENABLED: AtomicBool = AtomicBool::new(true);
static PROFILING_OVERRIDE: AtomicBool = AtomicBool::new(false);
/// `true` when an explicit override value is currently installed
/// (regardless of whether the override itself is `true` or `false`).
/// Cleared only by `set_profiling_override(None)`.
static PROFILING_OVERRIDE_INSTALLED: AtomicBool = AtomicBool::new(false);
/// Cached config-derived value, used to restore the state when the runtime
/// override is cleared.  `Unset` means "never set from config".
static CONFIG_BACKUP: AtomicU8 = AtomicU8::new(ProfilingState::Unset as u8);

const ENV_VAR: &str = "RAGENT_AGENT_PERF";

/// Initialise the perf subsystem from the process environment.
///
/// Reads the `RAGENT_AGENT_PERF` environment variable and updates the
/// cached state.  Idempotent: calling more than once is cheap and has no
/// surprising side effects.  This is a no-op if a runtime override is
/// currently installed.
pub fn init_from_env() {
    if profiling_override_installed() {
        return;
    }
    let state = match std::env::var(ENV_VAR) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" | "enable" | "enabled" => ProfilingState::Enabled,
            "0" | "false" | "no" | "off" | "disable" | "disabled" => ProfilingState::Disabled,
            // Unknown values fall back to "disabled" — fail closed.
            _ => ProfilingState::Disabled,
        },
        Err(_) => ProfilingState::Unset,
    };
    PROFILING_STATE.store(state as u8, Ordering::Relaxed);
}

/// Set the master enable flag for the entire perf subsystem.
///
/// When `false`, every performance optimisation short-circuits.  This is
/// the runtime equivalent of `agent_perf.enabled: false` in `ragent.json`.
pub fn set_master_enabled(enabled: bool) {
    MASTER_ENABLED.store(enabled, Ordering::Relaxed);
}

/// Return whether the perf subsystem is enabled.
///
/// Defaults to `true`; consults [`set_master_enabled`] for runtime overrides.
#[must_use]
pub fn agent_perf_enabled() -> bool {
    MASTER_ENABLED.load(Ordering::Relaxed)
}

/// Set the runtime profiling override.
///
/// When `Some(true)` or `Some(false)`, the override takes precedence over
/// the environment variable and config.  When `None`, the override is
/// cleared and the cached state falls back to whatever `init_from_env` or
/// [`set_profiling_from_config`] last set.
pub fn set_profiling_override(value: Option<bool>) {
    match value {
        Some(true) => {
            PROFILING_OVERRIDE.store(true, Ordering::Relaxed);
            PROFILING_OVERRIDE_INSTALLED.store(true, Ordering::Relaxed);
            PROFILING_STATE.store(ProfilingState::Enabled as u8, Ordering::Relaxed);
        }
        Some(false) => {
            PROFILING_OVERRIDE.store(false, Ordering::Relaxed);
            PROFILING_OVERRIDE_INSTALLED.store(true, Ordering::Relaxed);
            PROFILING_STATE.store(ProfilingState::Disabled as u8, Ordering::Relaxed);
        }
        None => {
            PROFILING_OVERRIDE.store(false, Ordering::Relaxed);
            PROFILING_OVERRIDE_INSTALLED.store(false, Ordering::Relaxed);
            // Restore the most recent config-derived value (or the
            // environment, if no config has been set).
            let backup = CONFIG_BACKUP.load(Ordering::Relaxed);
            if backup == ProfilingState::Unset as u8 {
                PROFILING_STATE.store(ProfilingState::Unset as u8, Ordering::Relaxed);
                init_from_env();
            } else {
                PROFILING_STATE.store(backup, Ordering::Relaxed);
            }
        }
    }
}

/// Return whether the runtime profiling override is currently active.
///
/// "Active" here means an override has been explicitly installed (with
/// `Some(_)`); a cleared override (set with `None`) returns `false`.
#[must_use]
pub fn profiling_override_active() -> bool {
    PROFILING_OVERRIDE_INSTALLED.load(Ordering::Relaxed)
}

/// Internal helper: return whether an override is currently installed.
fn profiling_override_installed() -> bool {
    PROFILING_OVERRIDE_INSTALLED.load(Ordering::Relaxed)
}

/// Set the profiling flag from the resolved `ragent.json` config.
///
/// `enabled` is the value of the `agent_perf.profiling` field.  If the
/// override is active, this is a no-op so the operator's runtime
/// decision always wins.  The value is also cached so that clearing the
/// runtime override restores the most recent config-derived value.
pub fn set_profiling_from_config(enabled: bool) {
    let state = if enabled {
        ProfilingState::Enabled
    } else {
        ProfilingState::Disabled
    };
    CONFIG_BACKUP.store(state as u8, Ordering::Relaxed);
    if profiling_override_active() {
        return;
    }
    PROFILING_STATE.store(state as u8, Ordering::Relaxed);
}

/// Return whether per-scope profiling logs are currently enabled.
///
/// Resolution order (first match wins):
/// 1. Runtime override (set via [`set_profiling_override`]).
/// 2. Cached state from [`init_from_env`] / [`set_profiling_from_config`].
/// 3. `false` (fail closed).
#[must_use]
pub fn is_profiling_enabled() -> bool {
    let raw = PROFILING_STATE.load(Ordering::Relaxed);
    let state = match raw {
        1 => ProfilingState::Enabled,
        2 => ProfilingState::Disabled,
        _ => ProfilingState::Unset,
    };
    state.to_bool().unwrap_or(false)
}

/// Diagnostic helper used by tests: returns the raw env-var value if set.
#[must_use]
pub fn env_var_name() -> &'static str {
    ENV_VAR
}
#[cfg(test)]
mod tests {
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
}
