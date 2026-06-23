//! `ResearchSession` — the gathering orchestration engine.
//!
//! Combines the [`WebGatherer`] (T-014), [`LocalGatherer`] (T-016), and
//! [`LocalGatherer`] cross-referencing (T-018) into a single pass that
//! produces a fully-populated [`ResearchDocument`] ready for
//! [`ResearchManager::write_document`].
//!
//! This is the engine the TUI `/research create` slash command, the CLI
//! `ragent research create` sub-command, and the `POST /research` HTTP
//! endpoint all call (T-019, T-027, T-034, T-036).

use crate::document::{ResearchDocument, mark_in_progress};
use crate::io::ResearchIo;
use crate::item::ResearchItem;
use crate::local_gatherer::{LocalGatherConfig, LocalGatherer, LocalTool};
use crate::manager::{ResearchError, ResearchManager, Result};
use crate::research_name::ResearchName;
use crate::source::{LocalSourceKind, Source};
use crate::web_gatherer::WebGatherer;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::info;

/// Inputs the caller supplies to [`ResearchSession::run`].
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Free-form research topic — used to derive web queries and grep terms.
    pub topic: String,
    /// Optional FR-019 extra sources directory.
    pub sources_dir: Option<PathBuf>,
    /// Optional FR-020 template file (resolved against `_templates/`).
    pub template: Option<String>,
    /// Maximum web sources to capture (default `5`).
    pub max_web_results: usize,
    /// Maximum in-project local sources to capture (default `10`).
    pub max_local_sources: usize,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            topic: String::new(),
            sources_dir: None,
            template: None,
            max_web_results: 5,
            max_local_sources: 10,
        }
    }
}

/// Phases of a research session, in execution order. Surfaced via the
/// [`SessionEvent::Phase`] callback so the TUI log panel and the CLI JSON
/// emitter can show progress (T-027, T-035).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPhase {
    /// Validating the supplied name and creating the item directory.
    Setup,
    /// Issuing web searches and fetching pages.
    Web,
    /// Scanning the project and any extra sources dir.
    Local,
    /// Cross-referencing prior specs.
    Specs,
    /// Assembling the final `RESEARCH.md`.
    Assemble,
    /// Marking the item `Complete` and refreshing the index.
    Finalize,
}

impl SessionPhase {
    /// Human-readable label for log output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Web => "web",
            Self::Local => "local",
            Self::Specs => "specs",
            Self::Assemble => "assemble",
            Self::Finalize => "finalize",
        }
    }
}

/// Progress event emitted as a research session runs. The TUI/CLI/HTTP
/// layers subscribe to this to render streaming progress.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// A new phase has started.
    Phase {
        /// The phase that just started.
        phase: SessionPhase,
    },
    /// The web-gathering phase captured a single source.
    WebCaptured {
        /// URL of the captured page.
        url: String,
        /// Page title (may be empty).
        title: String,
    },
    /// The local-gathering phase scored and captured a file.
    LocalCaptured {
        /// Project-relative path of the captured file.
        path: String,
        /// Relevance score from the keyword matcher.
        score: usize,
    },
    /// The session captured a prior spec as a cross-reference.
    SpecCaptured {
        /// Spec identifier.
        spec_id: String,
    },
    /// The session has finished and a fully-populated document was written.
    Done {
        /// Total number of sources captured.
        total_sources: usize,
    },
}

/// Trait implemented by the TUI/CLI/HTTP callers to receive streaming
/// progress. The default [`NoopObserver`] discards all events.
pub trait SessionObserver: Send + Sync {
    /// Receive a progress event. Implementations should be cheap; the
    /// session calls this once per source.
    fn on_event(&self, event: SessionEvent);
}

/// Default observer that drops all events. Used when the caller doesn't
/// need progress streaming.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopObserver;

impl SessionObserver for NoopObserver {
    fn on_event(&self, _event: SessionEvent) {}
}

/// Orchestrates a single research session.
///
/// `ResearchSession` is cheap to clone (internally `Arc`s) so the TUI, CLI,
/// and HTTP layer can hold one instance per request and call
/// [`ResearchSession::run`] concurrently.
#[derive(Clone)]
pub struct ResearchSession {
    manager: ResearchManager,
    web: Option<WebGatherer>,
    local: Option<LocalGatherer>,
}

impl std::fmt::Debug for ResearchSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResearchSession")
            .field("research_root", &self.manager.root())
            .field("has_web", &self.web.is_some())
            .field("has_local", &self.local.is_some())
            .finish()
    }
}

impl ResearchSession {
    /// Build a session over the given on-disk manager. Both web and local
    /// gatherers are optional; a session with neither is effectively a no-op
    /// (FR-006 graceful degradation).
    pub fn new(
        manager: ResearchManager,
        web: Option<WebGatherer>,
        local: Option<LocalGatherer>,
    ) -> Self {
        Self {
            manager,
            web,
            local,
        }
    }

    /// Build a session backed only by a local tool (no web search).
    pub fn with_local_tool(manager: ResearchManager, local_tool: Arc<dyn LocalTool>) -> Self {
        Self::new(manager, None, Some(LocalGatherer::new(local_tool)))
    }

    /// Run a complete research session end-to-end. The flow is:
    ///
    /// 1. Validate name + create the on-disk item (if absent).
    /// 2. Mark the item `InProgress` and load the optional template.
    /// 3. Run web-gathering (T-014, T-015).
    /// 4. Run local-gathering (T-016, T-017, T-018).
    /// 5. Cross-reference prior specs (T-018).
    /// 6. Assemble `RESEARCH.md` (T-020, T-021, T-022).
    /// 7. Persist + mark `Complete` (T-012, T-013).
    pub async fn run(
        &self,
        name_str: &str,
        title: &str,
        config: &SessionConfig,
        observer: Arc<dyn SessionObserver>,
    ) -> Result<RunOutcome> {
        let name = ResearchName::try_new(name_str).map_err(ResearchError::InvalidName)?;
        let project_root = project_root_for(self.manager.root()).to_path_buf();

        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::Setup,
        });
        let item_exists = ResearchIo::item_exists(self.manager.root(), &name).await;
        let mut item = if item_exists {
            self.manager.show(name_str).await?
        } else {
            self.manager.create(name_str, title, &config.topic).await?
        };
        mark_in_progress(&mut item);
        self.manager.start_gathering(name_str).await?;
        let topic = if config.topic.is_empty() {
            item.topic.clone()
        } else {
            config.topic.clone()
        };
        let template_body = load_template(self.manager.root(), config.template.as_deref()).await;

        // ── Web phase ──────────���──────────────────────────────────────────
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::Web,
        });
        let mut sources = Vec::new();
        if let Some(web) = &self.web {
            match web.gather(&topic, config.max_web_results).await {
                Ok(web_sources) => {
                    for src in &web_sources {
                        if let Source::Web { url, title, .. } = src {
                            observer.on_event(SessionEvent::WebCaptured {
                                url: url.clone(),
                                title: title.clone(),
                            });
                        }
                    }
                    sources.extend(web_sources);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "research: web phase failed; continuing");
                }
            }
        }

        // ── Local phase ───────────────────────────────────────────────────
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::Local,
        });
        let local_gathered = if let Some(local) = &self.local {
            let cfg = LocalGatherConfig {
                max_local_sources: config.max_local_sources,
                ..LocalGatherConfig::default()
            };
            match local
                .gather(&project_root, &topic, config.sources_dir.as_deref(), &cfg)
                .await
            {
                Ok(s) => {
                    for src in &s {
                        if let Source::Local {
                            path, relevance, ..
                        } = src
                        {
                            let score = relevance
                                .split_whitespace()
                                .next()
                                .and_then(|n| n.parse::<usize>().ok())
                                .unwrap_or(1);
                            observer.on_event(SessionEvent::LocalCaptured {
                                path: path.clone(),
                                score,
                            });
                        }
                    }
                    s
                }
                Err(e) => {
                    tracing::warn!(error = %e, "research: local phase failed; continuing");
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };
        sources.extend(local_gathered);

        // ── Spec phase ────────────────────────────────────────────────────
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::Specs,
        });
        let spec_sources: Vec<Source> = sources
            .iter()
            .filter(|s| matches!(s, Source::Spec { .. }))
            .cloned()
            .collect();
        for src in &spec_sources {
            if let Source::Spec { spec_id, .. } = src {
                observer.on_event(SessionEvent::SpecCaptured {
                    spec_id: spec_id.clone(),
                });
            }
        }

        // ── Assemble ──────────────────────────────────────────────────────
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::Assemble,
        });
        let mut item_with_sources = ResearchItem::new(name.clone(), title, &topic);
        for s in &sources {
            item_with_sources.add_source(s.clone());
        }
        let doc = ResearchDocument {
            item: item_with_sources,
            summary: default_summary(&sources, &topic),
            findings: default_findings(&sources, &topic),
            cross_references: cross_references_from(&sources),
            open_questions: Vec::new(),
            template_body,
        };
        let assembled = self.manager.write_document(&doc).await?;

        // ── Finalize ──────────────────────────────────────────────────────
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::Finalize,
        });
        self.manager.complete_gathering(name_str).await?;

        let total_sources = sources.len();
        observer.on_event(SessionEvent::Done { total_sources });

        info!(
            name = %name,
            total = total_sources,
            "research: session complete"
        );

        Ok(RunOutcome {
            research_name: name.to_string(),
            sources,
            document: assembled,
        })
    }
}

/// What [`ResearchSession::run`] returns to the caller.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    /// The validated research name.
    pub research_name: String,
    /// Every captured source (web + local + spec).
    pub sources: Vec<Source>,
    /// The fully assembled document that was written to disk.
    pub document: crate::document::AssembledDocument,
}

// ── Free helpers ─────────────────────────────────────────────────────────

/// Compute the project root from the `research/` root (its parent).
fn project_root_for(research_root: &Path) -> &Path {
    research_root.parent().unwrap_or(research_root)
}

/// Load a FR-020 template body from `_templates/<name>.md` if it exists.
/// Returns `None` when no template was requested, or when the file does
/// not exist.
async fn load_template(research_root: &Path, template: Option<&str>) -> Option<String> {
    let name = template?;
    let path = ResearchIo::template_path(research_root, name);
    match tokio::fs::read_to_string(&path).await {
        Ok(body) => Some(body),
        Err(e) => {
            tracing::warn!(
                template = %name,
                path = %path.display(),
                error = %e,
                "research: template not loaded"
            );
            None
        }
    }
}

fn default_summary(sources: &[Source], topic: &str) -> String {
    let web_count = sources
        .iter()
        .filter(|s| matches!(s, Source::Web { .. }))
        .count();
    let local_count = sources
        .iter()
        .filter(|s| matches!(s, Source::Local { .. }))
        .count();
    let spec_count = sources
        .iter()
        .filter(|s| matches!(s, Source::Spec { .. }))
        .count();
    format!(
        "Gathered {total} sources for '{topic}' ({web} web, {local} local, {spec} spec).",
        total = sources.len(),
        topic = topic,
        web = web_count,
        local = local_count,
        spec = spec_count,
    )
}

fn default_findings(sources: &[Source], topic: &str) -> Vec<String> {
    let mut out = Vec::new();
    let web = sources
        .iter()
        .filter(|s| matches!(s, Source::Web { .. }))
        .count();
    let local = sources
        .iter()
        .filter(|s| matches!(s, Source::Local { .. }))
        .count();
    if web > 0 {
        out.push(format!(
            "Web search surfaced {web} relevant result(s) for '{topic}'. See references [#1..#{n}] for the captured pages.",
            web = web,
            topic = topic,
            n = web,
        ));
    }
    if local > 0 {
        let n_end = web + local;
        out.push(format!(
            "In-project cross-referencing identified {local} relevant file(s). See references [#{}..=#{}] for the captured excerpts.",
            web + 1,
            n_end,
        ));
    }
    if sources.is_empty() {
        out.push("No sources were captured during this gathering pass. Consider re-running with a more specific topic.".to_string());
    }
    out
}

fn cross_references_from(sources: &[Source]) -> Vec<crate::document::CrossReference> {
    sources
        .iter()
        .filter_map(|s| match s {
            Source::Local {
                path,
                relevance,
                kind,
                ..
            } => Some(crate::document::CrossReference {
                path: path.clone(),
                relevance: format_with_kind(relevance, *kind),
            }),
            _ => None,
        })
        .collect()
}

fn format_with_kind(relevance: &str, kind: LocalSourceKind) -> String {
    match kind {
        LocalSourceKind::InProject => relevance.to_string(),
        LocalSourceKind::Extra => format!("{relevance} (from --sources-dir)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_gatherer::{GrepMatch, LocalTool};
    use crate::web_gatherer::{WebFetchTool, WebFetchedPage, WebSearchHit, WebSearchTool};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use tempfile::TempDir;

    struct FakeSearch {
        hits: Vec<WebSearchHit>,
    }
    #[async_trait]
    impl WebSearchTool for FakeSearch {
        async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
            Ok(self.hits.clone())
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
                    snippet: "snippet".into(),
                }],
            }),
            Arc::new(FakeFetch {
                pages: HashMap::from([(
                    "https://example.com".into(),
                    WebFetchedPage {
                        url: "https://example.com".into(),
                        title: "Example".into(),
                        body: "body".into(),
                    },
                )]),
            }),
        );
        let local_tool = Arc::new(FakeLocal {
            files: HashMap::from([(f.clone(), "Rust async programming is great.".into())]),
        });
        let local = LocalGatherer::new(local_tool);

        let session = ResearchSession::new(manager, Some(web), Some(local));
        let cfg = SessionConfig {
            topic: "Rust async".into(),
            ..SessionConfig::default()
        };
        let observer = Arc::new(CollectObserver::default());
        let outcome = session
            .run("rust-async", "Rust Async", &cfg, observer.clone())
            .await
            .unwrap();
        assert_eq!(outcome.research_name, "rust-async");
        assert!(outcome.sources.len() >= 1);
        // Document should exist on disk.
        let p = research_root.join("rust-async/RESEARCH.md");
        assert!(p.is_file());
        let body = tokio::fs::read_to_string(&p).await.unwrap();
        assert!(body.contains("Rust Async"));
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
    async fn session_handles_missing_web_gatherer() {
        let tmp = TempDir::new().unwrap();
        let research_root = tmp.path().join("research");
        tokio::fs::create_dir_all(&research_root).await.unwrap();
        let manager = ResearchManager::new(&research_root);
        let session = ResearchSession::new(manager, None, None);
        let cfg = SessionConfig {
            topic: "topic".into(),
            ..SessionConfig::default()
        };
        let outcome = session
            .run("rust-async", "Rust Async", &cfg, Arc::new(NoopObserver))
            .await
            .unwrap();
        assert_eq!(outcome.sources.len(), 0);
    }

    #[tokio::test]
    async fn session_rejects_invalid_name() {
        let tmp = TempDir::new().unwrap();
        let manager = ResearchManager::new(tmp.path());
        let session = ResearchSession::new(manager, None, None);
        let cfg = SessionConfig::default();
        let err = session
            .run("AB", "t", &cfg, Arc::new(NoopObserver))
            .await
            .unwrap_err();
        assert!(matches!(err, ResearchError::InvalidName(_)));
    }

    #[test]
    fn default_summary_counts_each_source_type() {
        let s = vec![
            Source::Web {
                url: "u".into(),
                title: "t".into(),
                captured_at: chrono::Utc::now(),
                body_path: PathBuf::from("sources/web-01.md"),
            },
            Source::Local {
                path: "x.md".into(),
                kind: LocalSourceKind::InProject,
                captured_at: chrono::Utc::now(),
                body_path: PathBuf::from("sources/local-01.md"),
                relevance: "r".into(),
            },
        ];
        let out = default_summary(&s, "topic");
        assert!(out.contains("2 sources"));
        assert!(out.contains("1 web"));
        assert!(out.contains("1 local"));
    }

    #[test]
    fn default_findings_handles_zero_sources() {
        let out = default_findings(&[], "x");
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("No sources"));
    }
}
