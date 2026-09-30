//! Inline tests for `run_request.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::session::DEFAULT_WEB_PHASE_TIMEOUT_SECS;

#[test]
fn default_request_has_no_subject() {
    let req = ResearchRunRequest::default();
    assert!(req.missing_subject());
}

#[test]
fn request_with_topic_has_subject() {
    let req = ResearchRunRequest::new("rust", "Rust programming language");
    assert!(!req.missing_subject());
    assert_eq!(req.name, "rust");
    assert_eq!(req.topic, "Rust programming language");
}

#[test]
fn build_session_config_applies_defaults() {
    let req = ResearchRunRequest::new("test", "a topic");
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.input.topic, "a topic");
    assert_eq!(cfg.engine.tier, Tier::Full);
    assert_eq!(cfg.output.output_format, OutputFormat::Report);
    // Default request has use_local=false / use_specs=false, which map to
    // disable_local=true / disable_specs=true in SessionConfig.
    assert!(cfg.local.disable_local);
    assert!(cfg.local.disable_specs);
    assert!(!cfg.resilience.open_access_recovery);
    assert_eq!(
        cfg.resilience.oa_min_full_text_chars,
        DEFAULT_OA_MIN_FULL_TEXT_CHARS
    );
}

#[test]
fn build_session_config_parses_tier_and_format() {
    let req = ResearchRunRequest {
        name: "test".into(),
        topic: "topic".into(),
        tier: Some("light".into()),
        output_format: Some("imrad".into()),
        use_local: true,
        use_specs: true,
        ..ResearchRunRequest::default()
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.engine.tier, Tier::Light);
    assert_eq!(cfg.output.output_format, OutputFormat::Imrad);
    assert!(!cfg.local.disable_local);
    assert!(!cfg.local.disable_specs);
}

#[test]
fn build_session_config_parses_mode() {
    let req = ResearchRunRequest {
        name: "test".into(),
        topic: "topic".into(),
        mode: Some("competitive".into()),
        ..ResearchRunRequest::default()
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(cfg.engine.mode, ResearchMode::Competitive);
}

#[test]
fn nested_defaults_match_legacy_flat_defaults() {
    let cfg = SessionConfig::default();
    assert!(cfg.input.topic.is_empty());
    assert!(cfg.input.sources_dir.is_none());
    assert!(cfg.input.from_urls.is_empty());
    assert!(cfg.input.from_files.is_empty());
    assert!(cfg.output.template.is_none());
    assert_eq!(cfg.output.output_format, OutputFormat::Report);
    // Default config uses the 0 sentinel = derive the web budget from
    // the selected depth (`SessionConfig::effective_web_budget`).
    assert_eq!(cfg.web.max_web_results, 0);
    assert_eq!(cfg.web.fetch_concurrency, DEFAULT_FETCH_CONCURRENCY);
    assert_eq!(cfg.web.fetch_timeout_secs, 30);
    assert!(!cfg.web.use_low_relevance);
    assert!(!cfg.web.disable_scholarly);
    assert!(!cfg.web.use_pdf_web_sources);
    assert_eq!(
        cfg.web.web_phase_timeout_secs,
        Some(DEFAULT_WEB_PHASE_TIMEOUT_SECS)
    );
    assert_eq!(cfg.local.max_local_sources, 10);
    assert!(!cfg.local.disable_local);
    assert!(!cfg.local.disable_specs);
    assert_eq!(cfg.local.local_concurrency, DEFAULT_LOCAL_CONCURRENCY);
    assert!(cfg.local.local_phase_timeout_secs.is_none());
    assert!(cfg.analysis.depth.is_none());
    assert!(cfg.analysis.iterations.is_none());
    assert!(cfg.analysis.max_synthesis_sources.is_none());
    assert_eq!(
        cfg.resilience.search_max_retries,
        DEFAULT_SEARCH_MAX_RETRIES
    );
    assert_eq!(
        cfg.resilience.search_retry_base_delay_ms,
        DEFAULT_SEARCH_RETRY_BASE_DELAY_MS
    );
    assert!(!cfg.resilience.open_access_recovery);
    assert!(cfg.resilience.contact_email.is_none());
    assert_eq!(
        cfg.resilience.oa_min_full_text_chars,
        DEFAULT_OA_MIN_FULL_TEXT_CHARS
    );
    assert_eq!(cfg.engine.tier, Tier::Full);
}

#[test]
fn build_session_config_zero_web_phase_timeout_disables_deadline() {
    let req = ResearchRunRequest {
        name: "test".into(),
        topic: "topic".into(),
        web_phase_timeout_secs: Some(0),
        ..ResearchRunRequest::default()
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(
        cfg.web.web_phase_timeout_secs,
        Some(0),
        "Some(0) must be preserved as the disabled-deadline sentinel"
    );
}

#[test]
fn build_session_config_default_web_phase_timeout_is_60() {
    let req = ResearchRunRequest::new("test", "topic");
    let cfg = build_session_config(&req, None);
    assert_eq!(
        cfg.web.web_phase_timeout_secs,
        Some(DEFAULT_WEB_PHASE_TIMEOUT_SECS),
        "default web_phase_timeout must be 180 seconds (NFR-003)"
    );
}

#[test]
fn build_session_config_parses_all_modes() {
    for (raw, expected) in [
        ("tiered", ResearchMode::Tiered),
        ("supervisor", ResearchMode::Supervisor),
        ("competitive", ResearchMode::Competitive),
    ] {
        let req = ResearchRunRequest {
            name: "test".into(),
            topic: "topic".into(),
            mode: Some(raw.into()),
            ..ResearchRunRequest::default()
        };
        let cfg = build_session_config(&req, None);
        assert_eq!(
            cfg.engine.mode, expected,
            "`--mode {raw}` must parse to {expected:?}"
        );
    }
}

/// Research items are discovered by directory under `<research_root>/`
/// regardless of research mode, so the RESEARCH.md write path is the same
/// for `tiered` (default `/research create`) and the supervisor/competitive
/// `--mode` runs. Pin that invariant so a mode branch can never drift to a
/// different output folder.
#[test]
fn research_md_path_is_mode_independent() {
    let root = std::path::Path::new("research");
    let name = crate::research_name::ResearchName::try_new("mode-output").expect("valid name");
    let path = crate::io::ResearchIo::research_md_path(root, &name);
    assert_eq!(
        path,
        root.join("mode-output").join("RESEARCH.md"),
        "RESEARCH.md must always be written under <research_root>/<name>/"
    );
}
