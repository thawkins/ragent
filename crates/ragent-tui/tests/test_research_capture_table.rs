//! Tests for the per-engine capture summary table in research progress
//! tracking.
//!
//! Captured web sources are aggregated per backend search engine with counts
//! by media type (page/pdf/youtube) and per-language article counts, rendered
//! as a compact ASCII table in the message window instead of one log line per
//! captured URL.

use ragent_research::session::SessionEvent;
use ragent_tui::research_progress::{
    CaptureDelta, EngineCaptureRow, ResearchProgress, SessionPhase, StepStatus,
    decode_progress_event, encode_progress_event,
};

fn capture(engines: &[&str], media_type: &str, language: &str) -> CaptureDelta {
    CaptureDelta {
        engines: engines.iter().map(|e| e.to_string()).collect(),
        media_type: media_type.to_string(),
        language: language.to_string(),
        url: String::new(),
    }
}

fn capture_url(engines: &[&str], media_type: &str, language: &str, url: &str) -> CaptureDelta {
    CaptureDelta {
        url: url.to_string(),
        ..capture(engines, media_type, language)
    }
}

#[test]
fn test_capture_delta_roundtrip_through_payload() {
    let event = SessionEvent::WebCaptured {
        url: "https://example.com/article".into(),
        title: "An Example Article".into(),
        search_tool: "mf_search".into(),
        search_engine: "duckduckgo, brave".into(),
        body_preview: "Some body text".into(),
        language: "ENGLISH".into(),
        media_type: "page".into(),
        oa_recovery: None,
    };
    let encoded = encode_progress_event("run", "topic", &event);
    let decoded = decode_progress_event(&encoded).expect("decode");
    let delta = decoded.capture.expect("capture delta present");
    assert_eq!(delta.engines, vec!["duckduckgo", "brave"]);
    assert_eq!(delta.media_type, "page");
    assert_eq!(delta.language, "ENGLISH");
    assert_eq!(delta.url, "https://example.com/article");
}

#[test]
fn test_capture_delta_absent_on_non_capture_events() {
    let encoded = encode_progress_event(
        "run",
        "topic",
        &SessionEvent::WebFetchFailed {
            url: "https://x.com".into(),
            error: "403".into(),
        },
    );
    let decoded = decode_progress_event(&encoded).expect("decode");
    assert!(decoded.capture.is_none());
}

#[test]
fn test_record_capture_aggregates_per_engine_and_media_type() {
    let mut p = ResearchProgress::new("rust-async", "async rust");
    p.record_capture(&capture(&["duckduckgo", "brave"], "page", "ENGLISH"));
    p.record_capture(&capture(&["brave"], "page", "ENGLISH"));
    p.record_capture(&capture(&["openalex"], "pdf", "ENGLISH"));
    p.record_capture(&capture(&["wikipedia"], "page", "FRENCH"));

    let find = |name: &str| {
        p.engines
            .iter()
            .find(|r| r.engine == name)
            .unwrap_or_else(|| panic!("missing engine row {name}"))
    };

    let ddg = find("duckduckgo");
    assert_eq!(ddg.page, 1);
    assert_eq!(ddg.pdf, 0);
    assert_eq!(ddg.youtube, 0);
    assert_eq!(ddg.total(), 1);
    assert_eq!(ddg.languages.get("ENGLISH"), Some(&1));

    let brave = find("brave");
    assert_eq!(brave.page, 2, "shared hit counts once per engine row");
    assert_eq!(brave.total(), 2);

    let oa = find("openalex");
    assert_eq!(oa.pdf, 1);
    assert_eq!(oa.languages.get("ENGLISH"), Some(&1));

    let wiki = find("wikipedia");
    assert_eq!(wiki.page, 1);
    assert_eq!(wiki.languages.get("FRENCH"), Some(&1));

    // Row order is first-appearance so the table is stable across redraws.
    let names: Vec<&str> = p.engines.iter().map(|r| r.engine.as_str()).collect();
    assert_eq!(names, vec!["duckduckgo", "brave", "openalex", "wikipedia"]);
}

#[test]
fn test_render_capture_table_shows_engine_rows_and_languages() {
    let mut p = ResearchProgress::new("rust-async", "async rust");
    p.apply(SessionPhase::Web, StepStatus::Started, "searching the web");
    p.record_capture(&capture(&["duckduckgo", "brave"], "page", "ENGLISH"));
    p.record_capture(&capture(&["brave"], "page", "FRENCH"));
    p.record_capture(&capture(&["openalex"], "pdf", "ENGLISH"));

    let rendered = p.render();
    // No per-URL lines in the message window.
    assert!(
        !rendered.contains("captured https://"),
        "per-URL lines must not appear in the message window:\n{rendered}"
    );
    // The table header and per-engine rows are present.
    assert!(rendered.contains("[captures] Captured sources by search engine:"));
    assert!(rendered.contains("duckduckgo"));
    assert!(rendered.contains("brave"));
    assert!(rendered.contains("openalex"));
    // Totals row aggregates every media type and language.
    let total_row = rendered
        .lines()
        .find(|l| l.trim_start().starts_with("| total"))
        .expect("totals row in table");
    assert!(
        total_row.contains("ENGLISH:3, FRENCH:1"),
        "totals row should aggregate languages: {total_row}"
    );
    // PDF capture surfaced in the pdf column of the openalex row.
    let oa_row = rendered
        .lines()
        .find(|l| l.trim_start().starts_with("| openalex"))
        .expect("openalex row in table");
    assert!(
        oa_row.contains("|    0 |   1 |"),
        "pdf count in pdf column: {oa_row}"
    );
}

#[test]
fn test_render_capture_table_numeric_columns_right_aligned() {
    let mut p = ResearchProgress::new("rust-async", "async rust");
    for _ in 0..10 {
        p.record_capture(&capture(&["brave"], "page", "ENGLISH"));
    }

    let rendered = p.render();
    // Two-digit counts stay right-aligned within the column borders.
    assert!(
        rendered.contains("brave"),
        "engine row present:\n{rendered}"
    );
    let brave_row = rendered
        .lines()
        .find(|l| l.trim_start().starts_with("| brave"))
        .expect("brave row in table");
    assert_eq!(
        brave_row,
        "| brave  |   10 |   0 |  0 |    10 | ENGLISH:10 |"
    );
}

#[test]
fn test_apply_with_capture_does_not_append_log_lines() {
    let mut p = ResearchProgress::new("rust-async", "async rust");
    p.apply(SessionPhase::Web, StepStatus::Started, "searching the web");
    p.apply_with_capture(
        SessionPhase::Web,
        StepStatus::Done,
        "[ENGLISH] captured https://a.com — A",
        Some(capture(&["brave"], "page", "ENGLISH")),
    );
    p.apply_with_capture(
        SessionPhase::Web,
        StepStatus::Done,
        "[ENGLISH] captured https://b.com — B",
        Some(capture(&["duckduckgo"], "page", "ENGLISH")),
    );

    assert_eq!(
        p.steps.len(),
        1,
        "capture events must not append per-URL step lines"
    );
    assert_eq!(p.fetched_count, 2);
    assert_eq!(p.engines.len(), 2);
    assert_eq!(p.engines[0].engine, "brave");
    assert_eq!(p.engines[0].page, 1);
    assert_eq!(p.engines[1].engine, "duckduckgo");
}

#[test]
fn test_render_without_captures_has_no_table() {
    let mut p = ResearchProgress::new("rust-async", "async rust");
    p.apply(SessionPhase::Web, StepStatus::Started, "searching the web");
    let rendered = p.render();
    assert!(
        !rendered.contains("[captures]"),
        "no table should render before any capture:\n{rendered}"
    );
}

#[test]
fn test_record_capture_tracks_unique_urls_across_engines() {
    let mut p = ResearchProgress::new("rust-async", "async rust");
    // Same URL returned by two engines: one unique source, two attributions.
    p.record_capture(&capture_url(
        &["exa", "tavily"],
        "page",
        "ENGLISH",
        "https://example.com/a",
    ));
    p.record_capture(&capture_url(
        &["exa"],
        "page",
        "ENGLISH",
        "https://example.com/b",
    ));
    // Case-normalised dedup: same URL in different case counts once.
    p.record_capture(&capture_url(
        &["tavily"],
        "page",
        "ENGLISH",
        "HTTPS://EXAMPLE.COM/A",
    ));

    assert_eq!(p.unique_urls.len(), 2, "duplicate URLs must dedupe");
    let attributions: usize = p.engines.iter().map(EngineCaptureRow::total).sum();
    assert_eq!(attributions, 4, "2 + 1 + 1 per-engine attributions");
    assert_eq!(
        attributions - p.unique_urls.len(),
        2,
        "surplus = multi-engine hits"
    );
}

#[test]
fn test_render_capture_table_footer_shows_unique_and_attributions() {
    let mut p = ResearchProgress::new("rust-async", "async rust");
    p.record_capture(&capture_url(
        &["exa", "tavily"],
        "page",
        "ENGLISH",
        "https://example.com/a",
    ));
    p.record_capture(&capture_url(
        &["exa"],
        "page",
        "ENGLISH",
        "https://example.com/b",
    ));

    let rendered = p.render();
    assert!(
        rendered.contains("[captures] 3 engine attribution(s), 2 unique source(s)."),
        "footer should separate attributions from unique sources:\n{rendered}"
    );
}

#[test]
fn test_engine_capture_row_total_sums_media_types() {
    let row = EngineCaptureRow {
        engine: "brave".into(),
        page: 3,
        pdf: 2,
        youtube: 1,
        languages: std::collections::BTreeMap::new(),
    };
    assert_eq!(row.total(), 6);
}
