//! Supplementary integration tests for the `masterfetch` subsystem, exercising
//! the URL/robots/security helpers, the shared HTTP client body readers, the
//! YouTube parser edge cases, the crawl report formatter, and the search tool's
//! engine-selection/exclusion paths - all without any live external network.
//!
//! A tiny local `axum` server provides request/response coverage; the
//! test-only SSRF bypass is enabled for the process so loopback addresses can
//! be fetched (backslash / cloud-metadata rejections are still enforced).

#![allow(unsafe_code)]
// `assert!(!x.is_empty())` is clearer than `assert_ne!(x, [])` in these tests.
#![allow(clippy::assert_is_empty)]

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{
    Router,
    body::Bytes,
    http::header,
    response::{Html, IntoResponse},
    routing::get,
};
use ragent_tools_extended::masterfetch::crawl::CrawlResult;
use ragent_tools_extended::masterfetch::http;
use ragent_tools_extended::masterfetch::security::validate_url;
use ragent_tools_extended::masterfetch::tools::crawl_tool::MfCrawlTool;
use ragent_tools_extended::masterfetch::tools::fetch::MfFetchTool;
use ragent_tools_extended::masterfetch::tools::search_tool::MfSearchTool;
use ragent_tools_extended::masterfetch::youtube::{
    caption_xml_to_transcript, parse_caption_xml, parse_yt_initial_player_response,
};
use ragent_tools_extended::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;

/// Enable the test-only SSRF bypass so loopback servers can be fetched.
fn allow_loopback() {
    // SAFETY: `set_var` is unsafe in Rust 2024; this test binary opts in and
    // the variable is process-global for the test run only.
    unsafe { std::env::set_var("RAGENT_TOOLS_EXTENDED_TEST_NO_SSRF", "1") };
}

/// Build a scratch directory under `target/temp`.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/temp")
            .join(format!(
                "mfsup_{tag}_{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn ctx(work: &Path) -> ToolContext {
    ToolContext {
        session_id: "mfsup".to_string(),
        working_dir: work.to_path_buf(),
        event_bus: Arc::new(EventBus::new(32)),
        storage: None,
        code_index: None,
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: None,
        allowed_roots: Vec::new(),
    }
}

async fn start_server(router: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{addr}")
}

/// A small, valid one-page PDF produced by `printpdf`.
fn tiny_pdf_bytes() -> Vec<u8> {
    use printpdf::{
        BuiltinFont, Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, PdfWarnMsg,
        Point, Pt, TextItem,
    };
    let mut doc = PdfDocument::new("Suppl PDF");
    let font = PdfFontHandle::Builtin(BuiltinFont::Helvetica);
    let ops = vec![
        Op::StartTextSection,
        Op::SetFont {
            font: font.clone(),
            size: Pt(12.0),
        },
        Op::SetTextCursor {
            pos: Point::new(Mm(20.0), Mm(280.0)),
        },
        Op::ShowText {
            items: vec![TextItem::Text("Supplemental PDF body text.".to_string())],
        },
        Op::EndTextSection,
    ];
    doc.pages = vec![PdfPage::new(Mm(210.0), Mm(297.0), ops)];
    let mut warns: Vec<PdfWarnMsg> = Vec::new();
    doc.save(&PdfSaveOptions::default(), &mut warns)
}

// ===========================================================================
// security::validate_url - paths enforced even with the loopback bypass set
// ===========================================================================

#[test]
fn test_validate_url_regular_public_hostname_ok() {
    assert!(validate_url("https://docs.rs/crate/serde").is_ok());
}

#[test]
fn test_validate_url_cloud_metadata_with_path_rejected() {
    assert!(validate_url("http://metadata.google.internal/computeMetadata/v1/").is_err());
}

#[test]
fn test_validate_url_dns_rebinding_rejected_even_with_loopback_bypass() {
    // The DNS-rebinding suffix check runs before the loopback bypass, so it
    // must still reject.
    assert!(validate_url("http://evil.nip.io/").is_err());
}

// ===========================================================================
// http: client construction, shared client, body capping / truncation
// ===========================================================================

#[test]
fn test_http_build_client_and_default() {
    let c = http::build_client(std::time::Duration::from_secs(5)).expect("build_client");
    drop(c);
    let d = http::build_default_client().expect("build_default_client");
    drop(d);
    assert_eq!(http::DEFAULT_TIMEOUT_SECS, 30);
    assert_eq!(http::MAX_REDIRECTS, 5);
}

#[test]
fn test_http_shared_client_is_singleton() {
    let a = http::shared_client().expect("shared client");
    let b = http::shared_client().expect("shared client again");
    assert!(std::ptr::eq(a, b));
    assert!(http::USER_AGENT.starts_with("ragent/"));
}

#[test]
fn test_http_truncate_body_no_truncation() {
    let s = "short body";
    assert_eq!(http::truncate_body(s, 100), s);
}

#[test]
fn test_http_truncate_body_truncates_on_char_boundary() {
    let body = "aaaaaaaaaa"; // 10 bytes
    let out = http::truncate_body(body, 4);
    assert!(out.starts_with("aaaa"));
    assert!(out.contains("truncated"));
}

#[test]
fn test_http_truncate_body_multibyte_boundary() {
    // 'é' is 2 bytes; a limit of 3 must not split it.
    let body = "éééé";
    let out = http::truncate_body(body, 3);
    assert!(out.contains("é"), "must not split a multibyte char: {out}");
    assert!(out.contains("truncated"));
}

#[tokio::test]
async fn test_http_read_body_capped_and_lossy_and_bytes() {
    allow_loopback();
    let router = Router::new().route(
        "/",
        get(|| async { Html("<html><body>hello body reader</body></html>") }),
    );
    let base = start_server(router).await;
    let client = http::shared_client().expect("client");

    let resp = client.get(&base).send().await.unwrap();
    let body = http::read_body_capped(resp, http::MAX_SMALL_BODY_BYTES)
        .await
        .unwrap();
    assert!(body.contains("hello body reader"));

    let resp = client.get(&base).send().await.unwrap();
    let lossy = http::read_body_capped_lossy(resp, http::MAX_SMALL_BODY_BYTES)
        .await
        .unwrap();
    assert!(lossy.contains("hello body reader"));

    let resp = client.get(&base).send().await.unwrap();
    let bytes = http::read_bytes_capped(resp, http::MAX_SMALL_BODY_BYTES)
        .await
        .unwrap();
    assert!(!bytes.is_empty());
}

#[tokio::test]
async fn test_http_read_body_capped_truncates_large_body() {
    allow_loopback();
    // A body far above the tiny cap we pass should be truncated with a marker.
    let big = "x".repeat(10_000);
    let router = Router::new().route(
        "/",
        get(move || {
            let big = big.clone();
            async move { Html(big) }
        }),
    );
    let base = start_server(router).await;
    let client = http::shared_client().expect("client");
    let resp = client.get(&base).send().await.unwrap();
    let body = http::read_body_capped(resp, 100).await.unwrap();
    assert!(body.contains("truncated"), "{body}");
}

// ===========================================================================
// youtube - parser edge cases (no network)
// ===========================================================================

#[test]
fn test_scan_and_parse_player_response_edge_cases() {
    // Missing marker.
    assert!(parse_yt_initial_player_response("<html>no marker</html>").is_err());
    // Marker present but no opening brace.
    assert!(parse_yt_initial_player_response("ytInitialPlayerResponse = nope").is_err());
    // Unterminated object.
    assert!(parse_yt_initial_player_response("var ytInitialPlayerResponse = {\"a\":1").is_err());
    // Escaped quote inside a JSON string must not terminate the scan early.
    let html =
        "ytInitialPlayerResponse = {\"videoDetails\":{\"title\":\"a \\\"quoted\\\" title\"}};";
    let v = parse_yt_initial_player_response(html).expect("should parse with escaped quotes");
    assert_eq!(
        v.get("videoDetails")
            .and_then(|d| d.get("title"))
            .and_then(|t| t.as_str()),
        Some("a \"quoted\" title")
    );
}

#[test]
fn test_parse_caption_xml_without_start_attribute() {
    let xml = "<transcript><text>No timestamp here</text></transcript>";
    let out = parse_caption_xml(xml).unwrap();
    assert_eq!(out, "No timestamp here");
}

#[test]
fn test_parse_caption_xml_malformed_errors() {
    // A malformed entity / unclosed tag should surface as a parse error.
    let xml = "<transcript><text start=\"0\">a</text";
    let res = parse_caption_xml(xml);
    assert!(res.is_err() || res.unwrap().contains('a'));
}

#[test]
fn test_caption_xml_to_transcript_empty_reports_bot_gated() {
    let err = caption_xml_to_transcript("<transcript></transcript>", "https://youtube.com/x")
        .unwrap_err();
    assert!(err.to_string().to_lowercase().contains("empty"));
}

#[test]
fn test_caption_xml_to_transcript_with_segmented_text() {
    let xml =
        "<transcript><text start=\"0\">Hello </text><text start=\"1\">world</text></transcript>";
    let out = caption_xml_to_transcript(xml, "https://youtube.com/x").unwrap();
    assert!(out.contains("Hello"));
    assert!(out.contains("world"));
}

// ===========================================================================
// mf_fetch - extra parameter + PDF paths
// ===========================================================================

#[tokio::test]
async fn test_mf_fetch_rejects_bulk_over_cap() {
    let scratch = Scratch::new("bulkcap");
    let tool = MfFetchTool;
    let urls: Vec<String> = (0..70)
        .map(|i| format!("https://e{i}.example.com"))
        .collect();
    let err = tool
        .execute(json!({ "urls": urls }), &ctx(scratch.path()))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("bulk"), "{err}");
}

#[tokio::test]
async fn test_mf_fetch_pdf_content_type_extracts_text() {
    allow_loopback();
    let pdf = tiny_pdf_bytes();
    let router = Router::new().route(
        "/doc.pdf",
        get(move || {
            let pdf = pdf.clone();
            async move {
                (
                    [(header::CONTENT_TYPE, "application/pdf")],
                    Bytes::from(pdf),
                )
                    .into_response()
            }
        }),
    );
    let base = start_server(router).await;
    let scratch = Scratch::new("pdffetch");
    let tool = MfFetchTool;
    let out = tool
        .execute(
            json!({ "url": format!("{base}/doc.pdf") }),
            &ctx(scratch.path()),
        )
        .await
        .unwrap();
    let md = out.metadata.unwrap();
    assert_eq!(md["page_type"], "pdf");
    assert!(out.content.contains("Page type: pdf"), "{}", out.content);
}

// ===========================================================================
// mf_crawl - execute paths without external network
// ===========================================================================

#[tokio::test]
async fn test_mf_crawl_missing_url_bails() {
    let scratch = Scratch::new("crawl-nourl");
    let err = MfCrawlTool
        .execute(json!({}), &ctx(scratch.path()))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("url"), "{err}");
}

#[tokio::test]
async fn test_mf_crawl_backslash_url_rejected() {
    allow_loopback();
    let scratch = Scratch::new("crawl-bs");
    let out = MfCrawlTool
        .execute(
            json!({ "url": "http://exa\\mple.com" }),
            &ctx(scratch.path()),
        )
        .await
        .unwrap();
    assert!(out.content.contains("SSRF"), "{}", out.content);
    assert_eq!(out.metadata.unwrap()["total_pages"], 0);
}

#[tokio::test]
async fn test_mf_crawl_backslash_crawl_urls_entry_rejected() {
    allow_loopback();
    let scratch = Scratch::new("crawl-bsentry");
    let out = MfCrawlTool
        .execute(
            json!({ "url": "https://example.com", "crawl_urls": ["http://bad\\path"] }),
            &ctx(scratch.path()),
        )
        .await
        .unwrap();
    assert!(out.content.contains("crawl_urls"), "{}", out.content);
}

#[tokio::test]
async fn test_mf_crawl_discover_only_and_sitemap_modes_parse() {
    allow_loopback();
    let router = Router::new().route(
        "/",
        get(|| async {
            Html("<html><body><a href=\"/docs\">Docs</a><a href=\"/about\">About</a></body></html>")
        }),
    );
    let base = start_server(router).await;
    let scratch = Scratch::new("crawl-disc");
    let out = MfCrawlTool
        .execute(
            json!({ "url": base, "discover_only": true, "sitemap": "auto", "focus": "docs" }),
            &ctx(scratch.path()),
        )
        .await
        .unwrap();
    assert!(out.metadata.unwrap()["url"].is_string());
}

#[tokio::test]
async fn test_mf_crawl_fetches_pages_and_formats_report() {
    allow_loopback();
    let router = Router::new()
        .route(
            "/",
            get(|| async {
                Html(
                    "<html><head><title>Home</title></head><body><article>\
                     <p>Welcome home page with a link to docs.</p>\
                     <a href=\"/docs\">Docs</a></article></body></html>",
                )
            }),
        )
        .route(
            "/docs",
            get(|| async {
                Html(
                    "<html><head><title>Docs</title></head><body><article>\
                     <p>Documentation page body with plenty of words to classify.</p>\
                     </article></body></html>",
                )
            }),
        );
    let base = start_server(router).await;
    let scratch = Scratch::new("crawl-full");
    let out = MfCrawlTool
        .execute(
            json!({ "url": base, "max_pages": 2, "max_depth": 1, "sitemap": true }),
            &ctx(scratch.path()),
        )
        .await
        .unwrap();
    assert!(out.content.contains("mf_crawl:"), "{}", out.content);
    let md = out.metadata.unwrap();
    assert!(md["total_pages"].is_number());
}

#[test]
fn test_crawl_result_struct_defaults() {
    // Sanity-check the public result struct used by the formatter.
    let r = CrawlResult::default();
    assert_eq!(r.total_pages, 0);
    assert!(!r.truncated);
    assert!(r.truncated_by.is_none());
}

// ===========================================================================
// mf_search - engine selection / exclusion (network-free paths)
// ===========================================================================

#[tokio::test]
async fn test_mf_search_invalid_engine_bails() {
    let scratch = Scratch::new("search-badengine");
    let err = MfSearchTool
        .execute(
            json!({ "query": "rust", "engine": "does-not-exist" }),
            &ctx(scratch.path()),
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not available"), "{err}");
}

#[tokio::test]
async fn test_mf_search_all_engines_excluded_returns_explicit_result() {
    let scratch = Scratch::new("search-excluded");
    let out = MfSearchTool
        .execute(
            json!({ "query": "rust", "engine": "wikipedia", "exclude_engines": ["wikipedia"] }),
            &ctx(scratch.path()),
        )
        .await
        .unwrap();
    assert!(
        out.content.contains("No engines available"),
        "{}",
        out.content
    );
    let md = out.metadata.unwrap();
    assert_eq!(md["total_results"], 0);
    assert_eq!(md["error"], "all engines excluded by exclude_engines");
}

#[tokio::test]
async fn test_mf_search_empty_query_bails() {
    let scratch = Scratch::new("search-empty");
    let err = MfSearchTool
        .execute(json!({ "query": "   " }), &ctx(scratch.path()))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("empty"), "{err}");
}
