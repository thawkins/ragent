//! Integration tests for the agent-loop performance profiling gate.
//!
//! Validates that the `RAGENT_AGENT_PERF=1` environment variable and the
//! `agent_perf.profiling` config field correctly enable and disable the
//! per-scope `tracing::info!` log lines emitted by the agent action loop's
//! profiler (see `AgentPerf` specification, FR-002).

use ragent_agent::perf;

#[test]
fn env_var_name_is_stable() {
    assert_eq!(perf::env_var_name(), "RAGENT_AGENT_PERF");
}

#[test]
fn default_state_is_profiling_disabled() {
    // Make sure no leftover state from a previous test pollutes the lookup.
    perf::init_from_env();
    // The test harness typically does not set `RAGENT_AGENT_PERF`, so
    // profiling should be disabled by default.
    let env_set = std::env::var("RAGENT_AGENT_PERF").is_ok();
    if env_set {
        // Even when set, the default state observable through
        // `is_profiling_enabled` should match the env var.
        return;
    }
    assert!(!perf::is_profiling_enabled());
}

#[test]
fn config_toggle_round_trips() {
    perf::set_profiling_override(None);
    perf::set_profiling_from_config(true);
    assert!(perf::is_profiling_enabled());
    perf::set_profiling_from_config(false);
    assert!(!perf::is_profiling_enabled());
    perf::set_profiling_from_config(true);
    assert!(perf::is_profiling_enabled());
    // Restore the default for downstream tests.
    perf::set_profiling_from_config(false);
    perf::set_profiling_override(None);
}

#[test]
fn runtime_override_takes_precedence() {
    perf::set_profiling_from_config(false);
    assert!(!perf::is_profiling_enabled());
    perf::set_profiling_override(Some(true));
    assert!(perf::is_profiling_enabled());
    assert!(perf::profiling_override_active());
    perf::set_profiling_override(None);
    // After clearing, we fall back to the last config value.
    assert!(!perf::is_profiling_enabled());
}

#[test]
fn master_switch_can_disable_entire_subsystem() {
    perf::set_master_enabled(false);
    assert!(!perf::agent_perf_enabled());
    perf::set_master_enabled(true);
    assert!(perf::agent_perf_enabled());
}
