//! Web search tool for querying search engines.
//!
//! Provides [`WebSearchTool`], which performs web searches using external search
//! APIs and returns structured results with titles, URLs, and snippets.
//! Currently supports the [Tavily](https://tavily.com/) search API.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Performs a web search and returns structured results.
pub struct WebSearchTool;

const TAVILY_API_URL: &str = "https://api.tavily.com/search";
const DEFAULT_NUM_RESULTS: u64 = 5;
const MAX_NUM_RESULTS: u64 = 20;
const REQUEST_TIMEOUT_SECS: u64 = 30;
const USER_AGENT: &str = "ragent/0.1 (https://github.com/thawkins/ragent)";

#[async_trait::async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "websearch"
    }

    fn description(&self) -> &str {
        "Search the web and return results with titles, URLs, and snippets. \
         Requires a TAVILY_API_KEY environment variable to be set."
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
            "required": ["query"]
        })
    }

    fn permission_category(&self) -> &str {
        "web"
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let query = input["query"]
            .as_str()
            .context("Missing 'query' parameter")?;

        if query.trim().is_empty() {
            bail!("Search query must not be empty");
        }

        let num_results = input["num_results"]
            .as_u64()
            .unwrap_or(DEFAULT_NUM_RESULTS)
            .min(MAX_NUM_RESULTS);

        let api_key = std::env::var("TAVILY_API_KEY").map_err(|_| {
            anyhow::anyhow!(
                "No search API key configured. Set the TAVILY_API_KEY environment \
                 variable to enable web search. Get a free key at https://tavily.com"
            )
        })?;

        let results = tavily_search(&api_key, query, num_results).await?;

        // Format results as readable text
        let mut output = String::new();
        for (i, result) in results.iter().enumerate() {
            if i > 0 {
                output.push('\n');
            }
            output.push_str(&format!("{}. {}\n", i + 1, result.title));
            output.push_str(&format!("   {}\n", result.url));
            if !result.snippet.is_empty() {
                output.push_str(&format!("   {}\n", result.snippet));
            }
        }

        if results.is_empty() {
            output.push_str("No results found.");
        }

        let lines = output.lines().count();
        let result_count = results.len();

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "query": query,
                "results": result_count,
                "lines": lines,
            })),
        })
    }
}

// ── Tavily API ───────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct TavilyRequest<'a> {
    query: &'a str,
    max_results: u64,
    include_answer: bool,
}

#[derive(Debug, Deserialize)]
struct TavilyResponse {
    results: Vec<TavilyResult>,
}

#[derive(Debug, Deserialize)]
struct TavilyResult {
    title: String,
    url: String,
    content: String,
}

/// A single search result.
#[derive(Debug, Clone)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

async fn tavily_search(
    api_key: &str,
    query: &str,
    max_results: u64,
) -> Result<Vec<SearchResult>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(USER_AGENT)
        .build()
        .context("Failed to build HTTP client")?;

    let request_body = TavilyRequest {
        query,
        max_results,
        include_answer: false,
    };

    let response = client
        .post(TAVILY_API_URL)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await
        .with_context(|| format!("Failed to call Tavily search API for: {}", query))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        if status.as_u16() == 401 || status.as_u16() == 403 {
            bail!(
                "Tavily API authentication failed (HTTP {}). Check your TAVILY_API_KEY.",
                status
            );
        }
        bail!("Tavily API error (HTTP {}): {}", status, body);
    }

    let tavily_response: TavilyResponse = response
        .json()
        .await
        .context("Failed to parse Tavily API response")?;

    let results = tavily_response
        .results
        .into_iter()
        .map(|r| {
            // Truncate snippet to ~200 chars
            let snippet = if r.content.len() > 200 {
                let end = r
                    .content
                    .char_indices()
                    .map(|(i, _)| i)
                    .take_while(|&i| i <= 200)
                    .last()
                    .unwrap_or(0);
                format!("{}…", &r.content[..end])
            } else {
                r.content
            };
            SearchResult {
                title: r.title,
                url: r.url,
                snippet,
            }
        })
        .collect();

    Ok(results)
}
