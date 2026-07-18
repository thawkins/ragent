//! Regression tests for status-bar context and stream byte metrics.

use ragent_agent::event::Event;
use ragent_tui::app::{ConfiguredProvider, ProviderSource};

#[path = "support/mod.rs"]
mod support;

#[test]
fn test_usage_display_shows_pct_then_context_window_size() {
    let mut app = support::make_app();
    app.configured_provider = Some(ConfiguredProvider {
        id: "ollama_cloud".to_string(),
        name: "Ollama Cloud".to_string(),
        source: ProviderSource::Database,
    });
    app.selected_model = Some("ollama_cloud/kimi-k2.6".to_string());
    app.selected_model_ctx_window = Some(200_000);
    app.last_input_tokens = 50_000;

    let (label, unknown) = app.usage_display();

    assert!(!unknown);
    assert_eq!(label, "ctx: 25% 50K/200K");
}

#[test]
fn test_request_started_resets_inbound_and_sets_outbound_bytes() {
    let mut app = support::make_app();
    app.session_id = Some("session-1".to_string());
    app.stream_in_bytes = 321;

    app.handle_event(Event::RequestStarted {
        session_id: "session-1".to_string(),
        outbound_bytes: 4096,
    });
    app.handle_event(Event::TextDelta {
        session_id: "session-1".to_string(),
        text: "hello".to_string(),
    });

    assert_eq!(app.stream_out_bytes, 4096);
    assert_eq!(app.stream_in_bytes, 5);
}

#[test]
fn test_compression_finished_updates_last_input_tokens_and_status() {
    let mut app = support::make_app();
    app.session_id = Some("session-1".to_string());
    app.last_input_tokens = 90_000;

    app.handle_event(Event::CompressionStarted {
        session_id: "session-1".to_string(),
        reason: "test".to_string(),
    });
    assert!(app.compress_in_progress);
    assert_eq!(app.status, "compressing context...");
    assert!(app.needs_redraw);

    app.handle_event(Event::CompressionFinished {
        session_id: "session-1".to_string(),
        original_tokens: 90_000,
        compressed_tokens: 45_000,
        compression_ratio: 2.0,
        did_compress: true,
        reason: "test".to_string(),
    });
    assert!(!app.compress_in_progress);
    assert_eq!(app.last_input_tokens, 45_000);
    assert!(app.status.contains("saved 45000 tokens"));
    assert!(app.needs_redraw);
}

#[test]
fn test_compression_finished_no_change_updates_status() {
    let mut app = support::make_app();
    app.session_id = Some("session-1".to_string());
    app.last_input_tokens = 1_000;

    app.handle_event(Event::CompressionFinished {
        session_id: "session-1".to_string(),
        original_tokens: 1_000,
        compressed_tokens: 1_000,
        compression_ratio: 1.0,
        did_compress: false,
        reason: "test".to_string(),
    });
    assert!(!app.compress_in_progress);
    assert_eq!(app.last_input_tokens, 1_000);
    assert!(app.status.contains("compress: no change"));
}

#[test]
fn test_compression_events_ignored_for_other_session() {
    let mut app = support::make_app();
    app.session_id = Some("session-1".to_string());

    app.handle_event(Event::CompressionStarted {
        session_id: "session-2".to_string(),
        reason: "test".to_string(),
    });
    assert!(!app.compress_in_progress);
}
