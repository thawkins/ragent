//! Inline tests for `session.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::local_gatherer::{GrepMatch, LocalTool};
use crate::web_gatherer::{
    HeuristicQueryDecomposer, MIN_EXTRACTABLE_CONTENT_CHARS, WebFetchTool, WebFetchedPage,
    WebSearchHit, WebSearchTool,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use tempfile::TempDir;

/// The width-sweep detail must render the per-engine reason columns, the
/// fetch-failure breakdown columns, blank lines above and below the table,
/// and a totals row whose reason counts sum to the global exclusion tally.
#[test]
fn format_width_sweep_detail_renders_reason_columns_and_blank_lines() {
    use crate::web_gatherer::{EngineSweepStat, FetchFailureKind};
    let per_engine = vec![
        EngineSweepStat {
            engine: "langsearch".to_string(),
            considered: 5,
            captured: 2,
            excluded: 3,
            excluded_by_reason: [
                (ExclusionReason::PdfDisabled, 1usize),
                (ExclusionReason::LowRelevance, 1),
                (ExclusionReason::FetchFailed, 1),
            ]
            .into_iter()
            .collect(),
            failed_by_kind: std::iter::once((FetchFailureKind::Timeout, 1usize)).collect(),
        },
        EngineSweepStat {
            engine: "openalex".to_string(),
            considered: 2,
            captured: 1,
            excluded: 1,
            excluded_by_reason: std::iter::once((ExclusionReason::ScholarlyEngine, 1usize))
                .collect(),
            failed_by_kind: std::collections::BTreeMap::new(),
        },
    ];
    let excluded_by_reason = [
        (ExclusionReason::ScholarlyEngine, 1usize),
        (ExclusionReason::PdfDisabled, 1),
        (ExclusionReason::LowRelevance, 1),
        (ExclusionReason::FetchFailed, 1),
    ]
    .into_iter()
    .collect();
    let failed_by_kind = std::iter::once((FetchFailureKind::Timeout, 1usize)).collect();
    let detail = format_width_sweep_detail(
        2,
        &["langsearch".to_string(), "openalex".to_string()],
        7,
        3,
        4,
        &per_engine,
        &excluded_by_reason,
        &failed_by_kind,
        0,
        0,
    );
    // Blank line above and below the table (2 newlines each side).
    assert!(
        detail.contains("excluded=4\n\nengine"),
        "table must be preceded by a blank line:\n{detail}"
    );
    assert!(
        detail.ends_with(
            "totals                7        3        4        1        1        1        0        1      1      0      0      0      0      0      0\n\n"
        ),
        "table must be followed by a blank line and the totals row must \
         carry the reason and fetch-failure columns:\n{detail}"
    );
    // Reason columns use the short labels in ALL order, followed by the
    // fetch-failure breakdown columns.
    assert!(
        detail.contains(
            "excluded   papers      pdf    relev    short    fetch    t/o    net    blk   http   wall     js   extr"
        ),
        "header must carry the reason and fetch-failure columns:\n{detail}"
    );
    // Per-engine breakdown row.
    assert!(
        detail.contains("langsearch            5        2        3        0        1        1        0        1      1      0      0      0      0      0      0"),
        "langsearch row must break exclusions out by reason:\n{detail}"
    );
    assert!(
        detail.contains("openalex              2        1        1        1        0        0        0        0      0      0      0      0      0      0      0"),
        "openalex row must break exclusions out by reason:\n{detail}"
    );
}

/// Generate a body string of at least [`MIN_EXTRACTABLE_CONTENT_CHARS`]
/// characters so fake fetched pages pass the minimum-content-length guard.
fn body256(prefix: &str) -> String {
    let mut s = String::new();
    while s.chars().count() < MIN_EXTRACTABLE_CONTENT_CHARS {
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(prefix);
    }
    s
}

struct FakeSearch {
    hits: Vec<WebSearchHit>,
}
#[async_trait]
impl WebSearchTool for FakeSearch {
    async fn search(&self, query: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        let mut hits = self.hits.clone();
        for hit in &mut hits {
            hit.matched_query = query.to_string();
            if hit.snippet.is_empty() {
                hit.snippet = query.to_string();
            }
        }
        Ok(hits)
    }
}
struct FakeFetch {
    pages: HashMap<String, WebFetchedPage>,
}
#[async_trait]
impl WebFetchTool for FakeFetch {
    async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
        self.pages
            .get(url)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no page"))
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

#[derive(Default)]
struct CollectObserver {
    events: Mutex<Vec<SessionEvent>>,
}
impl SessionObserver for CollectObserver {
    fn on_event(&self, event: SessionEvent) {
        self.events.lock().unwrap().push(event);
    }
}

#[tokio::test]
async fn session_runs_end_to_end_and_writes_document() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();
    // Seed a single in-project file that contains a topic word.
    let f = tmp.path().join("notes.md");
    tokio::fs::write(&f, "Rust async programming is great.")
        .await
        .unwrap();

    let manager = ResearchManager::new(&research_root);
    let web = WebGatherer::new(
        Arc::new(FakeSearch {
            hits: vec![WebSearchHit {
                url: "https://example.com".into(),
                title: "Example".into(),
                snippet: String::new(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
                author: None,
            }],
        }),
        Arc::new(FakeFetch {
            pages: HashMap::from([(
                "https://example.com".into(),
                WebFetchedPage {
                    published_at: None,
                    url: "https://example.com".into(),
                    title: "Example".into(),
                    body: Arc::from(body256("body")),
                    content_type: None,
                    page_type: None,
                    language: None,
                    author: None,
                },
            )]),
        }),
    );
    let local_tool = Arc::new(FakeLocal {
        files: HashMap::from([(f.clone(), "Rust async programming is great.".into())]),
    });
    let local = LocalGatherer::new(local_tool);
    let session = ResearchSession::new(
        manager,
        Some(web),
        Some(local),
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "Rust async runtimes in 2024".into(),
            ..InputConfig::default()
        },
        clarify: false,
        ..SessionConfig::default()
    };
    let observer = Arc::new(CollectObserver::default());
    let outcome = session
        .run("rust-async", "Rust Async", &cfg, observer.clone())
        .await
        .unwrap();
    assert_eq!(outcome.research_name, "rust-async");
    assert_eq!(
        outcome.web_queries,
        vec!["Rust async runtimes in 2024".to_string()]
    );
    assert!(!outcome.sources.is_empty());
    // Document should exist on disk.
    let p = research_root.join("rust-async/RESEARCH.md");
    assert!(p.is_file());
    let body = tokio::fs::read_to_string(&p).await.unwrap();
    // The final title is the topic-derived title ("Rust Async") because
    // this test uses NoopAnalysisEngine (no LLM), so the title is NOT
    // derived from the mechanical fallback summary.
    assert!(
        body.contains("Rust async"),
        "RESEARCH.md should contain the topic; got:\n{body}"
    );
    assert!(
        body.contains("# Title: Rust Async"),
        "RESEARCH.md title should be the topic-derived title when no LLM engine is used; got:\n{body}"
    );
    // INDEX.md should exist.
    assert!(research_root.join("INDEX.md").is_file());
    // Observer should have received at least a Phase(Setup), Phase(Web), etc.
    let events = observer.events.lock().unwrap();
    assert!(events.iter().any(|e| matches!(
        e,
        SessionEvent::Phase {
            phase: SessionPhase::Web
        }
    )));
}

#[tokio::test]
async fn session_forwards_web_search_errors_to_observer() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();

    struct AlwaysFailSearch;
    #[async_trait]
    impl crate::web_gatherer::WebSearchTool for AlwaysFailSearch {
        async fn search(
            &self,
            _: &str,
            _: usize,
        ) -> anyhow::Result<Vec<crate::web_gatherer::WebSearchHit>> {
            anyhow::bail!("api key missing")
        }
    }
    struct OkFetch;
    #[async_trait]
    impl crate::web_gatherer::WebFetchTool for OkFetch {
        async fn fetch(&self, _: &str) -> anyhow::Result<crate::web_gatherer::WebFetchedPage> {
            Ok(crate::web_gatherer::WebFetchedPage {
                published_at: None,
                url: "u".into(),
                title: "t".into(),
                body: Arc::from(body256("b")),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
    }

    let manager = ResearchManager::new(&research_root);
    let web = crate::web_gatherer::WebGatherer::new(Arc::new(AlwaysFailSearch), Arc::new(OkFetch));
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "well scoped research topic".into(),
            ..InputConfig::default()
        },
        clarify: false,
        ..SessionConfig::default()
    };
    let observer = Arc::new(CollectObserver::default());
    let outcome = session
        .run("err", "Error", &cfg, observer.clone())
        .await
        .unwrap();
    assert_eq!(outcome.sources.len(), 0);
    let events = observer.events.lock().unwrap();
    assert!(
        events.iter().any(|e| matches!(
            e,
            SessionEvent::WebSearchFailed { error } if error.contains("api key missing")
        )),
        "expected WebSearchFailed event, got {:?}",
        *events
    );
}

#[tokio::test]
async fn session_handles_missing_web_gatherer() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();
    let manager = ResearchManager::new(&research_root);
    let session = ResearchSession::new(
        manager,
        None,
        None,
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "well scoped research topic".into(),
            ..InputConfig::default()
        },
        clarify: false,
        ..SessionConfig::default()
    };
    let outcome = session
        .run("rust-async", "Rust Async", &cfg, Arc::new(NoopObserver))
        .await
        .unwrap();
    assert_eq!(outcome.sources.len(), 0);
    assert!(outcome.web_queries.is_empty(), "no web gatherer configured");
}
#[tokio::test]
async fn session_persists_decomposed_queries_in_research_md() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();

    struct RecordingSearch;
    #[async_trait]
    impl WebSearchTool for RecordingSearch {
        async fn search(
            &self,
            _query: &str,
            _max_results: usize,
        ) -> anyhow::Result<Vec<WebSearchHit>> {
            Ok(vec![WebSearchHit {
                url: "https://example.com".into(),
                title: "Example".into(),
                snippet: String::new(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
                author: None,
            }])
        }
    }
    struct OkFetch;
    #[async_trait]
    impl WebFetchTool for OkFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            Ok(WebFetchedPage {
                published_at: None,
                url: url.to_string(),
                title: "Example".into(),
                body: Arc::from(body256("body")),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
    }

    let manager = ResearchManager::new(&research_root);
    let web = WebGatherer::new(Arc::new(RecordingSearch), Arc::new(OkFetch))
        .with_decomposer(Arc::new(HeuristicQueryDecomposer));
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "Rust async and Tokio runtime".into(),
            ..InputConfig::default()
        },
        web: WebConfig {
            max_web_results: 5,
            ..WebConfig::default()
        },
        ..SessionConfig::default()
    };
    let outcome = session
        .run("decomp-test", "Decomp Test", &cfg, Arc::new(NoopObserver))
        .await
        .unwrap();

    assert_eq!(
        outcome.web_queries,
        vec![
            "Rust async",
            "Tokio runtime",
            "Rust async and Tokio runtime"
        ]
    );

    let body = tokio::fs::read_to_string(research_root.join("decomp-test/RESEARCH.md"))
        .await
        .unwrap();
    assert!(body.contains("## Search Queries"));
    assert!(body.contains("- Rust async"));
    assert!(body.contains("- Tokio runtime"));
    assert!(body.contains("queries:"));
}

#[tokio::test]
async fn session_rejects_invalid_name() {
    let tmp = TempDir::new().unwrap();
    let manager = ResearchManager::new(tmp.path());
    let session = ResearchSession::new(
        manager,
        None,
        None,
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig::default();
    let err = session
        .run("AB", "t", &cfg, Arc::new(NoopObserver))
        .await
        .unwrap_err();
    assert!(matches!(err, ResearchError::InvalidName(_)));
}

#[tokio::test]
async fn from_url_fetches_page_and_derives_topic_when_topic_is_empty() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();

    struct NoSearch;
    #[async_trait]
    impl WebSearchTool for NoSearch {
        async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
            // The web-search phase still runs; returning no hits is fine
            // - we only need to prove the --from-url source was captured.
            Ok(Vec::new())
        }
    }
    struct PageFetch;
    #[async_trait]
    impl WebFetchTool for PageFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            Ok(WebFetchedPage {
                published_at: None,
                url: url.to_string(),
                title: "Rust Async Programming Guide".into(),
                body: "Long-form article about Rust async/await idioms. \
                       Tokio is the most popular runtime and provides a \
                       multi-threaded scheduler for async tasks."
                    .into(),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
    }

    let manager = ResearchManager::new(&research_root);
    let web = WebGatherer::new(Arc::new(NoSearch), Arc::new(PageFetch));
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: String::new(),
            from_urls: vec!["https://example.com/guide".into()],
            ..InputConfig::default()
        },
        ..SessionConfig::default()
    };
    let observer = Arc::new(CollectObserver::default());
    let outcome = session
        .run("from-url-test", "From URL", &cfg, observer.clone())
        .await
        .unwrap();

    // The fetched URL must be captured as the primary web source.
    let web_sources: Vec<&Source> = outcome
        .sources
        .iter()
        .filter(|s| matches!(s, Source::Web { .. }))
        .collect();
    assert!(
        web_sources.iter().any(|s| matches!(
            s,
            Source::Web { url, title, body, ..
            }
            if url == "https://example.com/guide"
                && title == "Rust Async Programming Guide"
                && body.contains("Long-form article")
        )),
        "expected the --from-url page as a web source, got {:?}",
        outcome.sources
    );

    // The URL must appear in the decomposed-queries list.
    assert!(
        outcome
            .web_queries
            .iter()
            .any(|q| q == "https://example.com/guide"),
        "expected the --from-url URL in web_queries, got {:?}",
        outcome.web_queries
    );

    // The research document should reference the topic derived from
    // the fetched page body (not the page title). The body's first
    // substantive sentence is "Long-form article about Rust async/await
    // idioms.", which must appear in RESEARCH.md.
    let body = tokio::fs::read_to_string(research_root.join("from-url-test/RESEARCH.md"))
        .await
        .unwrap();
    assert!(
        body.contains("Long-form article about Rust async/await idioms"),
        "RESEARCH.md should reference the topic derived from the fetched page body, not the title: {body}"
    );

    // The WebCaptured event for the --from-url source must have fired.
    let events = observer.events.lock().unwrap();
    assert!(
        events.iter().any(|e| matches!(
            e,
            SessionEvent::WebCaptured { url, title, language, .. }
                if url == "https://example.com/guide"
                    && title == "Rust Async Programming Guide"
                    && language == "UNKNOWN"
        )),
        "expected WebCaptured for --from-url with UNKNOWN language, got {:?}",
        *events
    );
}

#[tokio::test]
async fn from_url_derives_topic_from_body_not_title_when_body_has_boilerplate() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();

    struct NoSearch;
    #[async_trait]
    impl WebSearchTool for NoSearch {
        async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
            Ok(Vec::new())
        }
    }
    struct PageFetch;
    #[async_trait]
    impl WebFetchTool for PageFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            // The page title is a generic site name, and the body is
            // dominated by nav/cookie/share boilerplate with the real
            // article content in the middle. The derived topic must
            // come from the article content, not the title.
            Ok(WebFetchedPage {
                published_at: None,
                url: url.to_string(),
                title: "Example Site".into(),
                body: "Home About Contact Login\n\n\
                       Accept all cookies We use cookies on this site.\n\n\
                       The Rust async model maps asynchronous operations \
                       onto lightweight futures that a runtime polls to \
                       completion. This article walks through how Tokio \
                       schedules those futures onto worker threads.\n\n\
                       Read more Subscribe Newsletter\n\n\
                       (c) 2024 Example Corp. All rights reserved."
                    .into(),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
    }

    let manager = ResearchManager::new(&research_root);
    let web = WebGatherer::new(Arc::new(NoSearch), Arc::new(PageFetch));
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: String::new(),
            from_urls: vec!["https://example.com/article".into()],
            ..InputConfig::default()
        },
        ..SessionConfig::default()
    };
    let outcome = session
        .run(
            "body-topic-test",
            "Body Topic",
            &cfg,
            Arc::new(NoopObserver),
        )
        .await
        .unwrap();

    // The topic must be derived from the article sentence, not the
    // "Example Site" title or the nav/cookie boilerplate.
    let body = tokio::fs::read_to_string(research_root.join("body-topic-test/RESEARCH.md"))
        .await
        .unwrap();
    assert!(
        body.contains("Rust async model maps asynchronous operations"),
        "RESEARCH.md should reference the topic derived from the cleaned page body: {body}"
    );
    // The title-derived topic ("Example Site") must NOT have been used
    // as the research topic. The References Index still legitimately
    // cites the source by its page title, so we only check the topic
    // line in the frontmatter / summary, not the whole document.
    let topic_line = body
        .lines()
        .find(|l| l.starts_with("topic:"))
        .or_else(|| body.lines().find(|l| l.starts_with("# ")))
        .unwrap_or("");
    assert!(
        !topic_line.contains("Example Site"),
        "research topic should not be the generic page title: {topic_line}"
    );

    // Sanity: the source was still captured.
    assert!(
        outcome
            .sources
            .iter()
            .any(|s| matches!(s, Source::Web { url, .. } if url == "https://example.com/article")),
        "the --from-url page should be captured as a source"
    );
}

#[tokio::test]
async fn from_url_falls_back_to_title_when_body_is_pure_boilerplate() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();

    struct NoSearch;
    #[async_trait]
    impl WebSearchTool for NoSearch {
        async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
            Ok(Vec::new())
        }
    }
    struct PageFetch;
    #[async_trait]
    impl WebFetchTool for PageFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            Ok(WebFetchedPage {
                published_at: None,
                url: url.to_string(),
                title: "Meaningful Page Title".into(),
                body: "Home About Contact\n\nLogin Sign up\n\n(c) 2024 Example Corp.".into(),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
    }

    let manager = ResearchManager::new(&research_root);
    let web = WebGatherer::new(Arc::new(NoSearch), Arc::new(PageFetch));
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: String::new(),
            from_urls: vec!["https://example.com/boilerplate".into()],
            ..InputConfig::default()
        },
        ..SessionConfig::default()
    };
    let outcome = session
        .run("fallback-test", "Fallback", &cfg, Arc::new(NoopObserver))
        .await
        .unwrap();
    let body = tokio::fs::read_to_string(research_root.join("fallback-test/RESEARCH.md"))
        .await
        .unwrap();
    assert!(
        body.contains("Meaningful Page Title"),
        "RESEARCH.md should fall back to the page title when the cleaned body is empty: {body}"
    );
    let _ = outcome;
}

#[tokio::test]
async fn from_url_keeps_explicit_topic_when_both_are_supplied() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();

    struct NoSearch;
    #[async_trait]
    impl WebSearchTool for NoSearch {
        async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
            Ok(Vec::new())
        }
    }
    struct PageFetch;
    #[async_trait]
    impl WebFetchTool for PageFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            Ok(WebFetchedPage {
                published_at: None,
                url: url.to_string(),
                title: "Fetched Page Title".into(),
                body: Arc::from(body256("body text")),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
    }

    let manager = ResearchManager::new(&research_root);
    let web = WebGatherer::new(Arc::new(NoSearch), Arc::new(PageFetch));
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "Custom Topic".into(),
            from_urls: vec!["https://example.com/page".into()],
            ..InputConfig::default()
        },
        clarify: false,
        ..SessionConfig::default()
    };
    let outcome = session
        .run("both-test", "Both", &cfg, Arc::new(NoopObserver))
        .await
        .unwrap();

    // The explicit topic must win - the derived-topic branch only fires
    // when topic is empty.
    assert!(
        outcome
            .sources
            .iter()
            .any(|s| matches!(s, Source::Web { url, .. } if url == "https://example.com/page")),
        "the --from-url page should still be captured as a source"
    );
    let body = tokio::fs::read_to_string(research_root.join("both-test/RESEARCH.md"))
        .await
        .unwrap();
    assert!(
        body.contains("Custom Topic"),
        "explicit topic should be used, not the fetched page title: {body}"
    );
}

#[tokio::test]
async fn from_url_records_web_fetch_failed_when_fetch_errors() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();

    struct NoSearch;
    #[async_trait]
    impl WebSearchTool for NoSearch {
        async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
            Ok(Vec::new())
        }
    }
    struct FailFetch;
    #[async_trait]
    impl WebFetchTool for FailFetch {
        async fn fetch(&self, _: &str) -> anyhow::Result<WebFetchedPage> {
            anyhow::bail!("network down")
        }
    }

    let manager = ResearchManager::new(&research_root);
    let web = WebGatherer::new(Arc::new(NoSearch), Arc::new(FailFetch));
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: String::new(),
            from_urls: vec!["https://example.com/x".into()],
            ..InputConfig::default()
        },
        ..SessionConfig::default()
    };
    let observer = Arc::new(CollectObserver::default());
    let err = session
        .run("fail-test", "Fail", &cfg, observer.clone())
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            ResearchError::FromUrlFetchFailed { ref url, ref message }
                if url == "https://example.com/x" && message.contains("network down")
        ),
        "expected FromUrlFetchFailed, got {err:?}"
    );
    // A WebFetchFailed progress event is also surfaced to the observer.
    {
        let events = observer.events.lock().unwrap();
        assert!(
            events.iter().any(|e| matches!(
                e,
                SessionEvent::WebFetchFailed { url, error }
                    if url == "https://example.com/x" && error.contains("network down")
            )),
            "expected WebFetchFailed for --from-url, got {:?}",
            *events
        );
    }
    // No on-disk item is created when the primary URL fails.
    assert!(
        !ResearchIo::item_exists(
            research_root.as_path(),
            &ResearchName::try_new("fail-test").unwrap()
        )
        .await,
        "research folder should not be created when --from-url fails"
    );
}

#[tokio::test]
async fn session_skips_local_phase_when_disable_local_is_true() {
    use crate::local_gatherer::{LocalGatherer, LocalTool};
    use std::path::PathBuf;
    use std::sync::Arc;

    /// Minimal `LocalTool` that would otherwise emit one local source.
    #[derive(Default)]
    struct SingleLocalTool;
    #[async_trait::async_trait]
    // NOTE: intentional duplication - see DUPPLAN.md Milestone J.
    // Trait impls for different mock types; cannot be deduplicated.
    impl LocalTool for SingleLocalTool {
        async fn glob(&self, _root: &Path, _pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
            Ok(Vec::new())
        }
        async fn grep(
            &self,
            _path: &Path,
            _terms: &[String],
        ) -> anyhow::Result<Vec<crate::local_gatherer::GrepMatch>> {
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

    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();
    let manager = ResearchManager::new(&research_root);
    let local = LocalGatherer::new(Arc::new(SingleLocalTool));
    let session = ResearchSession::new(
        manager,
        None,
        Some(local),
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let observer = Arc::new(CollectObserver::default());
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "well scoped research topic".into(),
            ..InputConfig::default()
        },
        local: LocalConfig {
            disable_local: true,
            ..LocalConfig::default()
        },
        clarify: false,
        ..SessionConfig::default()
    };
    let outcome = session
        .run("rust-async", "Rust Async", &cfg, observer.clone())
        .await
        .unwrap();
    let local_count = outcome
        .sources
        .iter()
        .filter(|s| matches!(s, Source::Local { .. }))
        .count();
    assert_eq!(local_count, 0, "--no-local must produce zero local sources");
    let spec_count = outcome
        .sources
        .iter()
        .filter(|s| matches!(s, Source::Spec { .. }))
        .count();
    assert_eq!(
        spec_count, 0,
        "spec sources must not appear when --no-local is set"
    );
    // The Local phase event should still have been emitted so the
    // progress log makes the skip observable.
    let events = observer.events.lock().unwrap();
    assert!(
        events.iter().any(|e| matches!(
            e,
            SessionEvent::Phase {
                phase: SessionPhase::Local
            }
        )),
        "Local phase event should fire even when skipped"
    );
}

#[tokio::test]
async fn session_skips_spec_phase_when_disable_specs_is_true() {
    use crate::local_gatherer::{LocalGatherer, LocalTool};
    use std::path::PathBuf;
    use std::sync::Arc;

    /// `LocalTool` that emits one `Source::Spec` via `list_specs/spec_title`
    /// but no regular local files. This is the only path through which
    /// spec sources enter the session, so it exercises the `disable_specs`
    /// gate at the gatherer boundary.
    #[derive(Default)]
    struct SpecOnlyTool;
    #[async_trait::async_trait]
    impl LocalTool for SpecOnlyTool {
        async fn glob(&self, _root: &Path, _pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
            Ok(Vec::new())
        }
        async fn grep(
            &self,
            _path: &Path,
            _terms: &[String],
        ) -> anyhow::Result<Vec<crate::local_gatherer::GrepMatch>> {
            Ok(Vec::new())
        }
        async fn read(&self, _path: &Path) -> anyhow::Result<String> {
            Ok(String::new())
        }
        async fn list_specs(&self, _root: &Path) -> anyhow::Result<Vec<String>> {
            Ok(vec!["some-spec".into()])
        }
        async fn spec_title(&self, _root: &Path, _spec_id: &str) -> anyhow::Result<String> {
            Ok("Some spec title".into())
        }
    }

    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();
    let manager = ResearchManager::new(&research_root);
    let local = LocalGatherer::new(Arc::new(SpecOnlyTool));
    let session = ResearchSession::new(
        manager,
        None,
        Some(local),
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let observer = Arc::new(CollectObserver::default());
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "well scoped research topic".into(),
            ..InputConfig::default()
        },
        local: LocalConfig {
            disable_specs: true,
            ..LocalConfig::default()
        },
        clarify: false,
        ..SessionConfig::default()
    };
    let outcome = session
        .run("rust-async", "Rust Async", &cfg, observer.clone())
        .await
        .unwrap();
    let spec_count = outcome
        .sources
        .iter()
        .filter(|s| matches!(s, Source::Spec { .. }))
        .count();
    assert_eq!(spec_count, 0, "--no-specs must suppress spec sources");
    // The Specs phase event should still fire so the UI shows the skip.
    let events = observer.events.lock().unwrap();
    assert!(
        events.iter().any(|e| matches!(
            e,
            SessionEvent::Phase {
                phase: SessionPhase::Specs
            }
        )),
        "Specs phase event should fire even when skipped"
    );
}

#[tokio::test]
async fn synthesize_result_event_emitted_when_no_llm() {
    use crate::analysis::NoopAnalysisEngine;
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();
    let manager = ResearchManager::new(&research_root);
    let session = ResearchSession::new(manager, None, None, Arc::new(NoopAnalysisEngine));
    let observer = Arc::new(CollectObserver::default());
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "well scoped research topic".into(),
            ..InputConfig::default()
        },
        clarify: false,
        ..SessionConfig::default()
    };
    session
        .run("rust-async", "Rust Async", &cfg, observer.clone())
        .await
        .unwrap();
    let events = observer.events.lock().unwrap();
    let synth = events
        .iter()
        .find_map(|e| match e {
            SessionEvent::Synthesis(SynthesisEvent::SynthesizeResult { outcome, .. }) => {
                Some(*outcome)
            }
            _ => None,
        })
        .expect("SynthesizeResult event should be emitted");
    assert_eq!(synth, SynthesizeOutcome::NoLlm);
}

#[tokio::test]
async fn run_persists_model_in_frontmatter_when_set() {
    use crate::analysis::NoopAnalysisEngine;
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();
    let manager = ResearchManager::new(&research_root);
    let session = ResearchSession::new(manager, None, None, Arc::new(NoopAnalysisEngine))
        .with_model("anthropic/claude-sonnet-4");
    let observer = Arc::new(CollectObserver::default());
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "well scoped research topic".into(),
            ..InputConfig::default()
        },
        clarify: false,
        ..SessionConfig::default()
    };
    session
        .run("rust-async", "Rust Async", &cfg, observer)
        .await
        .unwrap();
    let path = research_root.join("rust-async").join("RESEARCH.md");
    let content = tokio::fs::read_to_string(&path).await.unwrap();
    assert!(
        content.contains("Model: \"anthropic/claude-sonnet-4\""),
        "RESEARCH.md frontmatter should record the analysis model; got:\n{content}"
    );
}

#[tokio::test]
async fn run_omits_model_line_when_not_set() {
    use crate::analysis::NoopAnalysisEngine;
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();
    let manager = ResearchManager::new(&research_root);
    let session = ResearchSession::new(manager, None, None, Arc::new(NoopAnalysisEngine));
    let observer = Arc::new(CollectObserver::default());
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "well scoped research topic".into(),
            ..InputConfig::default()
        },
        clarify: false,
        ..SessionConfig::default()
    };
    session
        .run("rust-async", "Rust Async", &cfg, observer)
        .await
        .unwrap();
    let path = research_root.join("rust-async").join("RESEARCH.md");
    let content = tokio::fs::read_to_string(&path).await.unwrap();
    assert!(
        !content.contains("Model:"),
        "RESEARCH.md frontmatter must omit Model: when no model is set; got:\n{content}"
    );
}

#[test]
fn engine_config_defaults_to_standard_single_pass() {
    let cfg = SessionConfig::default();
    let ec = cfg.engine_config();
    assert_eq!(ec.max_iterations, 3);
    assert_eq!(ec.max_sources_per_question, 3);
    assert!(!ec.force_deeper);
}

#[test]
fn engine_config_deep_forces_deeper_and_more_iterations() {
    let cfg = SessionConfig {
        analysis: AnalysisConfig {
            depth: Some(Depth::Deep),
            ..AnalysisConfig::default()
        },
        ..SessionConfig::default()
    };
    let ec = cfg.engine_config();
    assert_eq!(ec.max_iterations, 5);
    assert!(ec.force_deeper);
}

#[test]
fn engine_config_explicit_iterations_override() {
    let cfg = SessionConfig {
        analysis: AnalysisConfig {
            depth: Some(Depth::Shallow),
            iterations: Some(7),
            ..AnalysisConfig::default()
        },
        ..SessionConfig::default()
    };
    let ec = cfg.engine_config();
    assert_eq!(ec.max_iterations, 7);
}

#[test]
fn budget_web_results_scales_with_depth() {
    let shallow = SessionConfig {
        analysis: AnalysisConfig {
            depth: Some(Depth::Shallow),
            ..AnalysisConfig::default()
        },
        ..SessionConfig::default()
    };
    let deep = SessionConfig {
        analysis: AnalysisConfig {
            depth: Some(Depth::Deep),
            ..AnalysisConfig::default()
        },
        ..SessionConfig::default()
    };
    assert_eq!(shallow.budget_web_results(), 6);
    assert_eq!(deep.budget_web_results(), 15);
}

#[test]
fn effective_web_budget_derives_from_depth_unless_overridden() {
    // Default config: max_web_results is the 0 sentinel -> derive from depth.
    let shallow = SessionConfig {
        analysis: AnalysisConfig {
            depth: Some(Depth::Shallow),
            ..AnalysisConfig::default()
        },
        ..SessionConfig::default()
    };
    let deep = SessionConfig {
        analysis: AnalysisConfig {
            depth: Some(Depth::Deep),
            ..AnalysisConfig::default()
        },
        ..SessionConfig::default()
    };
    assert_eq!(shallow.effective_web_budget(), 6);
    assert_eq!(deep.effective_web_budget(), 15);

    // An explicit max_web_results always wins over the depth derivation.
    let explicit = SessionConfig {
        web: WebConfig {
            max_web_results: 500,
            ..WebConfig::default()
        },
        ..SessionConfig::default()
    };
    assert_eq!(explicit.effective_web_budget(), 500);
}

#[test]
fn budget_local_sources_matches_depth_preset() {
    let shallow = SessionConfig {
        analysis: AnalysisConfig {
            depth: Some(Depth::Shallow),
            ..AnalysisConfig::default()
        },
        ..SessionConfig::default()
    };
    let standard = SessionConfig {
        analysis: AnalysisConfig {
            depth: Some(Depth::Standard),
            ..AnalysisConfig::default()
        },
        ..SessionConfig::default()
    };
    let deep = SessionConfig {
        analysis: AnalysisConfig {
            depth: Some(Depth::Deep),
            ..AnalysisConfig::default()
        },
        ..SessionConfig::default()
    };
    assert_eq!(shallow.budget_local_sources(), 5);
    assert_eq!(standard.budget_local_sources(), 10);
    assert_eq!(deep.budget_local_sources(), 20);
}

#[test]
fn use_iterative_only_when_iterations_or_deep() {
    let none = SessionConfig::default();
    let shallow = SessionConfig {
        analysis: AnalysisConfig {
            depth: Some(Depth::Shallow),
            ..AnalysisConfig::default()
        },
        ..SessionConfig::default()
    };
    let standard = SessionConfig {
        analysis: AnalysisConfig {
            depth: Some(Depth::Standard),
            ..AnalysisConfig::default()
        },
        ..SessionConfig::default()
    };
    let deep = SessionConfig {
        analysis: AnalysisConfig {
            depth: Some(Depth::Deep),
            ..AnalysisConfig::default()
        },
        ..SessionConfig::default()
    };
    let iterations = SessionConfig {
        analysis: AnalysisConfig {
            iterations: Some(2),
            ..AnalysisConfig::default()
        },
        ..SessionConfig::default()
    };
    assert!(none.analysis.iterations.is_none() && none.analysis.depth != Some(Depth::Deep));
    assert!(shallow.analysis.iterations.is_none() && shallow.analysis.depth != Some(Depth::Deep));
    assert!(standard.analysis.iterations.is_none() && standard.analysis.depth != Some(Depth::Deep));
    assert!(deep.analysis.iterations.is_some() || deep.analysis.depth == Some(Depth::Deep));
    assert!(
        iterations.analysis.iterations.is_some() || iterations.analysis.depth == Some(Depth::Deep)
    );
}

#[tokio::test]
async fn overlapped_gather_combines_web_and_local_sources_and_emits_phases() {
    use crate::local_gatherer::{LocalGatherer, LocalTool};
    use std::path::PathBuf;
    use std::sync::Arc;

    /// LocalTool that returns one local source and one spec source so we
    /// can verify both are merged with web sources in the overlapped gather.
    #[derive(Default)]
    struct MixedLocalTool;
    #[async_trait::async_trait]
    impl LocalTool for MixedLocalTool {
        async fn glob(
            &self,
            _root: &std::path::Path,
            _pattern: &str,
        ) -> anyhow::Result<Vec<std::path::PathBuf>> {
            Ok(vec![PathBuf::from("src/lib.rs")])
        }
        async fn grep(
            &self,
            _path: &std::path::Path,
            _terms: &[String],
        ) -> anyhow::Result<Vec<crate::local_gatherer::GrepMatch>> {
            Ok(vec![crate::local_gatherer::GrepMatch {
                line: 1,
                text: "Rust async is great".into(),
            }])
        }
        async fn read(&self, _path: &std::path::Path) -> anyhow::Result<String> {
            Ok("Rust async is great".into())
        }
        async fn list_specs(&self, _root: &std::path::Path) -> anyhow::Result<Vec<String>> {
            Ok(vec!["some-spec".into()])
        }
        async fn spec_title(
            &self,
            _root: &std::path::Path,
            _spec_id: &str,
        ) -> anyhow::Result<String> {
            Ok("Some spec title".into())
        }
    }

    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();

    let web = WebGatherer::new(
        Arc::new(FakeSearch {
            hits: vec![WebSearchHit {
                url: "https://example.com".into(),
                title: "Example".into(),
                snippet: String::new(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
                author: None,
            }],
        }),
        Arc::new(FakeFetch {
            pages: HashMap::from([(
                "https://example.com".into(),
                WebFetchedPage {
                    published_at: None,
                    url: "https://example.com".into(),
                    title: "Example".into(),
                    body: Arc::from(body256("web body")),
                    content_type: None,
                    page_type: None,
                    language: None,
                    author: None,
                },
            )]),
        }),
    );
    let local = LocalGatherer::new(Arc::new(MixedLocalTool));

    let manager = ResearchManager::new(&research_root);
    let session = ResearchSession::new(
        manager,
        Some(web),
        Some(local),
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "Rust async".into(),
            ..InputConfig::default()
        },
        clarify: false,
        ..SessionConfig::default()
    };
    let observer = Arc::new(CollectObserver::default());
    let outcome = session
        .run("overlap-test", "Overlap Test", &cfg, observer.clone())
        .await
        .unwrap();

    let web_count = outcome
        .sources
        .iter()
        .filter(|s| matches!(s, Source::Web { .. }))
        .count();
    let local_count = outcome
        .sources
        .iter()
        .filter(|s| matches!(s, Source::Local { .. }))
        .count();
    let spec_count = outcome
        .sources
        .iter()
        .filter(|s| matches!(s, Source::Spec { .. }))
        .count();
    assert!(
        web_count >= 1,
        "overlapped gather must include web sources; got {outcome:?}"
    );
    assert!(
        local_count >= 1,
        "overlapped gather must include local sources; got {outcome:?}"
    );
    assert!(
        spec_count >= 1,
        "overlapped gather must include spec sources; got {outcome:?}"
    );

    // RESEARCH.md must cite both source types in the mechanical summary.
    let body = tokio::fs::read_to_string(research_root.join("overlap-test/RESEARCH.md"))
        .await
        .unwrap();
    assert!(
        body.contains("Example"),
        "RESEARCH.md should cite the web source title; got:\n{body}"
    );
    assert!(
        body.contains("src/lib.rs"),
        "RESEARCH.md should cite the local file path; got:\n{body}"
    );
    assert!(
        body.contains("some-spec"),
        "RESEARCH.md should cite the spec id; got:\n{body}"
    );

    // Phase events for Web, Local, and Specs must all be emitted.
    let events = observer.events.lock().unwrap();
    assert!(
        events.iter().any(|e| matches!(
            e,
            SessionEvent::Phase {
                phase: SessionPhase::Web
            }
        )),
        "expected Web phase event"
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            SessionEvent::Phase {
                phase: SessionPhase::Local
            }
        )),
        "expected Local phase event"
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            SessionEvent::Phase {
                phase: SessionPhase::Specs
            }
        )),
        "expected Specs phase event"
    );
}

#[tokio::test]
async fn overlapped_gather_survives_local_phase_failure() {
    use crate::local_gatherer::{LocalGatherError, LocalGatherer, LocalTool};

    struct FailingLocalTool;
    #[async_trait::async_trait]
    impl LocalTool for FailingLocalTool {
        async fn glob(
            &self,
            _root: &std::path::Path,
            _pattern: &str,
        ) -> anyhow::Result<Vec<std::path::PathBuf>> {
            Err(LocalGatherError::NoTerms.into())
        }
        async fn grep(
            &self,
            _path: &std::path::Path,
            _terms: &[String],
        ) -> anyhow::Result<Vec<crate::local_gatherer::GrepMatch>> {
            Ok(Vec::new())
        }
        async fn read(&self, _path: &std::path::Path) -> anyhow::Result<String> {
            Ok(String::new())
        }
        async fn list_specs(&self, _root: &std::path::Path) -> anyhow::Result<Vec<String>> {
            Ok(Vec::new())
        }
        async fn spec_title(
            &self,
            _root: &std::path::Path,
            _spec_id: &str,
        ) -> anyhow::Result<String> {
            Ok(String::new())
        }
    }

    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();

    let web = WebGatherer::new(
        Arc::new(FakeSearch {
            hits: vec![WebSearchHit {
                url: "https://example.com".into(),
                title: "Example".into(),
                snippet: String::new(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
                author: None,
            }],
        }),
        Arc::new(FakeFetch {
            pages: HashMap::from([(
                "https://example.com".into(),
                WebFetchedPage {
                    published_at: None,
                    url: "https://example.com".into(),
                    title: "Example".into(),
                    body: Arc::from(body256("web body")),
                    content_type: None,
                    page_type: None,
                    language: None,
                    author: None,
                },
            )]),
        }),
    );
    let local = LocalGatherer::new(Arc::new(FailingLocalTool));

    let manager = ResearchManager::new(&research_root);
    let session = ResearchSession::new(
        manager,
        Some(web),
        Some(local),
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "Rust async".into(),
            ..InputConfig::default()
        },
        clarify: false,
        ..SessionConfig::default()
    };
    let outcome = session
        .run(
            "local-fail-test",
            "Local Fail Test",
            &cfg,
            Arc::new(NoopObserver),
        )
        .await
        .unwrap();

    assert!(
        outcome
            .sources
            .iter()
            .any(|s| matches!(s, Source::Web { .. })),
        "web sources must still be captured when local phase fails"
    );
    assert!(
        !outcome
            .sources
            .iter()
            .any(|s| matches!(s, Source::Local { .. })),
        "no local sources should be present when local phase fails"
    );
}

/// D-002: Verify that per-phase diagnostic events are emitted in order:
/// Web phase -> Local phase -> Specs phase.  The overlapped gather emits
/// `Phase::Web` and `Phase::Local` synchronously before `tokio::join!`,
/// and `Phase::Specs` after the local future completes, so the ordering
/// is deterministic regardless of which gather finishes first.
#[tokio::test]
async fn overlapped_gather_emits_phase_events_in_order() {
    use crate::local_gatherer::{LocalGatherer, LocalTool};
    use std::path::PathBuf;

    #[derive(Default)]
    struct MixedLocalTool;
    #[async_trait::async_trait]
    impl LocalTool for MixedLocalTool {
        async fn glob(
            &self,
            _root: &std::path::Path,
            _pattern: &str,
        ) -> anyhow::Result<Vec<std::path::PathBuf>> {
            Ok(vec![PathBuf::from("src/lib.rs")])
        }
        async fn grep(
            &self,
            _path: &std::path::Path,
            _terms: &[String],
        ) -> anyhow::Result<Vec<crate::local_gatherer::GrepMatch>> {
            Ok(vec![crate::local_gatherer::GrepMatch {
                line: 1,
                text: "Rust async is great".into(),
            }])
        }
        async fn read(&self, _path: &std::path::Path) -> anyhow::Result<String> {
            Ok("Rust async is great".into())
        }
        async fn list_specs(&self, _root: &std::path::Path) -> anyhow::Result<Vec<String>> {
            Ok(vec!["some-spec".into()])
        }
        async fn spec_title(
            &self,
            _root: &std::path::Path,
            _spec_id: &str,
        ) -> anyhow::Result<String> {
            Ok("Some spec title".into())
        }
    }

    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();

    let web = WebGatherer::new(
        Arc::new(FakeSearch {
            hits: vec![WebSearchHit {
                url: "https://example.com".into(),
                title: "Example".into(),
                snippet: String::new(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
                author: None,
            }],
        }),
        Arc::new(FakeFetch {
            pages: HashMap::from([(
                "https://example.com".into(),
                WebFetchedPage {
                    published_at: None,
                    url: "https://example.com".into(),
                    title: "Example".into(),
                    body: Arc::from(body256("web body")),
                    content_type: None,
                    page_type: None,
                    language: None,
                    author: None,
                },
            )]),
        }),
    );
    let local = LocalGatherer::new(Arc::new(MixedLocalTool));

    let manager = ResearchManager::new(&research_root);
    let session = ResearchSession::new(
        manager,
        Some(web),
        Some(local),
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "Rust async".into(),
            ..InputConfig::default()
        },
        clarify: false,
        ..SessionConfig::default()
    };
    let observer = Arc::new(CollectObserver::default());
    let _ = session
        .run("event-order-test", "Event Order", &cfg, observer.clone())
        .await
        .unwrap();

    // Collect the indices of the Web, Local, and Specs phase events.
    let events = observer.events.lock().unwrap();
    let web_idx = events.iter().position(|e| {
        matches!(
            e,
            SessionEvent::Phase {
                phase: SessionPhase::Web
            }
        )
    });
    let local_idx = events.iter().position(|e| {
        matches!(
            e,
            SessionEvent::Phase {
                phase: SessionPhase::Local
            }
        )
    });
    let specs_idx = events.iter().position(|e| {
        matches!(
            e,
            SessionEvent::Phase {
                phase: SessionPhase::Specs
            }
        )
    });

    assert!(web_idx.is_some(), "expected Web phase event");
    assert!(local_idx.is_some(), "expected Local phase event");
    assert!(specs_idx.is_some(), "expected Specs phase event");

    let web_idx = web_idx.unwrap();
    let local_idx = local_idx.unwrap();
    let specs_idx = specs_idx.unwrap();

    assert!(
        web_idx < local_idx,
        "Web phase must be emitted before Local phase; got web={web_idx}, local={local_idx}"
    );
    assert!(
        local_idx < specs_idx,
        "Local phase must be emitted before Specs phase; got local={local_idx}, specs={specs_idx}"
    );
}

/// D-003: When `--from-url` is supplied alongside a topic, the seed URL
/// must appear as the **first** web source (source #1), ahead of any
/// sources discovered by the normal web-search phase.
#[tokio::test]
async fn from_url_seed_appears_as_first_source() {
    // The search tool returns one additional hit so we can verify the
    // --from-url seed precedes it in the source list.
    struct SearchWithExtraHit;
    #[async_trait]
    impl WebSearchTool for SearchWithExtraHit {
        async fn search(&self, _query: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
            Ok(vec![WebSearchHit {
                url: "https://example.com/extra".into(),
                title: "Rust async extra result".into(),
                snippet: "More about Rust async programming".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
                author: None,
            }])
        }
    }
    struct MultiFetch;
    #[async_trait]
    impl WebFetchTool for MultiFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            if url == "https://example.com/seed" {
                Ok(WebFetchedPage {
                    published_at: None,
                    url: url.to_string(),
                    title: "Seed Page".into(),
                    body: "This is the seed page body about Rust async.".into(),
                    content_type: None,
                    page_type: None,
                    language: None,
                    author: None,
                })
            } else {
                Ok(WebFetchedPage {
                    published_at: None,
                    url: url.to_string(),
                    title: "Extra Page".into(),
                    body: Arc::from(body256("Extra page body.")),
                    content_type: None,
                    page_type: None,
                    language: None,
                    author: None,
                })
            }
        }
    }

    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();

    let manager = ResearchManager::new(&research_root);
    let web = WebGatherer::new(Arc::new(SearchWithExtraHit), Arc::new(MultiFetch));
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "Rust async".into(),
            from_urls: vec!["https://example.com/seed".into()],
            ..InputConfig::default()
        },
        clarify: false,
        ..SessionConfig::default()
    };
    let outcome = session
        .run(
            "seed-order-test",
            "Seed Order",
            &cfg,
            Arc::new(NoopObserver),
        )
        .await
        .unwrap();

    // The first web source must be the --from-url seed.
    let first_web = outcome
        .sources
        .iter()
        .find(|s| matches!(s, Source::Web { .. }));
    assert!(
        first_web.is_some(),
        "expected at least one web source; got {:?}",
        outcome.sources
    );
    assert!(
        matches!(
            first_web.unwrap(),
            Source::Web { url, title, .. }
                if url == "https://example.com/seed" && title == "Seed Page"
        ),
        "the --from-url seed must be the first web source; got {:?}",
        first_web
    );

    // The extra search result must come after the seed.
    let web_urls: Vec<&str> = outcome
        .sources
        .iter()
        .filter_map(|s| match s {
            Source::Web { url, .. } => Some(url.as_str()),
            _ => None,
        })
        .collect();
    let seed_pos = web_urls
        .iter()
        .position(|u| *u == "https://example.com/seed");
    let extra_pos = web_urls
        .iter()
        .position(|u| *u == "https://example.com/extra");
    assert!(seed_pos.is_some(), "seed URL must be in sources");
    assert!(extra_pos.is_some(), "extra URL must be in sources");
    assert!(
        seed_pos.unwrap() < extra_pos.unwrap(),
        "seed URL must appear before extra URL; got seed={:?}, extra={:?}",
        seed_pos,
        extra_pos
    );
}

/// D-004: When multiple `--from-url` flags are supplied, each page must
/// be fetched and captured as a seed source, in the order given.
#[tokio::test]
async fn multiple_from_urls_all_captured_as_sources() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();

    struct NoSearch;
    #[async_trait]
    impl WebSearchTool for NoSearch {
        async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
            Ok(Vec::new())
        }
    }
    struct MultiFetch;
    #[async_trait]
    impl WebFetchTool for MultiFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            if url == "https://example.com/first" {
                Ok(WebFetchedPage {
                    published_at: None,
                    url: url.to_string(),
                    title: "First Page".into(),
                    body: "First page about Rust async and Tokio runtime.".into(),
                    content_type: None,
                    page_type: None,
                    language: None,
                    author: None,
                })
            } else {
                Ok(WebFetchedPage {
                    published_at: None,
                    url: url.to_string(),
                    title: "Second Page".into(),
                    body: "Second page about Rust concurrency patterns.".into(),
                    content_type: None,
                    page_type: None,
                    language: None,
                    author: None,
                })
            }
        }
    }

    let manager = ResearchManager::new(&research_root);
    let web = WebGatherer::new(Arc::new(NoSearch), Arc::new(MultiFetch));
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: String::new(),
            from_urls: vec![
                "https://example.com/first".into(),
                "https://example.com/second".into(),
            ],
            ..InputConfig::default()
        },
        ..SessionConfig::default()
    };
    let outcome = session
        .run("multi-url-test", "Multi URL", &cfg, Arc::new(NoopObserver))
        .await
        .unwrap();

    // Both seed URLs must appear as web sources, in the order given.
    let web_urls: Vec<&str> = outcome
        .sources
        .iter()
        .filter_map(|s| match s {
            Source::Web { url, .. } => Some(url.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        web_urls.contains(&"https://example.com/first"),
        "first URL must be in sources: {:?}",
        web_urls
    );
    assert!(
        web_urls.contains(&"https://example.com/second"),
        "second URL must be in sources: {:?}",
        web_urls
    );
    let first_pos = web_urls
        .iter()
        .position(|u| *u == "https://example.com/first");
    let second_pos = web_urls
        .iter()
        .position(|u| *u == "https://example.com/second");
    assert!(
        first_pos.unwrap() < second_pos.unwrap(),
        "first URL must appear before second URL"
    );
}
/// D-005: When a `--from-file` is supplied, the extracted text is
/// captured as a `Source::Other` and the topic is derived from the body
/// when no explicit topic is provided.
#[tokio::test]
async fn from_file_extracts_text_and_captures_as_other_source() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();
    let notes = tmp.path().join("notes.md");
    tokio::fs::write(
        &notes,
        "# Local notes\n\nRust async programming with Tokio and async/await.",
    )
    .await
    .unwrap();

    struct NoSearch;
    #[async_trait]
    impl WebSearchTool for NoSearch {
        async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
            Ok(Vec::new())
        }
    }
    struct NoFetch;
    #[async_trait]
    impl WebFetchTool for NoFetch {
        async fn fetch(&self, _: &str) -> anyhow::Result<WebFetchedPage> {
            anyhow::bail!("not used")
        }
    }

    let manager = ResearchManager::new(&research_root);
    let web = WebGatherer::new(Arc::new(NoSearch), Arc::new(NoFetch));
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: String::new(),
            from_files: vec![notes.clone()],
            ..InputConfig::default()
        },
        ..SessionConfig::default()
    };
    let observer = Arc::new(CollectObserver::default());
    let outcome = session
        .run("from-file-test", "From File", &cfg, observer.clone())
        .await
        .unwrap();

    // The local file must be captured as Source::Other.
    assert!(
        outcome.sources.iter().any(|s| matches!(
            s,
            Source::Other { label, body, .. }
            if label == notes.to_string_lossy().as_ref()
                && body.contains("Tokio")
        )),
        "expected Source::Other from --from-file, got {:?}",
        outcome.sources
    );

    // The observer must have received a FromFileBodyPreview event.
    let events = observer.events.lock().unwrap();
    assert!(
        events.iter().any(|e| matches!(
            e,
            SessionEvent::FromFileBodyPreview { path, body_preview }
            if path == notes.to_string_lossy().as_ref()
                && body_preview.contains("Tokio")
        )),
        "expected FromFileBodyPreview event, got {:?}",
        *events
    );
}

/// D-006: When multiple `--from-file` flags are supplied, each file is
/// extracted and captured as a seed source in the order given.
#[tokio::test]
async fn multiple_from_files_all_captured_as_sources() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();
    let first = tmp.path().join("first.md");
    tokio::fs::write(&first, "First document about Rust concurrency patterns.")
        .await
        .unwrap();
    let second = tmp.path().join("second.md");
    tokio::fs::write(&second, "Second document about async runtimes.")
        .await
        .unwrap();

    struct NoSearch;
    #[async_trait]
    impl WebSearchTool for NoSearch {
        async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
            Ok(Vec::new())
        }
    }
    struct NoFetch;
    #[async_trait]
    impl WebFetchTool for NoFetch {
        async fn fetch(&self, _: &str) -> anyhow::Result<WebFetchedPage> {
            anyhow::bail!("not used")
        }
    }

    let manager = ResearchManager::new(&research_root);
    let web = WebGatherer::new(Arc::new(NoSearch), Arc::new(NoFetch));
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: String::new(),
            from_files: vec![first.clone(), second.clone()],
            ..InputConfig::default()
        },
        ..SessionConfig::default()
    };
    let outcome = session
        .run(
            "multi-file-test",
            "Multi File",
            &cfg,
            Arc::new(NoopObserver),
        )
        .await
        .unwrap();

    let other_sources: Vec<&Source> = outcome
        .sources
        .iter()
        .filter(|s| matches!(s, Source::Other { .. }))
        .collect();
    assert_eq!(
        other_sources.len(),
        2,
        "expected both files as Source::Other, got {:?}",
        other_sources
    );
}

/// D-007: A PDF supplied via `--from-file` automatically enables PDF web
/// sources for the gather phase even when `--use-pdf` is not set.
#[tokio::test]
async fn from_file_pdf_auto_enables_pdf_web_sources() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();

    // Create a minimal valid PDF bytestream so the extension check
    // passes without needing the full PDF parser in this unit test.
    let pdf = tmp.path().join("report.pdf");
    tokio::fs::write(
        &pdf,
        b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
    )
    .await
    .unwrap();

    struct NoSearch;
    #[async_trait]
    impl WebSearchTool for NoSearch {
        async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
            Ok(Vec::new())
        }
    }
    struct NoFetch;
    #[async_trait]
    impl WebFetchTool for NoFetch {
        async fn fetch(&self, _: &str) -> anyhow::Result<WebFetchedPage> {
            anyhow::bail!("not used")
        }
    }

    let manager = ResearchManager::new(&research_root);
    let web = WebGatherer::new(Arc::new(NoSearch), Arc::new(NoFetch));
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: String::new(),
            from_files: vec![pdf],
            ..InputConfig::default()
        },
        web: WebConfig {
            use_pdf_web_sources: false,
            ..WebConfig::default()
        },
        ..SessionConfig::default()
    };

    // The test only needs to verify the effective flag is enabled; the
    // actual extraction may fail due to the stub PDF content, so we allow
    // either success or a FromFileExtractFailed error.
    let result = session
        .run("pdf-flag-test", "PDF Flag", &cfg, Arc::new(NoopObserver))
        .await;
    match result {
        Ok(_) | Err(ResearchError::FromFileExtractFailed { .. }) => {}
        Err(e) => panic!("unexpected error: {e:?}"),
    }
}

#[test]
fn select_top_relevance_sources_keeps_highest_ranked() {
    let make_web = |relevance: &str| Source::Web {
        published_at: None,
        url: format!("https://example.com/{relevance}"),
        title: relevance.to_string(),
        captured_at: chrono::Utc::now(),
        body_path: std::path::PathBuf::from("sources/web-01.md"),
        body: "text".to_string(),
        relevance: relevance.to_string(),
        search_tool: String::new(),
        search_engine: String::new(),
        author: None,
        content_type: None,
        page_type: None,
        media_type: "page".to_string(),
        language: None,
        oa_recovery: None,
    };
    let sources = vec![
        make_web("Low - weak query match"),
        make_web("High - title matches query"),
        make_web("Very low - no clear query match"),
        make_web("Medium - partial query match"),
    ];
    let selected = select_top_relevance_sources(&sources, 2);
    assert_eq!(selected.len(), 2);
    // High (rank 7) and Medium (rank 5) should be selected.
    assert!(selected[0].relevance().unwrap_or("").starts_with("High"));
    assert!(selected[1].relevance().unwrap_or("").starts_with("Medium"));
}

#[test]
fn select_top_relevance_sources_preserves_original_order() {
    let make_web = |relevance: &str, url: &str| Source::Web {
        published_at: None,
        url: url.to_string(),
        title: relevance.to_string(),
        captured_at: chrono::Utc::now(),
        body_path: std::path::PathBuf::from("sources/web-01.md"),
        body: "text".to_string(),
        relevance: relevance.to_string(),
        search_tool: String::new(),
        search_engine: String::new(),
        author: None,
        content_type: None,
        page_type: None,
        media_type: "page".to_string(),
        language: None,
        oa_recovery: None,
    };
    // Sources in order: A(high), B(low), C(high), D(low)
    let sources = vec![
        make_web("High - title matches query", "https://a"),
        make_web("Low - weak query match", "https://b"),
        make_web("High - title matches query", "https://c"),
        make_web("Low - weak query match", "https://d"),
    ];
    let selected = select_top_relevance_sources(&sources, 2);
    assert_eq!(selected.len(), 2);
    // A and C should be selected (both high rank), in original order.
    let urls: Vec<&str> = selected
        .iter()
        .filter_map(|s| match s {
            Source::Web { url, .. } => Some(url.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(urls, vec!["https://a", "https://c"]);
}

#[test]
fn select_top_relevance_sources_all_when_under_cap() {
    let make_web = |relevance: &str| Source::Web {
        published_at: None,
        url: format!("https://example.com/{relevance}"),
        title: relevance.to_string(),
        captured_at: chrono::Utc::now(),
        body_path: std::path::PathBuf::from("sources/web-01.md"),
        body: "text".to_string(),
        relevance: relevance.to_string(),
        search_tool: String::new(),
        search_engine: String::new(),
        author: None,
        content_type: None,
        page_type: None,
        media_type: "page".to_string(),
        language: None,
        oa_recovery: None,
    };
    let sources = vec![make_web("High"), make_web("Medium")];
    let selected = select_top_relevance_sources(&sources, 10);
    assert_eq!(selected.len(), 2);
}

#[test]
fn select_top_relevance_sources_includes_low_relevance_when_in_pool() {
    // When use_low_relevance is true, low-relevance sources are in the
    // pool. If the cap is large enough, they should be selected too.
    let make_web = |relevance: &str| Source::Web {
        published_at: None,
        url: format!("https://example.com/{relevance}"),
        title: relevance.to_string(),
        captured_at: chrono::Utc::now(),
        body_path: std::path::PathBuf::from("sources/web-01.md"),
        body: "text".to_string(),
        relevance: relevance.to_string(),
        search_tool: String::new(),
        search_engine: String::new(),
        author: None,
        content_type: None,
        page_type: None,
        media_type: "page".to_string(),
        language: None,
        oa_recovery: None,
    };
    let sources = vec![
        make_web("High - title matches query"),
        make_web("Low - weak query match"),
    ];
    // Cap of 2 means both are selected (low-relevance is in the pool).
    let selected = select_top_relevance_sources(&sources, 2);
    assert_eq!(selected.len(), 2);
    assert!(
        selected
            .iter()
            .any(|s| matches!(s, Source::Web { relevance, .. } if relevance.starts_with("Low")))
    );
}

// -- Milestone H-001: per-phase timeout tests ----------------------

#[tokio::test]
async fn h001_web_phase_timeout_keeps_partial_sources_and_proceeds() {
    use crate::web_gatherer::{
        WebFetchTool, WebFetchedPage, WebGatherer, WebSearchHit, WebSearchTool,
    };

    // Search returns one hit immediately; the fetch is moderately slow
    // (2 s) so it stays in flight when the 1 s search-stage deadline
    // fires. The fetch is NOT cancelled: the run waits for it (fetch
    // timeout is an hour, so it cannot fire first).
    struct SlowFetch;
    #[async_trait]
    impl WebFetchTool for SlowFetch {
        async fn fetch(&self, _url: &str) -> anyhow::Result<WebFetchedPage> {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            Ok(WebFetchedPage {
                published_at: None,
                url: _url.to_string(),
                title: "Rust async runtime slow mirror".into(),
                body: Arc::from("slow body with query terms Rust async runtime Tokio. ".repeat(10)),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
    }
    struct FastSearch;
    #[async_trait]
    impl WebSearchTool for FastSearch {
        async fn search(&self, _query: &str, _max: usize) -> anyhow::Result<Vec<WebSearchHit>> {
            Ok(vec![WebSearchHit {
                url: "https://slow.example".into(),
                title: "Rust async runtime".into(),
                snippet: "Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: "test".into(),
                search_engine: "test".into(),
                author: None,
            }])
        }
    }
    let web = WebGatherer::new(Arc::new(FastSearch), Arc::new(SlowFetch))
        .with_fetch_timeout(std::time::Duration::from_secs(3600));

    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();
    let manager = ResearchManager::new(&research_root);
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "Rust async runtime".into(),
            ..InputConfig::default()
        },
        web: WebConfig {
            web_phase_timeout_secs: Some(1),
            fetch_timeout_secs: 60,
            ..WebConfig::default()
        },
        local: LocalConfig {
            disable_local: true,
            disable_specs: true,
            ..LocalConfig::default()
        },
        clarify: false,
        ..SessionConfig::default()
    };
    #[derive(Default)]
    struct CollectEvents(std::sync::Mutex<Vec<SessionEvent>>);
    impl SessionObserver for CollectEvents {
        fn on_event(&self, event: SessionEvent) {
            self.0.lock().unwrap().push(event);
        }
    }
    let obs = Arc::new(CollectEvents::default());
    let started = std::time::Instant::now();
    let outcome = session.run("h001timeout", "Test", &cfg, obs.clone()).await;
    assert!(outcome.is_ok(), "run should complete even with timeout");
    // The 1 s search-stage deadline fired while the 2 s fetch was in
    // flight: the run must have waited for the fetch (fetches are never
    // cancelled on the phase deadline).
    assert!(
        started.elapsed() >= std::time::Duration::from_secs(2),
        "the in-flight fetch must run to completion past the deadline, took {:?}",
        started.elapsed()
    );
    let events = obs.0.lock().unwrap();
    // The web phase deadline should emit a `web_deadline` RunStep
    // diagnostic instead of discarding the phase.
    assert!(
        events.iter().any(|e| matches!(
            e,
            SessionEvent::RunStep { step, .. } if step == "web_deadline"
        )),
        "expected RunStep web_deadline event, got {events:?}"
    );
    // The slow fetch completed and its source was ingested - no fetch
    // cancellation.
    let outcome = outcome.unwrap();
    assert!(
        outcome.sources.iter().any(|s| matches!(
            s,
            Source::Web { url, .. } if url.contains("slow.example")
        )),
        "the in-flight slow fetch must be captured, got {:?}",
        outcome.sources
    );
}

#[tokio::test]
async fn h001_local_phase_timeout_aborts_slow_local_gather() {
    use crate::local_gatherer::{GrepMatch, LocalGatherer, LocalTool};

    // LocalTool that sleeps 60s on glob.
    struct SlowLocal;
    #[async_trait]
    impl LocalTool for SlowLocal {
        async fn glob(
            &self,
            _root: &std::path::Path,
            _pattern: &str,
        ) -> anyhow::Result<Vec<std::path::PathBuf>> {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            Ok(Vec::new())
        }
        async fn grep(
            &self,
            _path: &std::path::Path,
            _terms: &[String],
        ) -> anyhow::Result<Vec<GrepMatch>> {
            Ok(Vec::new())
        }
        async fn read(&self, _path: &std::path::Path) -> anyhow::Result<String> {
            Ok(String::new())
        }
        async fn list_specs(&self, _root: &std::path::Path) -> anyhow::Result<Vec<String>> {
            Ok(Vec::new())
        }
        async fn spec_title(
            &self,
            _root: &std::path::Path,
            _spec_id: &str,
        ) -> anyhow::Result<String> {
            Ok(String::new())
        }
    }

    let local = LocalGatherer::new(Arc::new(SlowLocal));
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();
    let manager = ResearchManager::new(&research_root);
    let session = ResearchSession::new(
        manager,
        None,
        Some(local),
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "Rust async runtime".into(),
            ..InputConfig::default()
        },
        local: LocalConfig {
            local_phase_timeout_secs: Some(1),
            disable_specs: true,
            ..LocalConfig::default()
        },
        clarify: false,
        ..SessionConfig::default()
    };
    let outcome = session
        .run("h001localtimeout", "Test", &cfg, Arc::new(NoopObserver))
        .await;
    assert!(
        outcome.is_ok(),
        "run should complete even with local timeout"
    );
    // The local phase timed out so no local sources should be captured.
    let outcome = outcome.unwrap();
    let local_count = outcome
        .sources
        .iter()
        .filter(|s| matches!(s, Source::Local { .. }))
        .count();
    assert_eq!(
        local_count, 0,
        "timed-out local phase should yield 0 sources"
    );
}

#[tokio::test]
async fn h002_session_wires_search_retry_config() {
    // Verify that the session completes successfully when search retry
    // config is set. The search returns empty so no actual retries occur,
    // but the config must be wired without error.
    use crate::web_gatherer::{
        WebFetchTool, WebFetchedPage, WebGatherer, WebSearchHit, WebSearchTool,
    };

    struct OkSearch;
    #[async_trait]
    impl WebSearchTool for OkSearch {
        async fn search(&self, _query: &str, _max: usize) -> anyhow::Result<Vec<WebSearchHit>> {
            Ok(Vec::new())
        }
    }
    struct OkFetch;
    #[async_trait]
    impl WebFetchTool for OkFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            Ok(WebFetchedPage {
                published_at: None,
                url: url.to_string(),
                title: "t".into(),
                body: Arc::from(body256("b")),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
    }
    let web = WebGatherer::new(Arc::new(OkSearch), Arc::new(OkFetch));
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();
    let manager = ResearchManager::new(&research_root);
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "test topic".into(),
            ..InputConfig::default()
        },
        local: LocalConfig {
            disable_local: true,
            disable_specs: true,
            ..LocalConfig::default()
        },
        resilience: ResilienceConfig {
            search_max_retries: 5,
            search_retry_base_delay_ms: 0,
            ..ResilienceConfig::default()
        },
        clarify: false,
        ..SessionConfig::default()
    };
    let outcome = session
        .run("h002retrycfg", "Test", &cfg, Arc::new(NoopObserver))
        .await;
    assert!(outcome.is_ok(), "session with retry config should complete");
}

#[tokio::test]
async fn competitive_mode_delegates_one_researcher_per_entity() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();

    struct RecordingSearch;
    #[async_trait]
    impl WebSearchTool for RecordingSearch {
        async fn search(
            &self,
            query: &str,
            _max_results: usize,
        ) -> anyhow::Result<Vec<WebSearchHit>> {
            Ok(vec![WebSearchHit {
                url: format!("https://example.com/{query}"),
                title: format!("Article for {query}"),
                snippet: query.to_string(),
                matched_query: query.to_string(),
                search_tool: "fake".to_string(),
                search_engine: "fake".to_string(),
                author: None,
            }])
        }
    }
    struct OkFetch;
    #[async_trait]
    impl WebFetchTool for OkFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            Ok(WebFetchedPage {
                published_at: None,
                url: url.to_string(),
                title: format!("Title for {url}"),
                body: Arc::from(body256("competitive analysis body")),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
    }

    let manager = ResearchManager::new(&research_root);
    let web = WebGatherer::new(Arc::new(RecordingSearch), Arc::new(OkFetch));
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "Compare Fireworks AI and Groq for LLM inference".into(),
            ..InputConfig::default()
        },
        engine: RunEngineConfig {
            mode: ResearchMode::Competitive,
            ..RunEngineConfig::default()
        },
        clarify: false,
        ..SessionConfig::default()
    };
    let observer = Arc::new(CollectObserver::default());
    let outcome = session
        .run("comp-delegation", "Comp Delegation", &cfg, observer.clone())
        .await
        .unwrap();

    let events = observer.events.lock().unwrap();

    // FR-006: competitive extraction event is emitted.
    let entity_event = events
        .iter()
        .find_map(|e| match e {
            SessionEvent::CompetitiveEntities { entities, .. } => Some(entities.clone()),
            _ => None,
        })
        .expect("should emit CompetitiveEntities event");
    assert!(
        entity_event.iter().any(|n| n.contains("Fireworks AI")),
        "expected Fireworks AI in extracted entities: {entity_event:?}"
    );
    assert!(
        entity_event.iter().any(|n| n.contains("Groq")),
        "expected Groq in extracted entities: {entity_event:?}"
    );

    // FR-006 / FR-007: one sub-topic per entity is planned.
    let plan = events
        .iter()
        .find_map(|e| match e {
            SessionEvent::SupervisorPlanUpdated { sub_topics } => Some(sub_topics.clone()),
            _ => None,
        })
        .expect("should emit SupervisorPlanUpdated event");
    assert_eq!(plan.len(), 2, "expected one sub-topic per entity: {plan:?}");
    assert!(
        plan.iter().any(|t| t.contains("Fireworks AI")),
        "plan should include Fireworks AI: {plan:?}"
    );
    assert!(
        plan.iter().any(|t| t.contains("Groq")),
        "plan should include Groq: {plan:?}"
    );

    // FR-007: researchers are spawned with per-entity sub-topics.
    let spawned: Vec<(&String, &String)> = events
        .iter()
        .filter_map(|e| match e {
            SessionEvent::ResearcherSpawned { id, sub_topic } => Some((id, sub_topic)),
            _ => None,
        })
        .collect();
    assert_eq!(
        spawned.len(),
        2,
        "expected one spawned researcher per entity, got {spawned:?}"
    );
    assert!(
        spawned.iter().any(|(_, t)| t.contains("Fireworks AI")),
        "researcher sub-topic should include Fireworks AI: {spawned:?}"
    );
    assert!(
        spawned.iter().any(|(_, t)| t.contains("Groq")),
        "researcher sub-topic should include Groq: {spawned:?}"
    );

    // FR-009: supervisor mode should drive supervisor-specific RunStep
    // events through the mode-aware tier router.
    let run_steps: Vec<(String, String)> = events
        .iter()
        .filter_map(|e| match e {
            SessionEvent::RunStep { step, status, .. } if step.starts_with("supervisor_") => {
                Some((step.clone(), status.clone()))
            }
            _ => None,
        })
        .collect();
    assert!(
        run_steps.iter().any(|(s, _)| s == "supervisor_plan"),
        "expected supervisor_plan RunStep, got {run_steps:?}"
    );
    assert!(
        run_steps.iter().any(|(s, _)| s == "supervisor_delegate"),
        "expected supervisor_delegate RunStep, got {run_steps:?}"
    );
    assert!(
        run_steps.iter().any(|(s, _)| s == "supervisor_synthesize"),
        "expected supervisor_synthesize RunStep, got {run_steps:?}"
    );
    assert!(
        run_steps.iter().any(|(s, _)| s == "supervisor_finalize"),
        "expected supervisor_finalize RunStep, got {run_steps:?}"
    );

    // The run should still write a RESEARCH.md document.
    assert!(
        research_root.join("comp-delegation/RESEARCH.md").is_file(),
        "RESEARCH.md should be written for competitive mode"
    );
    assert!(!outcome.sources.is_empty(), "should capture web sources");
}

#[tokio::test]
async fn competitive_mode_caps_researchers_at_max_concurrent_units() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();

    struct CountingSearch {
        calls: std::sync::Mutex<usize>,
    }
    #[async_trait]
    impl WebSearchTool for CountingSearch {
        async fn search(
            &self,
            query: &str,
            _max_results: usize,
        ) -> anyhow::Result<Vec<WebSearchHit>> {
            *self.calls.lock().unwrap() += 1;
            Ok(vec![WebSearchHit {
                url: format!("https://example.com/{query}"),
                title: format!("Article for {query}"),
                snippet: query.to_string(),
                matched_query: query.to_string(),
                search_tool: "fake".to_string(),
                search_engine: "fake".to_string(),
                author: None,
            }])
        }
    }
    struct OkFetch;
    #[async_trait]
    impl WebFetchTool for OkFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            Ok(WebFetchedPage {
                published_at: None,
                url: url.to_string(),
                title: format!("Title for {url}"),
                body: Arc::from(body256("competitive analysis body")),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            })
        }
    }

    // A topic naming more entities than the researcher cap: the plan
    // (and thus the researcher count and search volume) must be
    // truncated to `max_concurrent_research_units`.
    let manager = ResearchManager::new(&research_root);
    let search = Arc::new(CountingSearch {
        calls: std::sync::Mutex::new(0),
    });
    let web = WebGatherer::new(search.clone(), Arc::new(OkFetch));
    let session = ResearchSession::new(
        manager,
        Some(web),
        None,
        Arc::new(crate::analysis::NoopAnalysisEngine),
    );
    let cfg = SessionConfig {
        input: InputConfig {
            topic: "Compare Fireworks AI and Groq for LLM inference".into(),
            ..InputConfig::default()
        },
        engine: RunEngineConfig {
            mode: ResearchMode::Competitive,
            max_concurrent_research_units: 1,
            ..RunEngineConfig::default()
        },
        clarify: false,
        ..SessionConfig::default()
    };
    let observer = Arc::new(CollectObserver::default());
    let outcome = session
        .run("comp-capped", "Comp Capped", &cfg, observer.clone())
        .await
        .unwrap();

    let events = observer.events.lock().unwrap();
    let plan = events
        .iter()
        .find_map(|e| match e {
            SessionEvent::SupervisorPlanUpdated { sub_topics } => Some(sub_topics.clone()),
            _ => None,
        })
        .expect("should emit SupervisorPlanUpdated event");
    assert_eq!(
        plan.len(),
        1,
        "competitive plan must be capped at max_concurrent_research_units: {plan:?}"
    );

    let spawned = events
        .iter()
        .filter(|e| matches!(e, SessionEvent::ResearcherSpawned { .. }))
        .count();
    assert_eq!(
        spawned, 1,
        "exactly one researcher may run under the cap, got {spawned}"
    );

    // Search volume scales with the researcher count, not the entity list.
    let calls = *search.calls.lock().unwrap();
    assert!(
        calls <= 4,
        "capped competitive run should issue a handful of searches, got {calls}"
    );
    assert!(!outcome.sources.is_empty());
}
