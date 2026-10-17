//! Coverage-oriented tests for small auxiliary paths in `ragent-tools-extended`:
//! the `pdf_common::truncate_output` helper and the `mf_fetch` network-error
//! path (a connection to a closed local port), plus a few `mf_crawl` metadata
//! shapes. No external network is used.

#![allow(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ragent_tools_extended::masterfetch::tools::crawl_tool::MfCrawlTool;
use ragent_tools_extended::masterfetch::tools::fetch::MfFetchTool;
use ragent_tools_extended::pdf_common::{MAX_OUTPUT_BYTES, truncate_output};
use ragent_tools_extended::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/temp")
            .join(format!(
                "mfax_{tag}_{}_{}",
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
        session_id: "mfax".to_string(),
        working_dir: work.to_path_buf(),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        code_index: None,
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: None,
        allowed_roots: Vec::new(),
    }
}

// -- pdf_common::truncate_output ------------------------------------------

#[test]
fn test_truncate_output_short_is_unchanged() {
    let s = "a short string".to_string();
    assert_eq!(truncate_output(s.clone()), s);
}

#[test]
fn test_truncate_output_long_is_truncated_with_notice() {
    let big = "line\n".repeat(MAX_OUTPUT_BYTES); // far over the cap
    let out = truncate_output(big);
    assert!(out.len() <= MAX_OUTPUT_BYTES, "{} bytes", out.len());
    assert!(out.contains("Output truncated"), "notice missing");
}

#[test]
fn test_truncate_output_multibyte_boundary_is_safe() {
    // A long run of a multibyte character forces the boundary snap-back path.
    let big = "é".repeat(MAX_OUTPUT_BYTES);
    let out = truncate_output(big);
    assert!(out.len() <= MAX_OUTPUT_BYTES);
    assert!(out.contains("truncated"));
    // Must remain valid UTF-8 (implicitly true if we reach here without panic
    // constructing the &str slices; assert explicitly for clarity).
    assert!(std::str::from_utf8(out.as_bytes()).is_ok());
}

// -- mf_fetch network-error path (closed port, loopback only) -------------

#[tokio::test]
async fn test_mf_fetch_connection_refused_returns_error_output() {
    // Bind then drop a listener to obtain a port that is almost certainly
    // closed, so the fetch fails with a transport error (no external network).
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let scratch = Scratch::new("refused");
    let tool = MfFetchTool;
    let out = tool
        .execute(
            json!({ "url": format!("http://{addr}/") }),
            &ctx(scratch.path()),
        )
        .await
        .unwrap();
    let md = out.metadata.unwrap();
    assert_eq!(md["content_ok"], false);
    assert_eq!(md["fetcher_used"], "none");
}

// -- mf_crawl: report formatter branches via discover-only + truncation ----

#[tokio::test]
async fn test_mf_crawl_discover_only_empty_reports_no_urls() {
    // A page with no outbound links, crawl_urls listing only the start URL in
    // discover_only mode, exercises the "no pages fetched" report branch.
    let scratch = Scratch::new("crawl-empty");
    let out = MfCrawlTool
        .execute(
            json!({
                "url": "https://example.com/",
                "discover_only": true,
                "crawl_urls": ["https://example.com/"]
            }),
            &ctx(scratch.path()),
        )
        .await
        .unwrap();
    assert!(out.content.contains("mf_crawl:"), "{}", out.content);
}

#[tokio::test]
async fn test_mf_crawl_respect_robots_flag_parses() {
    // exercises the respect_robots branch construction (robots check against a
    // non-resolvable host simply allows by default and the crawl finds nothing).
    let scratch = Scratch::new("crawl-robots");
    let out = MfCrawlTool
        .execute(
            json!({ "url": "https://nonexistent.invalid.example/", "respect_robots": true, "max_pages": 1 }),
            &ctx(scratch.path()),
        )
        .await
        .unwrap();
    assert!(out.content.contains("mf_crawl:"), "{}", out.content);
}
