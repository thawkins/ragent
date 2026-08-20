//! Tests for the web-gather URL instrumentation log (`GatherLog`) wired
//! through `WebGatherer::with_gather_log`.

use std::sync::Arc;

use async_trait::async_trait;
use ragent_research::gather_log::GatherLog;
use ragent_research::{WebFetchTool, WebFetchedPage, WebGatherer, WebSearchHit, WebSearchTool};

struct FakeSearch;

#[async_trait]
impl WebSearchTool for FakeSearch {
    async fn search(&self, query: &str, _max: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        Ok(vec![
            WebSearchHit {
                url: "https://example.com/kept".into(),
                title: format!("kept {query}"),
                snippet: format!("kept {query}"),
                matched_query: query.to_string(),
                search_tool: "fake-search".into(),
                search_engine: "fake-engine".into(),
                author: None,
            },
            WebSearchHit {
                url: "https://example.com/short".into(),
                title: format!("short {query}"),
                snippet: format!("short {query}"),
                matched_query: query.to_string(),
                search_tool: "fake-search".into(),
                search_engine: "fake-engine".into(),
                author: None,
            },
        ])
    }
}

struct FakeFetch;

#[async_trait]
impl WebFetchTool for FakeFetch {
    async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
        let body = if url.contains("short") {
            "tiny".to_string() // below MIN_EXTRACTABLE_CONTENT_CHARS
        } else {
            format!("Long body about {url}. ").repeat(40)
        };
        Ok(WebFetchedPage {
            url: url.to_string(),
            title: format!("Title for {url}"),
            body,
            published_at: None,
            content_type: None,
            page_type: None,
            language: Some("English".into()),
            author: None,
        })
    }
}

fn read_log_lines(path: &std::path::Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[tokio::test]
async fn gather_writes_considered_captured_and_rejected_records() {
    let dir = tempfile::tempdir().unwrap();
    let web = WebGatherer::new(Arc::new(FakeSearch), Arc::new(FakeFetch))
        .with_gather_log(GatherLog::new(dir.path(), "unit-test").unwrap());
    let result = web.gather("rust lifetimes", 10).await.unwrap();
    assert_eq!(result.len(), 1);

    let log_path = GatherLog::new(dir.path(), "unused")
        .unwrap()
        .path()
        .parent()
        .unwrap()
        .to_path_buf();
    // Only one log file exists: the one created for the gather pass.
    let entries: Vec<_> = std::fs::read_dir(&log_path)
        .unwrap()
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    assert_eq!(entries.len(), 1);
    let file_name = entries[0]
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    assert!(
        file_name.starts_with("research-unit-test-"),
        "got {file_name}"
    );
    assert!(file_name.ends_with("-web.jsonl"), "got {file_name}");

    let lines = read_log_lines(&entries[0]);
    let find = |status: &str, url: &str| {
        lines
            .iter()
            .find(|v| v["status"] == status && v["url"] == url)
            .cloned()
    };

    // Gather markers.
    assert!(lines.iter().any(|v| v["event"] == "gather_start"));
    assert!(lines.iter().any(|v| v["event"] == "queries_decomposed"));
    let summary = lines
        .iter()
        .find(|v| v["event"] == "gather_summary")
        .unwrap();
    assert_eq!(summary["considered"], 2);
    assert_eq!(summary["captured"], 1);
    assert_eq!(summary["rejected"], 1);

    // Per-URL records.
    let considered_kept = find("considered", "https://example.com/kept").unwrap();
    assert_eq!(considered_kept["search_tool"], "fake-search");
    assert_eq!(considered_kept["query"], "rust lifetimes");

    let captured = find("captured", "https://example.com/kept").unwrap();
    assert!(captured["reason"].is_null());
    assert!(captured["content_chars"].as_u64().unwrap() > 256);

    let rejected = find("rejected", "https://example.com/short").unwrap();
    let reason = rejected["reason"].as_str().unwrap();
    assert!(reason.contains("too short"), "reason: {reason}");
}

#[tokio::test]
async fn gather_logs_search_failure_and_zero_hits_summary() {
    struct FailSearch;
    #[async_trait]
    impl WebSearchTool for FailSearch {
        async fn search(&self, _q: &str, _m: usize) -> anyhow::Result<Vec<WebSearchHit>> {
            Ok(vec![]) // circuit breaker disabled path: no hits
        }
    }

    let dir = tempfile::tempdir().unwrap();
    let web = WebGatherer::new(Arc::new(FailSearch), Arc::new(FakeFetch))
        .with_gather_log(GatherLog::new(dir.path(), "zero-hits").unwrap());
    let result = web.gather("nothing", 5).await.unwrap();
    assert!(result.is_empty());

    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    assert_eq!(entries.len(), 1);
    assert!(
        entries[0]
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("research-zero-hits-")
    );

    let lines = read_log_lines(&entries[0]);
    let summary = lines
        .iter()
        .find(|v| v["event"] == "gather_summary")
        .unwrap();
    assert_eq!(summary["considered"], 0);
    assert_eq!(summary["captured"], 0);
}
