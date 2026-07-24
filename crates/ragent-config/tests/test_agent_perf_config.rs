//! Integration tests for the `agent_perf` config block
//! (`AgentPerf` T-019 / FR-027).

use ragent_config::{AgentPerfConfig, Config};

#[test]
fn default_agent_perf_config_matches_spec() {
    let cfg = AgentPerfConfig::default();
    assert!(cfg.enabled);
    assert!(!cfg.profiling);
    assert_eq!(cfg.step_budget_secs, 300);
    assert_eq!(cfg.stall_timeout_secs, 60);
    assert!(cfg.parallel_independent_tools);
    assert!(cfg.max_concurrent_tools >= 1);
    assert!(cfg.max_concurrent_tools <= 4);
}

#[test]
fn validate_rejects_too_short_step_budget() {
    let cfg = AgentPerfConfig {
        step_budget_secs: 1,
        ..AgentPerfConfig::default()
    };
    let problems = cfg.validate();
    assert!(!problems.is_empty());
    assert!(problems[0].contains("step_budget_secs"));
}

#[test]
fn validate_rejects_zero_concurrent_tools() {
    let cfg = AgentPerfConfig {
        max_concurrent_tools: 0,
        ..AgentPerfConfig::default()
    };
    let problems = cfg.validate();
    assert!(!problems.is_empty());
    assert!(problems[0].contains("max_concurrent_tools"));
}

#[test]
fn validate_accepts_default_config() {
    let cfg = AgentPerfConfig::default();
    assert!(cfg.validate().is_empty());
}

#[test]
fn top_level_config_parses_agent_perf_block() {
    let json = r#"{
        "agent_perf": {
            "enabled": true,
            "profiling": true,
            "step_budget_secs": 600,
            "stall_timeout_secs": 120,
            "max_concurrent_tools": 8,
            "parallel_independent_tools": false
        }
    }"#;
    let cfg: Config = serde_json::from_str(json).expect("parse");
    assert!(cfg.agent_perf.enabled);
    assert!(cfg.agent_perf.profiling);
    assert_eq!(cfg.agent_perf.step_budget_secs, 600);
    assert_eq!(cfg.agent_perf.stall_timeout_secs, 120);
    assert_eq!(cfg.agent_perf.max_concurrent_tools, 8);
    assert!(!cfg.agent_perf.parallel_independent_tools);
}

#[test]
fn top_level_config_defaults_agent_perf_when_absent() {
    let json = "{}";
    let cfg: Config = serde_json::from_str(json).expect("parse");
    assert_eq!(cfg.agent_perf, AgentPerfConfig::default());
}
