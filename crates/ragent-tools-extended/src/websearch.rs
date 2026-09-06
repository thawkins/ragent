//! Legacy web search tool — compatibility wrapper around `mf_search`.
//!
//! [`WebSearchTool`] is retained for direct agent use and backwards
//! compatibility. It delegates to the multi-engine `mf_search` pipeline
//! (via [`MfSearchTool::build_orchestrator`]) so that all configured
//! backends — OpenAlex, Wikipedia, LangSearch, and Tavily — contribute
//! results. The tool preserves its original name (`websearch`), parameter
//! schema (`query`, `num_results`), and human-readable output format.
//!
//! New research workflows use `mf_search` directly; the research adapter
//! selects `mf_search` when present and falls back to this tool otherwise.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::masterfetch::search::SearchOptions;
use crate::masterfetch::tools::search_tool::MfSearchTool;

/// Performs a web search and returns structured results.
///
/// Delegates to the `mf_search` multi-engine pipeline so results include
/// OpenAlex, Wikipedia, and optionally LangSearch / Tavily when their API
/// keys are configured.
pub struct WebSearchTool;

const DEFAULT_NUM_RESULTS: u64 = 5;
const MAX_NUM_RESULTS: u64 = 20;

#[async_trait::async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &'static str {
        "websearch"
    }

    fn description(&self) -> &'static str {
        "Search the web and return results with titles, URLs, and snippets. \
         Required parameter: 'query'. Optional 'num_results' (default 5, max 20). \
         By default uses the keyless mf_search pipeline (OpenAlex, Wikipedia); \
         optional Tavily/LangSearch API keys improve quality but are not required."
    }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                },
                "num_results": {
                    "type": "integer",
                    "description": "Number of results to return (default: 5, max: 20)"
                }
            },
            "required": ["query"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "web"
    }

    /// Execute a web search by delegating to the `mf_search` multi-engine
    /// pipeline.
    ///
    /// # Errors
    ///
    /// Returns an error if the `query` parameter is missing or empty.
    /// Backend failures (rate limits, missing keys) are reported in
    /// `engine_blocked` within the metadata, not as `Err`.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let query = input["query"]
            .as_str()
            .context("Missing required 'query' parameter")?;

        if query.trim().is_empty() {
            bail!("Search query must not be empty");
        }

        let num_results = input["num_results"]
            .as_u64()
            .unwrap_or(DEFAULT_NUM_RESULTS)
            .min(MAX_NUM_RESULTS);

        // Build search options and run through the mf_search orchestrator.
        let opts = SearchOptions::new(num_results as usize);
        let orchestrator = MfSearchTool::build_orchestrator(ctx);
        let search_output = orchestrator.search(query, &opts).await;

        // Format results as human-readable text matching the legacy shape.
        let mut output = String::new();
        for (i, result) in search_output.merge.results.iter().enumerate() {
            if i > 0 {
                output.push('\n');
            }
            output.push_str(&format!("{}. {}\n", i + 1, result.title));
            output.push_str(&format!("   {}\n", result.url));
            if !result.snippet.is_empty() {
                output.push_str(&format!("   {}\n", result.snippet));
            }
        }

        if search_output.merge.results.is_empty() {
            if !search_output.merge.blocked_engines.is_empty() {
                output.push_str(&format!(
                    "No results found. Blocked engines: {}\n",
                    search_output.merge.blocked_engines.join(", ")
                ));
            } else {
                output.push_str("No results found.");
            }
        }

        let line_count = output.lines().count();
        let result_count = search_output.merge.results.len();

        // Build metadata with the legacy `results` array shape so existing
        // parsers (`hits_from_metadata`, research adapter) keep working.
        let results_json: Vec<Value> = search_output
            .merge
            .results
            .iter()
            .map(|r| {
                json!({
                    "title": r.title,
                    "url": r.url,
                    "snippet": r.snippet,
                    "search_tool": "websearch",
                    "search_engine": r.source,
                    "author": r.author,
                })
            })
            .collect();

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "query": query,
                "count": result_count,
                "line_count": line_count,
                "results": results_json,
                "engines_used": search_output.engines_used,
                "engine_blocked": search_output.merge.blocked_engines,
                "cached": search_output.cached,
                "duration_ms": search_output.duration_ms,
            })),
        })
    }
}

// ── Shared types ──────────────────────────────────────────────────

/// A single search result.
///
/// Deserialized from the `results` array in the JSON metadata emitted by
/// [`WebSearchTool`]. The research adapter uses [`hits_from_metadata`] to
/// extract these rows.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SearchResult {
    /// The title of the search result.
    pub title: String,
    /// The URL of the search result.
    pub url: String,
    /// A short snippet/summary from the search result.
    pub snippet: String,
    /// Search tool that produced this result. Always `"websearch"` for the
    /// compatibility wrapper, present so the research layer can show
    /// provenance without special-casing the tool.
    #[serde(default)]
    pub search_tool: String,
    /// Backend search engine(s) that returned this result (comma-separated
    /// when multiple engines agree).
    #[serde(default)]
    pub search_engine: String,
    /// Author name when a contributing search engine exposed one in its result
    /// payload. `None` when no engine provided author metadata.
    #[serde(default)]
    pub author: Option<String>,
}

/// Extract structured search results from the JSON metadata emitted by
/// [`WebSearchTool`].
///
/// The metadata is a JSON object with a `results` array, where each element
/// has `title`, `url`, and `snippet` fields. Returns an empty vector if the
/// metadata is missing the `results` key or if parsing fails.
#[must_use]
pub fn hits_from_metadata(metadata: &serde_json::Value) -> Vec<SearchResult> {
    metadata
        .get("results")
        .and_then(|r| serde_json::from_value::<Vec<SearchResult>>(r.clone()).ok())
        .unwrap_or_default()
}
