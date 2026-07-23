//! `mf_search` tool — keyless multi-engine web search with consensus ranking.
//!
//! Implements FR-008 through FR-010, FR-022, FR-023, FR-025, FR-026.
//!
//! Queries multiple public search-engine backends in parallel (DuckDuckGo,
//! Brave, and optionally more), merges and deduplicates results by normalised
//! URL, ranks by relevance with cross-engine consensus boosting, and returns a
//! formatted result list with structured signals.
//!
//! # Pipeline
//!
//! 1. Validate the query (non-empty after trim).
//! 2. Build [`SearchOptions`] from the tool's input parameters (site,
//!    exclude_sites, freshness, max_results, page).
//! 3. Run [`SearchOrchestrator::search`] to query all backends in parallel,
//!    merge via consensus, and cache for 5 minutes.
//! 4. Format the ranked results as a human-readable text report.
//! 5. Populate structured metadata with FR-009 signals: `relevance_score`,
//!    `fetch_relevance`, `engines_consensus`, `related_queries`,
//!    `fetch_hint`, `engine_blocked`, `cached`, `duration_ms`.
//!
//! # No API keys
//!
//! This tool is **keyless** (FR-023): it scrapes public search-engine HTML
//! result pages. No API keys, tokens, or accounts are required.

use anyhow::Result;
use serde_json::{Value, json};

use super::super::MASTERFETCH_VERSION;
use super::super::search::consensus::MergeOutput;
use super::super::search::{Freshness, SearchOptions, SearchOrchestrator};

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

#[async_trait::async_trait]
impl Tool for MfSearchTool {
    fn name(&self) -> &'static str {
        "mf_search"
    }

    fn description(&self) -> &'static str {
        "Local keyless web search. Multiple backends in parallel (DuckDuckGo, \
         Brave, and more), merges + ranks with cross-engine consensus. No API \
         keys required. Each result carries relevance_score, fetch_relevance \
         (high/med/low), and engines_consensus. Supports site, exclude_sites, \
         freshness, max_results, and page filters."
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
            "required": ["query"]
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
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
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

        // Run the search orchestrator (parallel backends + consensus merge + cache).
        let orchestrator = SearchOrchestrator::new();
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
