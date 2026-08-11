//! Tests for the mf_fetch YouTube transcript pipeline in
//! `ragent_tools_extended::masterfetch::youtube`.
//!
//! The research web-gather phase classifies `youtube.com` / `youtu.be` URLs
//! as `WebSourceKind::YouTube` and expects the fetch layer to recover the
//! video transcript from the watch page's embedded `ytInitialPlayerResponse`.
//! These tests cover the parsing chain end to end: locating and parsing the
//! player response, choosing a caption track, and turning caption XML into a
//! transcript — plus the failure paths where no transcript can be recovered
//! (which the fetch layer turns into a `youtube_error_output` rather than a
//! silent page-chrome body).

use ragent_tools_extended::masterfetch::youtube::{
    caption_track_url, caption_xml_to_transcript, fallback_title_from_html, is_youtube_url,
    parse_caption_xml, parse_yt_initial_player_response,
};

/// A minimal but realistic `ytInitialPlayerResponse` object with nested
/// braces and a JSON string containing a literal `}` — the case that breaks
/// naive regex/non-greedy extraction.
const PLAYER_RESPONSE_HTML: &str = r#"
<html><head><title>Some Video - YouTube</title></head>
<body>
<script>
var ytInitialPlayerConfig = {"innertubeContext": {}};
var ytInitialPlayerResponse = {"videoDetails":{"videoId":"abc123","title":"Rust async explained","shortDescription":"An async walkthrough"},"captions":{"playerCaptionsTracklistRenderer":{"captionTracks":[{"baseUrl":"https://www.youtube.com/api/timedtext?v=abc123&lang=en","languageCode":"en","isDefault":true},{"baseUrl":"https://www.youtube.com/api/timedtext?v=abc123&lang=de","languageCode":"de"}],"audioTracks":[]}},"playabilityStatus":{"status":"OK","reason":"Available at https://example.com/watch?v=abc123&x={1,2}"}};
</script>
</body></html>
"#;

#[test]
fn parse_player_response_recovers_json_with_nested_braces_and_strings() {
    let value = parse_yt_initial_player_response(PLAYER_RESPONSE_HTML)
        .expect("player response should parse");

    assert_eq!(
        value
            .get("videoDetails")
            .and_then(|d| d.get("videoId"))
            .and_then(|v| v.as_str()),
        Some("abc123")
    );
    assert_eq!(
        value
            .get("videoDetails")
            .and_then(|d| d.get("title"))
            .and_then(|v| v.as_str()),
        Some("Rust async explained")
    );
    // A `}` inside a JSON string must not terminate the object early.
    assert_eq!(
        value
            .get("playabilityStatus")
            .and_then(|s| s.get("reason"))
            .and_then(|v| v.as_str()),
        Some("Available at https://example.com/watch?v=abc123&x={1,2}")
    );
}

#[test]
fn parse_player_response_assignment_without_semicolon() {
    // Real pages often close the script tag on the same line without a
    // trailing `;` after the object.
    let html = r#"<script>ytInitialPlayerResponse = {"a":{"b":"}}; "}}</script>"#;
    let value = parse_yt_initial_player_response(html).expect("should parse");
    assert_eq!(
        value
            .get("a")
            .and_then(|a| a.get("b"))
            .and_then(|v| v.as_str()),
        Some("}}; ")
    );
}

#[test]
fn parse_player_response_missing_marker_errors() {
    let err = parse_yt_initial_player_response("<html><body>no player response here</body></html>")
        .expect_err("missing marker must error");
    assert!(
        err.to_string()
            .contains("ytInitialPlayerResponse not found"),
        "unexpected error: {err}"
    );
}

#[test]
fn parse_player_response_unterminated_object_errors() {
    // Marker found but the JSON object never closes.
    let html = r#"<script>ytInitialPlayerResponse = {"a":{"b":1}"#;
    let err = parse_yt_initial_player_response(html).expect_err("unterminated JSON must error");
    assert!(
        err.to_string().contains("unterminated"),
        "unexpected error: {err}"
    );
}

#[test]
fn caption_track_url_prefers_default_track() {
    let value = parse_yt_initial_player_response(PLAYER_RESPONSE_HTML).unwrap();
    let url = caption_track_url(&value, "Rust async explained").expect("caption track selected");
    assert_eq!(
        url,
        "https://www.youtube.com/api/timedtext?v=abc123&lang=en"
    );
}

#[test]
fn caption_track_url_falls_back_to_first_english_track() {
    let value = serde_json::json!({
        "captions": {
            "playerCaptionsTracklistRenderer": {
                "captionTracks": [
                    {"baseUrl": "https://www.youtube.com/api/timedtext?lang=de", "languageCode": "de"},
                    {"baseUrl": "https://www.youtube.com/api/timedtext?lang=en", "languageCode": "en"}
                ]
            }
        }
    });
    let url = caption_track_url(&value, "title").expect("english track selected");
    assert_eq!(url, "https://www.youtube.com/api/timedtext?lang=en");
}

#[test]
fn caption_track_url_accepts_legacy_flat_layout() {
    // Older/embedded player responses used a flat `captions.captionTracks`
    // layout — keep accepting it so transcript extraction does not regress.
    let value = serde_json::json!({
        "captions": {
            "captionTracks": [
                {"baseUrl": "https://www.youtube.com/api/timedtext?flat=1&lang=en", "languageCode": "en"}
            ]
        }
    });
    let url = caption_track_url(&value, "title").expect("flat track selected");
    assert_eq!(url, "https://www.youtube.com/api/timedtext?flat=1&lang=en");
}

#[test]
fn caption_track_url_errors_when_no_tracks() {
    let value = serde_json::json!({"videoDetails": {"title": "no captions"}});
    let err = caption_track_url(&value, "no captions").expect_err("no caption tracks must error");
    assert!(
        err.to_string().contains("no caption tracks available"),
        "unexpected error: {err}"
    );
}

#[test]
fn caption_track_url_errors_when_tracks_empty() {
    let value = serde_json::json!({
        "captions": {"playerCaptionsTracklistRenderer": {"captionTracks": []}}
    });
    let err = caption_track_url(&value, "title").expect_err("empty caption tracks must error");
    assert!(
        err.to_string().contains("no caption tracks available"),
        "unexpected error: {err}"
    );
}

#[test]
fn parse_caption_xml_empty_body_yields_empty_transcript() {
    // A 0-byte caption response (bot-gating / consent interstitial) parses
    // successfully — the empty-body guard lives in
    // `extract_transcript_from_watch_page` / `caption_xml_to_transcript`.
    assert!(
        parse_caption_xml("")
            .expect("empty XML must not error")
            .is_empty()
    );
    assert!(
        parse_caption_xml("<transcript></transcript>")
            .expect("empty transcript element must not error")
            .is_empty()
    );
}

#[test]
fn caption_xml_to_transcript_accepts_timed_caption_payload() {
    // Happy path: a parsed transcript with timestamped lines passes the guard
    // unchanged.
    let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<transcript>
  <text start="0.0" dur="2.4">welcome to the channel</text>
  <text start="65.7" dur="3.3">today we talk about rust</text>
</transcript>"#;
    let transcript = caption_xml_to_transcript(
        xml,
        "https://www.youtube.com/api/timedtext?v=abc123&lang=en",
    )
    .expect("caption XML should yield a transcript");
    assert_eq!(
        transcript,
        "[00:00] welcome to the channel\n[01:05] today we talk about rust"
    );
}

#[test]
fn caption_xml_to_transcript_rejects_empty_body_as_bot_gated() {
    // A 0-byte caption response (bot-gating / consent interstitial) must be
    // surfaced as a distinct error, not collapsed into "no captions
    // available", so researchers can tell blocked captions from absent
    // captions.
    let err =
        caption_xml_to_transcript("", "https://www.youtube.com/api/timedtext?v=abc123&lang=en")
            .expect_err("empty caption body must error");
    let msg = err.to_string();
    assert!(
        msg.contains("empty") && msg.contains("bot-gated"),
        "unexpected error: {err}"
    );
    // The caption track URL is included to aid debugging.
    assert!(
        msg.contains("timedtext?v=abc123&lang=en"),
        "error should mention the caption URL: {err}"
    );
}

#[test]
fn caption_xml_to_transcript_rejects_whitespace_only_body() {
    let err = caption_xml_to_transcript(
        "   \n  ",
        "https://www.youtube.com/api/timedtext?v=x&lang=en",
    )
    .expect_err("whitespace-only caption body must error");
    assert!(err.to_string().contains("empty"), "unexpected error: {err}");
}

#[test]
fn parse_caption_xml_renders_timestamped_lines() {
    let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<transcript>
  <text start="0.0" dur="2.4">welcome to the channel</text>
  <text start="65.7" dur="3.3">today we talk about rust</text>
  <text start="125.0" dur="1.0">  </text>
</transcript>"#;
    let transcript = parse_caption_xml(xml).expect("caption XML should parse");
    assert_eq!(
        transcript,
        "[00:00] welcome to the channel\n[01:05] today we talk about rust"
    );
}

#[test]
fn fallback_title_from_html_strips_youtube_suffix() {
    assert_eq!(
        fallback_title_from_html("<title>Some Video - YouTube</title>"),
        Some("Some Video".to_string())
    );
}

#[test]
fn is_youtube_url_recognises_watch_and_short_urls() {
    assert!(is_youtube_url("https://www.youtube.com/watch?v=abc"));
    assert!(is_youtube_url("https://youtu.be/abc"));
    assert!(!is_youtube_url("https://example.com/watch?v=abc"));
}
