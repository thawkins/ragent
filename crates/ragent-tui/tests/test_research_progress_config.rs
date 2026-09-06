//! Tests for the TUI research progress encoding of the `ConfigSnapshot` event,
//! including the options-first rendering on the [`ResearchProgress`] tracker.

#[path = "support/mod.rs"]
mod support;

use ragent_research::run_config::{Depth, OutputFormat};
use ragent_research::session::SessionEvent;
use ragent_tui::research_progress::{
    ResearchProgress, SessionPhase, StepStatus, decode_progress_event, encode_progress_event,
};

#[test]
fn encode_config_snapshot_shows_mode_format_and_depth() {
    let event = SessionEvent::ConfigSnapshot {
        mode: "tiered".to_string(),
        output_format: OutputFormat::Imrad.as_str().to_string(),
        depth: Some(Depth::Standard.as_str().to_string()),
        iterations: Some(2),
        tier: None,
        from_urls: Vec::new(),
        from_files: Vec::new(),
    };
    let encoded = encode_progress_event("my-run", "topic", &event);
    let decoded = decode_progress_event(&encoded).expect("decode");
    assert_eq!(decoded.phase, SessionPhase::Setup);
    assert!(decoded.detail.contains("mode: tiered"));
    assert!(decoded.detail.contains("output format: imrad"));
    assert!(decoded.detail.contains("depth: standard"));
    assert!(decoded.detail.contains("iterations: 2"));
    assert!(!decoded.detail.contains("from-url"));
}

#[test]
fn encode_config_snapshot_sanitizes_from_urls() {
    let event = SessionEvent::ConfigSnapshot {
        mode: "competitive".to_string(),
        output_format: OutputFormat::ComparisonTable.as_str().to_string(),
        depth: None,
        iterations: None,
        tier: None,
        from_urls: vec!["https://example.com\x1b[31m".to_string()],
        from_files: Vec::new(),
    };
    let encoded = encode_progress_event("run", "topic", &event);
    let decoded = decode_progress_event(&encoded).expect("decode");
    assert!(decoded.detail.contains("mode: competitive"));
    assert!(decoded.detail.contains("output format: comparison-table"));
    assert!(decoded.detail.contains("from-url: https://example.com"));
    assert!(!decoded.detail.contains('\u{1b}'));
}

#[test]
fn config_snapshot_stored_as_options_line_not_step() {
    let event = SessionEvent::ConfigSnapshot {
        mode: "supervisor".to_string(),
        output_format: OutputFormat::Report.as_str().to_string(),
        depth: None,
        iterations: None,
        tier: Some("full".to_string()),
        from_urls: Vec::new(),
        from_files: Vec::new(),
    };
    let encoded = encode_progress_event("run", "topic", &event);
    let decoded = decode_progress_event(&encoded).expect("decode");

    let mut p = ResearchProgress::new("run", "topic");
    p.apply(decoded.phase, decoded.status, decoded.detail);
    assert!(
        p.options
            .as_deref()
            .is_some_and(|o| o.contains("mode: supervisor") && o.contains("tier: full")),
        "options should capture mode and tier: {:?}",
        p.options
    );
    assert!(
        p.steps.is_empty(),
        "config snapshot should not add a step line"
    );
}

#[test]
fn render_shows_options_line_after_topic() {
    let mut p = ResearchProgress::new("run", "topic");
    p.apply(
        SessionPhase::Setup,
        StepStatus::Done,
        "options in use: mode: tiered, output format: report",
    );
    p.apply(SessionPhase::Web, StepStatus::Started, "searching the web");
    p.finish(3, 0, 0, 0);

    let rendered = p.render();
    let topic_idx = rendered.find("Topic: topic").expect("topic line");
    let opts_idx = rendered
        .find("Options: mode: tiered")
        .expect("options line");
    let web_idx = rendered.find("web").expect("web step");
    assert!(
        topic_idx < opts_idx && opts_idx < web_idx,
        "options line should sit between Topic and the step log:\n{rendered}"
    );
    assert!(
        !rendered.contains("config"),
        "config snapshot must not render as a step line:\n{rendered}"
    );
}

#[test]
fn render_without_config_event_has_no_options_line() {
    let mut p = ResearchProgress::new("run", "topic");
    p.apply(SessionPhase::Web, StepStatus::Started, "searching the web");
    let rendered = p.render();
    assert!(
        !rendered.contains("Options:"),
        "no options line without a config snapshot:\n{rendered}"
    );
}
