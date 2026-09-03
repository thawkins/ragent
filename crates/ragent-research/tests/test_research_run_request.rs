#![allow(clippy::assert_is_empty)]
//! Tests for the shared [`ResearchRunRequest`] / [`build_session_config`] builder
//! (RESEARCHPLAN.md Phase 4 — R-001/R-032 convergence).
//!
//! All three front-ends (CLI, TUI, HTTP) construct a `ResearchRunRequest` and
//! call `build_session_config` to produce a `SessionConfig`. These tests
//! verify that the builder correctly maps every field and applies the
//! documented defaults, so the three front-ends produce equivalent configs
//! from the same input fixture.

use ragent_config::Config;
use ragent_research::{
    ResearchRunRequest, build_session_config,
    run_config::{Depth, OutputFormat, Tier},
    session::DEFAULT_WEB_PHASE_TIMEOUT_SECS,
};

// ── Defaults ───────────────────────────────────────────────────────────

#[test]
fn build_session_config_applies_defaults_with_no_overrides() {
    let req = ResearchRunRequest::new("test-item", "test topic");
    let cfg = build_session_config(&req, None);

    // Input
    assert_eq!(cfg.input.topic, "test topic");
    assert!(cfg.input.from_urls.is_empty());
    assert!(cfg.input.from_files.is_empty());
    assert!(cfg.input.sources_dir.is_none());

    // Output
    assert_eq!(cfg.output.output_format, OutputFormat::Report);
    assert!(cfg.output.template.is_none());

    // Web
    assert!(cfg.web.max_web_results > 0);
    assert!(cfg.web.fetch_concurrency > 0);
    assert!(cfg.web.fetch_timeout_secs > 0);
    assert!(!cfg.web.use_low_relevance);
    assert!(!cfg.web.disable_scholarly);
    assert!(!cfg.web.use_pdf_web_sources);
    assert_eq!(
        cfg.web.web_phase_timeout_secs,
        Some(DEFAULT_WEB_PHASE_TIMEOUT_SECS),
        "omitted web_phase_timeout_secs must default to DEFAULT_WEB_PHASE_TIMEOUT_SECS"
    );

    // Local
    assert!(cfg.local.max_local_sources > 0);
    assert!(cfg.local.disable_local); // use_local defaults false → disabled
    assert!(cfg.local.disable_specs); // use_specs defaults false → disabled

    // Analysis
    assert!(cfg.analysis.depth.is_none());
    assert!(cfg.analysis.iterations.is_none());
    assert!(cfg.analysis.max_synthesis_sources.is_none());
    assert!(cfg.analysis.contradiction.is_none());

    // Resilience
    assert!(cfg.resilience.search_max_retries > 0);
    assert!(cfg.resilience.search_retry_base_delay_ms > 0);
    assert!(cfg.resilience.search_circuit_breaker_threshold > 0);
    assert!(!cfg.resilience.open_access_recovery);

    // Engine
    assert_eq!(cfg.engine.tier, Tier::Full);
}

// ── Explicit overrides ─────────────────────���────────────────────────────

#[test]
fn build_session_config_maps_all_explicit_fields() {
    let req = ResearchRunRequest {
        name: "full-test".to_string(),
        topic: "deep topic".to_string(),
        title: Some("Custom Title".to_string()),
        from_urls: vec!["https://example.com".to_string()],
        from_files: vec!["docs/paper.pdf".to_string()],
        sources_dir: Some("extra-sources".to_string()),
        template: Some("imrad".to_string()),
        depth: Some("deep".to_string()),
        tier: Some("dissertation".to_string()),
        iterations: Some(5),
        output_format: Some("imrad".to_string()),
        use_local: true,
        use_specs: true,
        use_low_relevance: true,
        no_scholarly: true,
        use_pdf: true,
        fetch_concurrency: Some(20),
        local_concurrency: Some(16),
        fetch_timeout_secs: Some(60),
        web_phase_timeout_secs: Some(120),
        local_phase_timeout_secs: Some(90),
        search_max_retries: Some(4),
        search_retry_base_delay_ms: Some(500),
        search_circuit_breaker_threshold: Some(5),
        max_web_results: Some(50),
        max_local_sources: Some(30),
        max_synthesis_sources: Some(15),
    };
    let cfg = build_session_config(&req, None);

    // Input
    assert_eq!(cfg.input.topic, "deep topic");
    assert_eq!(cfg.input.from_urls, vec!["https://example.com"]);
    assert_eq!(
        cfg.input.from_files,
        vec![std::path::PathBuf::from("docs/paper.pdf")]
    );
    assert_eq!(
        cfg.input.sources_dir.as_deref(),
        Some(std::path::Path::new("extra-sources"))
    );

    // Output
    assert_eq!(cfg.output.output_format, OutputFormat::Imrad);
    assert_eq!(cfg.output.template.as_deref(), Some("imrad"));

    // Web
    assert_eq!(cfg.web.max_web_results, 50);
    assert_eq!(cfg.web.fetch_concurrency, 20);
    assert_eq!(cfg.web.fetch_timeout_secs, 60);
    assert!(cfg.web.use_low_relevance);
    assert!(cfg.web.disable_scholarly);
    assert!(cfg.web.use_pdf_web_sources);
    assert_eq!(cfg.web.web_phase_timeout_secs, Some(120));

    // Local
    assert_eq!(cfg.local.max_local_sources, 30);
    assert!(!cfg.local.disable_local); // use_local=true → enabled
    assert!(!cfg.local.disable_specs); // use_specs=true → enabled
    assert_eq!(cfg.local.local_concurrency, 16);
    assert_eq!(cfg.local.local_phase_timeout_secs, Some(90));

    // Analysis
    assert_eq!(cfg.analysis.depth, Some(Depth::Deep));
    assert_eq!(cfg.analysis.iterations, Some(5));
    assert_eq!(cfg.analysis.max_synthesis_sources, Some(15));

    // Resilience
    assert_eq!(cfg.resilience.search_max_retries, 4);
    assert_eq!(cfg.resilience.search_retry_base_delay_ms, 500);
    assert_eq!(cfg.resilience.search_circuit_breaker_threshold, 5);

    // Engine
    assert_eq!(cfg.engine.tier, Tier::Dissertation);
}

#[test]
fn build_session_config_zero_web_phase_timeout_disables_deadline() {
    let req = ResearchRunRequest {
        web_phase_timeout_secs: Some(0),
        ..ResearchRunRequest::new("zero", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(
        cfg.web.web_phase_timeout_secs,
        Some(0),
        "explicit 0 must be preserved so the deadline is disabled"
    );
}

// ── Tier parsing ────────────────────────────────────────────────────────

#[test]
fn build_session_config_parses_tier_light() {
    let req = ResearchRunRequest {
        tier: Some("light".to_string()),
        ..ResearchRunRequest::new("t", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.engine.tier, Tier::Light);
}

#[test]
fn build_session_config_parses_tier_full() {
    let req = ResearchRunRequest {
        tier: Some("full".to_string()),
        ..ResearchRunRequest::new("t", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.engine.tier, Tier::Full);
}

#[test]
fn build_session_config_parses_tier_dissertation() {
    let req = ResearchRunRequest {
        tier: Some("dissertation".to_string()),
        ..ResearchRunRequest::new("t", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.engine.tier, Tier::Dissertation);
}

#[test]
fn build_session_config_invalid_tier_falls_back_to_full() {
    let req = ResearchRunRequest {
        tier: Some("nonexistent".to_string()),
        ..ResearchRunRequest::new("t", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.engine.tier, Tier::Full);
}

// ── Output format parsing ───────────────────────────────────────────────

#[test]
fn build_session_config_parses_output_format_executive_summary() {
    let req = ResearchRunRequest {
        output_format: Some("executive-summary".to_string()),
        ..ResearchRunRequest::new("t", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.output.output_format, OutputFormat::ExecutiveSummary);
}

#[test]
fn build_session_config_parses_output_format_comparison_table() {
    let req = ResearchRunRequest {
        output_format: Some("comparison-table".to_string()),
        ..ResearchRunRequest::new("t", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.output.output_format, OutputFormat::ComparisonTable);
}

#[test]
fn build_session_config_parses_output_format_source_bibliography() {
    let req = ResearchRunRequest {
        output_format: Some("source-bibliography".to_string()),
        ..ResearchRunRequest::new("t", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.output.output_format, OutputFormat::SourceBibliography);
}

#[test]
fn build_session_config_parses_output_format_imrad() {
    let req = ResearchRunRequest {
        output_format: Some("imrad".to_string()),
        ..ResearchRunRequest::new("t", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.output.output_format, OutputFormat::Imrad);
}

#[test]
fn build_session_config_invalid_format_falls_back_to_report() {
    let req = ResearchRunRequest {
        output_format: Some("unknown-format".to_string()),
        ..ResearchRunRequest::new("t", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.output.output_format, OutputFormat::Report);
}

// ── Depth parsing ────────────────────────────────────────────────────────

#[test]
fn build_session_config_parses_depth_shallow() {
    let req = ResearchRunRequest {
        depth: Some("shallow".to_string()),
        ..ResearchRunRequest::new("t", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.analysis.depth, Some(Depth::Shallow));
}

#[test]
fn build_session_config_parses_depth_standard() {
    let req = ResearchRunRequest {
        depth: Some("standard".to_string()),
        ..ResearchRunRequest::new("t", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.analysis.depth, Some(Depth::Standard));
}

#[test]
fn build_session_config_parses_depth_deep() {
    let req = ResearchRunRequest {
        depth: Some("deep".to_string()),
        ..ResearchRunRequest::new("t", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.analysis.depth, Some(Depth::Deep));
}

#[test]
fn build_session_config_invalid_depth_is_none() {
    let req = ResearchRunRequest {
        depth: Some("invalid".to_string()),
        ..ResearchRunRequest::new("t", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert!(cfg.analysis.depth.is_none());
}

// ── missing_subject ─────────────────────────────────────────────────────

#[test]
fn missing_subject_true_when_no_topic_no_urls_no_files() {
    let req = ResearchRunRequest::new("test", "");
    assert!(req.missing_subject());
}

#[test]
fn missing_subject_false_when_topic_present() {
    let req = ResearchRunRequest::new("test", "some topic");
    assert!(!req.missing_subject());
}

#[test]
fn missing_subject_false_when_from_urls_present() {
    let req = ResearchRunRequest {
        from_urls: vec!["https://example.com".to_string()],
        ..ResearchRunRequest::new("test", "")
    };
    assert!(!req.missing_subject());
}

#[test]
fn missing_subject_false_when_from_files_present() {
    let req = ResearchRunRequest {
        from_files: vec!["doc.md".to_string()],
        ..ResearchRunRequest::new("test", "")
    };
    assert!(!req.missing_subject());
}

// ── Config integration ──────────────────────────────────────────────────

#[test]
fn build_session_config_reads_oa_settings_from_app_config() {
    let mut config = Config::default();
    config.research.open_access_recovery = true;
    config.research.contact_email = Some("test@example.com".to_string());
    config.research.oa_min_full_text_chars = 5000;

    let req = ResearchRunRequest::new("test", "topic");
    let cfg = build_session_config(&req, Some(&config));

    assert!(cfg.resilience.open_access_recovery);
    assert_eq!(
        cfg.resilience.contact_email.as_deref(),
        Some("test@example.com")
    );
    assert_eq!(cfg.resilience.oa_min_full_text_chars, 5000);
}

#[test]
fn build_session_config_oa_disabled_when_no_app_config() {
    let req = ResearchRunRequest::new("test", "topic");
    let cfg = build_session_config(&req, None);

    assert!(!cfg.resilience.open_access_recovery);
    assert!(cfg.resilience.contact_email.is_none());
}

// ── session_event_json helper ───────────────────────────────────────────

#[test]
fn session_event_json_returns_pure_json_without_prefix() {
    use ragent_research::cli::session_event_json;
    use ragent_research::session::SessionEvent;

    let event = SessionEvent::Phase {
        phase: ragent_research::session::SessionPhase::Setup,
    };
    let json = session_event_json(&event);

    // Should NOT start with the CLI prefix
    assert!(!json.starts_with("ragent-research: "));
    // Should be valid JSON with kind+payload structure
    assert!(json.starts_with('{'));
    assert!(json.contains("\"kind\""));
    assert!(json.contains("\"payload\""));
}

#[test]
fn render_session_event_json_wraps_session_event_json_with_prefix() {
    use ragent_research::cli::{render_session_event_json, session_event_json};
    use ragent_research::session::SessionEvent;

    let event = SessionEvent::Phase {
        phase: ragent_research::session::SessionPhase::Setup,
    };
    let pure_json = session_event_json(&event);
    let cli_json = render_session_event_json(&event);

    assert!(cli_json.starts_with("ragent-research: "));
    assert_eq!(cli_json, format!("ragent-research: {}", pure_json));
}

#[test]
fn session_event_json_config_snapshot_is_valid_json() {
    use ragent_research::cli::session_event_json;
    use ragent_research::run_config::{Depth, OutputFormat};
    use ragent_research::session::SessionEvent;

    let event = SessionEvent::ConfigSnapshot {
        output_format: OutputFormat::Imrad.as_str().to_string(),
        depth: Some(Depth::Deep.as_str().to_string()),
        iterations: Some(3),
        tier: Some("full".to_string()),
        from_urls: vec!["https://example.com".to_string()],
        from_files: vec!["doc.md".to_string()],
    };
    let json = session_event_json(&event);

    // Parse it as JSON to verify validity
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["kind"], "config");
    assert_eq!(parsed["payload"]["output_format"], "imrad");
    assert_eq!(parsed["payload"]["depth"], "deep");
    assert_eq!(parsed["payload"]["iterations"], 3);
    assert_eq!(parsed["payload"]["tier"], "full");
}
