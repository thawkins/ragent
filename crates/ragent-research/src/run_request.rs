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
    DEFAULT_FETCH_CONCURRENCY, DEFAULT_FETCH_TIMEOUT, DEFAULT_SEARCH_CIRCUIT_BREAKER_THRESHOLD,
    DEFAULT_SEARCH_MAX_RETRIES, DEFAULT_SEARCH_RETRY_BASE_DELAY_MS,
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
    /// `--use-local` — enable the local-file scanning phase.
    pub use_local: bool,
    /// `--use-specs` — enable the prior-spec cross-reference phase.
    pub use_specs: bool,
    /// `--use-low-relevance` — keep low-relevance web sources.
    pub use_low_relevance: bool,
    /// `--no-scholarly` — disable scholarly search engines.
    pub no_scholarly: bool,
    /// `--use-pdf` — allow PDF documents from web search/`--from-url`.
    pub use_pdf: bool,
    /// `--fetch-concurrently N` — max parallel page fetches.
    pub fetch_concurrency: Option<usize>,
    /// `--local-concurrently N` — max parallel local scoring/spec-scan tasks.
    pub local_concurrency: Option<usize>,
    /// `--fetch-timeout-secs N` — per-page fetch timeout.
    pub fetch_timeout_secs: Option<u64>,
    /// `--web-phase-timeout-secs N` — wall-clock timeout for the web phase.
    pub web_phase_timeout_secs: Option<u64>,
    /// `--local-phase-timeout-secs N` — wall-clock timeout for the local phase.
    pub local_phase_timeout_secs: Option<u64>,
    /// `--search-max-retries N` — max retry attempts for failed sub-query search.
    pub search_max_retries: Option<u32>,
    /// `--search-retry-base-delay-ms N` — first retry backoff base delay.
    pub search_retry_base_delay_ms: Option<u64>,
    /// `--search-circuit-breaker-threshold N` — consecutive failures before
    /// circuit breaker opens.
    pub search_circuit_breaker_threshold: Option<u32>,
    /// Override the maximum number of web sources to capture.
    pub max_web_results: Option<usize>,
    /// `--max-search-calls N` — hard cap on total web-search calls for the
    /// run, shared across all supervisor/competitive researchers and gather
    /// passes. When `None`, no cap is applied.
    pub max_search_calls: Option<usize>,
    /// Override the maximum number of in-project local sources to capture.
    pub max_local_sources: Option<usize>,
    /// Override the maximum number of sources sent to the LLM synthesis engine.
    pub max_synthesis_sources: Option<usize>,
    /// `--summarization-model <provider:model>` override.
    pub summarization_model: Option<String>,
    /// `--mode tiered|supervisor|competitive` research execution strategy.
    pub mode: Option<String>,
    /// `--max-concurrent-research-units N` for supervisor/competitive modes.
    pub max_concurrent_research_units: Option<usize>,
    /// `--no-clarify` disables the single clarifying question.
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
    /// - CLI argv (recorded verbatim): `<binary> research create <name> …`
    /// - TUI slash command: `/research create <name> …`
    /// - HTTP summary: `POST /research <name> "topic" --flag …` (the `create`
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
                no_papers,
                use_pdf,
                fetch_timeout_secs,
                local_concurrency,
                web_phase_timeout_secs,
                local_phase_timeout_secs,
                search_max_retries,
                search_retry_base_delay_ms,
                search_circuit_breaker_threshold,
                max_web_results,
                max_search_calls,
                max_local_sources,
                max_synthesis_sources,
                brief,
                evaluate,
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
                    no_scholarly: no_papers,
                    use_pdf,
                    fetch_concurrency,
                    local_concurrency,
                    fetch_timeout_secs,
                    web_phase_timeout_secs,
                    local_phase_timeout_secs,
                    search_max_retries,
                    search_retry_base_delay_ms,
                    search_circuit_breaker_threshold,
                    max_web_results,
                    max_search_calls,
                    max_local_sources,
                    max_synthesis_sources,
                    summarization_model,
                    mode,
                    max_concurrent_research_units,
                    clarify,
                    brief,
                    research_model,
                    compression_model,
                    final_report_model,
                    evaluate: Some(evaluate),
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

/// Normalize a recorded invocation to the shared `create …` grammar used by
/// [`crate::cli::ResearchCliCommand::parse`].
///
/// - TUI slash form `/research create …` keeps its verb and drops the prefix.
/// - HTTP summary form `POST /research <name> …` has the `create` verb
///   inserted.
/// - CLI argv form `<binary> research create …` skips every leading token up
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
            disable_scholarly: req.no_scholarly,
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
            summarization_model: req.summarization_model.clone(),
            contradiction: None,
        },
        resilience: ResilienceConfig {
            search_max_retries: req.search_max_retries.unwrap_or(DEFAULT_SEARCH_MAX_RETRIES),
            search_retry_base_delay_ms: req
                .search_retry_base_delay_ms
                .unwrap_or(DEFAULT_SEARCH_RETRY_BASE_DELAY_MS),
            search_circuit_breaker_threshold: req
                .search_circuit_breaker_threshold
                .unwrap_or(DEFAULT_SEARCH_CIRCUIT_BREAKER_THRESHOLD),
            open_access_recovery: cfg_research
                .map(|r| r.open_access_recovery)
                .unwrap_or(false),
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
        clarify: req.clarify.unwrap_or(true),
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
mod tests {
    #![allow(clippy::assert_is_empty)]
    use super::*;
    use crate::session::DEFAULT_WEB_PHASE_TIMEOUT_SECS;

    #[test]
    fn default_request_has_no_subject() {
        let req = ResearchRunRequest::default();
        assert!(req.missing_subject());
    }

    #[test]
    fn request_with_topic_has_subject() {
        let req = ResearchRunRequest::new("rust", "Rust programming language");
        assert!(!req.missing_subject());
        assert_eq!(req.name, "rust");
        assert_eq!(req.topic, "Rust programming language");
    }

    #[test]
    fn build_session_config_applies_defaults() {
        let req = ResearchRunRequest::new("test", "a topic");
        let cfg = build_session_config(&req, None);
        assert_eq!(cfg.input.topic, "a topic");
        assert_eq!(cfg.engine.tier, Tier::Full);
        assert_eq!(cfg.output.output_format, OutputFormat::Report);
        // Default request has use_local=false / use_specs=false, which map to
        // disable_local=true / disable_specs=true in SessionConfig.
        assert!(cfg.local.disable_local);
        assert!(cfg.local.disable_specs);
        assert!(!cfg.resilience.open_access_recovery);
        assert_eq!(
            cfg.resilience.oa_min_full_text_chars,
            DEFAULT_OA_MIN_FULL_TEXT_CHARS
        );
    }

    #[test]
    fn build_session_config_parses_tier_and_format() {
        let req = ResearchRunRequest {
            name: "test".into(),
            topic: "topic".into(),
            tier: Some("light".into()),
            output_format: Some("imrad".into()),
            use_local: true,
            use_specs: true,
            ..ResearchRunRequest::default()
        };
        let cfg = build_session_config(&req, None);
        assert_eq!(cfg.engine.tier, Tier::Light);
        assert_eq!(cfg.output.output_format, OutputFormat::Imrad);
        assert!(!cfg.local.disable_local);
        assert!(!cfg.local.disable_specs);
    }

    #[test]
    fn build_session_config_parses_mode() {
        let req = ResearchRunRequest {
            name: "test".into(),
            topic: "topic".into(),
            mode: Some("competitive".into()),
            ..ResearchRunRequest::default()
        };
        let cfg = build_session_config(&req, None);
        assert_eq!(cfg.engine.mode, ResearchMode::Competitive);
    }

    #[test]
    fn nested_defaults_match_legacy_flat_defaults() {
        let cfg = SessionConfig::default();
        assert!(cfg.input.topic.is_empty());
        assert!(cfg.input.sources_dir.is_none());
        assert!(cfg.input.from_urls.is_empty());
        assert!(cfg.input.from_files.is_empty());
        assert!(cfg.output.template.is_none());
        assert_eq!(cfg.output.output_format, OutputFormat::Report);
        // Default config uses the 0 sentinel = derive the web budget from
        // the selected depth (`SessionConfig::effective_web_budget`).
        assert_eq!(cfg.web.max_web_results, 0);
        assert_eq!(cfg.web.fetch_concurrency, DEFAULT_FETCH_CONCURRENCY);
        assert_eq!(cfg.web.fetch_timeout_secs, 30);
        assert!(!cfg.web.use_low_relevance);
        assert!(!cfg.web.disable_scholarly);
        assert!(!cfg.web.use_pdf_web_sources);
        assert_eq!(
            cfg.web.web_phase_timeout_secs,
            Some(DEFAULT_WEB_PHASE_TIMEOUT_SECS)
        );
        assert_eq!(cfg.local.max_local_sources, 10);
        assert!(!cfg.local.disable_local);
        assert!(!cfg.local.disable_specs);
        assert_eq!(cfg.local.local_concurrency, DEFAULT_LOCAL_CONCURRENCY);
        assert!(cfg.local.local_phase_timeout_secs.is_none());
        assert!(cfg.analysis.depth.is_none());
        assert!(cfg.analysis.iterations.is_none());
        assert!(cfg.analysis.max_synthesis_sources.is_none());
        assert_eq!(
            cfg.resilience.search_max_retries,
            DEFAULT_SEARCH_MAX_RETRIES
        );
        assert_eq!(
            cfg.resilience.search_retry_base_delay_ms,
            DEFAULT_SEARCH_RETRY_BASE_DELAY_MS
        );
        assert_eq!(
            cfg.resilience.search_circuit_breaker_threshold,
            DEFAULT_SEARCH_CIRCUIT_BREAKER_THRESHOLD
        );
        assert!(!cfg.resilience.open_access_recovery);
        assert!(cfg.resilience.contact_email.is_none());
        assert_eq!(
            cfg.resilience.oa_min_full_text_chars,
            DEFAULT_OA_MIN_FULL_TEXT_CHARS
        );
        assert_eq!(cfg.engine.tier, Tier::Full);
    }

    #[test]
    fn build_session_config_zero_web_phase_timeout_disables_deadline() {
        let req = ResearchRunRequest {
            name: "test".into(),
            topic: "topic".into(),
            web_phase_timeout_secs: Some(0),
            ..ResearchRunRequest::default()
        };
        let cfg = build_session_config(&req, None);
        assert_eq!(
            cfg.web.web_phase_timeout_secs,
            Some(0),
            "Some(0) must be preserved as the disabled-deadline sentinel"
        );
    }

    #[test]
    fn build_session_config_default_web_phase_timeout_is_60() {
        let req = ResearchRunRequest::new("test", "topic");
        let cfg = build_session_config(&req, None);
        assert_eq!(
            cfg.web.web_phase_timeout_secs,
            Some(DEFAULT_WEB_PHASE_TIMEOUT_SECS),
            "default web_phase_timeout must be 60 seconds (NFR-003)"
        );
    }

    #[test]
    fn build_session_config_parses_all_modes() {
        for (raw, expected) in [
            ("tiered", ResearchMode::Tiered),
            ("supervisor", ResearchMode::Supervisor),
            ("competitive", ResearchMode::Competitive),
        ] {
            let req = ResearchRunRequest {
                name: "test".into(),
                topic: "topic".into(),
                mode: Some(raw.into()),
                ..ResearchRunRequest::default()
            };
            let cfg = build_session_config(&req, None);
            assert_eq!(
                cfg.engine.mode, expected,
                "`--mode {raw}` must parse to {expected:?}"
            );
        }
    }

    /// Research items are discovered by directory under `<research_root>/`
    /// regardless of research mode, so the RESEARCH.md write path is the same
    /// for `tiered` (default `/research create`) and the supervisor/competitive
    /// `--mode` runs. Pin that invariant so a mode branch can never drift to a
    /// different output folder.
    #[test]
    fn research_md_path_is_mode_independent() {
        let root = std::path::Path::new("research");
        let name = crate::research_name::ResearchName::try_new("mode-output").expect("valid name");
        let path = crate::io::ResearchIo::research_md_path(root, &name);
        assert_eq!(
            path,
            root.join("mode-output").join("RESEARCH.md"),
            "RESEARCH.md must always be written under <research_root>/<name>/"
        );
    }
}
