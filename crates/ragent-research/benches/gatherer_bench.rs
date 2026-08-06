//! Micro-benchmarks for the individual research gatherers (Milestone A-002).
//!
//! Isolates [`WebGatherer::gather`] and [`LocalGatherer::gather`] from the
//! rest of a [`ResearchSession::run`] so regressions in discovery, scoring,
//! fetch ordering, or local candidate ranking are visible without running a
//! full end-to-end session.

use async_trait::async_trait;
use criterion::{Criterion, criterion_group, criterion_main};
use ragent_research::{
    GrepMatch, LocalGatherConfig, LocalGatherer, LocalTool, WebFetchTool, WebFetchedPage,
    WebGatherer, WebSearchHit, WebSearchTool,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Deterministic fake search tool.
struct FakeSearch {
    hits: Vec<WebSearchHit>,
}

#[async_trait]
impl WebSearchTool for FakeSearch {
    async fn search(&self, _query: &str, _max_results: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        Ok(self.hits.clone())
    }
}

/// Deterministic fake fetch tool backed by an in-memory page map.
struct FakeFetch {
    pages: HashMap<String, WebFetchedPage>,
}

#[async_trait]
impl WebFetchTool for FakeFetch {
    async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
        self.pages
            .get(url)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing page {url}"))
    }
}

/// Deterministic fake filesystem tool backed by an in-memory map.
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

fn fake_hits(n: usize) -> Vec<WebSearchHit> {
    (0..n)
        .map(|i| WebSearchHit {
            url: format!("https://example.com/{i}"),
            title: format!("Hit {i}"),
            snippet: "Rust async runtime benchmark snippet".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "duckduckgo, brave".into(),
        })
        .collect()
}

fn fake_pages(n: usize) -> HashMap<String, WebFetchedPage> {
    (0..n)
        .map(|i| {
            (
                format!("https://example.com/{i}"),
                WebFetchedPage {
                    published_at: None,
                    url: format!("https://example.com/{i}"),
                    title: format!("Page {i}"),
                    body: "Rust async runtime design and performance. ".repeat(40),
                    content_type: None,
                    page_type: None,
                    language: None,
                },
            )
        })
        .collect()
}

fn bench_web_gatherer(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let n = 50;
    let web = WebGatherer::new(
        Arc::new(FakeSearch { hits: fake_hits(n) }),
        Arc::new(FakeFetch {
            pages: fake_pages(n),
        }),
    );

    c.bench_function("web_gatherer_50_sources", |b| {
        b.iter(|| rt.block_on(web.gather("Rust async runtime", n)).unwrap());
    });
}

fn bench_local_gatherer(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let mut files = HashMap::new();
    for i in 0..50 {
        let p = root.join(format!("notes/file-{i}.md"));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        let body = format!("Rust async programming note {i}");
        std::fs::write(&p, &body).unwrap();
        files.insert(p, body);
    }
    let local = LocalGatherer::new(Arc::new(FakeLocal { files }));
    let cfg = LocalGatherConfig {
        max_local_sources: 10,
        ..LocalGatherConfig::default()
    };

    c.bench_function("local_gatherer_50_files", |b| {
        b.iter(|| {
            rt.block_on(local.gather(&root, "Rust async", None, &cfg))
                .unwrap()
        });
    });
}

criterion_group!(benches, bench_web_gatherer, bench_local_gatherer);
criterion_main!(benches);
