//! `ResearchSession` - the gathering orchestration engine.
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
use crate::engine::{Critic, EngineConfig, IterativeEngine};
use crate::io::ResearchIo;
use crate::item::ResearchItem;
use crate::local_gatherer::{LocalGatherConfig, LocalGatherer, LocalTool};
use crate::locus::{analyze_loci, investigate_depth};
use crate::manager::{ResearchError, ResearchManager, Result};
use crate::patcher::{PatchResult, build_surgical_patches};
use crate::planner::Planner;
use crate::readability::{PolishResult, ReadabilityAudit, audit_readability, polish_analysis};
use crate::reconcile::{build_cross_locus_reconcile, build_source_tensions};
use crate::research_name::ResearchName;
use crate::run_config::{Depth, OutputFormat, ResearchMode, Tier};
use crate::run_manifest::RunStep;
use crate::source::Source;
use crate::source_vault::SourceVault;
use crate::tier_router::{TierRouter, TierRouterObserver, TierRouterToSessionObserver};
use crate::web_gatherer::{
    DEFAULT_FETCH_CONCURRENCY, ExclusionReason, FetchFailureKind, GatherEvent, GatherObserver,
    WebGatherer,
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

/// Build the width-sweep progress detail: a one-line summary followed by a
/// blank line, a per-engine `considered/captured/excluded` table with the
/// exclusion reasons broken out by column (T-006), and a trailing blank line.
/// The balance `considered == captured + excluded + capped + cancelled` always
/// holds, where `capped` URLs were dropped by the fetch budget; the reason
/// columns always sum back to `excluded`. The `fetch` reason column is
/// additionally broken down by fine-grained fetch-failure cause so the cause of
/// each network-stage drop is visible. `cancelled` is structurally 0 - the
/// fetch stage is not deadline-bounded and never cancels fetches on the phase
/// deadline; it is rendered only if a future truncation site ever makes it
/// non-zero.
fn format_width_sweep_detail(
    query_count: usize,
    engines: &[String],
    considered: usize,
    captured: usize,
    excluded: usize,
    per_engine: &[crate::web_gatherer::EngineSweepStat],
    excluded_by_reason: &std::collections::BTreeMap<ExclusionReason, usize>,
    failed_by_kind: &std::collections::BTreeMap<FetchFailureKind, usize>,
    capped: usize,
    cancelled: usize,
) -> String {
    let engines = engines.join(", ");
    let mut out = format!(
        "queries={query_count}, engines=[{engines}], considered={considered}, \
         captured={captured}, excluded={excluded}"
    );
    if capped > 0 || cancelled > 0 {
        out.push_str(&format!(", capped={capped}, cancelled={cancelled}"));
    }
    if per_engine.is_empty() {
        return out;
    }
    // Column order for the exclusion-reason breakdown, then the fetch-failure
    // breakdown of the `fetch` column.
    let reasons = ExclusionReason::ALL;
    let kinds = FetchFailureKind::ALL;
    let mut header = format!(
        "{:<12} {:>10} {:>8} {:>8}",
        "engine", "considered", "captured", "excluded"
    );
    for reason in reasons {
        header.push_str(&format!(" {:>8}", reason.short_label()));
    }
    for kind in kinds {
        header.push_str(&format!(" {:>6}", kind.short_label()));
    }
    let render_row = |label: &str,
                      considered: usize,
                      captured: usize,
                      counts: &[usize],
                      fail: &[usize]|
     -> String {
        let mut row = format!(
            "{:<12} {:>10} {:>8} {:>8}",
            label,
            considered,
            captured,
            counts.iter().sum::<usize>()
        );
        for count in counts {
            row.push_str(&format!(" {:>8}", count));
        }
        for count in fail {
            row.push_str(&format!(" {:>6}", count));
        }
        row
    };
    // Blank line separating the summary line from the table.
    out.push_str("\n\n");
    out.push_str(&header);
    for stat in per_engine {
        let counts: Vec<usize> = reasons
            .iter()
            .map(|reason| stat.excluded_by_reason.get(reason).copied().unwrap_or(0))
            .collect();
        let fails: Vec<usize> = kinds
            .iter()
            .map(|kind| stat.failed_by_kind.get(kind).copied().unwrap_or(0))
            .collect();
        out.push('\n');
        out.push_str(&render_row(
            &stat.engine,
            stat.considered,
            stat.captured,
            &counts,
            &fails,
        ));
    }
    let totals: Vec<usize> = reasons
        .iter()
        .map(|reason| excluded_by_reason.get(reason).copied().unwrap_or(0))
        .collect();
    // The reason columns must sum back to the authoritative `excluded`; guard
    // the invariant so a future exclusion site that bumps `excluded` without a
    // reason is caught in tests rather than silently mis-reported.
    debug_assert_eq!(
        totals.iter().sum::<usize>(),
        excluded,
        "per-reason exclusion counts must sum to `excluded`"
    );
    let total_fails: Vec<usize> = kinds
        .iter()
        .map(|kind| failed_by_kind.get(kind).copied().unwrap_or(0))
        .collect();
    out.push('\n');
    out.push_str(&render_row(
        "totals",
        considered,
        captured,
        &totals,
        &total_fails,
    ));
    // Blank line separating the table from any trailing diagnostics.
    out.push_str("\n\n");
    if capped > 0 || cancelled > 0 {
        out.push_str(&format!(
            "(unfetched: {capped} fetch-capped, {cancelled} deadline-cancelled)"
        ));
    }
    out
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
            // H-002: retry events are logged so the UI surfaces the
            // transient failure transparently.
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
            GatherEvent::WidthSweepSummary {
                queries,
                engines,
                considered,
                captured,
                excluded,
                per_engine,
                excluded_by_reason,
                failed_by_kind,
                capped,
                cancelled,
            } => {
                let detail = format_width_sweep_detail(
                    queries.len(),
                    &engines,
                    considered,
                    captured,
                    excluded,
                    &per_engine,
                    &excluded_by_reason,
                    &failed_by_kind,
                    capped,
                    cancelled,
                );
                self.observer.on_event(SessionEvent::RunStep {
                    step: crate::run_manifest::RunStep::WidthSweep
                        .as_str()
                        .to_string(),
                    status: crate::run_manifest::StepStatus::InProgress
                        .as_str()
                        .to_string(),
                    detail: Some(detail),
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
            GatherEvent::SearchBudgetExhausted { used, limit } => {
                let limit_desc =
                    limit.map_or_else(|| "unlimited".to_string(), |l| format!("{l} calls"));
                self.observer.on_event(SessionEvent::RunStep {
                    step: "search_budget".to_string(),
                    status: crate::run_manifest::StepStatus::Skipped.as_str().to_string(),
                    detail: Some(format!(
                        "run search budget of {limit_desc} exhausted after {used}; proceeding with sources gathered so far"
                    )),
                });
            }
            GatherEvent::ProviderCallsSummary { tool_calls } => {
                let detail = if tool_calls.is_empty() {
                    "no search provider calls were issued this run".to_string()
                } else {
                    let per_tool = tool_calls
                        .iter()
                        .map(|(tool, count)| format!("{tool}: {count}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let total: usize = tool_calls.iter().map(|(_, c)| c).sum();
                    format!("{total} search request(s) ({per_tool})")
                };
                self.observer.on_event(SessionEvent::RunStep {
                    step: "search_providers".to_string(),
                    status: crate::run_manifest::StepStatus::Completed
                        .as_str()
                        .to_string(),
                    detail: Some(detail),
                });
            }
            GatherEvent::SearchPartiallyFailed {
                failed,
                total,
                error,
            } => {
                // FUNC-030: a partially-failed sweep is not a clean run.
                self.observer.on_event(SessionEvent::RunStep {
                    step: "search_partial_failure".to_string(),
                    status: crate::run_manifest::StepStatus::Skipped
                        .as_str()
                        .to_string(),
                    detail: Some(format!(
                        "{failed} of {total} sub-query searches failed; coverage is incomplete (last error: {error})"
                    )),
                });
            }
            GatherEvent::VaultStoreFailed { url, error } => {
                // FUNC-030: the source is captured but not persisted; surface
                // the divergence instead of counting it silently.
                self.observer.on_event(SessionEvent::RunStep {
                    step: "vault_store_failed".to_string(),
                    status: crate::run_manifest::StepStatus::Skipped
                        .as_str()
                        .to_string(),
                    detail: Some(format!(
                        "failed to persist source to vault ({url}): {error}"
                    )),
                });
            }
        }
    }
}
/// Inputs the caller supplies to [`ResearchSession::run`].
#[derive(Debug, Clone, Default)]
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
    /// the topic is ambiguous (FR-005, FR-017). Defaults to `false`; front-end
    /// callers opt in with `--clarify`.
    pub clarify: bool,
    /// Explicit research brief generated from the user's prompt. When `Some`,
    /// downstream agents use this as their mission statement instead of
    /// deriving one from the topic (FR-004 brief context).
    pub brief: Option<String>,
    /// Per-phase model overrides (FR-013).
    pub models: ModelConfig,
    /// Verbatim front-end invocation (CLI command, TUI slash command, or HTTP
    /// request summary) recorded in `RESEARCH.md` frontmatter for replay.
    pub invocation: Option<String>,
    /// When `true`, run the deterministic self-evaluation scorecard and append
    /// it to the assembled report (FR-008 / T-015).
    pub evaluate: bool,
}

/// Topic and seed inputs for a research session.
#[derive(Debug, Clone, Default)]
pub struct InputConfig {
    /// Free-form research topic - used to derive web queries and grep terms.
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
    /// `--url-cloak`: when `true`, web URLs emitted in the `Sources` bullets
    /// and the `References Index` / `Sources Reference` tables are defanged
    /// (`hxxps://` scheme + `[.]` dots, wrapped in a code span) so automated
    /// URL scanners do not treat them as live links.
    pub url_cloak: bool,
}

/// Web-gathering knobs for a research session.
#[derive(Debug, Clone)]
pub struct WebConfig {
    /// Maximum web sources to capture. A value of `0` means "derive the
    /// budget from the selected depth" (see
    /// [`SessionConfig::effective_web_budget`]); the default config uses the
    /// sentinel so `--depth` actually bounds gathering volume unless the
    /// caller passes an explicit `--max-web-results`.
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
    /// `--max-search-calls N`: hard cap on the number of web-search calls the
    /// whole run may issue (including supervisor/competitive researchers and
    /// retries). When `None`, no cap is applied (historic behaviour). The cap
    /// is shared via `Arc` across every gatherer in the run, and exhausted
    /// budgets degrade the run to its partial results instead of failing.
    pub max_search_calls: Option<usize>,
}

/// Default wall-clock budget for the entire web-gathering phase
/// ([`WebConfig::web_phase_timeout_secs`]). 180 seconds gives the web phase
/// room to gather a fuller source set by default: when the budget elapses the
/// gatherer stops issuing new searches and fetches and returns everything
/// captured so far, so the run proceeds to analysis/synthesis with the
/// partial source set. Override per run with `--web-time N`
/// (`--web-phase-timeout-secs N`), or disable with `--web-time 0`.
pub const DEFAULT_WEB_PHASE_TIMEOUT_SECS: u64 = 180;

/// Character budget for the `--from-url` / `--from-file` seed body preview.
///
/// The preview is surfaced in the `FromUrlBodyPreview` / `FromFileBodyPreview`
/// session events. Named here rather than inline so the cap is visible and
/// shared by the seed helpers (ANTIPAT F-22).
const BODY_PREVIEW_CHARS: usize = 200;

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
    /// `ragent_research::local_gatherer::DEFAULT_LOCAL_CONCURRENCY` (8).
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
#[derive(Debug, Clone)]
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
    /// Maximum number of concept sections retained in the report's
    /// `## Concepts` block (`--max-concepts`, FR-001..FR-008). Defaults to
    /// [`crate::limits::DEFAULT_MAX_CONCEPTS`] (5). A value of `0` means
    /// "unbounded" and applies no truncation (FR-016).
    pub max_concepts: usize,
    /// Maximum number of findings retained in the report's `## Findings`
    /// block (`--max-findings`, FR-001..FR-008). Defaults to
    /// [`crate::limits::DEFAULT_MAX_FINDINGS`] (20). A value of `0` means
    /// "unbounded" and applies no truncation (FR-016).
    pub max_findings: usize,
}

impl Default for AnalysisConfig {
    /// Built-in analysis defaults, using the shared output limits so both
    /// findings and concepts default to the same caps as a caller that sets
    /// neither flag (FR-002, FR-014).
    fn default() -> Self {
        Self {
            depth: None,
            iterations: None,
            max_synthesis_sources: None,
            summarization_model: None,
            contradiction: None,
            max_concepts: crate::limits::DEFAULT_MAX_CONCEPTS,
            max_findings: crate::limits::DEFAULT_MAX_FINDINGS,
        }
    }
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
    /// `research.supervisor.max_concurrent_research_units` - maximum parallel
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

    /// Effective web-source budget for gather passes: an explicit
    /// `--max-web-results` (`max_web_results > 0`) always wins; otherwise the
    /// budget is derived from the selected depth via [`Self::budget_web_results`]
    /// so `--depth shallow` genuinely bounds web volume instead of every
    /// preset collapsing into the 500-source default.
    #[must_use]
    pub fn effective_web_budget(&self) -> usize {
        if self.web.max_web_results > 0 {
            self.web.max_web_results
        } else {
            self.budget_web_results()
        }
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
            url_cloak: false,
        }
    }
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            // 0 = derive the effective budget from the selected depth (see
            // `SessionConfig::effective_web_budget`) so `--depth` bounds
            // web volume by default; an explicit `--max-web-results` always
            // overrides.
            max_web_results: 0,
            fetch_concurrency: DEFAULT_FETCH_CONCURRENCY,
            fetch_timeout_secs: 30,
            use_low_relevance: false,
            disable_scholarly: false,
            use_pdf_web_sources: false,
            web_phase_timeout_secs: Some(DEFAULT_WEB_PHASE_TIMEOUT_SECS),
            max_search_calls: None,
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
        /// Research mode (`tiered`, `supervisor`, `competitive`).
        mode: String,
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
/// `SessionEvent::SynthesizeResult`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynthesizeOutcome {
    /// An LLM produced a structured [`AnalysisResult`] and it was used.
    Llm,
    /// The LLM-backed engine returned empty content (e.g. parsing failed);
    /// the mechanical fallback supplied the summary/findings.
    FallbackEmpty,
    /// The LLM-backed engine returned an error (no key, network failure, ...)
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
/// `SynthesizeOutcome` emitted in the `SynthesizeResult` session event.
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

/// Inputs to [`ResearchSession::assemble_and_write`].
///
/// Grouped into a struct so the finalize call site stays readable and so new
/// document options do not keep lengthening the parameter list.
pub(crate) struct AssembleInput<'a> {
    /// URL-safe research item name (for the manager's run bookkeeping).
    pub name_str: &'a str,
    /// Parsed research name.
    pub name: &'a ResearchName,
    /// Human-readable title.
    pub title: &'a str,
    /// Research topic.
    pub topic: &'a str,
    /// Base item carrying frontmatter; cloned and augmented before writing.
    pub item: &'a ResearchItem,
    /// Merged sources to reference.
    pub sources: Vec<Source>,
    /// Synthesis result, completed with deterministic fallbacks when empty.
    pub analysis: AnalysisResult,
    /// Provenance of the synthesis result.
    pub synth_outcome: SynthesizeOutcome,
    /// Optional research brief.
    pub brief: Option<&'a str>,
    /// Decomposed web queries.
    pub queries: Vec<String>,
    /// Output format for the assembled document.
    pub output_format: OutputFormat,
    /// Optional pre-rendered comparison table.
    pub comparison_table: Option<String>,
    /// Whether to emit the self-evaluation scorecard.
    pub evaluate: bool,
    /// Recorded invocation for frontmatter replay.
    pub invocation: Option<String>,
    /// Whether to defang web source URLs in the output.
    pub url_cloak: bool,
    /// Concept-list cap.
    pub max_concepts: usize,
    /// Finding-list cap.
    pub max_findings: usize,
    /// Progress observer.
    pub observer: Arc<dyn SessionObserver>,
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

    /// Access the configured planner, falling back to the heuristic default
    /// when none was wired. Five call sites previously repeated this
    /// `unwrap_or_else` chain inline.
    fn planner_or_default(&self) -> Arc<dyn Planner> {
        self.planner
            .clone()
            .unwrap_or_else(|| Arc::new(crate::planner::HeuristicPlanner::new()))
    }

    /// Access the optional critic.
    #[must_use]
    pub fn critic(&self) -> Option<Arc<dyn Critic>> {
        self.critic.clone()
    }

    /// Access the configured critic, falling back to the simple default when
    /// none was wired.
    fn critic_or_default(&self) -> Arc<dyn Critic> {
        self.critic
            .clone()
            .unwrap_or_else(|| Arc::new(crate::engine::SimpleCritic))
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

    /// Access the optional run-scoped per-provider search-request counter.
    /// Cloning the `Arc` shares the same totals; attach it back via
    /// [`WebGatherer::with_provider_stats`] so every gather pass in the run
    /// contributes to one aggregate view.
    #[must_use]
    pub fn provider_stats(&self) -> Option<Arc<crate::provider_stats::ProviderCallStats>> {
        self.web.as_ref().and_then(WebGatherer::provider_stats)
    }

    /// Run the supervisor/researcher graph for `--mode supervisor|competitive`.
    ///
    /// `item` has already been created and marked `InProgress` by the caller.
    /// `seed_sources` and `seed_queries` come from `--from-url` / `--from-file`
    /// pre-steps. The graph runs: Plan -> Delegate -> Collect -> Synthesize ->
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
                    &extraction.entities,
                    &extraction.criteria,
                );
                // Cap the researcher count at the configured concurrency:
                // competitive topics are one-per-entity and previously ran
                // uncapped, so entity-heavy topics scaled search quota
                // linearly with the entity list.
                let topics = if competitive_topics.is_empty() {
                    // Fall back to generic supervisor planning if no entities were
                    // identified so the run still produces something useful.
                    let supervisor = SupervisorNode::new(self.planner_or_default())
                        .with_max_sub_topics(supervisor_cfg.max_concurrent_research_units);
                    supervisor.plan(topic).await.map_err(|e| {
                        ResearchError::EngineRunFailed(format!("supervisor planning failed: {e}"))
                    })?
                } else {
                    competitive_topics
                        .into_iter()
                        .take(supervisor_cfg.max_concurrent_research_units)
                        .collect::<Vec<_>>()
                };
                (topics, Some(extraction))
            } else {
                let supervisor = SupervisorNode::new(self.planner_or_default())
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

        // Run-scoped search controls shared by every researcher in this run:
        //
        // - Budget (`--max-search-calls`): one counter shared via `Arc`, so
        //   N parallel researchers draw from a single per-run pool.
        // - Query cache: competitive/supervisor researchers decompose their
        //   (entity-scoped) sub-topics into near-identical dimension queries;
        //   the shared cache turns duplicate queries into free lookups
        //   instead of repeated paid search calls.
        let search_budget = config
            .web
            .max_search_calls
            .map(|limit| Arc::new(crate::search_budget::SearchBudget::new(Some(limit))));
        let query_cache = Arc::new(crate::search_budget::SharedQueryCache::new());
        // Per-provider search-request counter: one shared Arc so every
        // researcher's searches roll up into a single per-run view,
        // reported at the end of the run.
        let provider_stats = Arc::new(crate::provider_stats::ProviderCallStats::new());
        let researcher_web = self.web.clone().map(|w| {
            let w = match &search_budget {
                Some(budget) => w.with_search_budget(budget.clone()),
                None => w,
            };
            // Competitive (comparison) runs cap search/fetch volume so one
            // comparison does not issue engine-max searches per entity:
            // per-query allowance + fetch budget both come from
            // `effective_web_budget()`. Tiered/supervisor runs stay uncapped -
            // the per-fetch timeout bounds each page instead.
            let w = if config.engine.mode == ResearchMode::Competitive {
                w.with_volume_cap(Some(config.effective_web_budget()))
            } else {
                w
            };
            w.with_query_cache(query_cache)
                .with_provider_stats(provider_stats.clone())
        });

        let node = IterativeResearcherNode::new(researcher_web, self.analysis.clone())
            .with_planner(self.planner_or_default())
            .with_critic(self.critic_or_default())
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
                    // Match the assignment generated FOR this entity (its
                    // sub-topic starts with "Research {entity}") rather than
                    // any sub-topic that merely mentions it - competitive
                    // sub-topics embed the full topic, which names every
                    // entity, so a `contains` match would attribute the first
                    // entity's summary to all rows.
                    let summary = state
                        .assignments
                        .iter()
                        .find(|a| {
                            crate::supervisor::sub_topic_matches_entity(&a.sub_topic, &entity.name)
                        })
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
            .assemble_and_write(AssembleInput {
                name_str,
                name: &name,
                title,
                topic,
                item,
                sources: merged_sources,
                analysis,
                synth_outcome,
                brief,
                queries: seed_queries,
                output_format: config.output.output_format,
                comparison_table,
                evaluate: config.evaluate,
                invocation: config.invocation.clone(),
                url_cloak: config.output.url_cloak,
                max_concepts: config.analysis.max_concepts,
                max_findings: config.analysis.max_findings,
                observer: observer.clone(),
            })
            .await?;

        router.complete_run(router_observer);

        Ok(outcome)
    }

    pub(crate) async fn assemble_and_write(&self, input: AssembleInput<'_>) -> Result<RunOutcome> {
        let AssembleInput {
            name_str,
            name,
            title,
            topic,
            item,
            sources,
            mut analysis,
            synth_outcome,
            brief,
            queries,
            output_format,
            comparison_table,
            evaluate,
            invocation,
            url_cloak,
            max_concepts,
            max_findings,
            observer,
        } = input;
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

        // ── Finding cap (spec researchmax; FR-003, FR-010, FR-020, FR-022) ──
        // Order the findings by reverse relevance and truncate to the effective
        // limit before the document is assembled, so the retained count and
        // `Finding N` numbering are contiguous and stable across modes.
        analysis.findings =
            crate::analysis::cap_findings_to_limit(analysis.findings, max_findings, &sources);

        let concepts_section = if let Some(engine) = &self.concepts_engine {
            self.extract_concepts_inner(name, &sources, engine, max_concepts)
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
        // Populate the References Index inputs: without this the supervisor
        // path ships `sources: 0` frontmatter and a "No sources captured"
        // table even when researchers captured evidence.
        for s in &sources {
            item_with_sources.add_source(s.clone());
        }
        if output_format != OutputFormat::Report {
            item_with_sources.output_format = Some(output_format.as_str().to_string());
        }
        item_with_sources.url_cloak = url_cloak;
        item_with_sources.invocation = invocation.or_else(|| item.invocation.clone());

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
            provider_stats: None,
        };

        let final_title = if llm_produced && !doc.summary.trim().is_empty() {
            crate::item::truncate_title(&doc.summary)
        } else {
            title.to_string()
        };
        doc.item.set_title(&final_title);

        // ── Per-provider search-request summary ────────────────────────────
        // Report how many search requests each provider received across the
        // whole run (default pipeline, tiered iterations, and every
        // supervisor/competitive researcher combined) and embed the table in
        // the assembled document.
        let provider_tool_calls = self.report_provider_stats(&observer);
        let provider_stats_md =
            crate::document::render_provider_stats_summary(&provider_tool_calls);
        let provider_stats = if provider_stats_md.is_empty() {
            None
        } else {
            Some(provider_stats_md)
        };
        doc.provider_stats = provider_stats;

        let assembled = self.manager.write_document(&doc).await?;
        self.manager.complete_gathering(name_str).await?;

        let counts = MediaCounts::of(&sources);

        observer.on_event(SessionEvent::Done {
            total_sources: sources.len(),
            pdf_count: counts.pdf,
            youtube_count: counts.youtube,
            excluded_count: 0,
        });

        Ok(RunOutcome {
            research_name: name.to_string(),
            sources,
            document: assembled,
            web_queries: doc.decomposed_queries.clone(),
            pdf_count: counts.pdf,
            youtube_count: counts.youtube,
            excluded_count: 0,
            provider_tool_calls,
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
        // PERF-049: per-record appends are buffered; flush run-log markers so
        // post-mortem tooling sees a completed run without waiting for the
        // buffer to fill or the process to exit.
        if let Err(e) = lock.flush() {
            tracing::warn!(error = %e, event, "research: run log flush failed");
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
            mode: config.engine.mode.as_str().to_string(),
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
        // Preserve the invocation recorded in an existing item's frontmatter
        // so `continue`/re-runs keep the original command when the front-end
        // does not supply a fresh one.
        let stored_invocation = item.invocation.clone();
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

        // ── Decide single-pass vs. iterative engine ─────────────────────
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
                    let budget = config.effective_web_budget().clamp(3, 10);
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
                // - synthesis failures are the reason RESEARCH.md ends up
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
                "_Synthesis engine failed ({}): {} - the fallback mechanical digest is shown below._\n\n{}",
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

        // ── Finding cap (spec researchmax; FR-003, FR-010, FR-020, FR-022) ──
        // The patcher and polish steps above may reorder or rewrite findings,
        // so the cap runs last: order by reverse relevance against the same
        // sources the References Index uses, truncate to the effective limit,
        // and renumber the survivors contiguously.
        analysis.findings = crate::analysis::cap_findings_to_limit(
            analysis.findings,
            config.analysis.max_findings,
            &synthesis_sources,
        );

        // ── Concepts (spec researchcluster) ──────────────────────────────
        // Extract the cross-source concept list from the same gathered corpus
        // the synthesis step consumed. The section renders directly above
        // `## Findings` in `RESEARCH.md`. When no concepts engine is wired
        // (or the extraction fails), the section is omitted entirely.
        let concepts_section = self
            .extract_concepts_section(
                &name,
                &synthesis_sources,
                &observer,
                config.analysis.max_concepts,
            )
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
        item_with_sources.url_cloak = config.output.url_cloak;
        item_with_sources.invocation = config.invocation.clone().or(stored_invocation);
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
            provider_stats: None,
        };
        // The frontmatter `title` should be a reduced-length version of the
        // final summary (max 80 chars) so the displayed headline reflects the
        // synthesis rather than the original prompt.  When the synthesis fell
        // back to the mechanical path (malformed model output, engine error, or
        // no LLM engine), the summary is a diagnostic placeholder - not a
        // meaningful headline - so we keep the topic-derived title instead.
        let final_title =
            if synth_outcome == SynthesizeOutcome::Llm && !doc.summary.trim().is_empty() {
                truncate_title(&doc.summary)
            } else {
                item_title
            };
        doc.item.set_title(&final_title);
        // ── Per-provider search-request summary ────────────────────────────
        // Report how many search requests each provider received across the
        // whole run (default pipeline, tiered iterations, and every
        // supervisor/competitive researcher combined) and embed the table in
        // the assembled document.
        let provider_tool_calls = self.report_provider_stats(&observer);
        let provider_stats_md =
            crate::document::render_provider_stats_summary(&provider_tool_calls);
        doc.provider_stats = if provider_stats_md.is_empty() {
            None
        } else {
            Some(provider_stats_md)
        };
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
        let finalize_counts = MediaCounts::of(&synthesis_sources);
        let pdf_count = pdf_count.max(finalize_counts.pdf);
        let youtube_count = youtube_count.max(finalize_counts.youtube);
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
            provider_tool_calls,
        })
    }
}

impl ResearchSession {
    /// Emit the end-of-run per-provider search-request summary.
    ///
    /// Sends one `search_providers` [`SessionEvent::RunStep`] carrying the
    /// per-tool totals gathered across the whole run (default pipeline,
    /// tiered iterations, and every supervisor/competitive researcher
    /// combined) and returns the `(tool, count)` snapshot for the
    /// [`RunOutcome`]. Emits nothing when no provider-stats counter is
    /// attached or no search calls were issued.
    fn report_provider_stats(&self, observer: &Arc<dyn SessionObserver>) -> Vec<(String, usize)> {
        let Some(stats) = self.provider_stats() else {
            return Vec::new();
        };
        let tool_calls = stats.by_tool();
        if tool_calls.is_empty() {
            return tool_calls;
        }
        let per_tool = tool_calls
            .iter()
            .map(|(tool, count)| format!("{tool}: {count}"))
            .collect::<Vec<_>>()
            .join(", ");
        let total: usize = tool_calls.iter().map(|(_, count)| count).sum();
        observer.on_event(SessionEvent::RunStep {
            step: "search_providers".to_string(),
            status: crate::run_manifest::StepStatus::Completed
                .as_str()
                .to_string(),
            detail: Some(format!("{total} search request(s) ({per_tool})")),
        });
        tool_calls
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

    /// Strip fenced code blocks and take the first [`BODY_PREVIEW_CHARS`]
    /// characters as a preview. Shared by the `--from-url` and `--from-file`
    /// seed helpers.
    ///
    /// Accumulates lazily and stops once the byte budget is reached, so a
    /// large body is never fully copied: `str::len()` is a lower bound on the
    /// character count, so `out.len() >= BODY_PREVIEW_CHARS` guarantees the
    /// character budget is already met.
    fn body_preview(body: &str) -> String {
        let mut out = String::new();
        for line in body.lines().filter(|l| !l.trim_start().starts_with("```")) {
            if out.len() >= BODY_PREVIEW_CHARS {
                break;
            }
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line);
        }
        out.chars().take(BODY_PREVIEW_CHARS).collect()
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
            // SEC-ragent-research-005 (SECTASKS T-016): a `from_files` entry
            // must resolve inside the project root. The extracted body is
            // streamed back to the caller and embedded as a research source,
            // so an uncontained entry is an arbitrary file read.
            let project_root = project_root_for(self.manager.root());
            if let Some(bad) = out_of_root_from_file(file_path, project_root) {
                return Err(ResearchError::FromFileExtractFailed {
                    path: path_str.clone(),
                    message: format!("path resolves outside the project root '{}'", bad.display()),
                });
            }
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
                let web_budget = config.effective_web_budget();
                // H-001 / --web-time: convert the optional phase timeout into
                // a wall-clock deadline for the *search stage*. When the
                // deadline passes the search stage returns with whatever was
                // searched so far (plus a `web_deadline` RunStep diagnostic)
                // and every candidate found is then fetched to completion -
                // fetches are never gated on or cancelled by the deadline.
                // `--web-time 0` disables the deadline entirely.
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
                // Run-scoped search budget (`--max-search-calls`): bounds
                // the paid search calls issued by the main gather pass.
                if let Some(limit) = config.web.max_search_calls {
                    web = web.with_search_budget(Arc::new(
                        crate::search_budget::SearchBudget::new(Some(limit)),
                    ));
                }
                // Run-scoped per-provider search-request counter so the main
                // gather pass contributes to the end-of-run provider totals.
                if let Some(stats) = self.provider_stats() {
                    web = web.with_provider_stats(stats);
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
        let planner = self.planner_or_default();
        let critic = self.critic_or_default();
        // Run-scoped search budget (`--max-search-calls`): every engine
        // iteration draws from the same per-run pool.
        let engine_web = self.web.clone().map(|w| {
            let w = match config.web.max_search_calls {
                Some(limit) => w.with_search_budget(Arc::new(
                    crate::search_budget::SearchBudget::new(Some(limit)),
                )),
                None => w,
            };
            // Run-scoped per-provider search-request counter so iterative
            // passes contribute to the end-of-run provider totals.
            match self.provider_stats() {
                Some(stats) => w.with_provider_stats(stats),
                None => w,
            }
        });
        let engine = IterativeEngine::new(
            planner,
            engine_web,
            self.analysis.clone(),
            critic,
            config.engine_config(),
        )
        // FR-006: the iterative path must honour the same web-phase deadline
        // as the overlapped single-pass path (search stage only; the fetch
        // stage is never deadline-bounded). `Some(0)` disables it.
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
    /// logged and reported via a `RunStep` event, but never aborts the run -
    /// the document simply renders without the section.
    pub async fn extract_concepts_section(
        &self,
        name: &ResearchName,
        sources: &[Source],
        observer: &Arc<dyn SessionObserver>,
        max_concepts: usize,
    ) -> Option<String> {
        let Some(engine) = &self.concepts_engine else {
            return None;
        };
        observer.on_event(SessionEvent::RunStep {
            step: "concepts".to_string(),
            status: "started".to_string(),
            detail: None,
        });
        let result = self
            .extract_concepts_inner(name, sources, engine, max_concepts)
            .await;
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
    ///
    /// `max_concepts` is the effective concept limit (0 = unbounded) and is
    /// injected into the prompt so the model is asked for at most that many
    /// concepts (FR-001, FR-015).
    pub(crate) async fn extract_concepts_inner(
        &self,
        name: &ResearchName,
        sources: &[Source],
        engine: &LlmAnalysisEngine,
        max_concepts: usize,
    ) -> anyhow::Result<Option<String>> {
        let research_root = self.manager.root().to_path_buf();
        let name = name.clone();
        let sources = sources.to_vec();
        let web_index_map = build_web_index_map(&sources);
        // Keep a copy for the post-extraction concept ordering (the `sources`
        // binding is consumed by the blocking body loader below).
        let rank_sources = sources.clone();
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
        let prompt =
            crate::cluster::build_concept_extraction_prompt_for_limit(&payload, max_concepts);
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
            &rank_sources,
            max_concepts,
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
    /// Per-provider search-request totals for the run as
    /// `(search tool, call count)` pairs, sorted by tool name. Empty when the
    /// run gathered no sources via the web pipeline or no provider-stats
    /// counter was attached.
    pub provider_tool_calls: Vec<(String, usize)>,
}

// ── Free helpers ─────────────────────────────────────────────────────────

/// Map web supporting-file numbers (`sources/web-NN.md`) to the combined
/// 1-based References Index position of each web source in `sources`, so
/// filename-style citations (`web-NN`) emitted by the concept-extraction LLM
/// can be rewritten to `[#N]` markers that resolve against `RESEARCH.md`.
fn build_web_index_map(sources: &[Source]) -> rustc_hash::FxHashMap<usize, usize> {
    static WEB_FILE_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let web_file_re =
        // INVARIANT: compile-time-constant regex; the call cannot fail at runtime.
        WEB_FILE_RE.get_or_init(|| regex::Regex::new(r"web-(\d+)\.md$").expect("valid regex"));
    let mut map = rustc_hash::FxHashMap::default();
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

/// Single-pass media-type tallies over a source list.
///
/// The run pipeline previously re-scanned `sources` with two separate
/// `matches!` filters (pdf, youtube) at three different sites (gather-complete
/// logging, finalize, document assembly); this struct computes all counts in
/// one pass and is shared by those sites.
#[derive(Debug, Default, Clone, Copy)]
struct MediaCounts {
    pdf: usize,
    youtube: usize,
}

impl MediaCounts {
    /// Count the web media types in `sources` in a single pass.
    fn of(sources: &[Source]) -> Self {
        let mut counts = Self::default();
        for source in sources {
            if let Source::Web { media_type, .. } = source {
                match media_type.as_str() {
                    "pdf" => counts.pdf += 1,
                    "youtube" => counts.youtube += 1,
                    _ => {}
                }
            }
        }
        counts
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

/// Return the resolved path when `candidate` escapes `root`, else `None`.
///
/// SEC-ragent-research-005 (SECTASKS T-016): shared by the `from_files` seed
/// reader. An existing path is canonicalised (so a symlink out of the tree is
/// caught); a not-yet-existing path falls back to a lexical check that rejects
/// any parent-directory component and any absolute path outside the root.
fn out_of_root_from_file(candidate: &Path, root: &Path) -> Option<std::path::PathBuf> {
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let resolved = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        canonical_root.join(candidate)
    };
    let contained = match resolved.canonicalize() {
        Ok(real) => real.starts_with(&canonical_root),
        Err(_) => {
            !resolved
                .components()
                .any(|c| c == std::path::Component::ParentDir)
                && resolved.starts_with(&canonical_root)
        }
    };
    if contained { None } else { Some(resolved) }
}

/// Load a FR-020 template body from `_templates/<name>.md` if it exists.
/// Returns `None` when no template was requested, or when the file does
/// not exist.
async fn load_template(research_root: &Path, template: Option<&str>) -> Option<String> {
    let name = template?;
    // SEC-ragent-research-001 (SECTASKS T-014): `template_path` returns `None`
    // for a traversal/absolute/separator name, so a rejected template simply
    // falls back to the built-in document shape.
    let path = ResearchIo::template_path(research_root, name)?;
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
#[path = "../tests/inline/session_tests.rs"]
mod tests;
