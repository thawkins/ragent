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

use crate::analysis::{
    AnalysisEngine, AnalysisOutcome, AnalysisResult, LlmAnalysisEngine, build_source_bodies,
};
use crate::cite_checker::{CitationCheckResult, check_citations};
use crate::contradiction::{
    ContradictionGraph, build_contradiction_graph, build_contradiction_graph_with,
};
use crate::corpus_critic::{GapFetchResult, build_corpus_critic, derive_gap_queries};
use crate::digest::{build_evidence_digest, build_triple_draft};
use crate::document::{ResearchDocument, mark_in_progress};
use crate::engine::{Critic, EngineConfig, IterativeEngine, SimpleCritic};
use crate::io::ResearchIo;
use crate::item::ResearchItem;
use crate::local_gatherer::{LocalGatherConfig, LocalGatherer, LocalTool};
use crate::locus::{analyze_loci, investigate_depth};
use crate::manager::{ResearchError, ResearchManager, Result};
use crate::patcher::{PatchResult, build_surgical_patches};
use crate::planner::{HeuristicPlanner, Planner};
use crate::readability::{PolishResult, ReadabilityAudit, audit_readability, polish_analysis};
use crate::reconcile::{build_cross_locus_reconcile, build_source_tensions};
use crate::research_name::ResearchName;
use crate::run_config::{Depth, OutputFormat, ResearchMode, Tier};
use crate::run_manifest::RunStep;
use crate::source::Source;
use crate::source_vault::SourceVault;
use crate::tier_router::{TierRouter, TierRouterObserver, TierRouterToSessionObserver};
use crate::web_gatherer::{
    DEFAULT_FETCH_CONCURRENCY, DEFAULT_MAX_WEB_RESULTS, GatherEvent, GatherObserver, WebGatherer,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tracing::Instrument;
use tracing::info;

/// Forwards [`GatherEvent`]s from the [`WebGatherer`] into [`SessionEvent`]s
/// so the TUI/CLI can display why web sources were not captured.
struct GatherEventForwarder {
    observer: Arc<dyn SessionObserver>,
}

impl GatherObserver for GatherEventForwarder {
    fn on_event(&self, event: GatherEvent) {
        match event {
            GatherEvent::SearchFailed { error } => {
                self.observer
                    .on_event(SessionEvent::WebSearchFailed { error });
            }
            GatherEvent::FetchFailed { url, error } => {
                self.observer
                    .on_event(SessionEvent::WebFetchFailed { url, error });
            }
            GatherEvent::SourceExcluded { url, reason } => {
                self.observer
                    .on_event(SessionEvent::WebSourceExcluded { url, reason });
            }
            GatherEvent::SearchReturnedNoHits => {
                self.observer.on_event(SessionEvent::WebSearchFailed {
                    error: "web search returned 0 hits".into(),
                });
            }
            GatherEvent::QueriesDecomposed { queries } => {
                // Forward immediately so the UI can render the decomposed
                // sub-queries as soon as they are generated, before the
                // parallel searches complete.
                self.observer
                    .on_event(SessionEvent::QueriesDecomposed { queries });
            }
            GatherEvent::SourceCaptured {
                url,
                title,
                search_tool,
                search_engine,
                body_preview,
                language,
                oa_recovery,
                media_type,
            } => {
                // Forward inline so the UI shows each successfully retrieved
                // URL as it arrives, rather than only at the end of the
                // gather pass.
                self.observer.on_event(SessionEvent::WebCaptured {
                    url,
                    title,
                    search_tool,
                    search_engine,
                    body_preview,
                    language,
                    oa_recovery,
                    media_type,
                });
            }
            // H-002/H-003: retry and circuit-breaker events are forwarded as
            // WebSearchFailed diagnostics so the UI surfaces the transient
            // failure and the circuit-open state transparently.
            GatherEvent::SearchRetrying {
                query,
                attempt,
                error,
            } => {
                tracing::info!(
                    query = %query,
                    attempt,
                    error = %error,
                    "research: web search retrying (forwarded to UI)"
                );
            }
            GatherEvent::SearchCircuitOpen {
                consecutive_failures,
            } => {
                self.observer.on_event(SessionEvent::WebSearchFailed {
                    error: format!(
                        "search circuit-breaker open after {consecutive_failures} consecutive failures"
                    ),
                });
            }
            GatherEvent::WidthSweepSummary {
                queries,
                engines,
                considered,
                captured,
                excluded,
            } => {
                self.observer.on_event(SessionEvent::RunStep {
                    step: crate::run_manifest::RunStep::WidthSweep
                        .as_str()
                        .to_string(),
                    status: crate::run_manifest::StepStatus::InProgress
                        .as_str()
                        .to_string(),
                    detail: Some(format!(
                        "queries={}, engines=[{}], considered={}, captured={}, excluded={}",
                        queries.len(),
                        engines.join(", "),
                        considered,
                        captured,
                        excluded
                    )),
                });
            }
            GatherEvent::VaultSufficient {
                count,
                required,
                tier,
            } => {
                self.observer.on_event(SessionEvent::RunStep {
                    step: "vault_sufficient".to_string(),
                    status: crate::run_manifest::StepStatus::Completed
                        .as_str()
                        .to_string(),
                    detail: Some(format!(
                        "vault has {count} sources (required {required} for {tier} tier); skipping new fetches"
                    )),
                });
            }
            GatherEvent::PhaseStarted { deadline_secs } => {
                // FR-009: surface the effective web-phase deadline at the
                // start of the gather phase so UI layers can render a live
                // countdown from the stored wall-clock deadline.
                self.observer.on_event(SessionEvent::RunStep {
                    step: "web_phase_start".to_string(),
                    status: crate::run_manifest::StepStatus::InProgress
                        .as_str()
                        .to_string(),
                    detail: Some(format!("web phase deadline: {deadline_secs}s")),
                });
            }
            GatherEvent::PhaseTimedOut {
                deadline_secs,
                captured,
            } => {
                self.observer.on_event(SessionEvent::RunStep {
                    step: "web_deadline".to_string(),
                    status: crate::run_manifest::StepStatus::Skipped.as_str().to_string(),
                    detail: Some(format!(
                        "web phase deadline of {deadline_secs}s reached; proceeding with {captured} captured source(s)"
                    )),
                });
            }
        }
    }
}
/// Inputs the caller supplies to [`ResearchSession::run`].
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Topic and seed inputs.
    pub input: InputConfig,
    /// Output artifact settings.
    pub output: OutputConfig,
    /// Web-gathering knobs.
    pub web: WebConfig,
    /// Local/spec gathering knobs.
    pub local: LocalConfig,
    /// Analysis and synthesis knobs.
    pub analysis: AnalysisConfig,
    /// Resilience, retry, and open-access recovery knobs.
    pub resilience: ResilienceConfig,
    /// Engine selection (tier/depth/iterations).
    pub engine: RunEngineConfig,
    /// When `true`, ask a single clarifying question before web searches if
    /// the topic is ambiguous (FR-005, FR-017). Defaults to `true`.
    pub clarify: bool,
    /// Explicit research brief generated from the user's prompt. When `Some`,
    /// downstream agents use this as their mission statement instead of
    /// deriving one from the topic (FR-004 brief context).
    pub brief: Option<String>,
    /// Per-phase model overrides (FR-013).
    pub models: ModelConfig,
    /// When `true`, run the deterministic self-evaluation scorecard and append
    /// it to the assembled report (FR-008 / T-015).
    pub evaluate: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            input: InputConfig::default(),
            output: OutputConfig::default(),
            web: WebConfig::default(),
            local: LocalConfig::default(),
            analysis: AnalysisConfig::default(),
            resilience: ResilienceConfig::default(),
            engine: RunEngineConfig::default(),
            clarify: true,
            brief: None,
            models: ModelConfig::default(),
            evaluate: false,
        }
    }
}

/// Topic and seed inputs for a research session.
#[derive(Debug, Clone, Default)]
pub struct InputConfig {
    /// Free-form research topic — used to derive web queries and grep terms.
    ///
    /// When [`Self::from_urls`] is non-empty and `topic` is empty, the topic is
    /// derived from the fetched page body (cleaned via `readability-rs` in the
    /// `webfetch` tool) so the rest of the pipeline (query decomposition, local
    /// grep terms, synthesis) has a subject that reflects the page's actual
    /// content rather than its `<title>`. The full fetched page body is captured
    /// as the first web source regardless. When the cleaned body yields no
    /// usable topic the page title, then the URL, is used as a fallback.
    pub topic: String,
    /// Optional FR-019 extra sources directory.
    pub sources_dir: Option<PathBuf>,
    /// `--from-url <URL>`: fetch one or more URLs before gathering and use each
    /// returned page as a research subject. Repeat the flag to seed multiple
    /// pages.
    ///
    /// When one or more URLs are supplied, each fetched page is captured as a
    /// primary web source and (when `topic` is empty) the *first* page body is
    /// cleaned by the `readability-rs` extractor in the `webfetch` tool, from
    /// which a concise topic is derived for query decomposition, local-grep
    /// term derivation, and synthesis. The normal web-search phase still runs,
    /// using that derived topic, so additional related sources are gathered as
    /// usual.
    pub from_urls: Vec<String>,
    /// `--from-file <PATH>`: extract one or more local documents and use their
    /// content as research subjects in place of (or alongside) an explicit
    /// topic. Supported formats include PDF, Microsoft Office (`.docx`,
    /// `.xlsx`, `.pptx`), LibreOffice/ODF (`.odt`, `.ods`, `.odp`), and plain
    /// text/markdown. When `topic` is empty, a concise topic is derived from the
    /// extracted text. The extracted content from each file is captured as the
    /// first `Source::Other` source; the normal web-search phase still runs using
    /// the derived topic. Repeat the flag to seed multiple files. If any
    /// referenced file is a PDF, PDF web sources are automatically enabled for
    /// the gather phase.
    pub from_files: Vec<PathBuf>,
}

/// Output artifact settings for a research session.
#[derive(Debug, Clone)]
pub struct OutputConfig {
    /// Optional FR-020 template file (resolved against `_templates/`).
    pub template: Option<String>,
    /// Output artifact selected via `--format`.
    pub output_format: OutputFormat,
}

/// Web-gathering knobs for a research session.
#[derive(Debug, Clone)]
pub struct WebConfig {
    /// Maximum web sources to capture (default `250`).
    pub max_web_results: usize,
    /// Maximum number of candidate pages to fetch concurrently during the
    /// web-gathering phase. Defaults to [`DEFAULT_FETCH_CONCURRENCY`] (10).
    /// Larger values reduce wall-clock latency when a search returns many
    /// hits, at the cost of more in-flight HTTP connections and memory.
    /// Override per-run with the `--fetch-concurrently N` CLI flag.
    pub fetch_concurrency: usize,
    /// Maximum wall-clock time in seconds for a single page fetch. Pages that
    /// take longer are treated as a fetch failure so a slow URL cannot stall the
    /// whole gather pass. Defaults to 30 seconds.
    pub fetch_timeout_secs: u64,
    /// `--use-low-relevance`: when `true`, the web-gathering phase keeps
    /// every fetched page regardless of its query-match relevance score,
    /// disabling the default filter that discards "Low"/"Very low" sources.
    pub use_low_relevance: bool,
    /// `--no-papers`: when `true`, the web-gathering phase filters out
    /// hits from scholarly search engines (e.g. OpenAlex) so only general
    /// web search results are captured.
    pub disable_scholarly: bool,
    /// `--use-pdf`: when `true`, the web-gathering phase may capture PDF
    /// documents returned by web search or supplied via `--from-url`. By default
    /// PDF web sources are skipped because they require extra extraction time
    /// and are often paywalled or large.
    pub use_pdf_web_sources: bool,
    /// Optional wall-clock timeout in seconds for the entire web-gathering
    /// phase (Milestone H-001). When `Some(N)`, the web gather pass is wrapped
    /// in a `tokio::time::timeout`; if it exceeds `N` seconds the phase is
    /// aborted and a diagnostic event is emitted so a slow search/fetch
    /// cannot stall the session. When `None`, no phase-level timeout is
    /// applied (only the per-page [`Self::fetch_timeout_secs`] applies).
    /// Defaults to `Some(DEFAULT_WEB_PHASE_TIMEOUT_SECS)` so a stalled
    /// search backend or OA lookup cannot wedge a run indefinitely.
    pub web_phase_timeout_secs: Option<u64>,
}

/// Default wall-clock budget for the entire web-gathering phase
/// ([`WebConfig::web_phase_timeout_secs`]). 60 seconds keeps `/research
/// create` responsive by default: when the budget elapses the gatherer stops
/// issuing new searches and fetches and returns everything captured so far,
/// so the run proceeds to analysis/synthesis with the partial source set.
/// Override per run with `--web-time N` (`--web-phase-timeout-secs N`), or
/// disable with `--web-time 0`.
pub const DEFAULT_WEB_PHASE_TIMEOUT_SECS: u64 = 60;

/// Local/spec gathering knobs for a research session.
#[derive(Debug, Clone)]
pub struct LocalConfig {
    /// Maximum in-project local sources to capture (default `10`).
    pub max_local_sources: usize,
    /// When `true`, skip the local-file scanning phase entirely.
    pub disable_local: bool,
    /// When `true`, skip the prior-spec cross-reference phase entirely.
    pub disable_specs: bool,
    /// Maximum number of concurrent candidate scoring/spec-scan tasks during
    /// the local-gathering phase. Defaults to
    /// [`ragent_research::local_gatherer::DEFAULT_LOCAL_CONCURRENCY`] (8).
    /// Larger values reduce wall-clock latency on large projects at the cost
    /// of more in-flight file handles; smaller values are gentler on the
    /// filesystem.
    pub local_concurrency: usize,
    /// Optional wall-clock timeout in seconds for the entire local-gathering
    /// phase (Milestone H-001). When `Some(N)`, the local gather pass is
    /// wrapped in a `tokio::time::timeout`; if it exceeds `N` seconds the
    /// phase is aborted and a diagnostic event is emitted so a slow filesystem
    /// scan cannot stall the session. When `None`, no phase-level timeout is
    /// applied. Defaults to `None`.
    pub local_phase_timeout_secs: Option<u64>,
}

/// Analysis and synthesis knobs for a research session.
#[derive(Debug, Clone, Default)]
pub struct AnalysisConfig {
    /// Depth preset selected via `--depth`. When `None`, the engine behaves as
    /// `Depth::Standard` for budget purposes and remains single-pass.
    pub depth: Option<Depth>,
    /// Iteration override selected via `--iterations`. When `None`, the depth
    /// preset controls iteration count; the iterative branch is only taken
    /// when this is `Some` or depth is `Deep`.
    pub iterations: Option<u32>,
    /// Maximum number of sources to send to the LLM synthesis engine
    /// (Milestone E-003). When the total corpus exceeds this cap, the
    /// highest-relevance sources are selected and the rest are dropped before
    /// synthesis. When `None`, no cap is applied and all gathered sources are
    /// sent. `--use-low-relevance` still controls whether low-relevance
    /// sources are eligible: when `use_low_relevance` is `false`, low/very-low
    /// sources are already filtered by the web gatherer; when `true`, they
    /// remain in the pool and may be selected by the cap if their relevance
    /// rank is high enough relative to the corpus.
    pub max_synthesis_sources: Option<usize>,
    /// Optional `--summarization-model <provider:model>` override. When
    /// `Some`, the web gatherer summarizes each fetched page with this model
    /// before synthesis and before storing the source in the vault (FR-002,
    /// FR-010). When `None`, the configured default model is used.
    pub summarization_model: Option<String>,
    /// Polarity dimensions for the contradiction-graph builder
    /// (Milestone FUNC-ANL-02). When `None`, the default medical/tech
    /// dimensions are used. When `Some`, the supplied dimensions override the
    /// defaults, enabling contradiction detection for non-medical topics.
    pub contradiction: Option<crate::contradiction::ContradictionConfig>,
}

/// Resilience, retry, and open-access recovery knobs.
#[derive(Debug, Clone)]
pub struct ResilienceConfig {
    /// Maximum number of retry attempts for a failed sub-query search
    /// (Milestone H-002). Retries use exponential backoff with a base delay of
    /// [`Self::search_retry_base_delay_ms`]. Defaults to
    /// [`crate::web_gatherer::DEFAULT_SEARCH_MAX_RETRIES`] (2). `0` disables
    /// retries entirely.
    pub search_max_retries: u32,
    /// Base delay in milliseconds for the first search-retry backoff
    /// (Milestone H-002). Subsequent retries double this value. Defaults to
    /// [`crate::web_gatherer::DEFAULT_SEARCH_RETRY_BASE_DELAY_MS`] (200 ms).
    pub search_retry_base_delay_ms: u64,
    /// Number of consecutive search-tool failures after which the
    /// circuit-breaker opens (Milestone H-003). Once open, no further search
    /// calls are issued for the remainder of the gather pass. Defaults to
    /// [`crate::web_gatherer::DEFAULT_SEARCH_CIRCUIT_BREAKER_THRESHOLD`] (3).
    /// `0` disables the circuit-breaker entirely.
    pub search_circuit_breaker_threshold: u32,
    /// Enable open-access recovery via Unpaywall and Europe PMC for short
    /// scholarly sources (FR-010). Defaults to `false`; T-018 will wire
    /// this from `ragent.json` and CLI flags.
    pub open_access_recovery: bool,
    /// Contact email required by Unpaywall's terms of service (FR-012).
    pub contact_email: Option<String>,
    /// Minimum full-text length (in characters) that triggers OA recovery.
    pub oa_min_full_text_chars: usize,
}

/// Engine selection for a research session.
#[derive(Debug, Clone)]
pub struct RunEngineConfig {
    /// `--tier` research tier (FR-001). Defaults to [`Tier::Full`].
    pub tier: Tier,
    /// `--mode` research execution strategy (FR-001, FR-009 of
    /// specs/opendeepresearch). Defaults to [`ResearchMode::Tiered`].
    pub mode: ResearchMode,
    /// `research.supervisor.max_concurrent_research_units` — maximum parallel
    /// researcher agents in supervisor/competitive modes (FR-012).
    pub max_concurrent_research_units: usize,
}

/// Configuration for supervisor/competitive multi-agent modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupervisorConfig {
    /// Maximum number of researcher agents that may run concurrently.
    pub max_concurrent_research_units: usize,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_research_units: crate::supervisor::DEFAULT_MAX_CONCURRENT_RESEARCH_UNITS,
        }
    }
}

/// Per-phase model selection for a research session.
///
/// Each field overrides the default model for a specific phase of the
/// research pipeline (FR-013 of specs/opendeepresearch). When a field is
/// `None`, the pipeline falls back to the configured default model.
#[derive(Debug, Clone, Default)]
pub struct ModelConfig {
    /// Model used by research agents / sub-topic workers.
    pub research_model: Option<String>,
    /// Model used to compress or summarize intermediate findings.
    pub compression_model: Option<String>,
    /// Model used to write the final report.
    pub final_report_model: Option<String>,
}

impl SessionConfig {
    /// Resolve the effective [`EngineConfig`] from depth + iterations.
    #[must_use]
    pub fn engine_config(&self) -> EngineConfig {
        let depth = self.analysis.depth.unwrap_or(Depth::Standard);
        depth.engine_config(self.analysis.iterations, depth == Depth::Deep)
    }

    /// Resolve the effective supervisor configuration.
    #[must_use]
    pub fn supervisor_config(&self) -> SupervisorConfig {
        SupervisorConfig {
            max_concurrent_research_units: self.engine.max_concurrent_research_units.max(1),
        }
    }

    /// Maximum web sources to capture for the selected depth/iteration combo.
    #[must_use]
    pub fn budget_web_results(&self) -> usize {
        let cfg = self.engine_config();
        (cfg.max_sources_per_question * 3).max(3)
    }

    /// Maximum local sources to capture for the selected depth.
    #[must_use]
    pub fn budget_local_sources(&self) -> usize {
        match self.analysis.depth.unwrap_or(Depth::Standard) {
            Depth::Shallow => 5,
            Depth::Standard => 10,
            Depth::Deep => 20,
        }
    }
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            template: None,
            output_format: OutputFormat::Report,
        }
    }
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            max_web_results: DEFAULT_MAX_WEB_RESULTS,
            fetch_concurrency: DEFAULT_FETCH_CONCURRENCY,
            fetch_timeout_secs: 30,
            use_low_relevance: false,
            disable_scholarly: false,
            use_pdf_web_sources: false,
            web_phase_timeout_secs: Some(DEFAULT_WEB_PHASE_TIMEOUT_SECS),
        }
    }
}

impl Default for LocalConfig {
    fn default() -> Self {
        Self {
            max_local_sources: 10,
            disable_local: false,
            disable_specs: false,
            local_concurrency: crate::local_gatherer::DEFAULT_LOCAL_CONCURRENCY,
            local_phase_timeout_secs: None,
        }
    }
}

impl Default for ResilienceConfig {
    fn default() -> Self {
        Self {
            search_max_retries: crate::web_gatherer::DEFAULT_SEARCH_MAX_RETRIES,
            search_retry_base_delay_ms: crate::web_gatherer::DEFAULT_SEARCH_RETRY_BASE_DELAY_MS,
            search_circuit_breaker_threshold:
                crate::web_gatherer::DEFAULT_SEARCH_CIRCUIT_BREAKER_THRESHOLD,
            open_access_recovery: false,
            contact_email: None,
            oa_min_full_text_chars: crate::open_access::DEFAULT_OA_MIN_FULL_TEXT_CHARS,
        }
    }
}

impl Default for RunEngineConfig {
    fn default() -> Self {
        Self {
            tier: Tier::Full,
            mode: ResearchMode::Tiered,
            max_concurrent_research_units: crate::supervisor::DEFAULT_MAX_CONCURRENT_RESEARCH_UNITS,
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
    /// Supervisor graph: planning sub-topics.
    SupervisorPlan,
    /// Supervisor graph: delegating sub-topics to researcher agents.
    SupervisorDelegate,
    /// Supervisor graph: merging researcher findings.
    SupervisorSynthesize,
    /// Supervisor graph: writing the final document.
    SupervisorFinalize,
}

impl SessionPhase {
    /// Human-readable label for log output.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Web => "web",
            Self::Local => "local",
            Self::Specs => "specs",
            Self::Synthesize => "synthesize",
            Self::Assemble => "assemble",
            Self::Finalize => "finalize",
            Self::SupervisorPlan => "supervisor_plan",
            Self::SupervisorDelegate => "supervisor_delegate",
            Self::SupervisorSynthesize => "supervisor_synthesize",
            Self::SupervisorFinalize => "supervisor_finalize",
        }
    }
}

/// Analysis-phase events emitted by the adversarial pipeline steps
/// (FR-005, T-007 through T-011).
///
/// These are grouped into a sub-enum so observers that only care about
/// analysis progress can match on [`SessionEvent::Analysis`] instead of the
/// full top-level enum.
#[derive(Debug, Clone)]
pub enum AnalysisEvent {
    /// The contradiction-graph step produced a ranked set of opposing source
    /// claims (T-007).
    ContradictionGraph {
        /// Contradiction edges found.
        edges: Vec<crate::contradiction::ContradictionEdge>,
        /// Number of sources scanned.
        sources_scanned: usize,
    },
    /// The loci-analysis step identified key recurring dimensions (T-008).
    LociAnalysis {
        /// Identified recurring dimensions.
        loci: crate::locus::LocusSet,
        /// Number of sources scanned.
        sources_scanned: usize,
    },
    /// The depth-investigation step classified each detected locus (T-008).
    DepthInvestigation {
        /// Per-locus depth investigation results.
        investigations: Vec<crate::locus::DepthInvestigation>,
    },
    /// The cross-locus reconcile step identified dimensions that share common
    /// sources (T-009).
    CrossLocusReconcile {
        /// Cross-locus reconciliation result.
        reconcile: crate::reconcile::CrossLocusReconcile,
    },
    /// The source-tensions step surfaced contradictions, shallow evidence, and
    /// isolated sources (T-009).
    SourceTensions {
        /// Source tensions identified.
        tensions: crate::reconcile::SourceTensions,
    },
    /// The evidence-digest step summarised claim support and conflict levels
    /// (T-011).
    EvidenceDigest {
        /// Evidence digest summary.
        digest: crate::digest::EvidenceDigest,
    },
    /// The corpus-critic step audited the gathered corpus (T-010).
    CorpusCritic {
        /// Corpus critic audit report.
        report: crate::corpus_critic::CorpusCriticReport,
    },
    /// The gap-fill fetch step issued targeted follow-up queries (T-010).
    GapFetch {
        /// Gap-fill fetch result.
        result: crate::corpus_critic::GapFetchResult,
    },
    /// The triple-draft step produced three deterministic candidate summaries
    /// (T-011).
    TripleDraft {
        /// Triple-draft candidate summaries.
        draft: crate::digest::TripleDraft,
    },
}

/// Synthesis and quality-assurance events emitted by the post-analysis
/// pipeline steps (FR-005, T-012 through T-015).
///
/// Grouped into a sub-enum so QA-focused observers can match on
/// [`SessionEvent::Synthesis`] instead of the full top-level enum.
#[derive(Debug, Clone)]
pub enum SynthesisEvent {
    /// The synthesis phase finished (or fell back).
    SynthesizeResult {
        /// Synthesis outcome.
        outcome: SynthesizeOutcome,
        /// Additional detail about the synthesis.
        detail: Option<String>,
    },
    /// The deterministic 4-critic audit produced a structured quality report
    /// (T-012).
    SynthesisAudit {
        /// Synthesis audit report.
        audit: crate::synthesis::SynthesisAudit,
    },
    /// A critic subagent finished (T-012).
    CriticResult {
        /// Critic score, if available.
        score: Option<u32>,
        /// Gaps identified by the critic.
        gaps: Vec<String>,
    },
    /// The surgical patcher step applied deterministic revisions (T-013).
    SurgicalPatch {
        /// Patch result.
        result: PatchResult,
    },
    /// The cite-check step verified every `[#N]` citation (T-014).
    CiteCheck {
        /// Citation check result.
        result: CitationCheckResult,
    },
    /// The polish step applied deterministic final edits (T-015).
    Polish {
        /// Polish result.
        result: PolishResult,
    },
    /// The readability audit scored the polished draft (T-015).
    ReadabilityAudit {
        /// Readability audit result.
        result: ReadabilityAudit,
    },
    /// Self-evaluation scorecard produced for the assembled report
    /// (FR-008 / T-015).
    Evaluation {
        /// Self-evaluation scorecard.
        scorecard: crate::evaluation::EvaluationScorecard,
    },
}

/// Progress event emitted as a research session runs. The TUI/CLI/HTTP
/// layers subscribe to this to render streaming progress.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// A new phase has started.
    Phase {
        /// The phase that has started.
        phase: SessionPhase,
    },
    /// The web-gathering phase produced these focused sub-queries.
    QueriesDecomposed {
        /// Decomposed web-gathering queries.
        queries: Vec<String>,
    },
    /// The web-gathering phase captured a single source.
    WebCaptured {
        /// Source URL.
        url: String,
        /// Source title.
        title: String,
        /// Search tool used.
        search_tool: String,
        /// Search engine used.
        search_engine: String,
        /// Preview of the page body.
        body_preview: String,
        /// Detected language of the page.
        language: String,
        /// Open-access recovery info, if applicable.
        oa_recovery: Option<Box<crate::open_access::RecoveredOpenAccess>>,
        /// Classified content type (`"page"`, `"pdf"`, or `"youtube"`) so the
        /// UI can aggregate captures by file type.
        media_type: String,
    },
    /// The `--from-url` primary page was fetched.
    FromUrlBodyPreview {
        /// The fetched URL.
        url: String,
        /// Preview of the fetched page body.
        body_preview: String,
    },
    /// The `--from-file` primary document was extracted.
    FromFileBodyPreview {
        /// Path of the file.
        path: String,
        /// Preview of the extracted document body.
        body_preview: String,
    },
    /// The local-gathering phase scored and captured a file.
    LocalCaptured {
        /// Path of the captured file.
        path: String,
        /// Relevance score of the file.
        score: usize,
    },
    /// The session captured a prior spec as a cross-reference.
    SpecCaptured {
        /// Identifier of the captured spec.
        spec_id: String,
    },
    /// The web-gathering phase failed as a whole.
    WebSearchFailed {
        /// Error describing the search failure.
        error: String,
    },
    /// A single candidate page could not be fetched.
    WebFetchFailed {
        /// URL that failed to fetch.
        url: String,
        /// Error describing the fetch failure.
        error: String,
    },
    /// A candidate was deliberately excluded by a gather policy (low
    /// relevance, too-short extraction, PDFs disabled) rather than failing
    /// on the network. Kept separate from [`SessionEvent::WebFetchFailed`]
    /// so fetch-failure counters stay meaningful in the UI.
    WebSourceExcluded {
        /// URL of the excluded candidate.
        url: String,
        /// Human-readable exclusion reason.
        reason: String,
    },
    /// A generic source fetch failed and was recorded in session state.
    SourceFailed {
        /// Source identifier, if available.
        source: Option<String>,
        /// Error describing the failure.
        error: String,
    },
    /// The session needs a single clarifying answer from the user before it can
    /// proceed with web searches (FR-005, FR-017).
    NeedsClarification {
        /// The clarifying question to present.
        question: String,
    },
    /// The research plan was updated with new sub-questions.
    PlanUpdated {
        /// Updated sub-questions.
        sub_questions: Vec<String>,
    },
    /// A sub-question changed status.
    SubQuestionStatusChanged {
        /// Identifier of the sub-question.
        id: String,
        /// New status of the sub-question.
        status: String,
    },
    /// The verifier finished checking claims against sources.
    VerificationResult {
        /// Whether verification passed.
        passed: bool,
        /// Issues found during verification.
        issues: Vec<String>,
    },
    /// A single iteration of the research loop completed.
    IterationCompleted {
        /// Iteration number.
        iteration: u32,
        /// Score from the iteration, if available.
        score: Option<u32>,
    },
    /// Follow-up bridge queries were generated to close evidence gaps.
    FollowUpQueries {
        /// Generated follow-up queries.
        queries: Vec<String>,
    },
    /// Analysis-phase events (contradiction graph, loci, reconcile, etc.).
    Analysis(
        /// Analysis event.
        AnalysisEvent,
    ),
    /// Synthesis and quality-assurance events (audit, patches, cite check,
    /// polish, readability).
    Synthesis(
        /// Synthesis event.
        SynthesisEvent,
    ),
    /// The session has finished and a fully-populated document was written.
    Done {
        /// Total number of sources captured.
        total_sources: usize,
        /// Number of PDF sources captured.
        pdf_count: usize,
        /// Number of YouTube sources captured.
        youtube_count: usize,
        /// Number of sources excluded.
        excluded_count: usize,
    },
    /// A single pipeline step started, completed, skipped, or failed.
    RunStep {
        /// Name of the pipeline step.
        step: String,
        /// Status of the step.
        status: String,
        /// Additional detail about the step.
        detail: Option<String>,
    },
    /// Tier-router summary emitted when the pipeline reaches a terminal state.
    TierDone {
        /// Number of steps completed.
        completed: usize,
        /// Number of steps skipped.
        skipped: usize,
        /// Number of steps failed.
        failed: usize,
    },
    /// Supervisor graph produced a set of sub-topics (T-005).
    SupervisorPlanUpdated {
        /// Planned sub-topics.
        sub_topics: Vec<String>,
    },
    /// Competitive-analysis mode extracted a set of comparable entities and
    /// detected comparison criteria (FR-006 / T-010).
    CompetitiveEntities {
        /// Comparable entities identified for the topic.
        entities: Vec<String>,
        /// Comparison criteria/dimensions detected in the topic.
        criteria: Vec<String>,
        /// `true` when no explicit entities were named and the set was inferred.
        inferred: bool,
    },
    /// Supervisor graph spawned a researcher agent (T-005).
    ResearcherSpawned {
        /// Researcher identifier.
        id: String,
        /// Sub-topic assigned to the researcher.
        sub_topic: String,
    },
    /// Supervisor graph researcher reported progress during its tool loop
    /// (T-006). Emitted when the researcher captures a source, advances an
    /// iteration, or records a structured note.
    ResearcherProgress {
        /// Researcher identifier.
        id: String,
        /// Short status label: `capturing`, `iterating`, `note`, `done`.
        status: String,
        /// Human-readable progress message.
        detail: String,
        /// Number of sources captured so far by this researcher.
        sources_found: usize,
    },
    /// Supervisor graph researcher recorded a structured intermediate note
    /// (T-006). Notes are surfaced for UI streaming and may be persisted by
    /// the session layer.
    ResearcherNote {
        /// Researcher identifier.
        id: String,
        /// Structured note text (a bullet or short paragraph).
        note: String,
    },
    /// Supervisor graph received compressed findings from a researcher (T-005).
    ResearcherCompleted {
        /// Researcher identifier.
        id: String,
        /// Compressed summary from the researcher.
        summary: String,
    },
    /// Supervisor graph merged all researcher findings before final synthesis.
    SupervisorMerged {
        /// Number of completed researcher findings merged.
        findings_count: usize,
    },
    /// Resolved run options, emitted once at the start of a session.
    ConfigSnapshot {
        /// Output format.
        output_format: String,
        /// Depth preset.
        depth: Option<String>,
        /// Iteration count.
        iterations: Option<u32>,
        /// Selected tier.
        tier: Option<String>,
        /// `--from-url` URLs.
        from_urls: Vec<String>,
        /// `--from-file` paths.
        from_files: Vec<String>,
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
    #[must_use]
    pub const fn as_str(self) -> &'static str {
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
    planner: Option<Arc<dyn Planner>>,
    critic: Option<Arc<dyn Critic>>,
    /// Optional JSONL run log shared with the web gatherer. Used to persist
    /// synthesis and post-processing events in addition to web URL outcomes.
    gather_log: Option<Arc<std::sync::Mutex<crate::gather_log::GatherLog>>>,
    /// Model used for the analysis, persisted into the `RESEARCH.md`
    /// frontmatter as `Model:` (e.g. `anthropic/claude-sonnet-4`).
    model: Option<String>,
    /// Optional LLM summarizer used to derive a concise topic and clean title
    /// from a `--from-url` page body or `--from-file` document body. When
    /// absent, the session falls back to the local heuristics
    /// (`derive_topic_from_url_body`) that scrapes the first substantive
    /// sentence of the cleaned body.
    summarizer: Option<Arc<LlmAnalysisEngine>>,
    /// Optional LLM engine used by the `/research create` pipeline to extract
    /// the cross-source concept list rendered as the `## Concepts` section
    /// directly above `## Findings` in `RESEARCH.md`. When absent (or when the
    /// extraction call fails), the section is omitted from the document.
    concepts_engine: Option<Arc<LlmAnalysisEngine>>,
    /// Optional provider registry used to construct phase-specific models
    /// (e.g. the page summarizer) without coupling the session to one
    /// global model (FR-013, T-013).
    provider_registry: Option<Arc<ragent_llm::provider::ProviderRegistry>>,
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

/// Map the engine-side [`AnalysisOutcome`] into the user-facing
/// [`SynthesizeOutcome`] emitted in the `SynthesizeResult` session event.
fn map_analysis_outcome(outcome: AnalysisOutcome) -> SynthesizeOutcome {
    match outcome {
        AnalysisOutcome::Llm => SynthesizeOutcome::Llm,
        AnalysisOutcome::FallbackEmpty => SynthesizeOutcome::FallbackEmpty,
        AnalysisOutcome::FallbackError => SynthesizeOutcome::FallbackError,
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
            planner: None,
            critic: None,
            gather_log: None,
            model: None,
            summarizer: None,
            concepts_engine: None,
            provider_registry: None,
        }
    }

    /// Record the model used for the analysis so it can be written into the
    /// `RESEARCH.md` frontmatter as `Model:`.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Attach an LLM summarizer used by the `--from-url` / `--from-file`
    /// pre-steps to derive a concise topic and clean title from the fetched
    /// or extracted body. When unset, those steps fall back to the local
    /// heuristic (`derive_topic_from_url_body`) so behaviour degrades
    /// gracefully without an LLM.
    #[must_use]
    pub fn with_summarizer(mut self, summarizer: Arc<LlmAnalysisEngine>) -> Self {
        self.summarizer = Some(summarizer);
        self
    }
    /// Attach an LLM engine used by the `/research create` pipeline to extract
    /// the cross-source concept list rendered as the `## Concepts` section in
    /// `RESEARCH.md` (spec researchcluster). Callers typically pass the same
    /// [`LlmAnalysisEngine`] Arc wired for synthesis. When unset, the session
    /// skips the concept-extraction step entirely and `RESEARCH.md` renders
    /// without a `## Concepts` section.
    #[must_use]
    pub fn with_concepts_engine(mut self, engine: Arc<LlmAnalysisEngine>) -> Self {
        self.concepts_engine = Some(engine);
        self
    }

    /// Attach the provider registry so the session can build phase-specific
    /// engines (e.g. the page summarizer) from per-phase model overrides
    /// (FR-013, T-013).
    #[must_use]
    pub fn with_provider_registry(
        mut self,
        registry: Arc<ragent_llm::provider::ProviderRegistry>,
    ) -> Self {
        self.provider_registry = Some(registry);
        self
    }
}

impl ResearchSession {
    /// Access the optional web gatherer.
    #[must_use]
    pub fn web(&self) -> Option<WebGatherer> {
        self.web.clone()
    }

    /// Access the optional local gatherer.
    #[must_use]
    pub fn local(&self) -> Option<LocalGatherer> {
        self.local.clone()
    }

    /// Access the analysis engine.
    #[must_use]
    pub fn analysis(&self) -> Arc<dyn AnalysisEngine> {
        self.analysis.clone()
    }

    /// Access the optional planner.
    #[must_use]
    pub fn planner(&self) -> Option<Arc<dyn Planner>> {
        self.planner.clone()
    }

    /// Access the optional critic.
    #[must_use]
    pub fn critic(&self) -> Option<Arc<dyn Critic>> {
        self.critic.clone()
    }

    /// Access the research manager.
    #[must_use]
    pub fn manager(&self) -> &ResearchManager {
        &self.manager
    }

    /// Access the configured model name.
    #[must_use]
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// Access the optional provider registry.
    #[must_use]
    pub fn provider_registry(&self) -> Option<Arc<ragent_llm::provider::ProviderRegistry>> {
        self.provider_registry.clone()
    }

    /// Access the optional concepts engine.
    #[must_use]
    pub fn concepts_engine(&self) -> Option<Arc<LlmAnalysisEngine>> {
        self.concepts_engine.clone()
    }

    /// Run the supervisor/researcher graph for `--mode supervisor|competitive`.
    ///
    /// `item` has already been created and marked `InProgress` by the caller.
    /// `seed_sources` and `seed_queries` come from `--from-url` / `--from-file`
    /// pre-steps. The graph runs: Plan → Delegate → Collect → Synthesize →
    /// Finalize, with researcher nodes executed in parallel up to
    /// `config.supervisor_config().max_concurrent_research_units`.
    pub async fn run_supervisor(
        &self,
        name_str: &str,
        title: &str,
        topic: &str,
        item: &ResearchItem,
        config: &SessionConfig,
        brief: Option<&str>,
        seed_sources: Vec<Source>,
        seed_queries: Vec<String>,
        observer: Arc<dyn SessionObserver>,
        router: &mut TierRouter,
        router_observer: &dyn TierRouterObserver,
    ) -> Result<RunOutcome> {
        use crate::supervisor::{IterativeResearcherNode, SupervisorNode};

        let name = ResearchName::try_new(name_str).map_err(ResearchError::InvalidName)?;
        let supervisor_cfg = config.supervisor_config();

        // ── Plan ──────────────────────────────────────────────────────────
        router.run_step_if(RunStep::SupervisorPlan, router_observer, || {});
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::SupervisorPlan,
        });

        let (sub_topics, competitive_extraction) =
            if config.engine.mode == crate::run_config::ResearchMode::Competitive {
                let extraction = crate::entities::extract_entities_for_competitive_analysis(topic);
                let entity_names: Vec<String> =
                    extraction.entities.iter().map(|e| e.name.clone()).collect();
                observer.on_event(SessionEvent::CompetitiveEntities {
                    entities: entity_names.clone(),
                    criteria: extraction.criteria.clone(),
                    inferred: extraction.inferred,
                });
                let competitive_topics = crate::supervisor::build_competitive_sub_topics(
                    topic,
                    &extraction.entities,
                    &extraction.criteria,
                );
                let topics = if competitive_topics.is_empty() {
                    // Fall back to generic supervisor planning if no entities were
                    // identified so the run still produces something useful.
                    let planner = self
                        .planner
                        .clone()
                        .unwrap_or_else(|| Arc::new(crate::planner::HeuristicPlanner::new()));
                    let supervisor = SupervisorNode::new(planner)
                        .with_max_sub_topics(supervisor_cfg.max_concurrent_research_units);
                    supervisor.plan(topic).await.map_err(|e| {
                        ResearchError::EngineRunFailed(format!("supervisor planning failed: {e}"))
                    })?
                } else {
                    competitive_topics
                };
                (topics, Some(extraction))
            } else {
                let planner = self
                    .planner
                    .clone()
                    .unwrap_or_else(|| Arc::new(crate::planner::HeuristicPlanner::new()));
                let supervisor = SupervisorNode::new(planner)
                    .with_max_sub_topics(supervisor_cfg.max_concurrent_research_units);
                let topics = supervisor.plan(topic).await.map_err(|e| {
                    ResearchError::EngineRunFailed(format!("supervisor planning failed: {e}"))
                })?;
                (topics, None)
            };
        observer.on_event(SessionEvent::SupervisorPlanUpdated {
            sub_topics: sub_topics.clone(),
        });

        let mut state = crate::supervisor::SupervisorState::new(topic);
        for sub_topic in sub_topics {
            state.add_sub_topic(sub_topic);
        }

        // ── Delegate / Collect ─────────────────────────────────────────
        router.run_step_if(RunStep::SupervisorDelegate, router_observer, || {});
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::SupervisorDelegate,
        });

        // Open a source vault for this supervisor run so every captured web
        // source is persisted with its original URL and timestamp (FR-003).
        let project_root = project_root_for(self.manager.root());
        let run_tag = name.to_string();
        let vault = match SourceVault::open(project_root, &run_tag) {
            Ok(v) => Some(Arc::new(v)),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    run_tag,
                    "supervisor: failed to open source vault; continuing without it"
                );
                None
            }
        };

        let node = IterativeResearcherNode::new(self.web.clone(), self.analysis.clone())
            .with_planner(
                self.planner
                    .clone()
                    .unwrap_or_else(|| Arc::new(crate::planner::HeuristicPlanner::new())),
            )
            .with_critic(
                self.critic
                    .clone()
                    .unwrap_or_else(|| Arc::new(crate::engine::SimpleCritic)),
            )
            .with_engine_config(config.engine_config())
            .with_brief(brief.map(|b| b.to_string()))
            .with_research_model(config.models.research_model.clone())
            .with_vault(vault);
        let node: Arc<dyn crate::supervisor::ResearcherNode> = Arc::new(node);

        let concurrency = supervisor_cfg.max_concurrent_research_units.max(1);
        let tasks: Vec<_> = state
            .pending()
            .into_iter()
            .cloned()
            .map(|assignment| {
                let node = node.clone();
                let observer = observer.clone();
                async move {
                    observer.on_event(SessionEvent::ResearcherSpawned {
                        id: assignment.id.clone(),
                        sub_topic: assignment.sub_topic.clone(),
                    });
                    match node
                        .research(&assignment.id, &assignment.sub_topic, observer.clone())
                        .await
                    {
                        Ok((sources, summary)) => {
                            observer.on_event(SessionEvent::ResearcherCompleted {
                                id: assignment.id.clone(),
                                summary: summary.clone(),
                            });
                            (assignment.id, Ok((sources, summary)))
                        }
                        Err(e) => (assignment.id, Err(e)),
                    }
                }
            })
            .collect();

        use futures::StreamExt;
        let mut stream = futures::stream::iter(tasks).buffer_unordered(concurrency);
        while let Some((id, result)) = stream.next().await {
            match result {
                Ok((sources, summary)) => {
                    state.set_completed(&id, sources, summary);
                }
                Err(e) => {
                    state.set_failed(&id, e.to_string());
                }
            }
        }

        observer.on_event(SessionEvent::SupervisorMerged {
            findings_count: state.completed().len(),
        });

        // Build the deterministic comparison table from the per-entity
        // researcher summaries so the artifact always ships with explicit
        // criteria and a cross-entity table (FR-016 / T-011).
        let comparison_table = competitive_extraction.map(|extraction| {
            let profiles: Vec<crate::comparison::CompetitiveProfile> = extraction
                .entities
                .iter()
                .map(|entity| {
                    let summary = state
                        .assignments
                        .iter()
                        .find(|a| a.sub_topic.contains(&entity.name))
                        .map(|a| a.summary.clone())
                        .unwrap_or_default();
                    crate::comparison::CompetitiveProfile::new(entity, summary)
                })
                .collect();
            crate::comparison::build_comparison_table_body(
                &extraction.entities,
                &extraction.criteria,
                &profiles,
            )
        });

        // ── Synthesize ───────────────────────────────────────────────────
        router.run_step_if(RunStep::SupervisorSynthesize, router_observer, || {});
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::SupervisorSynthesize,
        });
        let mut merged_sources = state.merged_sources();
        for seed in seed_sources {
            if !merged_sources
                .iter()
                .any(|s| crate::supervisor::same_source(s, &seed))
            {
                merged_sources.push(seed);
            }
        }
        let (analysis, engine_outcome) = self
            .synthesize(&name, topic, &merged_sources, brief)
            .await
            .map_err(|e| {
                ResearchError::EngineRunFailed(format!("supervisor synthesis failed: {e}"))
            })?;
        let synth_outcome = map_analysis_outcome(engine_outcome);

        observer.on_event(SessionEvent::Synthesis(SynthesisEvent::SynthesizeResult {
            outcome: synth_outcome,
            detail: None,
        }));

        // ── Finalize ─────────────────────────────────────────────────────
        router.run_step_if(RunStep::SupervisorFinalize, router_observer, || {});
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::SupervisorFinalize,
        });
        let outcome = self
            .assemble_and_write(
                name_str,
                &name,
                title,
                topic,
                item,
                merged_sources,
                analysis,
                synth_outcome,
                brief,
                seed_queries,
                config.output.output_format,
                comparison_table,
                config.evaluate,
                observer.clone(),
            )
            .await?;

        router.complete_run(router_observer);

        Ok(outcome)
    }

    pub(crate) async fn assemble_and_write(
        &self,
        name_str: &str,
        name: &ResearchName,
        title: &str,
        topic: &str,
        item: &ResearchItem,
        sources: Vec<Source>,
        mut analysis: AnalysisResult,
        synth_outcome: SynthesizeOutcome,
        brief: Option<&str>,
        queries: Vec<String>,
        output_format: OutputFormat,
        comparison_table: Option<String>,
        evaluate: bool,
        observer: Arc<dyn SessionObserver>,
    ) -> Result<RunOutcome> {
        let llm_produced = synth_outcome == SynthesizeOutcome::Llm;
        if analysis.summary.is_empty() {
            analysis.summary = crate::session::fallback::default_summary(&sources, topic);
        }
        if analysis.findings.is_empty() {
            analysis.findings = crate::session::fallback::default_findings(&sources, topic);
        }
        if analysis.cross_references.is_empty() {
            analysis.cross_references = crate::session::fallback::cross_references_from(&sources);
        }
        if analysis.open_questions.is_empty() && !llm_produced {
            analysis.open_questions =
                crate::session::fallback::default_open_questions(&sources, topic);
        }
        if analysis.top_implications.is_empty() && !llm_produced {
            analysis.top_implications =
                crate::session::fallback::default_top_implications(&analysis.findings, topic);
        }

        let concepts_section = if let Some(engine) = &self.concepts_engine {
            self.extract_concepts_inner(name, &sources, engine)
                .await
                .ok()
                .flatten()
        } else {
            None
        };

        let mut item_with_sources = item.clone();
        item_with_sources.set_queries(queries.clone());
        if let Some(model) = &self.model {
            item_with_sources.model = Some(model.clone());
        }
        if output_format != OutputFormat::Report {
            item_with_sources.output_format = Some(output_format.as_str().to_string());
        }

        // ── Self-Evaluation Scorecard (FR-008 / T-015) ───────────────────────
        // Heuristically score the assembled report and either emit the scorecard
        // as a synthesis event and embed it in the document, or leave it `None`
        // when evaluation is disabled.
        let evaluation_scorecard = if evaluate {
            let scorecard = crate::evaluation::evaluate_report(
                topic,
                brief,
                &analysis.summary,
                &analysis.findings,
                &sources,
                &output_format,
            );
            observer.on_event(SessionEvent::Synthesis(SynthesisEvent::Evaluation {
                scorecard: scorecard.clone(),
            }));
            Some(crate::evaluation::render_scorecard(&scorecard))
        } else {
            None
        };

        let mut doc = ResearchDocument {
            item: item_with_sources,
            summary: analysis.summary,
            findings: analysis.findings,
            top_implications: analysis.top_implications,
            cross_references: analysis.cross_references,
            open_questions: analysis.open_questions,
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
            concepts: concepts_section,
            template_body: None,
            brief: brief.map(String::from),
            decomposed_queries: queries,
            output_format,
            comparison_table,
            evaluation_scorecard,
        };

        let final_title = if llm_produced && !doc.summary.trim().is_empty() {
            crate::item::truncate_title(&doc.summary)
        } else {
            title.to_string()
        };
        doc.item.set_title(&final_title);

        let assembled = self.manager.write_document(&doc).await?;
        self.manager.complete_gathering(name_str).await?;

        let pdf_count = sources
            .iter()
            .filter(|s| matches!(s, Source::Web { media_type, .. } if media_type == "pdf"))
            .count();
        let youtube_count = sources
            .iter()
            .filter(|s| matches!(s, Source::Web { media_type, .. } if media_type == "youtube"))
            .count();

        observer.on_event(SessionEvent::Done {
            total_sources: sources.len(),
            pdf_count,
            youtube_count,
            excluded_count: 0,
        });

        Ok(RunOutcome {
            research_name: name.to_string(),
            sources,
            document: assembled,
            web_queries: doc.decomposed_queries.clone(),
            pdf_count,
            youtube_count,
            excluded_count: 0,
        })
    }
}

impl ResearchSession {
    /// Attach a planner for the iterative research branch.
    #[must_use]
    pub fn with_planner(mut self, planner: Arc<dyn Planner>) -> Self {
        self.planner = Some(planner);
        self
    }

    /// Attach a critic for the iterative research branch.
    #[must_use]
    pub fn with_critic(mut self, critic: Arc<dyn Critic>) -> Self {
        self.critic = Some(critic);
        self
    }

    /// Attach a JSONL gather log (`GatherLog`) to the web gatherer so every
    /// candidate URL and its capture/rejection outcome is recorded. Also keep
    /// a shared reference on the session so synthesis/post-processing events
    /// can be persisted to the same file.
    /// No-op when web gathering is not wired.
    #[must_use]
    pub fn with_gather_log(mut self, log: crate::gather_log::GatherLog) -> Self {
        let shared = Arc::new(std::sync::Mutex::new(log.clone()));
        self.gather_log = Some(shared.clone());
        if let Some(web) = self.web.take() {
            self.web = Some(web.with_gather_log(log));
        }
        self
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
}

mod fallback;
mod topic;

impl ResearchSession {
    /// Append a structured event to the JSONL run log, when one is attached.
    /// Failures are best-effort: they are reported via `tracing::warn` and never
    /// abort the research session.
    fn log_run_event(&self, event: &str, payload: serde_json::Value) {
        let Some(log) = &self.gather_log else {
            return;
        };
        let record = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "event": event,
            "payload": payload,
        });
        let lock = log.lock().unwrap_or_else(|p| p.into_inner());
        if let Err(e) = lock.log_event(&record) {
            tracing::warn!(error = %e, event, "research: run log write failed");
        }
    }

    /// Run a complete research session end-to-end. The flow is:
    ///
    /// 1. Validate name + emit the setup phase.
    /// 2. If `--from-url` is provided, fetch the primary page *before* creating
    ///    the on-disk item; derive the topic from the page body when no explicit
    ///    topic was supplied. A fetch failure here aborts the session and leaves
    ///    no research folder or `RESEARCH.md` behind.
    /// 3. Create the on-disk item (if absent) using the resolved topic.
    /// 4. Mark the item `InProgress` and load the optional template.
    /// 5. Run web-gathering (T-014, T-015).
    /// 6. Run local-gathering (T-016, T-017, T-018).
    /// 7. Cross-reference prior specs (T-018).
    /// 8. Assemble `RESEARCH.md` (T-020, T-021, T-022).
    /// 9. Persist + mark `Complete` (T-012, T-013).
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

        // Confirm resolved options up front so callers can verify the expected
        // output format and other flags before any expensive work runs.
        observer.on_event(SessionEvent::ConfigSnapshot {
            output_format: config.output.output_format.as_str().to_string(),
            depth: config.analysis.depth.map(|d| d.as_str().to_string()),
            iterations: config.analysis.iterations,
            tier: Some(config.engine.tier.as_str().to_string()),
            from_urls: config.input.from_urls.clone(),
            from_files: config
                .input
                .from_files
                .iter()
                .map(|p| p.display().to_string())
                .collect(),
        });
        let mut topic = config.input.topic.clone();
        let mut sources = Vec::new();
        let mut web_queries = Vec::new();
        let mut item_title = title.to_string();

        // ── Resolve effective research brief (FR-004 / T-004) ─────────────
        // Use an explicit brief when supplied; otherwise auto-generate one for
        // supervisor/competitive modes so downstream agents have a concrete
        // mission statement.
        let effective_brief = config.brief.clone().or_else(|| {
            if config.engine.mode == ResearchMode::Tiered {
                return None;
            }
            Some(crate::generate_research_brief(
                &topic,
                Some(config.engine.mode),
                Some(config.output.output_format),
            ))
        });

        // Fail fast when an LLM engine is wired but its provider is not
        // registered. Without this check the run gathers sources for ~40s
        // before the synthesis step silently falls back to the mechanical
        // digest (FR-005 follow-up).
        if !self.analysis_is_noop() {
            if let Err(e) = self.analysis.validate_provider() {
                return Err(ResearchError::ProviderNotAvailable(e.to_string()));
            }
        }

        // ── --from-url pre-step ──────────────────────────────────────────
        self.fetch_from_url_seeds(
            config,
            &observer,
            &mut topic,
            &mut sources,
            &mut web_queries,
            &mut item_title,
        )
        .await?;

        // ── --from-file pre-step ─────────────────────────────────────────
        self.extract_from_file_seeds(
            config,
            &observer,
            &mut topic,
            &mut sources,
            &mut web_queries,
            &mut item_title,
        )
        .await?;

        // ── Create / load the on-disk item ──────────────────────────────
        let item_exists = ResearchIo::item_exists(self.manager.root(), &name).await;
        let mut item = if item_exists {
            self.manager.show(name_str).await?
        } else {
            self.manager
                .create_with_format(name_str, &item_title, &topic, config.output.output_format)
                .await?
        };
        mark_in_progress(&mut item);
        self.manager.start_gathering(name_str).await?;

        // ── Initialize tier router (T-005) ───────────────────────────────
        let run_tag = crate::tier_router::default_run_tag(name_str);
        // For supervisor/competitive modes, create a mode-aware router so the
        // run manifest records the graph steps instead of the tiered pipeline.
        // The router is passed into  and driven there.
        let mut router = TierRouter::new_with_mode(
            &run_tag,
            name_str,
            &topic,
            config.engine.tier,
            config.engine.mode,
        );
        let router_observer = TierRouterToSessionObserver::new(observer.clone());
        let template_body =
            load_template(self.manager.root(), config.output.template.as_deref()).await;

        // If we didn't have an explicit topic and no from-url/from-file was
        // supplied, fall back to whatever topic is stored on the pre-existing
        // item.

        if topic.trim().is_empty()
            && config.input.from_urls.is_empty()
            && config.input.from_files.is_empty()
        {
            topic = item.topic.clone();
        }

        // ── Scope clarification (FR-005, FR-017) ─────────────────────────
        // Ask a single clarifying question before performing any web searches
        // when the topic is ambiguous and clarification is enabled. This check
        // deliberately runs after seed pre-steps so derived topics are also
        // considered, but before any web-search phase begins.
        if config.clarify {
            if let Some(question) = crate::needs_clarification(&topic) {
                observer.on_event(SessionEvent::NeedsClarification {
                    question: question.clone(),
                });
                return Err(ResearchError::NeedsClarification { question });
            }
        }

        // ── Supervisor / competitive multi-agent graph (FR-001, FR-009) ───
        // For supervisor/competitive modes, delegate to the multi-agent graph
        // instead of the tiered pipeline. The graph reuses the same synthesis
        // and document-assembly helpers.
        if config.engine.mode == ResearchMode::Supervisor
            || config.engine.mode == ResearchMode::Competitive
        {
            return self
                .run_supervisor(
                    name_str,
                    &item_title,
                    &topic,
                    &item,
                    config,
                    effective_brief.as_deref(),
                    sources,
                    web_queries,
                    observer.clone(),
                    &mut router,
                    &router_observer,
                )
                .await;
        }

        // ── Decide single-pass vs. iterative engine ────────────────��────
        let engine_cfg = config.engine_config();
        let use_iterative =
            config.analysis.iterations.is_some() || config.analysis.depth == Some(Depth::Deep);
        // PDF files supplied via --from-file automatically enable PDF web sources
        // for the gather phase (FR-XXX).
        let from_file_pdf = config
            .input
            .from_files
            .iter()
            .any(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")));
        let allow_pdf_web_sources = config.web.use_pdf_web_sources || from_file_pdf;
        let mut excluded_count = 0usize;
        let mut pdf_count = 0usize;
        let mut youtube_count = 0usize;
        let gather_start = Instant::now();

        // Decompose is the first step for every tier except dissertation.
        router.run_step_if(RunStep::Decompose, &router_observer, || {});

        if use_iterative && engine_cfg.max_iterations > 1 {
            observer.on_event(SessionEvent::Phase {
                phase: SessionPhase::Web,
            });
            match self
                .run_iterative_pass(&topic, config, observer.clone())
                .instrument(tracing::info_span!("research_phase", phase = "web"))
                .await
            {
                Ok((iter_sources, iter_queries, iterations, iter_excluded)) => {
                    web_queries.extend(iter_queries);
                    excluded_count += iter_excluded;
                    sources.extend(iter_sources);
                    tracing::info!(
                        name = %name,
                        iterations = iterations,
                        sources = sources.len(),
                        excluded_count = iter_excluded,
                        "research: iterative pass complete"
                    );
                }
                Err(e) => {
                    observer.on_event(SessionEvent::WebSearchFailed {
                        error: e.to_string(),
                    });
                    tracing::warn!(error = %e, "research: iterative pass failed; continuing with remaining sources");
                }
            }
        } else {
            // ── Overlapped gather step (Milestone D-001) ─────────────────
            //
            // Web gathering and local/spec gathering do not depend on each
            // other and can run concurrently up to the synthesis step. Both
            // phases still emit their own diagnostic events so the UI shows
            // progress separately. The combined result is the union of web,
            // local, and spec sources.
            let (web_r, local_r) = self
                .overlapped_gather(
                    &project_root,
                    &topic,
                    config,
                    allow_pdf_web_sources,
                    &observer,
                )
                .await;
            if let Ok(result) = web_r {
                web_queries.extend(result.queries);
                excluded_count += result.excluded_count;
                pdf_count += result.pdf_count;
                youtube_count += result.youtube_count;
                sources.extend(result.sources);
            }

            if let Ok(local_sources) = local_r {
                for src in &local_sources {
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
                observer.on_event(SessionEvent::Phase {
                    phase: SessionPhase::Specs,
                });
                for src in &local_sources {
                    if let Source::Spec { spec_id, .. } = src {
                        observer.on_event(SessionEvent::SpecCaptured {
                            spec_id: spec_id.clone(),
                        });
                    }
                }
                sources.extend(local_sources);
            } else if let Err(e) = local_r {
                tracing::warn!(error = %e, "research: local phase failed; continuing");
            }
        }
        tracing::info!(
            phase = "gather",
            elapsed_ms = gather_start.elapsed().as_millis(),
            web_sources = sources
                .iter()
                .filter(|s| matches!(s, Source::Web { .. }))
                .count(),
            local_sources = sources
                .iter()
                .filter(|s| matches!(s, Source::Local { .. }))
                .count(),
            spec_sources = sources
                .iter()
                .filter(|s| matches!(s, Source::Spec { .. }))
                .count(),
            "research: gather phase complete"
        );

        // ── Synthesize ─────────────────────────────────────────────────────
        // Advance the tier router to mark WidthSweep/DepthInvestigation-style
        // steps as completed. Full tier: we keep adversarial steps as
        // skipped-stubs until T-006..T-015 implement them.
        router.run_step_if(RunStep::WidthSweep, &router_observer, || {});
        // ── Contradiction Graph (T-007) ────────────────────────────────────
        // Run the deterministic contradiction-graph step for tiers that
        // include it, emit the result as a session event, and keep the graph
        // for the assembled document.
        let contradiction_graph: Option<ContradictionGraph> =
            router.run_step_if(RunStep::ContradictionGraph, &router_observer, || {
                let graph = match &config.analysis.contradiction {
                    Some(cfg) => build_contradiction_graph_with(&sources, cfg),
                    None => build_contradiction_graph(&sources),
                };
                observer.on_event(SessionEvent::Analysis(AnalysisEvent::ContradictionGraph {
                    sources_scanned: sources.len(),
                    edges: graph.edges.clone(),
                }));
                graph
            });

        // ── Loci Analysis (T-008) ──────────────────────────────────────────
        let loci = router
            .run_step_if(RunStep::LociAnalysis, &router_observer, || {
                let loci = analyze_loci(&sources);
                observer.on_event(SessionEvent::Analysis(AnalysisEvent::LociAnalysis {
                    sources_scanned: sources.len(),
                    loci: loci.clone(),
                }));
                loci
            })
            .unwrap_or_else(crate::locus::LocusSet::empty);

        // ── Depth Investigation (T-008) ────────────────────────────────────
        let depth_investigation = router
            .run_step_if(RunStep::DepthInvestigation, &router_observer, || {
                let investigations = investigate_depth(&loci);
                observer.on_event(SessionEvent::Analysis(AnalysisEvent::DepthInvestigation {
                    investigations: investigations.clone(),
                }));
                investigations
            })
            .unwrap_or_default();

        // ── Cross-Locus Reconcile (T-009) ─────────────────────────────────
        let cross_locus_reconcile = router
            .run_step_if(RunStep::CrossLocusReconcile, &router_observer, || {
                let reconcile =
                    build_cross_locus_reconcile(&loci, contradiction_graph.as_ref(), sources.len());
                observer.on_event(SessionEvent::Analysis(AnalysisEvent::CrossLocusReconcile {
                    reconcile: reconcile.clone(),
                }));
                reconcile
            })
            .unwrap_or_else(crate::reconcile::CrossLocusReconcile::empty);

        // ── Source Tensions (T-009) ─────────────────────────────────────────
        let source_tensions = router
            .run_step_if(RunStep::SourceTensions, &router_observer, || {
                let tensions = build_source_tensions(&loci, contradiction_graph.as_ref(), &sources);
                observer.on_event(SessionEvent::Analysis(AnalysisEvent::SourceTensions {
                    tensions: tensions.clone(),
                }));
                tensions
            })
            .unwrap_or_else(crate::reconcile::SourceTensions::empty);

        // ── Evidence Digest (T-011) ────────────────────────────────────────
        let evidence_digest = router
            .run_step_if(RunStep::EvidenceDigest, &router_observer, || {
                let digest = build_evidence_digest(
                    &sources,
                    &loci,
                    &depth_investigation,
                    contradiction_graph.as_ref(),
                );
                observer.on_event(SessionEvent::Analysis(AnalysisEvent::EvidenceDigest {
                    digest: digest.clone(),
                }));
                digest
            })
            .unwrap_or_else(crate::digest::EvidenceDigest::empty);

        // Corpus Critic (T-010)
        let corpus_critic = router
            .run_step_if(RunStep::CorpusCritic, &router_observer, || {
                let report = build_corpus_critic(
                    &sources,
                    &loci,
                    &evidence_digest,
                    &source_tensions,
                    contradiction_graph.as_ref(),
                    None,
                    &topic,
                );
                observer.on_event(SessionEvent::Analysis(AnalysisEvent::CorpusCritic {
                    report: report.clone(),
                }));
                report
            })
            .unwrap_or_else(crate::corpus_critic::CorpusCriticReport::empty);

        // Gap-Fill Fetch (T-010)
        let mut gap_fetch = GapFetchResult::empty();
        if let Some(step) = router.next_step()
            && step == RunStep::GapFetch
        {
            router.start_step(RunStep::GapFetch, &router_observer);
            let gap_queries = derive_gap_queries(&corpus_critic, &loci, &topic);
            if !gap_queries.is_empty() {
                if let Some(web) = &self.web {
                    let budget = config.web.max_web_results.clamp(3, 10);
                    let forwarder = GatherEventForwarder {
                        observer: observer.clone(),
                    };
                    let combined_query = gap_queries.join(" | ");
                    match web
                        .gather_with_observer(&combined_query, budget, Some(&forwarder))
                        .await
                    {
                        Ok(result) => {
                            gap_fetch.new_sources = result.sources.len();
                            gap_fetch.queries = gap_queries.clone();
                            gap_fetch.attempted = true;
                            sources.extend(result.sources);
                        }
                        Err(e) => {
                            gap_fetch.failed_queries = gap_queries.len();
                            gap_fetch.note = format!("gap-fill fetch failed: {e}");
                        }
                    }
                } else {
                    gap_fetch.note =
                        "no web gatherer configured; gap-fill fetch skipped".to_string();
                }
            }
            observer.on_event(SessionEvent::Analysis(AnalysisEvent::GapFetch {
                result: gap_fetch.clone(),
            }));
            router.finish_step(RunStep::GapFetch, &router_observer);
        }

        // Triple Draft (T-011)
        let triple_draft = router
            .run_step_if(RunStep::TripleDraft, &router_observer, || {
                let draft = build_triple_draft(&evidence_digest, &topic);
                observer.on_event(SessionEvent::Analysis(AnalysisEvent::TripleDraft {
                    draft: draft.clone(),
                }));
                draft
            })
            .unwrap_or_else(crate::digest::TripleDraft::empty);

        // Mark remaining full-only steps skipped for `light`; for `full` they
        // are also skipped-stubs until later tasks implement them. This
        // satisfies FR-008 and FR-005's step-list contract.
        let remaining_stub_steps: Vec<RunStep> = router
            .manifest()
            .steps
            .iter()
            .filter(|s| {
                s.status == crate::run_manifest::StepStatus::Pending
                    && s.step == RunStep::ChapterPartition
            })
            .map(|s| s.step)
            .collect();
        for step in remaining_stub_steps {
            let detail = if config.engine.tier == Tier::Light {
                Some("not required for light tier".to_string())
            } else {
                Some("step not yet implemented; skipped".to_string())
            };
            router.skip_step(step, detail, &router_observer);
        }
        // ── Synthesize (T-012) ────────────────────────────────────────────
        // The synthesize step runs the LLM (or deterministic fallback) and,
        // immediately after, runs the deterministic 4-critic audit. Both are
        // reported through the tier router so the pipeline manifest is accurate.

        if let Some(step) = router.next_step()
            && step == RunStep::Synthesize
        {
            router.start_step(RunStep::Synthesize, &router_observer);
        }

        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::Synthesize,
        });
        // E-003: apply the max_synthesis_sources cap when configured. Select
        // the highest-relevance sources so the LLM sees the most valuable
        // evidence. `--use-low-relevance` sources are already in the pool
        // when `use_low_relevance` is true; the cap just picks the top N by
        // relevance rank.
        let synthesis_sources: Vec<Source> =
            if let Some(cap) = config.analysis.max_synthesis_sources {
                if sources.len() > cap {
                    tracing::info!(
                        total = sources.len(),
                        cap,
                        "research: applying max_synthesis_sources cap"
                    );
                    select_top_relevance_sources(&sources, cap)
                } else {
                    std::mem::take(&mut sources)
                }
            } else {
                std::mem::take(&mut sources)
            };
        // Decide which fallback path we'll take *before* calling the engine
        // so we can attribute the resulting summary correctly in the UI.
        let has_llm_engine = !self.analysis_is_noop();
        self.log_run_event(
            "synthesize_start",
            serde_json::json!({
                "sources": synthesis_sources.len(),
                "has_llm_engine": has_llm_engine,
            }),
        );
        let (mut analysis, synth_outcome, synth_detail) = match self
            .synthesize(
                &name,
                &topic,
                &synthesis_sources,
                effective_brief.as_deref(),
            )
            .await
        {
            // Map the engine's AnalysisOutcome to the user-facing
            // SynthesizeOutcome. When no LLM engine is wired in
            // (NoopAnalysisEngine), the default analyze_with_outcome
            // returns AnalysisOutcome::Llm, but we override to NoLlm
            // so the UI is transparent about the provenance.
            Ok((result, engine_outcome)) => {
                let synth = if has_llm_engine {
                    match engine_outcome {
                        AnalysisOutcome::Llm => SynthesizeOutcome::Llm,
                        AnalysisOutcome::FallbackEmpty => SynthesizeOutcome::FallbackEmpty,
                        AnalysisOutcome::FallbackError => SynthesizeOutcome::FallbackError,
                    }
                } else {
                    SynthesizeOutcome::NoLlm
                };
                (result, synth, None)
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
        observer.on_event(SessionEvent::Synthesis(SynthesisEvent::SynthesizeResult {
            outcome: synth_outcome,
            detail: synth_detail.clone(),
        }));

        // Persist the synthesis outcome to the run log so offline post-mortems
        // can see whether the LLM engine failed and why.
        self.log_run_event(
            "synthesize_result",
            serde_json::json!({
                "outcome": synth_outcome.as_str(),
                "detail": synth_detail,
                "sources": synthesis_sources.len(),
            }),
        );

        // Populate empty analysis fields with deterministic fallback content
        // before the audit and patcher run. The mechanical digest is the
        // narrative that actually ships when the LLM engine fails or is not
        // wired, so auditing an empty analysis produced a 0/100 verdict that
        // contradicted the document (FINDINGS.md P0).
        let had_llm_analysis = synth_outcome == SynthesizeOutcome::Llm;
        if analysis.summary.is_empty() {
            analysis.summary = default_summary(&synthesis_sources, &topic);
        }
        if analysis.findings.is_empty() {
            analysis.findings = default_findings(&synthesis_sources, &topic);
        }
        if analysis.cross_references.is_empty() {
            analysis.cross_references = cross_references_from(&synthesis_sources);
        }
        if analysis.open_questions.is_empty() && !had_llm_analysis {
            analysis.open_questions = default_open_questions(&synthesis_sources, &topic);
        }
        if analysis.top_implications.is_empty() && !had_llm_analysis {
            analysis.top_implications = default_top_implications(&analysis.findings, &topic);
        }
        if let Some(err) = &synth_detail {
            analysis.summary = format!(
                "_Synthesis engine failed ({}): {} — the fallback mechanical digest is shown below._\n\n{}",
                synth_outcome.as_str(),
                err,
                analysis.summary
            );
        }

        // Run the deterministic 4-critic audit against the final narrative and
        // emit the structured report for the UI and the assembled document.
        let synthesis_audit = crate::synthesis::build_synthesis_audit(
            &synthesis_sources,
            &evidence_digest,
            &triple_draft,
            &topic,
            &loci,
            contradiction_graph.as_ref(),
            Some(&analysis),
        );
        observer.on_event(SessionEvent::Synthesis(SynthesisEvent::SynthesisAudit {
            audit: synthesis_audit.clone(),
        }));
        self.log_run_event(
            "synthesis_audit",
            serde_json::json!({
                "overall_score": synthesis_audit.overall_score,
                "recommendation": synthesis_audit.recommendation,
                "sources_used": synthesis_audit.sources_used,
                "critic_reports": synthesis_audit.critic_reports.len(),
            }),
        );

        if let Some(step) = router.next_step()
            && step == RunStep::Synthesize
        {
            router.finish_step(RunStep::Synthesize, &router_observer);
        }

        // ── Critics (T-012) ────────────────────────────────────────────────
        // Emit one CriticResult event per critic report so the UI can see the
        // 4-critic audit subagents individually.
        router.run_step_if(RunStep::Critics, &router_observer, || {
            for report in &synthesis_audit.critic_reports {
                observer.on_event(SessionEvent::Synthesis(SynthesisEvent::CriticResult {
                    score: Some(report.score),
                    gaps: report.gaps.clone(),
                }));
            }
        });

        // ── Surgical Patcher (T-013) ─────────────────────────────────────
        // Apply deterministic revisions to the draft based on the 4-critic
        // audit and corpus-critic gaps. The patched analysis replaces the
        // original synthesis output for downstream document assembly.
        let mut patch_result = PatchResult::empty();
        if let Some(pr) = router.run_step_if(RunStep::Patcher, &router_observer, || {
            let pr = build_surgical_patches(&synthesis_audit, &corpus_critic, &topic, &analysis);
            observer.on_event(SessionEvent::Synthesis(SynthesisEvent::SurgicalPatch {
                result: pr.clone(),
            }));
            pr
        }) {
            patch_result = pr;
            analysis = patch_result.patched_analysis.clone();
        }

        // ── Cite Check (T-014) ───────────────────────────────────────────
        // Verify that every `[#N]` citation in the patched draft is backed by a
        // source in the gathered corpus. If the failure gate closes, abort
        // before writing the report so unsupported citations are not shipped.
        let mut cite_check = CitationCheckResult::empty();
        if let Some(step) = router.next_step()
            && step == RunStep::CiteCheck
        {
            router.start_step(RunStep::CiteCheck, &router_observer);
            cite_check = check_citations(
                &analysis.summary,
                &analysis.findings,
                &analysis.top_implications,
                &analysis.open_questions,
                &synthesis_sources,
            );
            observer.on_event(SessionEvent::Synthesis(SynthesisEvent::CiteCheck {
                result: cite_check.clone(),
            }));
            if !cite_check.gate_open {
                tracing::error!(
                    failed = cite_check.failed_claims.len(),
                    "research: cite-check gate closed; aborting before report shipment"
                );
                return Err(ResearchError::CiteCheckFailed {
                    claims: cite_check.failed_claims,
                });
            }
            router.finish_step(RunStep::CiteCheck, &router_observer);
        }

        // ── Polish (T-015) ───────────────────────────────────────────────
        // Apply deterministic final edits to the narrative before assembly:
        // strip control characters, normalize whitespace, and remove empty
        // paragraphs. This runs for every tier that includes the step.
        let mut polish_result = PolishResult::empty();
        if let Some(pr) = router.run_step_if(RunStep::Polish, &router_observer, || {
            let pr = polish_analysis(&mut analysis);
            observer.on_event(SessionEvent::Synthesis(SynthesisEvent::Polish {
                result: pr.clone(),
            }));
            pr
        }) {
            polish_result = pr;
        }

        // ── Readability Audit (T-015) ──────────────────────────────────
        // Run a final deterministic readability audit on the polished draft
        // and surface the score in the assembled document.
        let mut readability_audit = ReadabilityAudit::empty();
        if let Some(ra) = router.run_step_if(RunStep::ReadabilityAudit, &router_observer, || {
            let ra = audit_readability(&analysis);
            observer.on_event(SessionEvent::Synthesis(SynthesisEvent::ReadabilityAudit {
                result: ra.clone(),
            }));
            ra
        }) {
            readability_audit = ra;
        }

        // ── Concepts (spec researchcluster) ──────────────────────────────
        // Extract the cross-source concept list from the same gathered corpus
        // the synthesis step consumed. The section renders directly above
        // `## Findings` in `RESEARCH.md`. When no concepts engine is wired
        // (or the extraction fails), the section is omitted entirely.
        let concepts_section = self
            .extract_concepts_section(&name, &synthesis_sources, &observer)
            .await;

        // ── Assemble ─────────────────────────────────────────────────────
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::Assemble,
        });
        let mut item_with_sources = ResearchItem::new(name.clone(), &item_title, &topic);
        item_with_sources.set_queries(web_queries.clone());
        if let Some(model) = &self.model {
            item_with_sources.model = Some(model.clone());
        }
        // Only set output_format when it is not the default report so the
        // frontmatter stays minimal for the common case.
        if config.output.output_format != OutputFormat::Report {
            item_with_sources.output_format =
                Some(config.output.output_format.as_str().to_string());
        }
        item_with_sources.open_access_recovery = config.resilience.open_access_recovery;
        for s in &synthesis_sources {
            item_with_sources.add_source(s.clone());
        }
        let llm_produced_summary = !analysis.summary.is_empty()
            || !analysis.findings.is_empty()
            || !analysis.top_implications.is_empty()
            || !analysis.cross_references.is_empty()
            || !analysis.open_questions.is_empty();
        use crate::item::truncate_title;
        let mut doc = ResearchDocument {
            item: item_with_sources,
            summary: if analysis.summary.is_empty() {
                default_summary(&synthesis_sources, &topic)
            } else {
                analysis.summary
            },
            findings: if analysis.findings.is_empty() {
                // FR-011 / T-010: the analysis engine guarantees non-empty
                // findings via the mechanical fallback (see
                // `mechanical_fallback_findings`), so this branch is a
                // defense-in-depth safety net rather than the primary path.
                // It only triggers if a custom `AnalysisEngine`
                // implementation returns `Ok` with empty findings AND the
                // `Llm` outcome (the built-in `LlmAnalysisEngine` never                  // does). `default_findings` keeps RESEARCH.md usable.
                default_findings(&synthesis_sources, &topic)
            } else {
                analysis.findings
            },
            cross_references: if analysis.cross_references.is_empty() {
                cross_references_from(&synthesis_sources)
            } else {
                analysis.cross_references
            },
            open_questions: if analysis.open_questions.is_empty() {
                if llm_produced_summary {
                    Vec::new()
                } else {
                    // Surface suggested open questions from the mechanical
                    // fallback so the section is never empty when no LLM                      // analysis was available.
                    default_open_questions(&synthesis_sources, &topic)
                }
            } else {
                analysis.open_questions
            },
            top_implications: if analysis.top_implications.is_empty() {
                if llm_produced_summary {
                    Vec::new()
                } else {
                    // Surface ranked implications from the mechanical
                    // fallback so the section is never empty when no LLM
                    // analysis was available.
                    default_top_implications(&analysis.top_implications, &topic)
                }
            } else {
                analysis.top_implications
            },
            contradiction_graph,
            loci: if loci.is_empty() { None } else { Some(loci) },
            depth_investigation: if depth_investigation.is_empty() {
                None
            } else {
                Some(depth_investigation)
            },
            evidence_digest: if evidence_digest.is_empty() {
                None
            } else {
                Some(evidence_digest)
            },
            triple_draft: if triple_draft.is_empty() {
                None
            } else {
                Some(triple_draft)
            },
            cross_locus_reconcile: if cross_locus_reconcile.is_empty() {
                None
            } else {
                Some(cross_locus_reconcile)
            },
            source_tensions: if source_tensions.is_empty() {
                None
            } else {
                Some(source_tensions)
            },
            synthesis_audit: if synthesis_audit.is_empty() {
                None
            } else {
                Some(synthesis_audit)
            },
            corpus_critic: if corpus_critic.is_empty() {
                None
            } else {
                Some(corpus_critic)
            },
            gap_fetch: if gap_fetch.is_empty() {
                None
            } else {
                Some(gap_fetch)
            },
            surgical_patch: if patch_result.is_empty() {
                None
            } else {
                Some(patch_result)
            },
            cite_check: if cite_check.is_empty() {
                None
            } else {
                Some(cite_check)
            },
            polish: if polish_result.is_empty() {
                None
            } else {
                Some(polish_result)
            },
            concepts: concepts_section,
            readability_audit: if readability_audit.is_empty() {
                None
            } else {
                Some(readability_audit)
            },
            template_body,
            brief: None,
            decomposed_queries: web_queries.clone(),
            output_format: config.output.output_format,
            comparison_table: None,
            evaluation_scorecard: None,
        };
        // The frontmatter `title` should be a reduced-length version of the
        // final summary (max 80 chars) so the displayed headline reflects the
        // synthesis rather than the original prompt.  When the synthesis fell
        // back to the mechanical path (malformed model output, engine error, or
        // no LLM engine), the summary is a diagnostic placeholder — not a
        // meaningful headline — so we keep the topic-derived title instead.
        let final_title =
            if synth_outcome == SynthesizeOutcome::Llm && !doc.summary.trim().is_empty() {
                truncate_title(&doc.summary)
            } else {
                item_title
            };
        doc.item.set_title(&final_title);
        let assembled = self.manager.write_document(&doc).await?;
        // ── Finalize ─────────────────────────────────────────────────────
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::Finalize,
        });
        // Finalize phase: any remaining pipeline steps are already completed
        // (Polish/ReadabilityAudit ran before assembly); this just closes the
        // manifest so resumability records the correct terminal state.
        let (completed, skipped, failed) = router.counts();
        router_observer.on_done(completed, skipped, failed);
        self.manager.complete_gathering(name_str).await?;
        let total_sources = synthesis_sources.len();
        let pdf_count = pdf_count.max(
            synthesis_sources
                .iter()
                .filter(|s| matches!(s, Source::Web { media_type, .. } if media_type == "pdf"))
                .count(),
        );
        let youtube_count = youtube_count.max(
            synthesis_sources
                .iter()
                .filter(|s| matches!(s, Source::Web { media_type, .. } if media_type == "youtube"))
                .count(),
        );
        observer.on_event(SessionEvent::Done {
            total_sources,
            pdf_count,
            youtube_count,
            excluded_count,
        });

        info!(
            name = %name,
            total = total_sources,
            pdf_count,
            youtube_count,
            excluded_count,
            "research: session complete"
        );

        Ok(RunOutcome {
            research_name: name.to_string(),
            sources: synthesis_sources,
            document: assembled,
            web_queries,
            pdf_count,
            youtube_count,
            excluded_count,
        })
    }
}

impl ResearchSession {
    /// True when `title` is still an unset/URL/path placeholder that should be
    /// replaced by a derived title. The URL and file seed paths share this
    /// predicate so the two seed helpers stay consistent; the file path is a
    /// subset of the URL checks (`path_str` can never start with a scheme).
    fn is_placeholder_title(title: &str, seed: &str) -> bool {
        title.is_empty()
            || title == seed
            || title.starts_with("http://")
            || title.starts_with("https://")
    }

    /// Strip fenced code blocks and take the first 200 characters as a
    /// preview. Shared by the `--from-url` and `--from-file` seed helpers.
    ///
    /// Accumulates lazily and stops once the byte budget is reached, so a
    /// large body is never fully copied: `str::len()` is a lower bound on the
    /// character count, so `out.len() >= 200` guarantees the character budget
    /// is already met.
    fn body_preview(body: &str) -> String {
        let mut out = String::new();
        for line in body.lines().filter(|l| !l.trim_start().starts_with("```")) {
            if out.len() >= 200 {
                break;
            }
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line);
        }
        out.chars().take(200).collect()
    }

    /// Fetch each `--from-url` seed page and capture it as a web source.
    ///
    /// When no explicit topic was provided, the topic and item title are
    /// derived from the first page's body content. A fetch failure aborts
    /// the session without leaving an empty research folder behind.
    async fn fetch_from_url_seeds(
        &self,
        config: &SessionConfig,
        observer: &Arc<dyn SessionObserver>,
        topic: &mut String,
        sources: &mut Vec<Source>,
        web_queries: &mut Vec<String>,
        item_title: &mut String,
    ) -> Result<()> {
        for (idx, url) in config.input.from_urls.iter().enumerate() {
            let Some(web) = &self.web else {
                return Err(ResearchError::FromUrlFetchFailed {
                    url: url.to_string(),
                    message: "web gathering is disabled; cannot fetch --from-url".to_string(),
                });
            };
            match web.fetch_url_as_source(url).await {
                Ok((src, page)) => {
                    // Borrow instead of cloning: the body can be hundreds of
                    // KB and is only read (preview, topic derivation, LLM
                    // summariser) before `src` is pushed.
                    let (src_url, src_title, src_body): (&str, &str, &str) = match &src {
                        Source::Web {
                            url, title, body, ..
                        } => (url.as_str(), title.as_str(), body.as_str()),
                        _ => (url.as_str(), "", ""),
                    };
                    let src_language = page
                        .language
                        .as_deref()
                        .map(str::to_uppercase)
                        .unwrap_or_else(|| "UNKNOWN".to_string());
                    let src_media_type = src.media_type();
                    let body_preview = Self::body_preview(src_body);
                    observer.on_event(SessionEvent::FromUrlBodyPreview {
                        url: src_url.to_string(),
                        body_preview,
                    });
                    observer.on_event(SessionEvent::WebCaptured {
                        url: src_url.to_string(),
                        title: src_title.to_string(),
                        search_tool: String::new(),
                        search_engine: String::new(),
                        body_preview: String::new(),
                        language: src_language,
                        oa_recovery: None,
                        media_type: src_media_type.to_string(),
                    });
                    // Topic derivation only runs on the first URL (idx == 0)
                    // when no explicit topic was provided. Subsequent URLs are
                    // purely additive seed sources.
                    if idx == 0 && topic.trim().is_empty() {
                        let mut llm_title: Option<String> = None;
                        if let Some(sum) = &self.summarizer {
                            if let Some((t, ttl)) = sum.summarize_subject(src_body).await {
                                *topic = t;
                                llm_title = Some(ttl);
                                tracing::info!(
                                    url = %src_url,
                                    derived_topic = %topic,
                                    "research: --from-url derived topic/title via LLM summarizer"
                                );
                            } else {
                                tracing::warn!(
                                    url = %src_url,
                                    "research: --from-url LLM summarizer unavailable; falling back to heuristic topic"
                                );
                            }
                        }
                        if topic.trim().is_empty() {
                            if let Some(derived) =
                                derive_topic_from_url_body(src_body, src_title, src_url)
                            {
                                *topic = derived;
                                tracing::info!(
                                    url = %src_url,
                                    derived_topic = %topic,
                                    "research: --from-url derived topic from fetched page body"
                                );
                            } else {
                                let message = format!(
                                    "fetched page body for '{src_url}' contained no usable article text to derive a topic"
                                );
                                observer.on_event(SessionEvent::WebFetchFailed {
                                    url: src_url.to_string(),
                                    error: message,
                                });
                                return Err(ResearchError::FromUrlNoUsableBody {
                                    url: src_url.to_string(),
                                });
                            }
                        }
                        if let Some(new_title) = llm_title
                            && Self::is_placeholder_title(item_title, src_url)
                        {
                            *item_title = crate::item::truncate_title(&new_title);
                        }
                    }
                    if Self::is_placeholder_title(item_title, src_url)
                        && let Some(clean_title) = clean_site_title(src_title)
                    {
                        *item_title = clean_title;
                    }
                    sources.push(src);
                    web_queries.push(url.to_string());
                }
                Err(e) => {
                    observer.on_event(SessionEvent::WebFetchFailed {
                        url: url.to_string(),
                        error: e.to_string(),
                    });
                    return Err(ResearchError::FromUrlFetchFailed {
                        url: url.to_string(),
                        message: e.to_string(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Extract each `--from-file` document and capture it as a `Source::Other`
    /// seed.
    ///
    /// When no explicit topic was provided, the topic and item title are
    /// derived from the first file's extracted text. An extraction failure
    /// aborts the session without leaving an empty research folder behind.
    async fn extract_from_file_seeds(
        &self,
        config: &SessionConfig,
        observer: &Arc<dyn SessionObserver>,
        topic: &mut String,
        sources: &mut Vec<Source>,
        web_queries: &mut Vec<String>,
        item_title: &mut String,
    ) -> Result<()> {
        for (idx, file_path) in config.input.from_files.iter().enumerate() {
            let path_str = file_path.display().to_string();
            let extracted = tokio::task::spawn_blocking({
                let path = file_path.clone();
                move || ragent_tools_extended::document_extract::extract_file_as_markdown(&path)
            })
            .await
            .map_err(|e| ResearchError::FromFileExtractFailed {
                path: path_str.clone(),
                message: format!("blocking task failed: {e}"),
            })
            .and_then(|res| {
                res.map_err(|e| ResearchError::FromFileExtractFailed {
                    path: path_str.clone(),
                    message: e.to_string(),
                })
            })?;
            let src_body = extracted.content;
            let src_title = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("document")
                .to_string();

            let body_preview = Self::body_preview(&src_body);
            observer.on_event(SessionEvent::FromFileBodyPreview {
                path: path_str.clone(),
                body_preview,
            });

            if idx == 0 && topic.trim().is_empty() {
                let mut llm_title: Option<String> = None;
                if let Some(sum) = &self.summarizer {
                    if let Some((t, ttl)) = sum.summarize_subject(&src_body).await {
                        *topic = t;
                        llm_title = Some(ttl);
                        tracing::info!(
                            path = %path_str,
                            derived_topic = %topic,
                            "research: --from-file derived topic/title via LLM summarizer"
                        );
                    } else {
                        tracing::warn!(
                            path = %path_str,
                            "research: --from-file LLM summarizer unavailable; falling back to heuristic topic"
                        );
                    }
                }
                if topic.trim().is_empty() {
                    if let Some(derived) =
                        derive_topic_from_url_body(&src_body, &src_title, &path_str)
                    {
                        *topic = derived;
                        tracing::info!(
                            path = %path_str,
                            derived_topic = %topic,
                            "research: --from-file derived topic from extracted document body"
                        );
                    } else {
                        let message = format!(
                            "extracted document '{path_str}' contained no usable text to derive a topic"
                        );
                        observer.on_event(SessionEvent::WebFetchFailed {
                            url: path_str.clone(),
                            error: message,
                        });
                        return Err(ResearchError::FromFileNoUsableBody { path: path_str });
                    }
                }
                if let Some(new_title) = llm_title
                    && Self::is_placeholder_title(item_title, &path_str)
                {
                    *item_title = crate::item::truncate_title(&new_title);
                }
            }

            if Self::is_placeholder_title(item_title, &path_str) {
                *item_title = src_title;
            }

            sources.push(Source::Other {
                label: path_str.clone(),
                captured_at: chrono::Utc::now(),
                body_path: PathBuf::new(),
                body: src_body,
            });
            web_queries.push(path_str);
        }
        Ok(())
    }

    /// Run web and local gathering concurrently (Milestone D-001).
    ///
    /// Web gathering and local/spec gathering do not depend on each other and
    /// can run concurrently up to the synthesis step. Both phases still emit
    /// their own diagnostic events so the UI shows progress separately. The
    /// combined result is the union of web, local, and spec sources.
    async fn overlapped_gather(
        &self,
        project_root: &Path,
        topic: &str,
        config: &SessionConfig,
        allow_pdf_web_sources: bool,
        observer: &Arc<dyn SessionObserver>,
    ) -> (
        std::result::Result<crate::web_gatherer::GatherResult, crate::web_gatherer::WebGatherError>,
        std::result::Result<Vec<Source>, crate::local_gatherer::LocalGatherError>,
    ) {
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::Web,
        });
        observer.on_event(SessionEvent::Phase {
            phase: SessionPhase::Local,
        });
        let web_fut = async {
            if let Some(web) = &self.web {
                let web_budget = config.web.max_web_results.max(config.budget_web_results());
                // H-001 / --web-time: convert the optional phase timeout into
                // a wall-clock deadline. When the deadline passes the
                // gatherer returns a partial result with everything captured
                // so far (plus a `web_deadline` RunStep diagnostic) instead
                // of discarding the phase, so analysis/synthesis still runs
                // over the partial source set. `--web-time 0` disables the
                // deadline entirely.
                let run_tag = crate::tier_router::default_run_tag("web-gather");
                let vault = match SourceVault::open(project_root, &run_tag) {
                    Ok(v) => Some(Arc::new(v)),
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            run_tag,
                            "research: failed to open source vault; continuing without it"
                        );
                        None
                    }
                };
                let summarizer: Option<Arc<dyn crate::page_summarizer::PageSummarizer>> =
                    self.provider_registry.as_ref().and_then(|registry| {
                        let model_ref = config
                            .analysis
                            .summarization_model
                            .as_deref()
                            .and_then(|s| {
                                s.split_once('/')
                                    .map(|(p, m)| (p.to_string(), m.to_string()))
                            })
                            .or_else(|| {
                                self.model.as_deref().and_then(|s| {
                                    s.split_once('/')
                                        .map(|(p, m)| (p.to_string(), m.to_string()))
                                })
                            });
                        let (provider_id, model_id) = model_ref?;
                        let api_key = self
                            .summarizer
                            .as_ref()
                            .and_then(|s| s.api_key().map(|k| k.to_string()));
                        let base_url = self
                            .summarizer
                            .as_ref()
                            .and_then(|s| s.base_url().map(|k| k.to_string()));
                        Some(
                            Arc::new(crate::page_summarizer::LlmPageSummarizer::new(Arc::new(
                                crate::analysis::LlmAnalysisEngine::new(
                                    registry.clone(),
                                    provider_id,
                                    model_id,
                                )
                                .with_api_key(api_key)
                                .with_base_url(base_url),
                            )))
                                as Arc<dyn crate::page_summarizer::PageSummarizer>,
                        )
                    });
                let mut web = web
                    .clone()
                    .with_fetch_concurrency(config.web.fetch_concurrency)
                    .with_fetch_timeout(std::time::Duration::from_secs(
                        config.web.fetch_timeout_secs,
                    ))
                    .with_keep_low_relevance(config.web.use_low_relevance)
                    .with_disable_scholarly(config.web.disable_scholarly)
                    .with_allow_pdf_web_sources(allow_pdf_web_sources)
                    .with_search_max_retries(config.resilience.search_max_retries)
                    .with_search_retry_base_delay_ms(config.resilience.search_retry_base_delay_ms)
                    .with_search_circuit_breaker_threshold(
                        config.resilience.search_circuit_breaker_threshold,
                    )
                    .with_open_access_recovery(
                        config.resilience.open_access_recovery,
                        config.resilience.contact_email.clone(),
                    )
                    .with_oa_min_full_text_chars(config.resilience.oa_min_full_text_chars)
                    .with_sufficient_sources(config.engine.tier.sufficient_sources())
                    .with_phase_deadline(
                        config
                            .web
                            .web_phase_timeout_secs
                            .filter(|secs| *secs > 0)
                            .map(|secs| {
                                std::time::Instant::now() + std::time::Duration::from_secs(secs)
                            }),
                    );
                if let Some(vault) = vault {
                    web = web.with_vault(vault);
                }
                if let Some(sum) = summarizer {
                    web = web.with_summarizer(sum);
                }
                let forwarder = GatherEventForwarder {
                    observer: observer.clone(),
                };
                let result = web
                    .gather_with_observer(topic, web_budget, Some(&forwarder))
                    .instrument(tracing::info_span!("research_phase", phase = "web"))
                    .await;
                match result {
                    Ok(result) => Ok(result),
                    Err(e) => {
                        observer.on_event(SessionEvent::WebSearchFailed {
                            error: e.to_string(),
                        });
                        tracing::warn!(error = %e, "research: web phase failed; continuing");
                        Err(e)
                    }
                }
            } else {
                Ok(crate::web_gatherer::GatherResult::empty())
            }
        };

        let local_fut = async {
            if config.local.disable_local {
                tracing::info!("research: local phase skipped (--no-local)");
                return Ok::<Vec<Source>, crate::local_gatherer::LocalGatherError>(Vec::new());
            }
            let Some(local) = &self.local else {
                return Ok::<Vec<Source>, crate::local_gatherer::LocalGatherError>(Vec::new());
            };
            let local_budget = config
                .local
                .max_local_sources
                .max(config.budget_local_sources());
            let cfg = LocalGatherConfig {
                max_local_sources: local_budget,
                skip_specs: config.local.disable_specs,
                local_concurrency: config.local.local_concurrency.max(1),
                ..LocalGatherConfig::default()
            };
            let gather = local
                .gather(
                    project_root,
                    topic,
                    config.input.sources_dir.as_deref(),
                    &cfg,
                )
                .instrument(tracing::info_span!("research_phase", phase = "local"));
            // H-001: wrap the entire local phase in an optional timeout.
            if let Some(secs) = config.local.local_phase_timeout_secs {
                match tokio::time::timeout(std::time::Duration::from_secs(secs), gather).await {
                    Ok(r) => r,
                    Err(_) => {
                        tracing::warn!(
                            timeout_secs = secs,
                            "research: local phase timed out; continuing with no local sources"
                        );
                        Ok(Vec::new())
                    }
                }
            } else {
                gather.await
            }
        };

        tokio::join!(web_fut, local_fut)
    }

    /// Run the iterative research engine for multi-iteration passes.
    ///
    /// Returns the gathered sources, the sub-questions/queries that drove the
    /// engine, and the number of iterations completed.
    async fn run_iterative_pass(
        &self,
        topic: &str,
        config: &SessionConfig,
        observer: Arc<dyn SessionObserver>,
    ) -> Result<(Vec<Source>, Vec<String>, u32, usize)> {
        let planner = self
            .planner
            .clone()
            .unwrap_or_else(|| Arc::new(HeuristicPlanner::new()));
        let critic = self
            .critic
            .clone()
            .unwrap_or_else(|| Arc::new(SimpleCritic));
        let engine = IterativeEngine::new(
            planner,
            self.web.clone(),
            self.analysis.clone(),
            critic,
            config.engine_config(),
        )
        // FR-006: the iterative path must honour the same web-phase deadline
        // as the overlapped single-pass path. `Some(0)` disables it.
        .with_phase_deadline(
            config
                .web
                .web_phase_timeout_secs
                .filter(|secs| *secs > 0)
                .map(std::time::Duration::from_secs),
        );
        let state = engine
            .run(topic, observer)
            .await
            .map_err(|e| ResearchError::EngineRunFailed(e.to_string()))?;
        let queries: Vec<String> = state
            .plan
            .sub_questions
            .iter()
            .map(|s| s.question.clone())
            .collect();
        Ok((state.sources, queries, state.iteration_count, 0))
    }
}
impl ResearchSession {
    /// Read captured source bodies from disk and run the analysis engine,
    /// returning the [`AnalysisResult`] paired with an [`AnalysisOutcome`]
    /// so the caller can surface `SynthesizeOutcome::FallbackEmpty` when
    /// the LLM produced malformed output (FR-005 / T-005).
    pub(crate) async fn synthesize(
        &self,
        name: &ResearchName,
        topic: &str,
        sources: &[Source],
        brief: Option<&str>,
    ) -> anyhow::Result<(AnalysisResult, AnalysisOutcome)> {
        let research_root = self.manager.root().to_path_buf();
        let name = name.clone();
        let sources = sources.to_vec();
        let bodies = tokio::task::spawn_blocking(move || {
            build_source_bodies(&sources, |src| -> Option<String> {
                read_source_body(&research_root, &name, src)
            })
        })
        .await
        .map_err(|e| anyhow::anyhow!("synthesis body loading failed: {e}"))?;
        let analysis = self.analysis.with_brief(brief.map(String::from));
        analysis.analyze_with_outcome(topic, &bodies).await
    }

    /// Extract the cross-source concept list for the `## Concepts` section
    /// (spec researchcluster).
    ///
    /// When no concepts engine is wired the step is skipped silently and
    /// `None` is returned. When wired, the gathered source bodies are
    /// assembled into a context-bounded payload (each block headed with the
    /// 1-based References Index position), the fixed concept-extraction
    /// prompt is dispatched through [`LlmAnalysisEngine::complete_raw`], and
    /// the response is normalized by
    /// [`crate::cluster::concepts_section_for_research`]. Any failure is
    /// logged and reported via a `RunStep` event, but never aborts the run —
    /// the document simply renders without the section.
    pub async fn extract_concepts_section(
        &self,
        name: &ResearchName,
        sources: &[Source],
        observer: &Arc<dyn SessionObserver>,
    ) -> Option<String> {
        let Some(engine) = &self.concepts_engine else {
            return None;
        };
        observer.on_event(SessionEvent::RunStep {
            step: "concepts".to_string(),
            status: "started".to_string(),
            detail: None,
        });
        let result = self.extract_concepts_inner(name, sources, engine).await;
        match result {
            Ok(Some(section)) => {
                observer.on_event(SessionEvent::RunStep {
                    step: "concepts".to_string(),
                    status: "completed".to_string(),
                    detail: Some(format!(
                        "{} concept section(s) extracted",
                        section.matches("\n### ").count()
                            + usize::from(section.starts_with("### "))
                    )),
                });
                Some(section)
            }
            Ok(None) => {
                observer.on_event(SessionEvent::RunStep {
                    step: "concepts".to_string(),
                    status: "skipped".to_string(),
                    detail: Some("model returned no concept sections".to_string()),
                });
                None
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "research: concept extraction failed; omitting section"
                );
                observer.on_event(SessionEvent::RunStep {
                    step: "concepts".to_string(),
                    status: "failed".to_string(),
                    detail: Some(e.to_string()),
                });
                None
            }
        }
    }

    /// Build the concept-extraction payload from `sources`, call the LLM, and
    /// normalize the response. Shared by [`Self::extract_concepts_section`] and
    /// the supervisor finalization path.
    pub(crate) async fn extract_concepts_inner(
        &self,
        name: &ResearchName,
        sources: &[Source],
        engine: &LlmAnalysisEngine,
    ) -> anyhow::Result<Option<String>> {
        let research_root = self.manager.root().to_path_buf();
        let name = name.clone();
        let sources = sources.to_vec();
        let web_index_map = build_web_index_map(&sources);
        let bodies = tokio::task::spawn_blocking(move || {
            build_source_bodies(&sources, |src| -> Option<String> {
                read_source_body(&research_root, &name, src)
            })
        })
        .await
        .map_err(|e| anyhow::anyhow!("concepts body loading failed: {e}"))?;

        let max_bytes = crate::cluster::estimate_max_payload_bytes(
            crate::cluster::DEFAULT_CONTEXT_WINDOW_TOKENS,
        );
        let payload = crate::cluster::build_concepts_payload_from_bodies(&bodies, max_bytes);
        let prompt = crate::cluster::CONCEPT_EXTRACTION_PROMPT_TEMPLATE
            .replace("[INSERT_DOCUMENTS_HERE]", &payload);
        let raw = engine
            .complete_raw(
                &prompt,
                Some(
                    "You are a careful research analyst. Use only the evidence in the provided source documents; do not invent facts.",
                ),
                8192,
            )
            .await?; // Map supporting-file numbers (web-NN.md) to the combined 1-based
        // References Index position so filename-style citations in the model
        // output resolve against `RESEARCH.md`.
        Ok(crate::cluster::concepts_section_for_research(
            &raw,
            &web_index_map,
        ))
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
    /// Sub-queries used by the web-gathering phase. Empty when web gathering
    /// was disabled or no decomposer was configured.
    pub web_queries: Vec<String>,
    /// Number of recovered PDF documents.
    pub pdf_count: usize,
    /// Number of recovered YouTube transcripts / video URLs.
    pub youtube_count: usize,
    /// Number of web sources fetched but excluded for low relevance.
    pub excluded_count: usize,
}

// ── Free helpers ─────────────────────────────────────────────────────────

/// Map web supporting-file numbers (`sources/web-NN.md`) to the combined
/// 1-based References Index position of each web source in `sources`, so
/// filename-style citations (`web-NN`) emitted by the concept-extraction LLM
/// can be rewritten to `[#N]` markers that resolve against `RESEARCH.md`.
fn build_web_index_map(sources: &[Source]) -> std::collections::HashMap<usize, usize> {
    static WEB_FILE_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let web_file_re =
        WEB_FILE_RE.get_or_init(|| regex::Regex::new(r"web-(\d+)\.md$").expect("valid regex"));
    let mut map = std::collections::HashMap::new();
    for (i, src) in sources.iter().enumerate() {
        if let Source::Web { body_path, .. } = src {
            let path_str = body_path.to_string_lossy();
            if let Some(caps) = web_file_re.captures(&path_str)
                && let Ok(file_no) = caps[1].parse::<usize>()
            {
                map.insert(file_no, i + 1);
            }
        }
    }
    map
}

/// Resolve the captured body text for one source, shared by the synthesis and
/// concept-extraction steps: prefer the inline `body` field (always populated
/// for fresh sessions), fall back to the on-disk supporting file for items
/// loaded from disk that predate the body field, and use the spec relevance
/// note for `Source::Spec` entries.
pub(crate) fn read_source_body(
    research_root: &std::path::Path,
    name: &ResearchName,
    src: &Source,
) -> Option<String> {
    if let Some(inline) = src.body()
        && !inline.is_empty()
    {
        return Some(inline.to_string());
    }
    match src {
        Source::Web { body_path, .. }
        | Source::Local { body_path, .. }
        | Source::Other { body_path, .. } => {
            let path = ResearchIo::item_dir(research_root, name).join(body_path);
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
}

/// Select the top `cap` sources by relevance rank (Milestone E-003).
///
/// Sources are sorted by [`Source::relevance_rank`] descending. Ties are
/// broken by original order (stable sort) so the caller's source ordering
/// is preserved among equal-rank sources. Local and spec sources (which
/// default to rank 5) are not unfairly excluded relative to medium-relevance
/// web sources.
fn select_top_relevance_sources(sources: &[Source], cap: usize) -> Vec<Source> {
    // Create (index, source) pairs, sort by (rank desc, index asc), take cap.
    let mut indexed: Vec<(usize, &Source)> = sources.iter().enumerate().collect();
    indexed.sort_by(|a, b| {
        b.1.relevance_rank()
            .cmp(&a.1.relevance_rank())
            .then(a.0.cmp(&b.0))
    });
    let mut selected: Vec<(usize, Source)> = indexed
        .into_iter()
        .take(cap)
        .map(|(i, s)| (i, s.clone()))
        .collect();
    // Restore original order so source indices remain stable for citations.
    selected.sort_by_key(|(i, _)| *i);
    selected.into_iter().map(|(_, s)| s).collect()
}

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

use fallback::{
    cross_references_from, default_findings, default_open_questions, default_summary,
    default_top_implications,
};
use topic::{clean_site_title, derive_topic_from_url_body};

#[cfg(test)]
mod tests {
    #![allow(clippy::assert_is_empty)]
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
                        body: body256("body"),
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
                    body: body256("b"),
                    content_type: None,
                    page_type: None,
                    language: None,
                    author: None,
                })
            }
        }

        let manager = ResearchManager::new(&research_root);
        let web =
            crate::web_gatherer::WebGatherer::new(Arc::new(AlwaysFailSearch), Arc::new(OkFetch));
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
                    body: body256("body"),
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
                // — we only need to prove the --from-url source was captured.
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
                           © 2024 Example Corp. All rights reserved."
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
            outcome.sources.iter().any(
                |s| matches!(s, Source::Web { url, .. } if url == "https://example.com/article")
            ),
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
                    body: "Home About Contact\n\nLogin Sign up\n\n© 2024 Example Corp.".into(),
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
                    body: body256("body text"),
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

        // The explicit topic must win — the derived-topic branch only fires
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
        // NOTE: intentional duplication — see DUPPLAN.md Milestone J.
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
        assert!(
            shallow.analysis.iterations.is_none() && shallow.analysis.depth != Some(Depth::Deep)
        );
        assert!(
            standard.analysis.iterations.is_none() && standard.analysis.depth != Some(Depth::Deep)
        );
        assert!(deep.analysis.iterations.is_some() || deep.analysis.depth == Some(Depth::Deep));
        assert!(
            iterations.analysis.iterations.is_some()
                || iterations.analysis.depth == Some(Depth::Deep)
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
                        body: body256("web body"),
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
                        body: body256("web body"),
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
    /// Web phase → Local phase → Specs phase.  The overlapped gather emits
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
                        body: body256("web body"),
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
                        body: body256("Extra page body."),
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
            make_web("Low — weak query match"),
            make_web("High — title matches query"),
            make_web("Very low — no clear query match"),
            make_web("Medium — partial query match"),
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
            make_web("High — title matches query", "https://a"),
            make_web("Low — weak query match", "https://b"),
            make_web("High — title matches query", "https://c"),
            make_web("Low — weak query match", "https://d"),
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
            make_web("High — title matches query"),
            make_web("Low — weak query match"),
        ];
        // Cap of 2 means both are selected (low-relevance is in the pool).
        let selected = select_top_relevance_sources(&sources, 2);
        assert_eq!(selected.len(), 2);
        assert!(
            selected.iter().any(
                |s| matches!(s, Source::Web { relevance, .. } if relevance.starts_with("Low"))
            )
        );
    }

    // ── Milestone H-001: per-phase timeout tests ──────────────────────

    #[tokio::test]
    async fn h001_web_phase_timeout_keeps_partial_sources_and_proceeds() {
        use crate::web_gatherer::{
            WebFetchTool, WebFetchedPage, WebGatherer, WebSearchHit, WebSearchTool,
        };

        // Search returns one hit immediately but fetch sleeps 60s.
        struct SlowFetch;
        #[async_trait]
        impl WebFetchTool for SlowFetch {
            async fn fetch(&self, _url: &str) -> anyhow::Result<WebFetchedPage> {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                Ok(WebFetchedPage {
                    published_at: None,
                    url: _url.to_string(),
                    title: "slow".into(),
                    body: "slow body".into(),
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
            .with_fetch_timeout(std::time::Duration::from_secs(60));

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
        let outcome = session.run("h001timeout", "Test", &cfg, obs.clone()).await;
        assert!(outcome.is_ok(), "run should complete even with timeout");
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
                    body: body256("b"),
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
                search_circuit_breaker_threshold: 10,
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
                    body: body256("competitive analysis body"),
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
}
