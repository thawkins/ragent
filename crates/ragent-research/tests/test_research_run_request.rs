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
    run_config::{Depth, OutputFormat, ResearchMode, Tier},
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
    // Default config uses the 0 sentinel = derive the effective budget from
    // the selected depth (SessionConfig::effective_web_budget).
    assert_eq!(cfg.web.max_web_results, 0);
    assert!(cfg.web.fetch_concurrency > 0);
    assert!(cfg.web.fetch_timeout_secs > 0);
    assert!(!cfg.web.use_low_relevance);
    assert!(!cfg.web.disable_scholarly);
    assert!(!cfg.web.use_pdf_web_sources);
    assert!(cfg.web.max_search_calls.is_none());
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

// ── Explicit overrides ─────────────────────────────────────────────────

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
        mode: Some("supervisor".to_string()),
        clarify: Some(true),
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
        max_search_calls: Some(40),
        max_local_sources: Some(30),
        max_synthesis_sources: Some(15),
        summarization_model: Some("openai:gpt-4.1-nano".to_string()),
        brief: Some("A generated research brief".to_string()),
        research_model: Some("openai:gpt-4.1".to_string()),
        compression_model: Some("openai:gpt-4.1-mini".to_string()),
        final_report_model: Some("anthropic:claude-sonnet-4".to_string()),
        max_concurrent_research_units: Some(7),
        evaluate: Some(true),
        invocation: Some("ragent research create --name full-test \"deep topic\"".to_string()),
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
    assert_eq!(cfg.web.max_search_calls, Some(40));

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
    assert_eq!(cfg.engine.mode, ResearchMode::Supervisor);
    assert_eq!(cfg.engine.max_concurrent_research_units, 7);

    // Brief and models (FR-004, FR-013)
    assert_eq!(cfg.brief.as_deref(), Some("A generated research brief"));
    assert_eq!(cfg.models.research_model.as_deref(), Some("openai:gpt-4.1"));
    assert_eq!(
        cfg.models.compression_model.as_deref(),
        Some("openai:gpt-4.1-mini")
    );
    assert_eq!(
        cfg.models.final_report_model.as_deref(),
        Some("anthropic:claude-sonnet-4")
    );

    // Invocation frontmatter replay value
    assert_eq!(
        cfg.invocation.as_deref(),
        Some("ragent research create --name full-test \"deep topic\"")
    );
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
        "explicit 0 web_phase_timeout_secs must disable the deadline"
    );
}

#[test]
fn build_session_config_parses_tier_light() {
    let req = ResearchRunRequest {
        tier: Some("light".to_string()),
        ..ResearchRunRequest::new("light-test", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.engine.tier, Tier::Light);
}

#[test]
fn build_session_config_parses_tier_full() {
    let req = ResearchRunRequest {
        tier: Some("full".to_string()),
        ..ResearchRunRequest::new("full-test", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.engine.tier, Tier::Full);
}

#[test]
fn build_session_config_parses_tier_dissertation() {
    let req = ResearchRunRequest {
        tier: Some("dissertation".to_string()),
        ..ResearchRunRequest::new("diss-test", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.engine.tier, Tier::Dissertation);
}

#[test]
fn build_session_config_invalid_tier_falls_back_to_full() {
    let req = ResearchRunRequest {
        tier: Some("unknown".to_string()),
        ..ResearchRunRequest::new("bad-tier", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.engine.tier, Tier::Full);
}

#[test]
fn build_session_config_parses_output_format_executive_summary() {
    let req = ResearchRunRequest {
        output_format: Some("executive-summary".to_string()),
        ..ResearchRunRequest::new("fmt-exec", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.output.output_format, OutputFormat::ExecutiveSummary);
}

#[test]
fn build_session_config_parses_output_format_comparison_table() {
    let req = ResearchRunRequest {
        output_format: Some("comparison-table".to_string()),
        ..ResearchRunRequest::new("fmt-cmp", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.output.output_format, OutputFormat::ComparisonTable);
}

#[test]
fn build_session_config_parses_output_format_source_bibliography() {
    let req = ResearchRunRequest {
        output_format: Some("source-bibliography".to_string()),
        ..ResearchRunRequest::new("fmt-bib", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.output.output_format, OutputFormat::SourceBibliography);
}

#[test]
fn build_session_config_parses_output_format_imrad() {
    let req = ResearchRunRequest {
        output_format: Some("imrad".to_string()),
        ..ResearchRunRequest::new("fmt-imrad", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.output.output_format, OutputFormat::Imrad);
}

#[test]
fn build_session_config_invalid_format_falls_back_to_report() {
    let req = ResearchRunRequest {
        output_format: Some("nonsense".to_string()),
        ..ResearchRunRequest::new("bad-fmt", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.output.output_format, OutputFormat::Report);
}

#[test]
fn build_session_config_parses_depth_shallow() {
    let req = ResearchRunRequest {
        depth: Some("shallow".to_string()),
        ..ResearchRunRequest::new("depth-shallow", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.analysis.depth, Some(Depth::Shallow));
}

#[test]
fn build_session_config_parses_depth_standard() {
    let req = ResearchRunRequest {
        depth: Some("standard".to_string()),
        ..ResearchRunRequest::new("depth-standard", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.analysis.depth, Some(Depth::Standard));
}

#[test]
fn build_session_config_parses_depth_deep() {
    let req = ResearchRunRequest {
        depth: Some("deep".to_string()),
        ..ResearchRunRequest::new("depth-deep", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.analysis.depth, Some(Depth::Deep));
}

#[test]
fn build_session_config_invalid_depth_is_none() {
    let req = ResearchRunRequest {
        depth: Some("unknown".to_string()),
        ..ResearchRunRequest::new("bad-depth", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert!(cfg.analysis.depth.is_none());
}

#[test]
fn missing_subject_true_when_no_topic_no_urls_no_files() {
    let req = ResearchRunRequest::default();
    assert!(req.missing_subject());
}

#[test]
fn missing_subject_false_when_topic_present() {
    let req = ResearchRunRequest::new("x", "topic");
    assert!(!req.missing_subject());
}

#[test]
fn missing_subject_false_when_from_urls_present() {
    let req = ResearchRunRequest {
        from_urls: vec!["https://example.com".to_string()],
        ..ResearchRunRequest::default()
    };
    assert!(!req.missing_subject());
}

#[test]
fn missing_subject_false_when_from_files_present() {
    let req = ResearchRunRequest {
        from_files: vec!["paper.pdf".to_string()],
        ..ResearchRunRequest::default()
    };
    assert!(!req.missing_subject());
}

#[test]
fn build_session_config_reads_oa_settings_from_app_config() {
    let mut cfg = Config::default();
    cfg.research = ragent_config::ResearchConfig {
        open_access_recovery: true,
        contact_email: Some("user@example.com".to_string()),
        oa_min_full_text_chars: 250,
        ..Default::default()
    };
    let req = ResearchRunRequest::new("oa", "topic");
    let session = build_session_config(&req, Some(&cfg));
    assert!(session.resilience.open_access_recovery);
    assert_eq!(
        session.resilience.contact_email.as_deref(),
        Some("user@example.com")
    );
    assert_eq!(session.resilience.oa_min_full_text_chars, 250);
}

#[test]
fn build_session_config_oa_disabled_when_no_app_config() {
    let req = ResearchRunRequest::new("no-oa", "topic");
    let cfg = build_session_config(&req, None);
    assert!(!cfg.resilience.open_access_recovery);
}

#[test]
fn session_event_json_returns_pure_json_without_prefix() {
    let json =
        ragent_research::cli::session_event_json(&ragent_research::session::SessionEvent::Phase {
            phase: ragent_research::session::SessionPhase::Web,
        });
    assert!(json.contains("\"phase\""));
    assert!(!json.starts_with("data: "));
}

#[test]
fn render_session_event_json_wraps_session_event_json_with_prefix() {
    let rendered = ragent_research::cli::render_session_event_json(
        &ragent_research::session::SessionEvent::Phase {
            phase: ragent_research::session::SessionPhase::Web,
        },
    );
    assert!(rendered.starts_with("ragent-research: "));
}

#[test]
fn session_event_json_config_snapshot_is_valid_json() {
    let req = ResearchRunRequest::new("cfg-snap", "topic");
    let cfg = build_session_config(&req, None);
    let event = ragent_research::session::SessionEvent::ConfigSnapshot {
        mode: cfg.engine.mode.as_str().to_string(),
        output_format: cfg.output.output_format.as_str().to_string(),
        depth: cfg.analysis.depth.map(|d| d.as_str().to_string()),
        iterations: cfg.analysis.iterations,
        tier: Some(cfg.engine.tier.as_str().to_string()),
        from_urls: cfg.input.from_urls.clone(),
        from_files: cfg
            .input
            .from_files
            .iter()
            .map(|p| p.display().to_string())
            .collect(),
    };
    let json = ragent_research::cli::session_event_json(&event);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("must be valid JSON");
    assert_eq!(
        parsed
            .get("payload")
            .and_then(|c| c.get("output_format"))
            .and_then(|v| v.as_str()),
        Some("report")
    );
}
// ── Competitive mode implies comparison-table format ─────────────────────

#[test]
fn build_session_config_competitive_defaults_to_comparison_table() {
    // `--mode competitive` without `--format` implies `--format comparison-table`.
    let req = ResearchRunRequest {
        mode: Some("competitive".to_string()),
        ..ResearchRunRequest::new("comp-default", "Compare A and B")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.output.output_format, OutputFormat::ComparisonTable);
    assert_eq!(cfg.engine.mode, ResearchMode::Competitive);
}

#[test]
fn build_session_config_competitive_explicit_format_wins() {
    // An explicit `--format` always overrides the competitive default.
    let req = ResearchRunRequest {
        mode: Some("competitive".to_string()),
        output_format: Some("imrad".to_string()),
        ..ResearchRunRequest::new("comp-override", "Compare A and B")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.output.output_format, OutputFormat::Imrad);
    assert_eq!(cfg.engine.mode, ResearchMode::Competitive);
}

#[test]
fn build_session_config_non_competitive_modes_still_default_to_report() {
    // The competitive default must not leak into tiered/supervisor runs.
    for (raw, expected) in [
        ("tiered", ResearchMode::Tiered),
        ("supervisor", ResearchMode::Supervisor),
    ] {
        let req = ResearchRunRequest {
            mode: Some(raw.to_string()),
            ..ResearchRunRequest::new("no-cmp-default", "topic")
        };
        let cfg = build_session_config(&req, None);
        assert_eq!(cfg.output.output_format, OutputFormat::Report);
        assert_eq!(cfg.engine.mode, expected);
    }
}

#[test]
fn build_session_config_no_mode_still_defaults_to_report() {
    // Plain create runs without `--mode` keep the report default.
    let req = ResearchRunRequest::new("plain-default", "topic");
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.output.output_format, OutputFormat::Report);
    assert_eq!(cfg.engine.mode, ResearchMode::Tiered);
}
