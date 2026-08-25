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

use crate::run_config::{Depth, OutputFormat, Tier};
use crate::session::{
    AnalysisConfig, InputConfig, LocalConfig, OutputConfig, ResilienceConfig, RunEngineConfig,
    SessionConfig, WebConfig,
};
use crate::web_gatherer::{
    DEFAULT_FETCH_CONCURRENCY, DEFAULT_FETCH_TIMEOUT, DEFAULT_MAX_WEB_RESULTS,
    DEFAULT_SEARCH_CIRCUIT_BREAKER_THRESHOLD, DEFAULT_SEARCH_MAX_RETRIES,
    DEFAULT_SEARCH_RETRY_BASE_DELAY_MS,
};
use crate::{DEFAULT_LOCAL_CONCURRENCY, DEFAULT_MAX_LOCAL_SOURCES, DEFAULT_OA_MIN_FULL_TEXT_CHARS};

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
    /// Override the maximum number of in-project local sources to capture.
    pub max_local_sources: Option<usize>,
    /// Override the maximum number of sources sent to the LLM synthesis engine.
    pub max_synthesis_sources: Option<usize>,
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
}

/// Build a fully populated [`SessionConfig`] from a front-end request.
///
/// Explicit fields in `req` take precedence. When a field is `None`/`false`, the
/// configured `ragent.json` `research.*` values are used for open-access
/// recovery settings, and the crate-wide constants are used for concurrency,
/// timeout, and retry defaults.
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

    SessionConfig {
        input: InputConfig {
            topic: req.topic.clone(),
            from_urls: req.from_urls.clone(),
            from_files: req.from_files.iter().map(PathBuf::from).collect(),
            sources_dir: req.sources_dir.as_ref().map(PathBuf::from),
        },
        output: OutputConfig {
            template: req.template.clone(),
            output_format: req
                .output_format
                .as_deref()
                .map_or(OutputFormat::Report, |s| {
                    OutputFormat::parse(s).unwrap_or(OutputFormat::Report)
                }),
        },
        web: WebConfig {
            max_web_results: req.max_web_results.unwrap_or(DEFAULT_MAX_WEB_RESULTS),
            fetch_concurrency: req.fetch_concurrency.unwrap_or(DEFAULT_FETCH_CONCURRENCY),
            fetch_timeout_secs: req
                .fetch_timeout_secs
                .unwrap_or(DEFAULT_FETCH_TIMEOUT.as_secs()),
            use_low_relevance: req.use_low_relevance,
            disable_scholarly: req.no_scholarly,
            use_pdf_web_sources: req.use_pdf,
            web_phase_timeout_secs: req.web_phase_timeout_secs,
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
        engine: RunEngineConfig { tier },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn nested_defaults_match_legacy_flat_defaults() {
        let cfg = SessionConfig::default();
        assert!(cfg.input.topic.is_empty());
        assert!(cfg.input.sources_dir.is_none());
        assert!(cfg.input.from_urls.is_empty());
        assert!(cfg.input.from_files.is_empty());
        assert!(cfg.output.template.is_none());
        assert_eq!(cfg.output.output_format, OutputFormat::Report);
        assert_eq!(cfg.web.max_web_results, DEFAULT_MAX_WEB_RESULTS);
        assert_eq!(cfg.web.fetch_concurrency, DEFAULT_FETCH_CONCURRENCY);
        assert_eq!(cfg.web.fetch_timeout_secs, 30);
        assert!(!cfg.web.use_low_relevance);
        assert!(!cfg.web.disable_scholarly);
        assert!(!cfg.web.use_pdf_web_sources);
        assert!(cfg.web.web_phase_timeout_secs.is_none());
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
}
