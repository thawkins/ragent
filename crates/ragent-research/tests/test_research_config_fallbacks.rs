//! Regression tests for #39: `research.models.*`,
//! `research.supervisor.max_concurrent_research_units` and
//! `research.evaluate.enabled` in `ragent.json` apply to a run when the
//! per-run field is unset, and a per-run value always wins.

use ragent_research::{ResearchRunRequest, build_session_config};

fn configured() -> ragent_config::Config {
    let mut config = ragent_config::Config::default();
    config.research.models.research_model = Some("cfg/research".to_string());
    config.research.models.compression_model = Some("cfg/compress".to_string());
    config.research.models.final_report_model = Some("cfg/final".to_string());
    config.research.supervisor.max_concurrent_research_units = 9;
    config.research.evaluate.enabled = true;
    config
}

#[test]
fn test_research_config_applies_when_request_is_unset() {
    let req = ResearchRunRequest::new("n", "topic");
    let cfg = build_session_config(&req, Some(&configured()));
    assert_eq!(cfg.models.research_model.as_deref(), Some("cfg/research"));
    assert_eq!(
        cfg.models.compression_model.as_deref(),
        Some("cfg/compress")
    );
    assert_eq!(cfg.models.final_report_model.as_deref(), Some("cfg/final"));
    assert_eq!(cfg.engine.max_concurrent_research_units, 9);
    assert!(cfg.evaluate);
}

#[test]
fn test_research_request_values_win_over_config() {
    let mut req = ResearchRunRequest::new("n", "topic");
    req.research_model = Some("run/research".to_string());
    req.compression_model = Some("run/compress".to_string());
    req.final_report_model = Some("run/final".to_string());
    req.max_concurrent_research_units = Some(2);
    req.evaluate = Some(false);
    let cfg = build_session_config(&req, Some(&configured()));
    assert_eq!(cfg.models.research_model.as_deref(), Some("run/research"));
    assert_eq!(
        cfg.models.compression_model.as_deref(),
        Some("run/compress")
    );
    assert_eq!(cfg.models.final_report_model.as_deref(), Some("run/final"));
    assert_eq!(cfg.engine.max_concurrent_research_units, 2);
    assert!(!cfg.evaluate);
}

#[test]
fn test_research_defaults_without_config() {
    let req = ResearchRunRequest::new("n", "topic");
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.models.research_model, None);
    assert_eq!(
        cfg.engine.max_concurrent_research_units,
        ragent_research::supervisor::DEFAULT_MAX_CONCURRENT_RESEARCH_UNITS
    );
    assert!(!cfg.evaluate);
}
