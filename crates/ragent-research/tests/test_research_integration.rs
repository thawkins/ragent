//! End-to-end integration tests for `ragent-research` (T-052).
//!
//! Exercises the full create → list → show → search → delete flow against a
//! real on-disk `research/` directory, plus the FR-016 duplicate-create and
//! FR-018 closest-match paths.

use ragent_research::{ResearchItem, ResearchManager, ResearchName, ResearchStatus, Source};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn full_create_list_show_delete_flow() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());

    // Create
    let item = mgr
        .create("rust-async", "Rust Async", "async/await idioms")
        .await
        .unwrap();
    assert_eq!(item.name, ResearchName::new("rust-async").unwrap());

    // List
    let list = mgr.list(false).await.unwrap();
    assert_eq!(list.len(), 1);

    // Show
    let shown = mgr.show("rust-async").await.unwrap();
    assert_eq!(shown.status, ResearchStatus::Draft);
    assert_eq!(shown.title, "Rust Async");
    assert!(shown.topic.contains("async/await"));

    // INDEX.md exists.
    let index_path = ragent_research::ResearchIo::index_path(tmp.path());
    assert!(index_path.is_file());

    // Delete
    mgr.delete("rust-async").await.unwrap();
    let list = mgr.list(true).await.unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn create_rejects_duplicate_with_fr016_error() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "topic")
        .await
        .unwrap();
    let err = mgr
        .create("rust-async", "Other", "Other")
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("already exists"));
    assert!(msg.contains("/research open"));
}

#[tokio::test]
async fn write_document_persists_supports_files_and_index() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "topic")
        .await
        .unwrap();
    let mut item: ResearchItem = mgr.show("rust-async").await.unwrap();
    item.add_source(Source::Web {
        published_at: None,
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: chrono::Utc::now(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: "page text".into(),
        relevance: "User-supplied seed URL".into(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    });
    item.add_source(Source::Local {
        path: "src/lib.rs".into(),
        kind: ragent_research::LocalSourceKind::InProject,
        captured_at: chrono::Utc::now(),
        body_path: PathBuf::from("sources/local-01.md"),
        relevance: "anchor file".into(),
        body: "fn main() {}".into(),
    });
    let doc = ragent_research::ResearchDocument {
        item,
        summary: "Captured one web source and one local cross-reference.".into(),
        findings: vec!["Finding 1".into()],
        top_implications: Vec::new(),
        cross_references: vec![ragent_research::CrossReference {
            path: "src/lib.rs".into(),
            relevance: "anchor file".into(),
        }],
        open_questions: vec!["What about errors?".into()],
        contradiction_graph: None,
        loci: None,
        depth_investigation: None,
        evidence_digest: None,
        triple_draft: None,
        cross_locus_reconcile: None,
        source_tensions: None,
        synthesis_audit: None,
        corpus_critic: None,
        gap_fetch: None,
        surgical_patch: None,
        cite_check: None,
        polish: None,
        readability_audit: None,
        template_body: None,
        decomposed_queries: Vec::new(),
        output_format: ragent_research::OutputFormat::Report,
    };
    mgr.write_document(&doc).await.unwrap();

    // RESEARCH.md on disk has the full body.
    let body = std::fs::read_to_string(ragent_research::ResearchIo::research_md_path(
        tmp.path(),
        &ResearchName::new("rust-async").unwrap(),
    ))
    .unwrap();
    assert!(body.contains("Captured one web source"));
    assert!(body.contains("What about errors?"));
    assert!(body.contains("src/lib.rs"));

    // INDEX.md was refreshed.
    let index =
        std::fs::read_to_string(ragent_research::ResearchIo::index_path(tmp.path())).unwrap();
    assert!(index.contains("rust-async"));
}

#[tokio::test]
async fn archive_then_default_list_excludes_item() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "topic")
        .await
        .unwrap();
    mgr.archive("rust-async").await.unwrap();
    assert!(mgr.list(false).await.unwrap().is_empty());
    let all = mgr.list(true).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].status, ResearchStatus::Archived);
}

#[tokio::test]
async fn not_found_suggests_closest_match_per_fr018() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "topic")
        .await
        .unwrap();
    mgr.create("tokio-runtime", "Tokio", "topic").await.unwrap();
    let err = mgr.show("rust-asynx").await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Closest matches"));
    assert!(msg.contains("rust-async"));
}

#[tokio::test]
async fn search_finds_text_across_research_items() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "topic")
        .await
        .unwrap();
    mgr.create("serde-json", "Serde JSON", "topic")
        .await
        .unwrap();
    let hits = mgr.search("Rust", 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, "rust-async");
}

#[tokio::test]
async fn session_uses_analysis_engine_to_synthesize_findings() {
    use async_trait::async_trait;
    use ragent_research::{
        AnalysisEngine, AnalysisResult, CrossReference, InputConfig, LocalConfig, LocalGatherer,
        LocalTool, NoopObserver, OutputConfig, ResearchManager, ResearchSession, SessionConfig,
        WebConfig,
    };
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    struct MockEngine;

    #[async_trait]
    impl AnalysisEngine for MockEngine {
        async fn analyze(
            &self,
            topic: &str,
            sources: &[ragent_research::SourceBody],
        ) -> anyhow::Result<AnalysisResult> {
            assert_eq!(topic, "Rust async");
            assert!(
                !sources.is_empty(),
                "sources should be passed to the engine"
            );
            Ok(AnalysisResult {
                summary: "LLM-generated summary of Rust async.".into(),
                findings: vec!["Finding A: async/await is useful.".into()],
                top_implications: Vec::new(),
                cross_references: vec![CrossReference {
                    path: "src/lib.rs".into(),
                    relevance: "core async code".into(),
                }],
                open_questions: vec!["What about cancellation safety?".into()],
            })
        }
    }

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
        async fn grep(
            &self,
            path: &Path,
            terms: &[String],
        ) -> anyhow::Result<Vec<ragent_research::GrepMatch>> {
            let body = self.files.get(path).cloned().unwrap_or_default();
            let mut out = Vec::new();
            for (i, line) in body.lines().enumerate() {
                let l = line.to_lowercase();
                if terms.iter().any(|t| l.contains(t)) {
                    out.push(ragent_research::GrepMatch {
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

    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();
    let f = tmp.path().join("notes.md");
    tokio::fs::write(&f, "Rust async programming is great.")
        .await
        .unwrap();

    let manager = ResearchManager::new(&research_root);
    let local_tool = Arc::new(FakeLocal {
        files: HashMap::from([(f.clone(), "Rust async programming is great.".into())]),
    });
    let local = LocalGatherer::new(local_tool);
    let session = ResearchSession::new(manager, None, Some(local), Arc::new(MockEngine));

    let config = SessionConfig {
        input: InputConfig {
            topic: "Rust async".into(),
            ..InputConfig::default()
        },
        output: OutputConfig {
            output_format: ragent_research::OutputFormat::Report,
            ..OutputConfig::default()
        },
        web: WebConfig {
            max_web_results: 5,
            ..WebConfig::default()
        },
        local: LocalConfig {
            max_local_sources: 5,
            disable_local: false,
            ..LocalConfig::default()
        },
        analysis: ragent_research::AnalysisConfig {
            depth: Some(ragent_research::Depth::Shallow),
            ..ragent_research::AnalysisConfig::default()
        },
        ..SessionConfig::default()
    };
    session
        .run("rust-async", "Rust Async", &config, Arc::new(NoopObserver))
        .await
        .unwrap();

    let body = tokio::fs::read_to_string(research_root.join("rust-async/RESEARCH.md"))
        .await
        .unwrap();
    assert!(
        body.contains("LLM-generated summary"),
        "RESEARCH.md should contain the synthesized summary\n{body}"
    );
    assert!(
        body.contains("Finding A: async/await is useful."),
        "RESEARCH.md should contain the synthesized finding\n{body}"
    );
    assert!(
        body.contains("What about cancellation safety?"),
        "RESEARCH.md should contain the synthesized open question\n{body}"
    );
    assert!(
        body.contains("src/lib.rs"),
        "RESEARCH.md should contain the synthesized cross-reference\n{body}"
    );

    // Supporting file must contain the actual captured body, not the legacy
    // "(see LocalGatherer for the captured excerpt)" placeholder.
    let supporting =
        tokio::fs::read_to_string(research_root.join("rust-async/sources/local-01.md"))
            .await
            .unwrap();
    assert!(
        supporting.contains("Rust async programming is great."),
        "supporting file should contain captured body, got:\n{supporting}"
    );
    assert!(
        !supporting.contains("(see LocalGatherer for the captured excerpt)"),
        "supporting file must not contain the legacy placeholder"
    );
}

#[tokio::test]
async fn session_writes_supporting_files_with_actual_web_bodies() {
    use async_trait::async_trait;
    use ragent_research::{
        AnalysisEngine, AnalysisResult, InputConfig, LocalConfig, LocalGatherer, LocalTool,
        ResearchManager, ResearchSession, SessionConfig, SessionEvent, SessionObserver,
        SynthesisEvent, WebConfig, WebFetchTool, WebFetchedPage, WebGatherer, WebSearchHit,
        WebSearchTool,
    };
    use std::sync::Mutex;

    struct FakeSearch;
    #[async_trait]
    impl WebSearchTool for FakeSearch {
        async fn search(
            &self,
            query: &str,
            _max_results: usize,
        ) -> anyhow::Result<Vec<WebSearchHit>> {
            Ok(vec![WebSearchHit {
                url: "https://example.com/page".into(),
                title: "Example Page".into(),
                snippet: query.to_string(),
                matched_query: "Rust lifetimes".into(),
                search_tool: String::new(),
                search_engine: String::new(),
                author: None,
            }])
        }
    }
    struct FakeFetch;
    #[async_trait]
    impl WebFetchTool for FakeFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            Ok(WebFetchedPage {
                published_at: None,
                url: url.to_string(),
                title: "Example Page".into(),
                body: "Real page body — talks about Rust lifetimes. ".repeat(10),
                content_type: None,
                page_type: None,
                language: Some("English".into()),
                author: None,
            })
        }
    }
    struct NoLocal;
    #[async_trait]
    impl LocalTool for NoLocal {
        async fn glob(&self, _root: &Path, _pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
            Ok(Vec::new())
        }
        async fn grep(
            &self,
            _path: &Path,
            _terms: &[String],
        ) -> anyhow::Result<Vec<ragent_research::GrepMatch>> {
            Ok(Vec::new())
        }
        async fn read(&self, _path: &Path) -> anyhow::Result<String> {
            Ok(String::new())
        }
        async fn list_specs(&self, _root: &Path) -> anyhow::Result<Vec<String>> {
            Ok(Vec::new())
        }
        async fn spec_title(&self, _root: &Path, _spec_id: &str) -> anyhow::Result<String> {
            Ok(String::new())
        }
    }

    #[derive(Default)]
    struct CaptureObserver {
        events: Mutex<Vec<SessionEvent>>,
    }
    impl SessionObserver for CaptureObserver {
        fn on_event(&self, event: SessionEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();
    let manager = ResearchManager::new(&research_root);
    let web = WebGatherer::new(Arc::new(FakeSearch), Arc::new(FakeFetch));
    let local = LocalGatherer::new(Arc::new(NoLocal));
    let session = ResearchSession::new(manager, Some(web), Some(local), Arc::new(EmptyAnalysis));

    struct EmptyAnalysis;
    #[async_trait]
    impl AnalysisEngine for EmptyAnalysis {
        async fn analyze(
            &self,
            _topic: &str,
            _sources: &[ragent_research::SourceBody],
        ) -> anyhow::Result<AnalysisResult> {
            // Simulate an LLM that returned empty content — exercises the
            // mechanical fallback path while still proving the supporting
            // files contain real body content.
            Ok(AnalysisResult::default())
        }
    }

    let observer = Arc::new(CaptureObserver::default());
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "rust lifetimes".into(),
            ..InputConfig::default()
        },
        web: WebConfig {
            max_web_results: 5,
            ..WebConfig::default()
        },
        local: LocalConfig {
            max_local_sources: 5,
            ..LocalConfig::default()
        },
        ..SessionConfig::default()
    };
    session
        .run("lifetime-check", "Lifetime Check", &cfg, observer.clone())
        .await
        .unwrap();

    // Supporting web file must contain the fetched body text.
    let supporting =
        tokio::fs::read_to_string(research_root.join("lifetime-check/sources/web-01.md"))
            .await
            .unwrap();
    assert!(
        supporting.contains("Real page body — talks about Rust lifetimes."),
        "web supporting file should contain actual body, got:\n{supporting}"
    );
    assert!(
        !supporting.contains("(see WebGatherer for the captured body)"),
        "web supporting file must not contain the legacy placeholder"
    );

    // RESEARCH.md must contain a real summary that names the web title and
    // either mentions the body excerpt (LLM path) or the title + a
    // fallback note.
    let research_md = tokio::fs::read_to_string(research_root.join("lifetime-check/RESEARCH.md"))
        .await
        .unwrap();
    assert!(
        research_md.contains("Example Page"),
        "RESEARCH.md must reference the captured web title, got:\n{research_md}"
    );

    // The References Index must surface the detected language.
    assert!(
        research_md.contains("English"),
        "RESEARCH.md References Index must show the detected language, got:\n{research_md}"
    );
    // The SynthesizeResult event must have fired.
    let events = observer.events.lock().unwrap();
    assert!(
        events.iter().any(|e| matches!(
            e,
            SessionEvent::Synthesis(SynthesisEvent::SynthesizeResult { .. })
        )),
        "session must emit a SynthesizeResult event"
    );
}
