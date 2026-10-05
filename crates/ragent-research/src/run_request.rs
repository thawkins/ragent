//! Shared research run request and [`SessionConfig`] builder.
//!
//! This module provides [`ResearchRunRequest`], a front-end-agnostic value type
//! that captures every input the research pipeline accepts from the CLI, TUI,
//! and HTTP API. The [`build_session_config`] function is the single place
//! where front-end requests are turned into a fully populated
//! [`SessionConfig`], applying `ragent.json` `research.*` defaults where the
//! caller did not supply an explicit override.
//!
//! Centralising the builder eliminates the three independent `SessionConfig`
//! literals that previously drifted out of sync (RESEARCHPLAN.md R-001).

use std::path::PathBuf;

use thiserror::Error;

use crate::run_config::{Depth, OutputFormat, ResearchMode, Tier};
use crate::session::{
    AnalysisConfig, InputConfig, LocalConfig, ModelConfig, OutputConfig, ResilienceConfig,
    RunEngineConfig, SessionConfig, WebConfig,
};
use crate::web_gatherer::{
    DEFAULT_FETCH_CONCURRENCY, DEFAULT_FETCH_TIMEOUT, DEFAULT_SEARCH_MAX_RETRIES,
    DEFAULT_SEARCH_RETRY_BASE_DELAY_MS,
};
use crate::{
    DEFAULT_LOCAL_CONCURRENCY, DEFAULT_MAX_LOCAL_SOURCES, DEFAULT_OA_MIN_FULL_TEXT_CHARS,
    DEFAULT_WEB_PHASE_TIMEOUT_SECS,
};

/// Front-end-agnostic inputs for a single research run.
///
/// `ResearchRunRequest` deliberately stores stringly-typed enumerations
/// (`depth`, `tier`, `output_format`) so that loosely-typed sources such as
/// CLI flags and JSON bodies can be fed in without each front-end reimplementing
/// parsing. [`build_session_config`] converts these into the crate's strongly
/// typed value types and applies defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResearchRunRequest {
    /// Research item name (URL-safe identifier).
    pub name: String,
    /// Free-form research topic. Optional when `from_urls` or `from_files` are
    /// supplied; in that case the topic is derived from the fetched/extracted
    /// content.
    pub topic: String,
    /// Optional human-readable title override. When `None`, callers should
    /// derive the title from `topic`/`from_urls`/`from_files`.
    pub title: Option<String>,
    /// URLs to fetch and use as primary research subjects.
    pub from_urls: Vec<String>,
    /// Local file paths to extract and use as primary research subjects.
    pub from_files: Vec<String>,
    /// Optional extra sources directory (FR-019).
    pub sources_dir: Option<String>,
    /// Optional template name (FR-020).
    pub template: Option<String>,
    /// `--depth shallow|standard|deep`.
    pub depth: Option<String>,
    /// `--tier light|full|dissertation`.
    pub tier: Option<String>,
    /// `--iterations N` override.
    pub iterations: Option<u32>,
    /// `--format report|executive-summary|comparison-table|source-bibliography|imrad`.
    /// When `None` and `--mode competitive` is set the builder defaults this to
    /// `comparison-table`; any explicit value wins.
    pub output_format: Option<String>,
    /// `--use-local` - enable the local-file scanning phase.
    pub use_local: bool,
    /// `--use-specs` - enable the prior-spec cross-reference phase.
    pub use_specs: bool,
    /// `--use-low-relevance` - keep low-relevance web sources.
    pub use_low_relevance: bool,
    /// `--papers` - include scholarly search engines. Scholarly engines are
    /// excluded from the web-gathering phase by default; the flag opts them
    /// back in.
    pub papers: bool,
    /// `--use-pdf` - allow PDF documents from web search/`--from-url`.
    pub use_pdf: bool,
    /// `--oa-enable` / `--no-oa` - per-run open-access recovery override.
    ///
    /// `Some(true)` forces OA recovery on, `Some(false)` forces it off, and
    /// `None` defers to `research.open_access_recovery` in the loaded
    /// configuration (which itself defaults to off).
    pub open_access_recovery: Option<bool>,
    /// `--fetch-concurrently N` - max parallel page fetches.
    pub fetch_concurrency: Option<usize>,
    /// `--local-concurrently N` - max parallel local scoring/spec-scan tasks.
    pub local_concurrency: Option<usize>,
    /// `--fetch-timeout-secs N` - per-page fetch timeout.
    pub fetch_timeout_secs: Option<u64>,
    /// `--web-phase-timeout-secs N` - wall-clock timeout for the web phase.
    pub web_phase_timeout_secs: Option<u64>,
    /// `--local-phase-timeout-secs N` - wall-clock timeout for the local phase.
    pub local_phase_timeout_secs: Option<u64>,
    /// `--search-max-retries N` - max retry attempts for failed sub-query search.
    pub search_max_retries: Option<u32>,
    /// `--search-retry-base-delay-ms N` - first retry backoff base delay.
    pub search_retry_base_delay_ms: Option<u64>,
    /// Override the maximum number of web sources to capture.
    pub max_web_results: Option<usize>,
    /// `--max-search-calls N` - hard cap on total web-search calls for the
    /// run, shared across all supervisor/competitive researchers and gather
    /// passes. When `None`, no cap is applied.
    pub max_search_calls: Option<usize>,
    /// Override the maximum number of in-project local sources to capture.
    pub max_local_sources: Option<usize>,
    /// Override the maximum number of sources sent to the LLM synthesis engine.
    pub max_synthesis_sources: Option<usize>,
    /// `--max-concepts N` - maximum number of concepts rendered in the report's
    /// `## Concepts` block. When `None`, the configured `research.max_concepts`
    /// value is used, falling back to the built-in default (5). `0` means
    /// "unbounded" (FR-012, FR-014, FR-016).
    pub max_concepts: Option<usize>,
    /// `--max-findings N` - maximum number of findings rendered in the report's
    /// `## Findings` block. When `None`, the configured `research.max_findings`
    /// value is used, falling back to the built-in default (20). `0` means
    /// "unbounded" (FR-012, FR-014, FR-016).
    pub max_findings: Option<usize>,
    /// `--summarization-model <provider:model>` override.
    pub summarization_model: Option<String>,
    /// `--mode tiered|supervisor|competitive` research execution strategy.
    pub mode: Option<String>,
    /// `--max-concurrent-research-units N` for supervisor/competitive modes.
    pub max_concurrent_research_units: Option<usize>,
    /// `--clarify` / `--no-clarify` control the single clarifying question
    /// asked before web searches when the topic looks ambiguous. When `None`
    /// the builder defaults it to `false` (no clarification); pass `Some(true)`
    /// via `--clarify` to enable it.
    pub clarify: Option<bool>,
    /// `--brief <TEXT>` explicit research brief.
    pub brief: Option<String>,
    /// `--research-model <provider:model>` per-phase model override.
    pub research_model: Option<String>,
    /// `--compression-model <provider:model>` per-phase model override.
    pub compression_model: Option<String>,
    /// `--final-report-model <provider:model>` per-phase model override.
    pub final_report_model: Option<String>,
    /// `--evaluate` enables deterministic self-evaluation scorecard.
    pub evaluate: Option<bool>,
    /// `--url-cloak` - defang web URLs in the `Sources` bullets and the
    /// `References Index` / `Sources Reference` tables of the generated
    /// `RESEARCH.md` and `CORPA.md` so automated URL scanners do not treat
    /// them as live, clickable links. Defaults to off.
    pub url_cloak: bool,
    /// Verbatim front-end invocation (e.g. `ragent research create --name x
    /// "topic" --tier full`) recorded in `RESEARCH.md` frontmatter so a future
    /// `/research update` command can replay the run.
    pub invocation: Option<String>,
}

impl ResearchRunRequest {
    /// Create a request with just a name and topic.
    #[must_use]
    pub fn new(name: impl Into<String>, topic: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            topic: topic.into(),
            ..Self::default()
        }
    }

    /// Return `true` when there is no explicit topic and no seed URL/file to
    /// derive one from.
    #[must_use]
    pub fn missing_subject(&self) -> bool {
        self.topic.is_empty() && self.from_urls.is_empty() && self.from_files.is_empty()
    }

    /// Rebuild a [`ResearchRunRequest`] from a recorded front-end invocation
    /// string so `/research update` can replay the original run.
    ///
    /// Three invocation grammars are recognized (see the `invocation` field
    /// documentation):
    ///
    /// - CLI argv (recorded verbatim): `<binary> research create <name> ...`
    /// - TUI slash command: `/research create <name> ...`
    /// - HTTP summary: `POST /research <name> "topic" --flag ...` (the `create`
    ///   verb is implied and inserted).
    ///
    /// The returned request keeps `invocation` set to the recorded command so
    /// a replayed run re-stamps the original invocation rather than the
    /// `update` command that triggered it. `title` is intentionally left
    /// `None`; callers derive it from the stored item.
    ///
    /// # Errors
    ///
    /// Returns [`InvocationParseError`] when the string is empty, does not
    /// describe a `create` run, or lacks an item name.
    pub fn from_invocation(invocation: &str) -> std::result::Result<Self, InvocationParseError> {
        let trimmed = invocation.trim();
        if trimmed.is_empty() {
            return Err(InvocationParseError::Empty);
        }
        let command = normalize_invocation(trimmed);
        match crate::cli::ResearchCliCommand::parse(&command) {
            crate::cli::ResearchCliCommand::Create {
                name,
                topic,
                from_urls,
                from_files,
                iterations,
                depth,
                tier,
                mode,
                summarization_model,
                research_model,
                compression_model,
                final_report_model,
                max_concurrent_research_units,
                clarify,
                format,
                sources_dir,
                template,
                fetch_concurrency,
                use_local,
                use_specs,
                use_low_relevance,
                papers,
                use_pdf,
                oa_recovery,
                fetch_timeout_secs,
                local_concurrency,
                web_phase_timeout_secs,
                local_phase_timeout_secs,
                search_max_retries,
                search_retry_base_delay_ms,
                max_web_results,
                max_search_calls,
                max_local_sources,
                max_synthesis_sources,
                max_concepts,
                max_findings,
                brief,
                evaluate,
                url_cloak,
            } => {
                if name.is_empty() {
                    return Err(InvocationParseError::MissingName);
                }
                Ok(Self {
                    name,
                    topic,
                    title: None,
                    from_urls,
                    from_files,
                    sources_dir,
                    template,
                    depth,
                    tier,
                    iterations,
                    output_format: format,
                    use_local,
                    use_specs,
                    use_low_relevance,
                    papers,
                    use_pdf,
                    open_access_recovery: oa_recovery,
                    fetch_concurrency,
                    local_concurrency,
                    fetch_timeout_secs,
                    web_phase_timeout_secs,
                    local_phase_timeout_secs,
                    search_max_retries,
                    search_retry_base_delay_ms,
                    max_web_results,
                    max_search_calls,
                    max_local_sources,
                    max_synthesis_sources,
                    max_concepts,
                    max_findings,
                    summarization_model,
                    mode,
                    max_concurrent_research_units,
                    clarify,
                    brief,
                    research_model,
                    compression_model,
                    final_report_model,
                    evaluate: Some(evaluate),
                    url_cloak,
                    // Keep the recorded command verbatim so the replayed run
                    // re-stamps the original invocation in frontmatter.
                    invocation: Some(trimmed.to_string()),
                })
            }
            crate::cli::ResearchCliCommand::Unknown(verb) if verb == "create" => {
                Err(InvocationParseError::MissingName)
            }
            crate::cli::ResearchCliCommand::Unknown(verb) => {
                Err(InvocationParseError::NotCreate(verb))
            }
            _ => Err(InvocationParseError::NotCreate(
                "a non-create research command".to_string(),
            )),
        }
    }
}

/// Errors surfaced when replaying a recorded invocation string
/// ([`ResearchRunRequest::from_invocation`]).
#[derive(Debug, Error)]
pub enum InvocationParseError {
    /// The recorded invocation was empty.
    #[error("recorded invocation is empty; nothing to replay")]
    Empty,
    /// The recorded invocation is not a `create` run (only create runs can
    /// be replayed).
    #[error(
        "recorded invocation is {0}, not a research create command; \
         only create runs can be replayed"
    )]
    NotCreate(String),
    /// The recorded create command carried no research item name.
    #[error("recorded invocation has no research item name")]
    MissingName,
}

/// Normalize a recorded invocation to the shared `create ...` grammar used by
/// [`crate::cli::ResearchCliCommand::parse`].
///
/// - TUI slash form `/research create ...` keeps its verb and drops the prefix.
/// - HTTP summary form `POST /research <name> ...` has the `create` verb
///   inserted.
/// - CLI argv form `<binary> research create ...` skips every leading token up
///   to and including the `research` subcommand token, so any binary path
///   (including paths containing spaces) is handled.
fn normalize_invocation(invocation: &str) -> String {
    let trimmed = invocation.trim();
    if let Some(rest) = trimmed.strip_prefix("/research ") {
        return rest.to_string();
    }
    if let Some(rest) = trimmed.strip_prefix("POST /research ") {
        return format!("create {rest}");
    }
    let first_token = trimmed.split(' ').next().unwrap_or_default();
    if first_token == "create" {
        // Already in the shared parser grammar.
        return trimmed.to_string();
    }
    // CLI argv form: skip leading tokens until the `research` subcommand.
    let mut rest = trimmed;
    while let Some((head, tail)) = rest.split_once(' ') {
        if head == "research" {
            return tail.to_string();
        }
        rest = tail;
    }
    // Unrecognized form; return as-is so the shared parser reports it.
    rest.to_string()
}

/// Build a fully populated [`SessionConfig`] from a front-end request.
///
/// Explicit fields in `req` take precedence. When a field is `None`/`false`, the
/// configured `ragent.json` `research.*` values are used for open-access
/// recovery settings, and the crate-wide constants are used for concurrency,
/// timeout, and retry defaults.
///
/// Mode-aware defaults: a `competitive` run without an explicit
/// `--format` defaults to `comparison-table`.
///
/// `app_config` is the loaded `ragent.json` configuration, when available.
/// When `None`, open-access recovery defaults to disabled and the default OA
/// minimum length is used.
#[must_use]
pub fn build_session_config(
    req: &ResearchRunRequest,
    app_config: Option<&ragent_config::Config>,
) -> SessionConfig {
    let cfg_research = app_config.map(|c| &c.research);

    // Concept and finding limits: an explicit per-run flag wins; otherwise the
    // configured `research.max_concepts` / `research.max_findings`; otherwise
    // the built-in default (FR-012, FR-014, FR-017).
    let max_concepts = req.max_concepts.unwrap_or_else(|| {
        cfg_research
            .map(|r| r.max_concepts)
            .unwrap_or(crate::limits::DEFAULT_MAX_CONCEPTS)
    });
    let max_findings = req.max_findings.unwrap_or_else(|| {
        cfg_research
            .map(|r| r.max_findings)
            .unwrap_or(crate::limits::DEFAULT_MAX_FINDINGS)
    });

    let tier = req
        .tier
        .as_deref()
        .and_then(Tier::parse)
        .unwrap_or(Tier::Full);

    let mode = req
        .mode
        .as_deref()
        .and_then(ResearchMode::parse)
        .unwrap_or(ResearchMode::Tiered);

    // `--mode competitive` implies `--format comparison-table` unless the
    // caller supplied an explicit `--format` (which always wins).
    let output_format = match req.output_format.as_deref() {
        Some(s) => OutputFormat::parse(s).unwrap_or(OutputFormat::Report),
        None if mode == ResearchMode::Competitive => OutputFormat::ComparisonTable,
        None => OutputFormat::Report,
    };

    SessionConfig {
        input: InputConfig {
            topic: req.topic.clone(),
            from_urls: req.from_urls.clone(),
            from_files: req.from_files.iter().map(PathBuf::from).collect(),
            sources_dir: req.sources_dir.as_ref().map(PathBuf::from),
        },
        output: OutputConfig {
            template: req.template.clone(),
            output_format,
            url_cloak: req.url_cloak,
        },
        web: WebConfig {
            // 0 = derive the effective budget from the selected depth (see
            // `SessionConfig::effective_web_budget`); an explicit
            // `--max-web-results` always wins.
            max_web_results: req.max_web_results.unwrap_or(0),
            fetch_concurrency: req.fetch_concurrency.unwrap_or(DEFAULT_FETCH_CONCURRENCY),
            fetch_timeout_secs: req
                .fetch_timeout_secs
                .unwrap_or(DEFAULT_FETCH_TIMEOUT.as_secs()),
            use_low_relevance: req.use_low_relevance,
            // Precedence: scholarly engines are excluded unless the per-run
            // `--papers` flag opts them back in. The `research.
            // exclude_academic_engines` config setting only records the
            // exclusion; scholarly engines are already off by default, so the
            // setting has no additional effect on a run.
            disable_scholarly: !req.papers,
            use_pdf_web_sources: req.use_pdf,
            web_phase_timeout_secs: req
                .web_phase_timeout_secs
                .or(Some(DEFAULT_WEB_PHASE_TIMEOUT_SECS)),
            max_search_calls: req.max_search_calls,
        },
        local: LocalConfig {
            max_local_sources: req.max_local_sources.unwrap_or(DEFAULT_MAX_LOCAL_SOURCES),
            disable_local: !req.use_local,
            disable_specs: !req.use_specs,
            local_concurrency: req.local_concurrency.unwrap_or(DEFAULT_LOCAL_CONCURRENCY),
            local_phase_timeout_secs: req.local_phase_timeout_secs,
        },
        analysis: AnalysisConfig {
            depth: req.depth.as_deref().and_then(Depth::parse),
            iterations: req.iterations,
            max_synthesis_sources: req.max_synthesis_sources,
            max_concepts,
            max_findings,
            summarization_model: req.summarization_model.clone(),
            contradiction: None,
        },
        resilience: ResilienceConfig {
            search_max_retries: req.search_max_retries.unwrap_or(DEFAULT_SEARCH_MAX_RETRIES),
            search_retry_base_delay_ms: req
                .search_retry_base_delay_ms
                .unwrap_or(DEFAULT_SEARCH_RETRY_BASE_DELAY_MS),
            // Precedence: explicit `--oa-enable` / `--no-oa` wins; otherwise
            // the configured `research.open_access_recovery`; otherwise off.
            open_access_recovery: req.open_access_recovery.unwrap_or_else(|| {
                cfg_research
                    .map(|r| r.open_access_recovery)
                    .unwrap_or(false)
            }),
            contact_email: cfg_research.and_then(|r| r.contact_email.clone()),
            oa_min_full_text_chars: cfg_research
                .map(|r| r.oa_min_full_text_chars)
                .unwrap_or(DEFAULT_OA_MIN_FULL_TEXT_CHARS),
        },
        engine: RunEngineConfig {
            tier,
            mode,
            max_concurrent_research_units: req
                .max_concurrent_research_units
                .unwrap_or(crate::supervisor::DEFAULT_MAX_CONCURRENT_RESEARCH_UNITS),
        },
        clarify: req.clarify.unwrap_or(false),
        brief: req.brief.clone(),
        invocation: req.invocation.clone(),
        models: ModelConfig {
            research_model: req.research_model.clone(),
            compression_model: req.compression_model.clone(),
            final_report_model: req.final_report_model.clone(),
        },
        evaluate: req.evaluate.unwrap_or(false),
    }
}

#[cfg(test)]
#[path = "../tests/inline/run_request_tests.rs"]
mod tests;
