//! Tests for the TUI research progress encoding of the `ConfigSnapshot` event.

#[path = "support/mod.rs"]
mod support;

use ragent_research::run_config::{Depth, OutputFormat};
use ragent_research::session::SessionEvent;
use ragent_tui::research_progress::{decode_progress_event, encode_progress_event};

#[test]
fn encode_config_snapshot_shows_format_and_depth() {
    let event = SessionEvent::ConfigSnapshot {
        output_format: OutputFormat::Imrad.as_str().to_string(),
        depth: Some(Depth::Standard.as_str().to_string()),
        iterations: Some(2),
        from_url: None,
    };
    let encoded = encode_progress_event("my-run", "topic", &event);
    let decoded = decode_progress_event(&encoded).expect("decode");
    assert_eq!(decoded.phase, ragent_research::session::SessionPhase::Setup);
    assert!(decoded.detail.contains("output format: imrad"));
    assert!(decoded.detail.contains("depth: standard"));
    assert!(decoded.detail.contains("iterations: 2"));
    assert!(!decoded.detail.contains("from-url"));
}

#[test]
fn encode_config_snapshot_sanitizes_from_url() {
    let event = SessionEvent::ConfigSnapshot {
        output_format: OutputFormat::ComparisonTable.as_str().to_string(),
        depth: None,
        iterations: None,
        from_url: Some("https://example.com\x1b[31m".to_string()),
    };
    let encoded = encode_progress_event("run", "topic", &event);
    let decoded = decode_progress_event(&encoded).expect("decode");
    assert!(decoded.detail.contains("output format: comparison-table"));
    assert!(decoded.detail.contains("from-url: https://example.com"));
    assert!(!decoded.detail.contains('\u{1b}'));
}
