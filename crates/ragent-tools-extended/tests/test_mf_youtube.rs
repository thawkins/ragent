//! Integration tests for `masterfetch::youtube` — transcript extraction helpers.
//!
//! Pure parsing functions are tested directly; the end-to-end caption-fetch
//! path is exercised through `mf_fetch` against a local mock YouTube server.

use std::net::SocketAddr;

use axum::{Router, routing::get};
use ragent_tools_extended::masterfetch::youtube::{
    caption_track_url, fallback_title_from_html, is_youtube_url, parse_caption_xml,
    parse_yt_initial_player_response,
};

const SAMPLE_CAPTIONS_XML: &str = r#"<?xml version="1.0" encoding="utf-8" ?>
<transcript>
    <text start="1.23" dur="2.00">Hello world</text>
    <text start="5.67" dur="3.00">Second line</text>
</transcript>
"#;

fn watch_page_html(caption_url: &str) -> String {
    format!(
        r#"<html>
<head><title>Mock Video - YouTube</title></head>
<body>
<script>
    var ytInitialPlayerResponse = {{"videoDetails":{{"title":"Mock Video"}},"captions":{{"captionTracks":[{{"baseUrl":"{caption_url}","languageCode":"en"}}]}}}};
</script>
</body>
</html>"#
    )
}

#[test]
fn test_is_youtube_url_recognises_watch_and_short_urls() {
    assert!(is_youtube_url("https://www.youtube.com/watch?v=abc123"));
    assert!(is_youtube_url("https://youtube.com/watch?v=abc123"));
    assert!(is_youtube_url("https://youtu.be/abc123"));
    assert!(!is_youtube_url("https://www.example.com/watch?v=abc123"));
    assert!(!is_youtube_url("not a url"));
}

#[test]
fn test_fallback_title_from_html_parses_title_tag() {
    let html = "<html><head><title>Rust Tutorial - YouTube</title></head></body></body></html>";
    assert_eq!(
        fallback_title_from_html(html),
        Some("Rust Tutorial".to_string())
    );
}

#[test]
fn test_parse_yt_initial_player_response_extracts_json() {
    let html = r#"<html>
<script>
    var ytInitialPlayerResponse = {"videoDetails":{"title":"Mock Video"},"captions":{"captionTracks":[{"baseUrl":"https://example.com/captions","languageCode":"en"}]}};
</script>
</html>"#;

    let response = parse_yt_initial_player_response(html).unwrap();
    assert_eq!(
        response["videoDetails"]["title"].as_str(),
        Some("Mock Video")
    );
}

#[test]
fn test_caption_track_url_prefers_default_then_english_then_first() {
    let no_default: serde_json::Value = serde_json::json!({
        "captions": {
            "captionTracks": [
                {"baseUrl": "https://example.com/es", "languageCode": "es"},
                {"baseUrl": "https://example.com/en", "languageCode": "en"}
            ]
        }
    });
    assert_eq!(
        caption_track_url(&no_default, "video").unwrap(),
        "https://example.com/en"
    );

    let with_default: serde_json::Value = serde_json::json!({
        "captions": {
            "captionTracks": [
                {"baseUrl": "https://example.com/en", "languageCode": "en"},
                {"baseUrl": "https://example.com/default", "languageCode": "de", "isDefault": true}
            ]
        }
    });
    assert_eq!(
        caption_track_url(&with_default, "video").unwrap(),
        "https://example.com/default"
    );
}

#[test]
fn test_caption_track_url_errors_when_no_tracks() {
    let empty: serde_json::Value = serde_json::json!({"captions": {"captionTracks": []}});
    assert!(caption_track_url(&empty, "video").is_err());
}

#[test]
fn test_parse_caption_xml_formats_timestamped_lines() {
    let transcript = parse_caption_xml(SAMPLE_CAPTIONS_XML).unwrap();
    assert!(transcript.contains("[00:01] Hello world"));
    assert!(transcript.contains("[00:05] Second line"));
}

// -----------------------------------------------------------------------------
// End-to-end transcript extraction against a mock caption server
// -----------------------------------------------------------------------------

async fn start_caption_server() -> String {
    let captions_xml = SAMPLE_CAPTIONS_XML.to_string();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let router = Router::new().route(
        "/captions",
        get(|| async move { ([("content-type", "text/xml")], captions_xml) }),
    );
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{addr}/captions")
}

#[tokio::test]
async fn test_extract_transcript_from_watch_page_fetches_captions() {
    use ragent_tools_extended::masterfetch::youtube::extract_transcript_from_watch_page;

    let caption_url = start_caption_server().await;
    let html = watch_page_html(&caption_url);

    let (title, transcript) = extract_transcript_from_watch_page(&html)
        .await
        .expect("extracting transcript from mock watch page");

    assert_eq!(title, "Mock Video");
    assert!(transcript.contains("[00:01] Hello world"));
    assert!(transcript.contains("[00:05] Second line"));
}
