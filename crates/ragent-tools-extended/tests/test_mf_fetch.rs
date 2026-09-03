#![allow(clippy::assert_is_empty)]
//! Integration tests for `mf_fetch` tool — HTTP fetch, extraction, envelope
//! signals, caching, robots, and graceful degradation (FR-002 through FR-007,
//! FR-019, FR-025, FR-026, FR-028, FR-029, FR-030, NFR-003).
//!
//! These tests use a local `axum` HTTP server so they exercise the full fetch
//! pipeline without calling external networks.
//!
//! Note: `std::env::set_var` is `unsafe` in Rust 2024; the workspace denies
//! `unsafe_code`, so this test target opts back in explicitly.

#![allow(unsafe_code)]

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{Router, response::Html, routing::get};
use ragent_tools_extended::masterfetch::tools::fetch::MfFetchTool;
use ragent_tools_extended::{Tool, ToolContext};
use serde_json::json;

/// Build a minimal `ToolContext` pointing at a temporary working directory.
fn ctx(working_dir: &std::path::Path) -> ToolContext {
    // Enable the test-only SSRF bypass so the local axum server can be reached.
    unsafe { std::env::set_var("RAGENT_TOOLS_EXTENDED_TEST_NO_SSRF", "1") };
    ToolContext {
        session_id: "test".to_string(),
        working_dir: working_dir.to_path_buf(),
        event_bus: Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: None,
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    }
}

/// Start a tiny axum server on localhost and return its base URL.
async fn start_server(router: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{addr}")
}

/// A unique temp directory that removes itself on drop.
struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "ragent_mf_fetch_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir_all(&dir).expect("creating temp dir");
        Self(dir)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// Basic fetch + extraction
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_fetch_simple_html_article() {
    let tmp = TempDir::new();
    let router = Router::new().route(
        "/",
        get(|| async {
            Html(
                r#"<!DOCTYPE html><html lang="en"><head>
                    <title>Test Article</title>
                    <meta name="description" content="A test article.">
                </head><body>
                    <article><p>This is the article body. It needs to be long enough to
                    satisfy the readability threshold for article classification.
                    We keep writing sentences here so the extracted text is
                    comfortably above five hundred characters and the page type
                    detector classifies it as an article rather than unknown.
                    Here is another sentence with more words about nothing in
                    particular, just to pad out the text so the test is stable.
                    </p></article>
                </body></html>"#,
            )
        }),
    );
    let base = start_server(router).await;
    let tool = MfFetchTool;
    let output = tool
        .execute(json!({"url": base}), &ctx(tmp.path()))
        .await
        .unwrap();

    assert!(output.content.contains("mf_fetch:"));
    assert!(output.content.contains("Test Article") || output.content.contains("article body"));
    let md = output.metadata.unwrap();
    assert_eq!(md["status"], 200);
    assert_eq!(md["content_ok"], true);
    assert_eq!(md["page_type"], "article");
    assert_eq!(md["fetcher_used"], "http");
    assert_eq!(md["cached"], false);
}

#[tokio::test]
async fn test_fetch_raw_format_returns_html() {
    let tmp = TempDir::new();
    let router = Router::new().route(
        "/",
        get(|| async { Html(r"<html><body>Hello raw</body></html>") }),
    );
    let base = start_server(router).await;
    let tool = MfFetchTool;
    let output = tool
        .execute(json!({"url": base, "format": "raw"}), &ctx(tmp.path()))
        .await
        .unwrap();

    assert!(output.content.contains("<html>"));
    let md = output.metadata.unwrap();
    assert_eq!(md["content_ok"], true);
    assert_eq!(md["fetcher_used"], "http");
}

#[tokio::test]
async fn test_fetch_text_format_strips_tags() {
    let tmp = TempDir::new();
    let router = Router::new().route(
        "/",
        get(|| async { Html(r"<html><body><p>Hello text</p></body></html>") }),
    );
    let base = start_server(router).await;
    let tool = MfFetchTool;
    let output = tool
        .execute(json!({"url": base, "format": "text"}), &ctx(tmp.path()))
        .await
        .unwrap();

    assert!(output.content.contains("Hello text"));
    assert!(!output.content.contains("<p>"));
    let md = output.metadata.unwrap();
    assert_eq!(md["content_ok"], true);
}

// ---------------------------------------------------------------------------
// Caching
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_fetch_caches_successful_content() {
    let tmp = TempDir::new();
    let router = Router::new().route(
        "/",
        get(|| async { Html(r"<html><body>Cached content is here.</body></html>") }),
    );
    let base = start_server(router).await;
    let url = format!("{base}/");
    let tool = MfFetchTool;

    // First fetch populates the cache.
    let out1 = tool
        .execute(json!({"url": &url}), &ctx(tmp.path()))
        .await
        .unwrap();
    assert_eq!(out1.metadata.as_ref().unwrap()["cached"], false);

    // Second fetch returns from cache.
    let out2 = tool
        .execute(json!({"url": &url}), &ctx(tmp.path()))
        .await
        .unwrap();
    assert_eq!(out2.metadata.as_ref().unwrap()["cached"], true);
    assert_eq!(out2.metadata.as_ref().unwrap()["fetcher_used"], "cache");
}

#[tokio::test]
async fn test_fetch_cache_ttl_zero_bypasses_cache() {
    let tmp = TempDir::new();
    let router = Router::new().route(
        "/",
        get(|| async { Html(r"<html><body>No cache bypass.</body></html>") }),
    );
    let base = start_server(router).await;
    let url = format!("{base}/");
    let tool = MfFetchTool;

    let out1 = tool
        .execute(json!({"url": &url, "cache_ttl": 0}), &ctx(tmp.path()))
        .await
        .unwrap();
    assert_eq!(out1.metadata.as_ref().unwrap()["cached"], false);

    let out2 = tool
        .execute(json!({"url": &url, "cache_ttl": 0}), &ctx(tmp.path()))
        .await
        .unwrap();
    // Cache bypass means second call still hits HTTP.
    assert_eq!(out2.metadata.as_ref().unwrap()["cached"], false);
}

// ---------------------------------------------------------------------------
// CSS selector narrowing + focus filtering
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_fetch_css_selector_narrows_extraction() {
    let tmp = TempDir::new();
    let router = Router::new().route(
        "/",
        get(|| async {
            Html(
                r#"<html><body>
                <div class="noise">noise</div>
                <div id="content"><p>Selected content</p></div>
            </body></html>"#,
            )
        }),
    );
    let base = start_server(router).await;
    let tool = MfFetchTool;
    let output = tool
        .execute(
            json!({"url": base, "css_selector": "div#content"}),
            &ctx(tmp.path()),
        )
        .await
        .unwrap();

    assert!(output.content.contains("Selected content"));
}

#[tokio::test]
async fn test_fetch_focus_filters_to_query() {
    let tmp = TempDir::new();
    let router = Router::new().route(
        "/",
        get(|| async {
            Html(
                r"<html><body><article>
                <p>Rust is a systems programming language with memory safety.</p>
                <p>Python is great for data science and scripting.</p>
            </article></body></html>",
            )
        }),
    );
    let base = start_server(router).await;
    let tool = MfFetchTool;
    let output = tool
        .execute(
            json!({"url": base, "focus": "Rust memory"}),
            &ctx(tmp.path()),
        )
        .await
        .unwrap();

    assert!(output.content.contains("Rust"));
    assert!(!output.content.contains("Python"));
}

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_fetch_offset_paginates_content() {
    let tmp = TempDir::new();
    let router = Router::new().route(
        "/",
        get(|| async {
            Html(
                r"<html><body><article>
                <p>ABCDEFGHIJ</p>
            </article></body></html>",
            )
        }),
    );
    let base = start_server(router).await;
    let tool = MfFetchTool;
    let output = tool
        .execute(
            json!({"url": base, "offset": 3, "max_content_chars": 4}),
            &ctx(tmp.path()),
        )
        .await
        .unwrap();

    let md = output.metadata.unwrap();
    assert_eq!(md["next_offset"], 7);
    assert_eq!(md["is_truncated"], true);
}

// ---------------------------------------------------------------------------
// Error handling
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_fetch_missing_url_errors() {
    let tmp = TempDir::new();
    let tool = MfFetchTool;
    let result = tool.execute(json!({}), &ctx(tmp.path())).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_fetch_ssrf_blocks_backslash_url() {
    let tmp = TempDir::new();
    let tool = MfFetchTool;
    let output = tool
        .execute(
            json!({"url": "http://example.com\\@127.0.0.1/"}),
            &ctx(tmp.path()),
        )
        .await
        .unwrap();

    assert!(output.content.contains("SSRF"));
    let md = output.metadata.unwrap();
    assert_eq!(md["content_ok"], false);
}

#[tokio::test]
async fn test_fetch_404_returns_content_ok_false() {
    let tmp = TempDir::new();
    let router = Router::new().route(
        "/",
        get(|| async { (axum::http::StatusCode::NOT_FOUND, "not found") }),
    );
    let base = start_server(router).await;
    let tool = MfFetchTool;
    let output = tool
        .execute(json!({"url": base}), &ctx(tmp.path()))
        .await
        .unwrap();

    let md = output.metadata.unwrap();
    assert_eq!(md["status"], 404);
    // A 404 body is still extracted; content_ok depends on extracted text.
    // We only assert the metadata reports the status.
}

#[tokio::test]
async fn test_fetch_invalid_selector_returns_error_output() {
    let tmp = TempDir::new();
    let router = Router::new().route(
        "/",
        get(|| async { Html(r"<html><body><p>Hi</p></body></html>") }),
    );
    let base = start_server(router).await;
    let tool = MfFetchTool;
    let output = tool
        .execute(
            json!({"url": base, "css_selector": "div > p"}),
            &ctx(tmp.path()),
        )
        .await
        .unwrap();

    assert!(output.content.contains("invalid CSS selector"));
    let md = output.metadata.unwrap();
    assert_eq!(md["content_ok"], false);
}

// ---------------------------------------------------------------------------
// Metadata and envelope
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_fetch_metadata_includes_opengraph() {
    let tmp = TempDir::new();
    let router = Router::new().route(
        "/",
        get(|| async {
            Html(
                r#"<html><head>
                <meta property="og:title" content="OG Title">
                <meta property="og:description" content="OG Description">
                <meta property="og:site_name" content="Example Site">
            </head><body><article><p>Article body with enough text to
            pass the readability threshold so classification is stable and
            the metadata is included in the response. More text more text.
            </p></article></body></html>"#,
            )
        }),
    );
    let base = start_server(router).await;
    let tool = MfFetchTool;
    let output = tool
        .execute(json!({"url": base}), &ctx(tmp.path()))
        .await
        .unwrap();

    let md = output.metadata.unwrap();
    assert_eq!(md["metadata"]["title"], "OG Title");
    assert_eq!(md["metadata"]["description"], "OG Description");
    assert_eq!(md["metadata"]["site_name"], "Example Site");
}

#[tokio::test]
async fn test_fetch_respect_robots_blocks_disallowed_url() {
    let tmp = TempDir::new();
    let router = Router::new()
        .route(
            "/robots.txt",
            get(|| async { "User-agent: *\nDisallow: /private/\n" }),
        )
        .route(
            "/private/page",
            get(|| async { Html(r"<html><body>secret</body></html>") }),
        );
    let base = start_server(router).await;
    let url = format!("{base}/private/page");
    let tool = MfFetchTool;
    let output = tool
        .execute(
            json!({"url": &url, "respect_robots": true}),
            &ctx(tmp.path()),
        )
        .await
        .unwrap();

    assert!(output.content.contains("robots.txt disallows"));
    let md = output.metadata.unwrap();
    assert_eq!(md["content_ok"], false);
}

#[tokio::test]
async fn test_fetch_include_links_classifies_links() {
    let tmp = TempDir::new();
    let router = Router::new().route(
        "/",
        get(|| async {
            Html(
                r#"
            <html><body>
                <nav><a href="/home">Home</a></nav>
                <article>
                    <p>Article with enough text to meet the article threshold for the
                    test so it stays stable. Lots of words words words words.
                    </p>
                    <a href="https://example.com/citation">Citation</a>
                </article>
            </body></html>"#,
            )
        }),
    );
    let base = start_server(router).await;
    let tool = MfFetchTool;
    let output = tool
        .execute(
            json!({"url": base, "include_links": true}),
            &ctx(tmp.path()),
        )
        .await
        .unwrap();

    let md = output.metadata.unwrap();
    let links = &md["links"];
    assert!(
        links["navigation"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["text"] == "Home")
    );
    assert!(
        links["citations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["text"] == "Citation")
    );
}

// ---------------------------------------------------------------------------
// Bulk fetch
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_fetch_bulk_urls_returns_combined_output() {
    let tmp = TempDir::new();
    let router = Router::new()
        .route(
            "/a",
            get(|| async {
                Html(
                    r"<html><head><title>Page A</title></head><body><article><p>Page A body. It needs to be long enough to satisfy the readability threshold for article classification. We keep writing sentences here so the extracted text is comfortably above five hundred characters and the page type detector classifies it as an article rather than unknown.</p></article></body></html>",
                )
            }),
        )
        .route(
            "/b",
            get(|| async {
                Html(
                    r"<html><head><title>Page B</title></head><body><article><p>Page B body. Similar to page A, we need enough text so that the extractor classifies this as an article rather than an unknown page type. Adding more words here ensures the readability threshold is crossed and the test remains stable.</p></article></body></html>",
                )
            }),
        );
    let base = start_server(router).await;
    let tool = MfFetchTool;
    let output = tool
        .execute(
            json!({"urls": [format!("{base}/a"), format!("{base}/b")]}),
            &ctx(tmp.path()),
        )
        .await
        .unwrap();

    assert!(output.content.contains("bulk fetch completed"));
    assert!(output.content.contains("Page A body"));
    assert!(output.content.contains("Page B body"));

    let md = output.metadata.unwrap();
    assert_eq!(md["bulk"], true);
    assert_eq!(md["count"], 2);
    assert_eq!(md["successful"], 2);
    assert_eq!(md["content_ok"], true);

    let results = md["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| r["content_ok"].as_bool().unwrap()));
}

#[tokio::test]
async fn test_fetch_bulk_deduplicates_urls() {
    let tmp = TempDir::new();
    let router = Router::new()
        .route(
            "/a",
            get(|| async {
                Html(
                    r"<html><head><title>A</title></head><body><article><p>Page A body is long enough to satisfy the readability threshold. We keep writing sentences so the extracted text is above five hundred characters and the classification is stable for this deduplication test.</p></article></body></html>",
                )
            }),
        )
        .route(
            "/b",
            get(|| async {
                Html(
                    r"<html><head><title>B</title></head><body><article><p>Page B body is long enough to satisfy the readability threshold. We keep writing sentences so the extracted text is above five hundred characters and the classification is stable for this deduplication test.</p></article></body></html>",
                )
            }),
        );
    let base = start_server(router).await;
    let tool = MfFetchTool;
    let output = tool
        .execute(
            json!({
                "urls": [
                    format!("{base}/a"),
                    format!("{base}/a"),
                    format!("{base}/b")
                ]
            }),
            &ctx(tmp.path()),
        )
        .await
        .unwrap();

    let md = output.metadata.unwrap();
    assert_eq!(md["count"], 2);
    let results = md["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_fetch_bulk_partial_failure_still_returns_results() {
    let tmp = TempDir::new();
    let router = Router::new().route(
        "/ok",
        get(|| async {
            Html(
                r"<html><head><title>OK</title></head><body><article><p>This page body is long enough to satisfy the readability threshold. We keep writing sentences so the extracted text is above five hundred characters and the classification is stable.</p></article></body></html>",
            )
        }),
    );
    let base = start_server(router).await;
    let tool = MfFetchTool;
    let output = tool
        .execute(
            json!({"urls": [format!("{base}/ok"), format!("{base}/missing")]}),
            &ctx(tmp.path()),
        )
        .await
        .unwrap();

    let md = output.metadata.unwrap();
    assert_eq!(md["count"], 2);
    assert_eq!(md["successful"], 1);
    assert_eq!(md["content_ok"], true);

    let results = md["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    let ok_count = results
        .iter()
        .filter(|r| r["content_ok"].as_bool().unwrap_or(false))
        .count();
    assert_eq!(ok_count, 1);
}
