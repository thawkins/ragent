//! `mf_search` tool — keyless multi-engine web search with consensus ranking.
//!
//! Implements FR-008 through FR-010, FR-022, FR-023, FR-025, FR-026.
//!
//! Queries multiple public search-engine backends in parallel (`DuckDuckGo`,
//! Brave, and optionally more), merges and deduplicates results by normalised
//! URL, ranks by relevance with cross-engine consensus boosting, and returns a
//! formatted result list with structured signals.
//!
//! # Pipeline
//!
//! 1. Validate the query (non-empty after trim).
//! 2. Build [`SearchOptions`] from the tool's input parameters (site,
//!    `exclude_sites`, freshness, `max_results`, page).
//! 3. Run [`SearchOrchestrator::search`] to query all backends in parallel,
//!    merge via consensus, and cache for 5 minutes.
//! 4. Format the ranked results as a human-readable text report.
//! 5. Populate structured metadata with FR-009 signals: `relevance_score`,
//!    `fetch_relevance`, `engines_consensus`, `related_queries`,
//!    `fetch_hint`, `engine_blocked`, `cached`, `duration_ms`.
//!
//! # API keys
//!
//! The tool is **keyless by default** (FR-023): it scrapes public search-engine
//! HTML result pages via DuckDuckGo and Brave. No API keys are required for
//! those backends. If a `langsearch_api_key` or `tavily_api_key` is configured
//! in `ragent.json` (or the corresponding environment variable is set), an
//! optional API-backed engine is added for higher-quality results; the keys are
//! masked in diagnostics and never logged.

use anyhow::Result;
use serde_json::{Value, json};

use std::collections::HashSet;
use std::sync::Arc;

use super::super::MASTERFETCH_VERSION;
use super::super::search::consensus::MergeOutput;
use super::super::search::langsearch::LangSearchEngine;
use super::super::search::tavily::TavilyEngine;
use super::super::search::{Freshness, SearchEngine, SearchOptions, SearchOrchestrator};

use crate::{Tool, ToolContext, ToolOutput};

// ---------------------------------------------------------------------------
// Tool struct
// ---------------------------------------------------------------------------

/// Keyless multi-engine web search with consensus ranking.
///
/// No API keys required. Multiple backends run in parallel; results are merged,
/// deduplicated, and ranked with cross-engine consensus boosting. Each result
/// carries `relevance_score`, `fetch_relevance` (high/med/low), and
/// `engines_consensus`.
pub struct MfSearchTool;

/// Result of probing a single search backend with a fixed diagnostic query.
///
/// Used by the TUI `/websearch test` command to report whether each engine
/// returned any results and how many.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineTestResult {
    /// Human-readable engine name.
    pub name: String,
    /// Whether the engine returned at least one result.
    pub returned_results: bool,
    /// Number of raw results returned by the engine.
    pub result_count: usize,
    /// Error or blocked message, empty when the engine succeeded.
    pub error: String,
}

/// Availability status of a single web-search backend.
///
/// Returned by [`MfSearchTool::engine_status`] for UI diagnostics (e.g. the
/// TUI `/websearch show` command). `enabled` means the engine can be used with
/// the current configuration; `in_use` means it is currently wired into the
/// search orchestrator; `failed` means it is unavailable due to missing or
/// invalid configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineStatus {
    /// Human-readable engine name.
    pub name: &'static str,
    /// Whether the engine is configured and available.
    pub enabled: bool,
    /// Whether the engine is currently active in the orchestrator.
    pub in_use: bool,
    /// Whether the engine is unavailable because configuration is missing.
    pub failed: bool,
}

impl MfSearchTool {
    /// Build the [`SearchOrchestrator`] for this tool based on the supplied
    /// [`ToolContext`].
    ///
    /// The orchestrator always includes the keyless `DuckDuckGo` and Brave
    /// backends. If `ctx.config` contains a non-empty `langsearch_api_key`, a
    /// [`LangSearchEngine`] is added as a third backend. If `ctx.config`
    /// contains a non-empty `tavily_api_key` (or `TAVILY_API_KEY` environment
    /// variable), a [`TavilyEngine`] is added as an additional backend.
    ///
    /// This helper is public so integration tests can verify backend wiring
    /// without making network requests.
    #[must_use]
    pub fn build_orchestrator(ctx: &ToolContext) -> SearchOrchestrator {
        let (langsearch_key, tavily_key) = Self::resolve_search_keys(ctx);
        let mut engines: Vec<Arc<dyn SearchEngine>> = vec![
            Arc::new(super::super::search::duckduckgo::DuckDuckGoEngine::new()),
            Arc::new(super::super::search::brave::BraveEngine::new()),
        ];
        if let Some(key) = langsearch_key
            && !key.is_empty()
        {
            engines.push(Arc::new(LangSearchEngine::new(key)));
        }
        if let Some(key) = tavily_key {
            engines.push(Arc::new(TavilyEngine::new(key)));
        }
        SearchOrchestrator::with_engines(engines)
    }

    /// Resolve configured API keys for optional search backends.
    ///
    /// Returns `(langsearch_key, tavily_key)` where `langsearch_key` is taken
    /// from `ctx.config.langsearch_api_key` and `tavily_key` is taken from the
    /// `TAVILY_API_KEY` environment variable or `ctx.config.tavily_api_key`.
    fn resolve_search_keys(ctx: &ToolContext) -> (Option<&str>, Option<String>) {
        let langsearch_key = ctx
            .config
            .as_ref()
            .and_then(|cfg| cfg.langsearch_api_key.as_deref());
        let tavily_key = std::env::var("TAVILY_API_KEY")
            .ok()
            .or_else(|| {
                ctx.config
                    .as_ref()
                    .and_then(|cfg| cfg.tavily_api_key.clone())
            })
            .filter(|k| !k.is_empty());
        (langsearch_key, tavily_key)
    }

    /// Return availability status for all possible search backends.
    ///
    /// DuckDuckGo and Brave are always considered available. LangSearch and
    /// Tavily require an API key and are marked as `failed` when the key is
    /// missing. `in_use` reflects the engines that are actually wired into the
    /// orchestrator built from the supplied [`ToolContext`].
    #[must_use]
    pub fn engine_status(ctx: &ToolContext) -> Vec<EngineStatus> {
        let (langsearch_key, tavily_key) = Self::resolve_search_keys(ctx);
        let orchestrator = Self::build_orchestrator(ctx);
        let in_use: HashSet<&str> = orchestrator.engine_names().into_iter().collect();
        vec![
            EngineStatus {
                name: "DuckDuckGo",
                enabled: true,
                in_use: in_use.contains("duckduckgo"),
                failed: false,
            },
            EngineStatus {
                name: "Brave",
                enabled: true,
                in_use: in_use.contains("brave"),
                failed: false,
            },
            EngineStatus {
                name: "LangSearch",
                enabled: langsearch_key.is_some_and(|k| !k.is_empty()),
                in_use: in_use.contains("langsearch"),
                failed: langsearch_key.is_none_or(|k| k.is_empty()),
            },
            EngineStatus {
                name: "Tavily",
                enabled: tavily_key.is_some(),
                in_use: in_use.contains("tavily"),
                failed: tavily_key.is_none(),
            },
        ]
    }

    /// Probe every configured search backend with a fixed diagnostic query.
    ///
    /// Runs the query `"what is websearch"` against each engine that is
    /// currently wired into the orchestrator and returns per-engine counts.
    /// This is a live network test, so it may take several seconds on a slow
    /// connection. Engines that are not configured (e.g. missing API keys) are
    /// omitted from the test run.
    pub async fn engine_test(ctx: &ToolContext) -> Vec<EngineTestResult> {
        let orchestrator = Self::build_orchestrator(ctx);
        let opts = SearchOptions::new(5);
        let reports = orchestrator
            .search_per_engine("what is websearch", &opts)
            .await;
        reports
            .into_iter()
            .map(|report| {
                let name = report.engine.clone();
                let count = report.result_count;
                let error = if report.engine_blocked {
                    format!("blocked: {}", report.error)
                } else if report.error.is_empty() {
                    String::new()
                } else {
                    report.error.clone()
                };
                EngineTestResult {
                    name,
                    returned_results: count > 0,
                    result_count: count,
                    error,
                }
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl Tool for MfSearchTool {
    fn name(&self) -> &'static str {
        "mf_search"
    }

    fn description(&self) -> &'static str {
        "Local keyless web search. Required parameter: 'query'. Multiple backends \
             run in parallel (DuckDuckGo, Brave, optional LangSearch / Tavily when \
             configured via 'langsearch_api_key' or 'tavily_api_key'). Keyless backends \
             do not require API keys. Optional 'site', 'exclude_sites', 'freshness' \
             (day/week/month/year), 'max_results' (1-50, default 6), and 'page' (0-10). \
             Each result carries relevance_score, fetch_relevance, and engines_consensus."
    }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                },
                "site": {
                    "type": "string",
                    "description": "Restrict results to this domain"
                },
                "exclude_sites": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Domains to exclude from results"
                },
                "freshness": {
                    "type": "string",
                    "enum": ["day", "week", "month", "year"],
                    "description": "Time filter for results"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum results to return (1-50, default: 6)"
                },
                "page": {
                    "type": "integer",
                    "description": "Result page (0-10, default: 0)"
                }
            },
            "required": ["query"],
            "additionalProperties": false
        })
    }
    fn permission_category(&self) -> &'static str {
        "web"
    }

    /// # Errors
    ///
    /// Returns an error if the `query` parameter is missing or empty. Search
    /// backend failures (rate limits, outages) are reported in `engine_blocked`
    /// within the metadata, not as `Err`.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let query = input["query"].as_str().unwrap_or("");

        if query.trim().is_empty() {
            anyhow::bail!("Search query must not be empty");
        }

        // Build search options from input parameters.
        let max_results = input["max_results"].as_u64().unwrap_or(6) as usize;
        let mut opts = SearchOptions::new(max_results);

        if let Some(site) = input["site"].as_str()
            && !site.is_empty()
        {
            opts = opts.with_site(site);
        }

        if let Some(exclude) = input["exclude_sites"].as_array() {
            let sites: Vec<String> = exclude
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .filter(|s| !s.is_empty())
                .collect();
            if !sites.is_empty() {
                opts = opts.with_exclude_sites(sites);
            }
        }

        if let Some(freshness) = input["freshness"].as_str()
            && let Ok(f) = freshness.parse::<Freshness>()
        {
            opts = opts.with_freshness(f);
        }

        if let Some(page) = input["page"].as_u64() {
            opts = opts.with_page(page as usize);
        }

        // Build the orchestrator from the tool context and run the search.
        let orchestrator = MfSearchTool::build_orchestrator(ctx);
        let output = orchestrator.search(query, &opts).await;

        // Format the text report.
        let content = format_search_report(&output.query, &output.merge, output.cached);

        // Build structured metadata (FR-009).
        let metadata = build_search_metadata(&output);

        Ok(ToolOutput { content, metadata })
    }
}

// ---------------------------------------------------------------------------
// Report formatting
// ---------------------------------------------------------------------------

/// Format the search results as a human-readable text report.
///
/// The report includes:
/// - The query and result count.
/// - Each ranked result with position, title, URL, snippet, relevance score,
///   fetch tier, and consensus.
/// - Related queries (if any).
/// - Blocked engines (if any).
fn format_search_report(query: &str, merge: &MergeOutput, cached: bool) -> String {
    let mut out = String::new();

    out.push_str(&format!("mf_search: \"{query}\"\n"));
    out.push_str(&format!(
        "Results: {} | Engines: {} ({} with results) | Cached: {}\n",
        merge.total_merged_results, merge.total_engines, merge.engines_with_results, cached,
    ));

    if !merge.blocked_engines.is_empty() {
        out.push_str(&format!(
            "Blocked engines: {}\n",
            merge.blocked_engines.join(", ")
        ));
    }

    out.push('\n');

    if merge.results.is_empty() {
        out.push_str("No results found.\n");
        if !merge.blocked_engines.is_empty() {
            out.push_str(
                "All search backends were blocked or returned no results. \
                 Try again later or refine your query.\n",
            );
        }
        return out;
    }

    for result in &merge.results {
        out.push_str(&format!(
            "{}. [{}] {}\n",
            result.position, result.fetch_relevance, result.title
        ));
        out.push_str(&format!("   URL: {}\n", result.url));
        if !result.snippet.is_empty() {
            out.push_str(&format!("   {}\n", result.snippet));
        }
        out.push_str(&format!(
            "   Relevance: {:.2} | Consensus: {} | Source: {}\n",
            result.relevance_score, result.engines_consensus, result.source
        ));
        out.push_str(&format!("   Hint: {}\n", result.fetch_hint));
        out.push('\n');
    }

    if !merge.related_queries.is_empty() {
        out.push_str("Related queries:\n");
        for rq in &merge.related_queries {
            out.push_str(&format!("  - {rq}\n"));
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Metadata construction (FR-009)
// ---------------------------------------------------------------------------

/// Build the structured metadata for the search tool output.
///
/// Populates all FR-009 signal fields: `query`, `results` (array of
/// `{title, url, snippet, source, position, relevance_score,
/// fetch_relevance, engines_consensus}`), `total_results`, `engines_used`,
/// `engine_blocked`, `cached`, `duration_ms`, `related_queries`, `error`.
fn build_search_metadata(output: &super::super::search::SearchOutput) -> Option<Value> {
    let results: Vec<Value> = output
        .merge
        .results
        .iter()
        .map(|r| {
            json!({
                "title": r.title,
                "url": r.url,
                "snippet": r.snippet,
                                    "source": r.source,
                                    "search_tool": "mf_search",
                                    "search_engine": r.source,
                                    "position": r.position,
                  "relevance_score": r.relevance_score,
                "fetch_relevance": r.fetch_relevance,
                "engines_consensus": r.engines_consensus,
                "fetch_hint": r.fetch_hint,
            })
        })
        .collect();

    Some(json!({
        "query": output.query,
        "results": results,
        "total_results": output.merge.total_merged_results,
        "total_raw_results": output.merge.total_raw_results,
        "engines_used": output.engines_used,
        "engine_blocked": output.merge.blocked_engines,
        "engines_with_results": output.merge.engines_with_results,
        "total_engines": output.merge.total_engines,
        "cached": output.cached,
        "duration_ms": output.duration_ms,
        "related_queries": output.merge.related_queries,
        "max_results": output.options.max_results,
        "site": output.options.site,
        "exclude_sites": output.options.exclude_sites,
        "freshness": output.options.freshness.to_string(),
        "page": output.options.page,
        "error": "",
        "version": MASTERFETCH_VERSION,
    }))
}
