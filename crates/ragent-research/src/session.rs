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

use crate::analysis::{AnalysisEngine, AnalysisResult, build_source_bodies};
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
    /// When `true`, skip the local-file scanning phase entirely.
    pub disable_local: bool,
    /// When `true`, skip the prior-spec cross-reference phase entirely.
    pub disable_specs: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            topic: String::new(),
            sources_dir: None,
            template: None,
            max_web_results: 5,
            max_local_sources: 10,
            disable_local: false,
            disable_specs: false,
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
    /// Synthesizing a structured analysis from gathered sources.
    Synthesize,
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
            Self::Synthesize => "synthesize",
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
    /// The synthesis phase finished (or fell back). Surfaces whether the
    /// final summary/findings came from an LLM or from the mechanical
    /// fallback so the UI can be transparent about it.
    SynthesizeResult {
        /// How the synthesis result was produced.
        outcome: SynthesizeOutcome,
        /// Optional human-readable detail (e.g. the LLM error message when
        /// the synthesis failed and the fallback was used).
        detail: Option<String>,
    },
    /// The session has finished and a fully-populated document was written.
    Done {
        /// Total number of sources captured.
        total_sources: usize,
    },
}

/// Outcome of the synthesis phase, surfaced via
/// [`SessionEvent::SynthesizeResult`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynthesizeOutcome {
    /// An LLM produced a structured [`AnalysisResult`] and it was used.
    Llm,
    /// The LLM-backed engine returned empty content (e.g. parsing failed);
    /// the mechanical fallback supplied the summary/findings.
    FallbackEmpty,
    /// The LLM-backed engine returned an error (no key, network failure, …)
    /// and the mechanical fallback supplied the summary/findings.
    FallbackError,
    /// No LLM engine was wired in (`NoopAnalysisEngine`) and the mechanical
    /// fallback supplied the summary/findings.
    NoLlm,
}

impl SynthesizeOutcome {
    /// Short label for log output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Llm => "llm",
            Self::FallbackEmpty => "fallback-empty",
            Self::FallbackError => "fallback-error",
            Self::NoLlm => "no-llm",
        }
    }
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
    analysis: Arc<dyn AnalysisEngine>,
}

impl std::fmt::Debug for ResearchSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResearchSession")
            .field("research_root", &self.manager.root())
            .field("has_web", &self.web.is_some())
            .field("has_local", &self.local.is_some())
            .field("has_analysis", &!self.analysis_is_noop())
            .finish()
    }
}

impl ResearchSession {
    /// Returns `true` when the wired-in [`AnalysisEngine`] is the
    /// [`crate::analysis::NoopAnalysisEngine`] (no LLM analysis available).
    ///
    /// We compare `TypeId::of` of the concrete struct against the type-id
    /// of the value behind the trait object. The standard `Any::type_id`
    /// trick does not work here because `Any::type_id` on a trait object
    /// returns the *trait object's* `TypeId`, which is the same regardless
    /// of the underlying concrete type.
    fn analysis_is_noop(&self) -> bool {
        // `Arc<dyn AnalysisEngine>::as_ref()` gives `&dyn AnalysisEngine`,
        // which we can't directly query for its underlying type. So we use
        // a small discriminator that the constructors attach via a marker
        // method on the trait. `NoopAnalysisEngine` overrides it to return
        // `true`; every other implementation returns `false`.
        self.analysis.is_noop_marker()
    }

    /// Build a session over the given on-disk manager. Both web and local
    /// gatherers are optional; a session with neither is effectively a no-op
    /// (FR-006 graceful degradation).
    pub fn new(
        manager: ResearchManager,
        web: Option<WebGatherer>,
        local: Option<LocalGatherer>,
        analysis: Arc<dyn AnalysisEngine>,
    ) -> Self {
        Self {
            manager,
            web,
            local,
            analysis,
        }
    }

    /// Build a session backed only by a local tool (no web search).
    pub fn with_local_tool(
        manager: ResearchManager,
        local_tool: Arc<dyn LocalTool>,
        analysis: Arc<dyn AnalysisEngine>,
    ) -> Self {
        Self::new(
            manager,
            None,
            Some(LocalGatherer::new(local_tool)),
            analysis,
        )
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
        let local_gathered = if config.disable_local {
            tracing::info!(
                name = %name,
                "research: local phase skipped (--no-local)"
            );
            Vec::new()
        } else if let Some(local) = &self.local {
            let cfg = LocalGatherConfig {
                max_local_sources: config.max_local_sources,
                skip_specs: config.disable_specs,
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
        if config.disable_specs {
            tracing::info!(
                name = %name,
                "research: spec phase skipped (--no-specs)"
            );
        }
        let spec_sources: Vec<Source> = if config.disable_specs {
            Vec::new()
        } else {
            sources
                .iter()
                .filter(|s| matches!(s, Source::Spec { .. }))
                .cloned()
                .collect()
        };
        for src in &spec_sources {
            if let Source::Spec { spec_id, .. } = src {
                observer.on_event(SessionEvent::SpecCaptured {
                    spec_id: spec_id.clone(),
                });
            }
        }

        // ── Synthesize ─────────────────────────────────────────────────────
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::Synthesize,
        });
        // Decide which fallback path we'll take *before* calling the engine
        // so we can attribute the resulting summary correctly in the UI.
        let has_llm_engine = !self.analysis_is_noop();
        let (analysis, synth_outcome, synth_detail) =
            match self.synthesize(&name, &topic, &sources).await {
                Ok(result) => {
                    let used_llm_content = !result.summary.is_empty()
                        || !result.findings.is_empty()
                        || !result.cross_references.is_empty()
                        || !result.open_questions.is_empty();
                    let outcome = if has_llm_engine && used_llm_content {
                        SynthesizeOutcome::Llm
                    } else if has_llm_engine {
                        SynthesizeOutcome::FallbackEmpty
                    } else {
                        SynthesizeOutcome::NoLlm
                    };
                    (result, outcome, None)
                }
                Err(e) => {
                    // Log at error level (not warn) so it's visible by default
                    // — synthesis failures are the reason RESEARCH.md ends up
                    // looking skeletal, and the user needs to know.
                    tracing::error!(
                        error = %e,
                        "research: synthesis failed; falling back to mechanical summary"
                    );
                    (
                        AnalysisResult::default(),
                        SynthesizeOutcome::FallbackError,
                        Some(e.to_string()),
                    )
                }
            };
        observer.on_event(SessionEvent::SynthesizeResult {
            outcome: synth_outcome,
            detail: synth_detail,
        });
        // ── Assemble ──────────────────────────────────────────────────────
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::Assemble,
        });
        let mut item_with_sources = ResearchItem::new(name.clone(), title, &topic);
        for s in &sources {
            item_with_sources.add_source(s.clone());
        }
        let llm_produced_summary = !analysis.summary.is_empty()
            || !analysis.findings.is_empty()
            || !analysis.cross_references.is_empty()
            || !analysis.open_questions.is_empty();
        let doc = ResearchDocument {
            item: item_with_sources,
            summary: if analysis.summary.is_empty() {
                default_summary(&sources, &topic)
            } else {
                analysis.summary
            },
            findings: if analysis.findings.is_empty() {
                default_findings(&sources, &topic)
            } else {
                analysis.findings
            },
            cross_references: if analysis.cross_references.is_empty() {
                cross_references_from(&sources)
            } else {
                analysis.cross_references
            },
            open_questions: if analysis.open_questions.is_empty() {
                if llm_produced_summary {
                    Vec::new()
                } else {
                    // Surface suggested open questions from the mechanical
                    // fallback so the section is never empty when no LLM
                    // analysis was available.
                    default_open_questions(&sources, &topic)
                }
            } else {
                analysis.open_questions
            },
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

impl ResearchSession {
    /// Read captured source bodies from disk and run the analysis engine.
    async fn synthesize(
        &self,
        name: &ResearchName,
        topic: &str,
        sources: &[Source],
    ) -> anyhow::Result<AnalysisResult> {
        // Prefer the inline `body` field on each source — it's the captured
        // text from the gatherer and is always populated for fresh sessions.
        // Fall back to reading the on-disk supporting file for items loaded
        // from disk that predate the body field.
        let research_root = self.manager.root().to_path_buf();
        let bodies = build_source_bodies(sources, |src| -> Option<String> {
            if let Some(inline) = src.body() {
                if !inline.is_empty() {
                    return Some(inline.to_string());
                }
            }
            match src {
                Source::Web { body_path, .. }
                | Source::Local { body_path, .. }
                | Source::Other { body_path, .. } => {
                    let path = ResearchIo::item_dir(&research_root, name).join(body_path);
                    match std::fs::read_to_string(&path) {
                        Ok(body) => Some(body),
                        Err(e) => {
                            tracing::warn!(
                                path = %path.display(),
                                error = %e,
                                "research: could not read supporting file for synthesis"
                            );
                            None
                        }
                    }
                }
                Source::Spec { relevance, .. } => Some(relevance.clone()),
            }
        });
        self.analysis.analyze(topic, &bodies).await
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
    let web = sources
        .iter()
        .filter(|s| matches!(s, Source::Web { .. }))
        .collect::<Vec<_>>();
    let local = sources
        .iter()
        .filter(|s| matches!(s, Source::Local { .. }))
        .collect::<Vec<_>>();
    let specs = sources
        .iter()
        .filter(|s| matches!(s, Source::Spec { .. }))
        .collect::<Vec<_>>();
    let total = sources.len();

    if sources.is_empty() {
        return format!(
            "No sources were captured for '{topic}'. Re-run with a more specific topic or after enabling the relevant tools."
        );
    }

    let mut out = format!(
        "Gathered {total} source(s) for '{topic}' ({w} web, {l} local, {s} spec).",
        w = web.len(),
        l = local.len(),
        s = specs.len(),
        topic = topic,
        total = total,
    );

    // Web: name the top 3 by title so the reader knows what was actually pulled in.
    if !web.is_empty() {
        out.push_str("\n\n**Web sources:** ");
        let titles: Vec<String> = web
            .iter()
            .filter_map(|s| match s {
                Source::Web { title, url, .. } if !title.is_empty() => Some(title.clone()),
                Source::Web { url, .. } => Some(url.clone()),
                _ => None,
            })
            .take(3)
            .collect();
        out.push_str(&titles.join("; "));
        if web.len() > 3 {
            out.push_str(&format!(" (and {} more)", web.len() - 3));
        }
        out.push('.');
    }

    // Local: name the top 3 paths so the reader knows which files were pulled in.
    if !local.is_empty() {
        out.push_str("\n\n**Local files:** ");
        let paths: Vec<String> = local
            .iter()
            .filter_map(|s| match s {
                Source::Local { path, .. } => Some(path.clone()),
                _ => None,
            })
            .take(3)
            .collect();
        out.push_str(&paths.join("; "));
        if local.len() > 3 {
            out.push_str(&format!(" (and {} more)", local.len() - 3));
        }
        out.push('.');
    }

    // Specs: name each spec so the reader sees which prior specs informed this research.
    if !specs.is_empty() {
        out.push_str("\n\n**Prior specs cross-referenced:** ");
        let ids: Vec<String> = specs
            .iter()
            .filter_map(|s| match s {
                Source::Spec { spec_id, .. } => Some(spec_id.clone()),
                _ => None,
            })
            .collect();
        out.push_str(&ids.join(", "));
        out.push('.');
    }

    out.push_str(
        "\n\n_No LLM analysis was applied to these sources — the section above is a mechanical digest. Re-run with a configured model for a synthesized analysis._",
    );
    out
}

fn default_findings(sources: &[Source], topic: &str) -> Vec<String> {
    let mut out = Vec::new();
    let web: Vec<&Source> = sources
        .iter()
        .filter(|s| matches!(s, Source::Web { .. }))
        .collect();
    let local: Vec<&Source> = sources
        .iter()
        .filter(|s| matches!(s, Source::Local { .. }))
        .collect();
    let specs: Vec<&Source> = sources
        .iter()
        .filter(|s| matches!(s, Source::Spec { .. }))
        .collect();

    // Per-web-source bullets. The reader gets the title and a 240-char
    // excerpt so the finding stands on its own without opening the
    // supporting file.
    for (idx, src) in web.iter().enumerate() {
        if let Source::Web {
            title, url, body, ..
        } = src
        {
            let label = if title.is_empty() {
                url.as_str()
            } else {
                title.as_str()
            };
            let excerpt = body_excerpt(body, 240);
            if excerpt.is_empty() {
                out.push(format!(
                    "{n}. **{label}** — web source captured from <{url}> (no body returned by fetch).",
                    n = idx + 1,
                    label = label,
                    url = url,
                ));
            } else {
                out.push(format!(
                    "{n}. **{label}** — web source captured from <{url}>: {excerpt}",
                    n = idx + 1,
                    label = label,
                    url = url,
                    excerpt = excerpt,
                ));
            }
        }
    }

    // Per-local-source bullets. We lead with the relevance note (which now
    // names the matched keywords and a snippet) followed by an excerpt.
    let local_offset = web.len();
    for (idx, src) in local.iter().enumerate() {
        if let Source::Local {
            path,
            relevance,
            body,
            ..
        } = src
        {
            let excerpt = body_excerpt(body, 240);
            if excerpt.is_empty() {
                out.push(format!(
                    "{n}. **`{path}`** — {relevance} (no excerpt captured).",
                    n = local_offset + idx + 1,
                    path = path,
                    relevance = relevance,
                ));
            } else {
                out.push(format!(
                    "{n}. **`{path}`** — {relevance}\n   > {excerpt}",
                    n = local_offset + idx + 1,
                    path = path,
                    relevance = relevance,
                    excerpt = excerpt,
                ));
            }
        }
    }

    // Per-spec bullets. We give the spec id and its relevance note (which
    // usually contains the title).
    let spec_offset = web.len() + local.len();
    for (idx, src) in specs.iter().enumerate() {
        if let Source::Spec {
            spec_id, relevance, ..
        } = src
        {
            let note = if relevance.is_empty() {
                format!("see specs/{spec_id}/SPEC.md")
            } else {
                relevance.clone()
            };
            out.push(format!(
                "{n}. **`{spec_id}`** — {note}.",
                n = spec_offset + idx + 1,
                spec_id = spec_id,
                note = note,
            ));
        }
    }

    if sources.is_empty() {
        out.push(format!(
            "No sources were captured for '{topic}'. Consider re-running with a more specific topic, or run inside a project with relevant files and specs so gathering has something to work with."
        ));
    }
    out
}

/// Build a per-source bullet title + short excerpt suitable for embedding
/// in the Findings section when no LLM analysis is available. Returns an
/// empty string when the body is empty / unavailable.
fn body_excerpt(body: &str, max_chars: usize) -> String {
    // Strip the "Excerpt — N keyword match(es)" header that the local
    // gatherer prepends so we don't double-print it in the Findings section.
    let stripped = body
        .strip_prefix("Excerpt —")
        .map(|rest| {
            rest.trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ' || c == '\n')
        })
        .unwrap_or(body);
    // Collapse whitespace so the excerpt fits on one logical line.
    let collapsed: String = stripped
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let collapsed = collapsed.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max_chars {
        collapsed
    } else {
        let mut out: String = collapsed.chars().take(max_chars).collect();
        out.push('…');
        out
    }
}

fn default_open_questions(sources: &[Source], topic: &str) -> Vec<String> {
    let mut out = Vec::new();
    let web = sources
        .iter()
        .filter(|s| matches!(s, Source::Web { .. }))
        .count();
    let local = sources
        .iter()
        .filter(|s| matches!(s, Source::Local { .. }))
        .count();
    let spec = sources
        .iter()
        .filter(|s| matches!(s, Source::Spec { .. }))
        .count();
    if sources.is_empty() {
        out.push(format!(
            "Why was nothing captured for '{topic}' — was a tool unavailable, the topic too narrow, or the search query off?"
        ));
    } else {
        if web == 0 {
            out.push("No web sources were captured — was `websearch` unavailable, or does the topic lack good public references?".into());
        }
        if local == 0 {
            out.push(
                "No in-project files matched — is there a code path or doc the topic should touch that grep did not surface?"
                    .into(),
            );
        }
        if spec == 0 {
            out.push(
                "No prior specs were cross-referenced — is the topic genuinely new, or were existing specs filtered out by the keyword match?"
                    .into(),
            );
        }
        out.push(format!(
            "Re-run `/research {topic}` with a configured LLM to produce an LLM-synthesized analysis instead of this mechanical digest."
        ));
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

        let session = ResearchSession::new(
            manager,
            Some(web),
            Some(local),
            Arc::new(crate::analysis::NoopAnalysisEngine),
        );
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
        let session = ResearchSession::new(
            manager,
            None,
            None,
            Arc::new(crate::analysis::NoopAnalysisEngine),
        );
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
          async fn session_skips_local_phase_when_disable_local_is_true() {
              use crate::local_gatherer::{LocalGatherer, LocalTool};
              use std::path::PathBuf;
              use std::sync::Arc;

              /// Minimal `LocalTool` that would otherwise emit one local source.
              #[derive(Default)]
              struct SingleLocalTool;
              #[async_trait::async_trait]
              impl LocalTool for SingleLocalTool {
                  async fn glob(
                      &self,
                      _root: &Path,
                      _pattern: &str,
                  ) -> anyhow::Result<Vec<PathBuf>> {
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
                  async fn spec_title(
                      &self,
                      _root: &Path,
                      _spec_id: &str,
                  ) -> anyhow::Result<String> {
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
                  topic: "anything".into(),
                  disable_local: true,
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
              assert_eq!(spec_count, 0, "spec sources must not appear when --no-local is set");
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

              /// LocalTool that emits one `Source::Spec` via list_specs/spec_title
              /// but no regular local files. This is the only path through which
              /// spec sources enter the session, so it exercises the disable_specs
              /// gate at the gatherer boundary.
              #[derive(Default)]
              struct SpecOnlyTool;
              #[async_trait::async_trait]
              impl LocalTool for SpecOnlyTool {
                  async fn glob(
                      &self,
                      _root: &Path,
                      _pattern: &str,
                  ) -> anyhow::Result<Vec<PathBuf>> {
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
                  async fn spec_title(
                      &self,
                      _root: &Path,
                      _spec_id: &str,
                  ) -> anyhow::Result<String> {
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
                  topic: "topic".into(),
                  disable_specs: true,
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
    #[test]
    fn default_summary_counts_each_source_type() {
        let s = vec![
            Source::Web {
                url: "u".into(),
                title: "t".into(),
                captured_at: chrono::Utc::now(),
                body_path: PathBuf::from("sources/web-01.md"),
                body: String::new(),
            },
            Source::Local {
                path: "x.md".into(),
                kind: LocalSourceKind::InProject,
                captured_at: chrono::Utc::now(),
                body_path: PathBuf::from("sources/local-01.md"),
                relevance: "r".into(),
                body: String::new(),
            },
        ];
        let out = default_summary(&s, "topic");
        assert!(out.contains("2 source(s)"));
        assert!(out.contains("1 web"));
        assert!(out.contains("1 local"));
        // Mechanical fallback must be transparent about its provenance.
        assert!(out.contains("No LLM analysis was applied"));
    }

    #[test]
    fn default_summary_names_web_titles_and_local_paths() {
        let s = vec![
            Source::Web {
                url: "https://a".into(),
                title: "Article A".into(),
                captured_at: chrono::Utc::now(),
                body_path: PathBuf::from("sources/web-01.md"),
                body: String::new(),
            },
            Source::Web {
                url: "https://b".into(),
                title: "Article B".into(),
                captured_at: chrono::Utc::now(),
                body_path: PathBuf::from("sources/web-02.md"),
                body: String::new(),
            },
            Source::Local {
                path: "src/lib.rs".into(),
                kind: LocalSourceKind::InProject,
                captured_at: chrono::Utc::now(),
                body_path: PathBuf::from("sources/local-01.md"),
                relevance: "anchor".into(),
                body: String::new(),
            },
        ];
        let out = default_summary(&s, "topic");
        assert!(out.contains("**Web sources:**"));
        assert!(out.contains("Article A"));
        assert!(out.contains("Article B"));
        assert!(out.contains("**Local files:**"));
        assert!(out.contains("src/lib.rs"));
    }

    #[test]
    fn default_summary_handles_empty_source_list() {
        let out = default_summary(&[], "topic");
        assert!(out.contains("No sources were captured"));
        assert!(!out.contains("No LLM analysis"));
    }

    #[test]
    fn default_findings_handles_zero_sources() {
        let out = default_findings(&[], "x");
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("No sources"));
    }

    #[test]
    fn default_findings_emits_per_source_with_excerpts() {
        let s = vec![
            Source::Web {
                url: "https://a".into(),
                title: "Article A".into(),
                captured_at: chrono::Utc::now(),
                body_path: PathBuf::from("sources/web-01.md"),
                body: "Body of article A — talks about cargo workspaces and lockfiles.".into(),
            },
            Source::Local {
                path: "src/lib.rs".into(),
                kind: LocalSourceKind::InProject,
                captured_at: chrono::Utc::now(),
                body_path: PathBuf::from("sources/local-01.md"),
                relevance: "anchor file".into(),
                body: "Excerpt — 2 keyword match(es)\n\n▶    1: fn main() { }".into(),
            },
            Source::Spec {
                spec_id: "foo".into(),
                captured_at: chrono::Utc::now(),
                relevance: "Foo spec".into(),
            },
        ];
        let out = default_findings(&s, "topic");
        // One bullet per source.
        assert_eq!(out.len(), 3, "expected 3 findings, got {:?}", out);
        // Web finding carries the title and excerpt.
        assert!(out[0].contains("Article A"));
        assert!(out[0].contains("cargo workspaces"));
        // Local finding carries the relevance note and excerpt.
        assert!(out[1].contains("src/lib.rs"));
        assert!(out[1].contains("anchor file"));
        // Spec finding carries the id.
        assert!(out[2].contains("foo"));
    }

    #[test]
    fn default_findings_falls_back_to_metadata_when_body_is_empty() {
        let s = vec![Source::Web {
            url: "https://a".into(),
            title: "Empty Page".into(),
            captured_at: chrono::Utc::now(),
            body_path: PathBuf::from("sources/web-01.md"),
            body: String::new(),
        }];
        let out = default_findings(&s, "topic");
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("Empty Page"));
        assert!(out[0].contains("no body returned"));
    }

    #[test]
    fn default_open_questions_suggests_re_run_with_llm() {
        let s = vec![Source::Spec {
            spec_id: "x".into(),
            captured_at: chrono::Utc::now(),
            relevance: String::new(),
        }];
        let out = default_open_questions(&s, "topic");
        assert!(out.iter().any(|q| q.contains("No web sources")));
        assert!(out.iter().any(|q| q.contains("No in-project files")));
        // Always suggest a re-run when no LLM analysis was applied.
        assert!(out.iter().any(|q| q.contains("Re-run")));
    }

    #[test]
    fn default_open_questions_handles_empty_source_list() {
        let out = default_open_questions(&[], "topic");
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("Why was nothing captured"));
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
            topic: "topic".into(),
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
                SessionEvent::SynthesizeResult { outcome, .. } => Some(*outcome),
                _ => None,
            })
            .expect("SynthesizeResult event should be emitted");
        assert_eq!(synth, SynthesizeOutcome::NoLlm);
    }
}
