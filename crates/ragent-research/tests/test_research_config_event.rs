//! Tests for the `ConfigSnapshot` session event emitted at the start of a
//! `/research create` run.

use ragent_research::{
    cli::render_session_event_json,
    run_config::{Depth, OutputFormat},
    session::SessionEvent,
};

#[test]
fn config_snapshot_json_includes_output_format() {
    let event = SessionEvent::ConfigSnapshot {
        output_format: OutputFormat::Imrad.as_str().to_string(),
        depth: Some(Depth::Deep.as_str().to_string()),
        iterations: Some(3),
        from_urls: Vec::new(),
        from_file: None,
    };
    let rendered = render_session_event_json(&event);
    assert!(rendered.starts_with("ragent-research: "));
    assert!(rendered.contains("\"kind\":\"config\""));
    assert!(rendered.contains("\"output_format\":\"imrad\""));
    assert!(rendered.contains("\"depth\":\"deep\""));
    assert!(rendered.contains("\"iterations\":3"));
    assert!(rendered.contains("\"from_urls\":[]"));
}

#[test]
fn config_snapshot_json_omits_optional_fields() {
    let event = SessionEvent::ConfigSnapshot {
        output_format: OutputFormat::Report.as_str().to_string(),
        depth: None,
        iterations: None,
        from_urls: Vec::new(),
        from_file: None,
    };
    let rendered = render_session_event_json(&event);
    assert!(rendered.contains("\"output_format\":\"report\""));
    assert!(rendered.contains("\"depth\":null"));
    assert!(rendered.contains("\"iterations\":null"));
    assert!(rendered.contains("\"from_urls\":[]"));
}

#[test]
fn config_snapshot_json_includes_from_urls() {
    let event = SessionEvent::ConfigSnapshot {
        output_format: OutputFormat::ExecutiveSummary.as_str().to_string(),
        depth: None,
        iterations: None,
        from_urls: vec!["https://example.com/page".to_string()],
        from_file: None,
    };
    let rendered = render_session_event_json(&event);
    assert!(rendered.contains("\"output_format\":\"executive-summary\""));
    assert!(rendered.contains("\"from_urls\":[\"https://example.com/page\"]"));
}

#[test]
fn config_snapshot_json_includes_multiple_from_urls() {
    let event = SessionEvent::ConfigSnapshot {
        output_format: OutputFormat::Report.as_str().to_string(),
        depth: None,
        iterations: None,
        from_urls: vec![
            "https://example.com/page1".to_string(),
            "https://example.com/page2".to_string(),
        ],
        from_file: None,
    };
    let rendered = render_session_event_json(&event);
    assert!(
        rendered.contains(
            "\"from_urls\":[\"https://example.com/page1\",\"https://example.com/page2\"]"
        )
    );
}
