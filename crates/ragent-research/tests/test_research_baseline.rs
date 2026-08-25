//! Baseline measurement harness for Milestone A-003.
//!
//! Runs three representative research topics end-to-end against an in-memory
//! (but otherwise real) web + local gatherer stack and records wall-clock
//! time, peak RSS, and total source-body bytes to
//! `target/temp/research_baseline_report.md`.

use async_trait::async_trait;
use ragent_research::session::NoopObserver;
use ragent_research::source::Source;
use ragent_research::{
    GrepMatch, LocalConfig, LocalGatherer, LocalTool, NoopAnalysisEngine, ResearchManager,
    ResearchSession, SessionConfig, WebConfig, WebFetchTool, WebFetchedPage, WebGatherer,
    WebSearchHit, WebSearchTool,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tempfile::TempDir;

/// Fake search tool that returns hits whose titles contain the query keywords.
/// This ensures the default relevance filter retains them for any baseline topic.
struct TopicFakeSearch;

#[async_trait]
impl WebSearchTool for TopicFakeSearch {
    async fn search(&self, query: &str, max_results: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        let n = max_results.min(20);
        let words: Vec<String> = query
            .split_whitespace()
            .map(|s| s.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|s| !s.is_empty())
            .take(3)
            .map(|s| s.to_lowercase())
            .collect();
        let keyword = words.join(" ");
        let hits = (0..n)
            .map(|i| WebSearchHit {
                url: format!("https://example.com/{keyword}/{i}"),
                title: format!("{keyword} — page {i}"),
                snippet: format!("A detailed article about {keyword}."),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "duckduckgo, brave".into(),
                author: None,
            })
            .collect();
        Ok(hits)
    }
}

/// Fake fetch tool that returns a deterministic page for every URL.
struct FakeFetch;

#[async_trait]
impl WebFetchTool for FakeFetch {
    async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
        Ok(WebFetchedPage {
            published_at: None,
            url: url.to_string(),
            title: format!("Article about {url}"),
            body: "Body text covering the topic in enough detail to serve as a research source. "
                .repeat(30),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        })
    }
}

/// Fake filesystem tool backed by an in-memory map.
struct FakeLocal {
    files: HashMap<PathBuf, String>,
}

#[async_trait]
impl LocalTool for FakeLocal {
    async fn glob(&self, _root: &Path, pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
        let ext = pattern.rsplit('.').next().unwrap_or("");
        Ok(self
            .files
            .keys()
            .filter(|p| p.extension().is_some_and(|e| e == ext))
            .cloned()
            .collect())
    }

    async fn grep(&self, path: &Path, terms: &[String]) -> anyhow::Result<Vec<GrepMatch>> {
        let body = self.files.get(path).cloned().unwrap_or_default();
        let mut out = Vec::new();
        for (i, line) in body.lines().enumerate() {
            let lower = line.to_lowercase();
            if terms.iter().any(|t| lower.contains(t)) {
                out.push(GrepMatch {
                    line: i + 1,
                    text: line.to_string(),
                });
            }
        }
        Ok(out)
    }

    async fn read(&self, path: &Path) -> anyhow::Result<String> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing file {}", path.display()))
    }

    async fn list_specs(&self, _root: &Path) -> anyhow::Result<Vec<String>> {
        Ok(Vec::new())
    }

    async fn spec_title(&self, _root: &Path, _id: &str) -> anyhow::Result<String> {
        Ok(String::new())
    }
}

fn read_peak_rss_kb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find(|l| l.starts_with("VmHWM:"))
        .and_then(|l| {
            l.split_whitespace()
                .nth(1)
                .and_then(|n| n.parse::<u64>().ok())
        })
}

fn source_body_bytes(sources: &[Source]) -> usize {
    sources
        .iter()
        .map(|s| s.body().map(|b| b.len()).unwrap_or(0))
        .sum()
}

#[tokio::test]
async fn research_baseline_three_topics() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    std::fs::create_dir_all(&research_root).unwrap();

    let mut files = HashMap::new();
    for i in 0..30 {
        let p = tmp.path().join(format!("notes/file-{i}.md"));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        let body = format!(
            "Rust async runtime design note {i}. Structured logging and SQLite WAL mode are related topics."
        );
        std::fs::write(&p, &body).unwrap();
        files.insert(p, body);
    }

    let web = WebGatherer::new(Arc::new(TopicFakeSearch), Arc::new(FakeFetch));
    let local = LocalGatherer::new(Arc::new(FakeLocal { files }));

    let manager = ResearchManager::new(&research_root);
    let session = ResearchSession::new(
        manager,
        Some(web),
        Some(local),
        Arc::new(NoopAnalysisEngine),
    );

    let cfg = SessionConfig {
        web: WebConfig {
            max_web_results: 20,
            ..WebConfig::default()
        },
        local: LocalConfig {
            max_local_sources: 10,
            ..LocalConfig::default()
        },
        ..SessionConfig::default()
    };

    let topics = vec![
        ("rust-async", "Rust async runtimes", "Rust async runtimes"),
        (
            "structured-logging",
            "Structured logging in Rust",
            "Structured logging in Rust",
        ),
        ("sqlite-wal", "SQLite WAL mode", "SQLite WAL mode"),
    ];

    let mut rows = Vec::new();
    let initial_rss = read_peak_rss_kb();

    for (name, title, topic) in topics {
        let mut cfg = cfg.clone();
        cfg.input.topic = topic.to_string();
        let start = Instant::now();
        let outcome = session
            .run(name, title, &cfg, Arc::new(NoopObserver))
            .await
            .unwrap();
        let elapsed = start.elapsed();
        let peak_rss = read_peak_rss_kb();
        let body_bytes = source_body_bytes(&outcome.sources);
        rows.push((
            name,
            title,
            elapsed.as_millis(),
            peak_rss,
            body_bytes,
            outcome.sources.len(),
        ));
    }

    let report_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("temp");
    std::fs::create_dir_all(&report_dir).unwrap();
    let report_path = report_dir.join("research_baseline_report.md");

    let mut md = String::new();
    md.push_str("# Research System Baseline Report (Milestone A-003)\n\n");
    md.push_str("Generated by `tests/test_research_baseline.rs` with fake web/local tools and `NoopAnalysisEngine`.\n\n");
    md.push_str("| Topic | Wall-clock (ms) | Peak RSS (kB) | Source body bytes | Sources |\n");
    md.push_str("|-------|----------------:|--------------:|------------------:|--------:|\n");
    for (name, title, elapsed, rss, bytes, count) in &rows {
        let rss_str = rss.map(|r| format!("{r}")).unwrap_or_else(|| "n/a".into());
        md.push_str(&format!(
            "| {title} (`{name}`) | {elapsed} | {rss_str} | {bytes} | {count} |\n"
        ));
    }
    if let Some(initial) = initial_rss {
        md.push_str(&format!("\nInitial VmHWM before runs: {initial} kB.\n"));
    }
    md.push_str("\n## Notes\n\n");
    md.push_str("- `Peak RSS` is read from `/proc/self/status` `VmHWM` and is a cumulative process maximum;\n");
    md.push_str("  it is useful for trend comparison rather than an exact per-run allocation.\n");
    md.push_str(
        "- `Source body bytes` is a proxy for the synthesis prompt size before summarization.\n",
    );
    md.push_str("- Re-run this test after later milestones to compare wall-clock and memory.\n");

    std::fs::write(&report_path, md).unwrap();

    println!("Baseline report written to {}", report_path.display());
    for (name, title, elapsed, rss, bytes, count) in &rows {
        println!(
            "{title} ({name}): {elapsed} ms, RSS {:?} kB, {bytes} bytes, {count} sources",
            rss
        );
    }

    // Sanity checks: every run must produce sources and a document.
    for (name, title, _elapsed, _rss, _bytes, count) in &rows {
        assert!(*count > 0, "topic {title} ({name}) produced zero sources");
    }
}
