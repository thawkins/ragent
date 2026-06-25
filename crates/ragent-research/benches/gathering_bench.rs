//! Criterion benchmark for the gathering engine (T-050, NFR-001).
//!
//! Exercises a full `ResearchSession::run` end-to-end against an in-memory
//! web + local tool stack. Use to track regressions in the gathering
//! pipeline as the codebase evolves.

use criterion::{Criterion, criterion_group, criterion_main};
use ragent_research::{
    GrepMatch, LocalGatherer, LocalTool, NoopAnalysisEngine, ResearchManager, ResearchSession,
    SessionConfig, SessionEvent, SessionObserver, WebFetchTool, WebFetchedPage, WebGatherer,
    WebSearchHit, WebSearchTool,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio::runtime::Runtime;

struct FakeSearch {
    hits: Mutex<Vec<WebSearchHit>>,
}
#[async_trait::async_trait]
impl WebSearchTool for FakeSearch {
    async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        Ok(self.hits.lock().unwrap().clone())
    }
}

struct FakeFetch {
    pages: HashMap<String, WebFetchedPage>,
}
#[async_trait::async_trait]
impl WebFetchTool for FakeFetch {
    async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
        self.pages
            .get(url)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing"))
    }
}

struct FakeLocal {
    files: HashMap<PathBuf, String>,
}
#[async_trait::async_trait]
impl LocalTool for FakeLocal {
    async fn glob(&self, _root: &Path, pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
        let ext = pattern.rsplit('.').next().unwrap_or("");
        Ok(self
            .files
            .keys()
            .filter(|p| p.extension().map(|e| e == ext).unwrap_or(false))
            .cloned()
            .collect())
    }
    async fn grep(&self, path: &Path, terms: &[String]) -> anyhow::Result<Vec<GrepMatch>> {
        let body = self.files.get(path).cloned().unwrap_or_default();
        let mut out = Vec::new();
        for (i, line) in body.lines().enumerate() {
            let l = line.to_lowercase();
            if terms.iter().any(|t| l.contains(t)) {
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
            .ok_or_else(|| anyhow::anyhow!("missing"))
    }
    async fn list_specs(&self, _root: &Path) -> anyhow::Result<Vec<String>> {
        Ok(Vec::new())
    }
    async fn spec_title(&self, _root: &Path, _id: &str) -> anyhow::Result<String> {
        Ok(String::new())
    }
}

struct Noop;
impl SessionObserver for Noop {
    fn on_event(&self, _event: SessionEvent) {}
}

fn make_hits(n: usize) -> Vec<WebSearchHit> {
    (0..n)
        .map(|i| WebSearchHit {
            url: format!("https://example.com/{i}"),
            title: format!("Hit {i}"),
            snippet: String::new(),
        })
        .collect()
}

fn make_pages(n: usize) -> HashMap<String, WebFetchedPage> {
    (0..n)
        .map(|i| {
            (
                format!("https://example.com/{i}"),
                WebFetchedPage {
                    url: format!("https://example.com/{i}"),
                    title: format!("Page {i}"),
                    body: "Rust async body content".repeat(20),
                },
            )
        })
        .collect()
}

fn make_files(dir: &Path, n: usize) -> HashMap<PathBuf, String> {
    let mut files = HashMap::new();
    for i in 0..n {
        let p = dir.join(format!("notes/file-{i}.md"));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, format!("Rust async programming note {i}")).unwrap();
        files.insert(p, format!("Rust async programming note {i}"));
    }
    files
}

fn bench_gathering(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    std::fs::create_dir_all(&research_root).unwrap();

    let n_hits = 5;
    let n_local = 5;
    let mut files = make_files(tmp.path(), n_local);
    // Add a NOTES.md file inside research_root so the local gatherer sees the project layout.
    let anchor = research_root.join("NOTES.md");
    std::fs::write(&anchor, "Rust async notes about the project").unwrap();
    files.insert(
        anchor.clone(),
        "Rust async notes about the project".to_string(),
    );

    let web = WebGatherer::new(
        Arc::new(FakeSearch {
            hits: Mutex::new(make_hits(n_hits)),
        }),
        Arc::new(FakeFetch {
            pages: make_pages(n_hits),
        }),
    );
    let local = LocalGatherer::new(Arc::new(FakeLocal { files }));

    let manager = ResearchManager::new(&research_root);
    let session = ResearchSession::new(
        manager,
        Some(web),
        Some(local),
        Arc::new(ragent_research::NoopAnalysisEngine),
    );
    let config = SessionConfig {
        topic: "Rust async".into(),
        max_web_results: n_hits,
        max_local_sources: n_local,
        ..SessionConfig::default()
    };

    c.bench_function("gathering_engine_full_run", |b| {
        b.iter(|| {
            rt.block_on(async {
                session
                    .run("rust-async", "Rust Async", &config, Arc::new(Noop))
                    .await
                    .unwrap();
            });
        });
    });
}

criterion_group!(benches, bench_gathering);
criterion_main!(benches);
