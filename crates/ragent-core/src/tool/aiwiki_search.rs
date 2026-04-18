//! AIWiki search tool for agents.
//!
//! Provides [`AiwikiSearchTool`], which searches the AIWiki knowledge base
//! for pages matching a query. Returns results with titles, paths, and excerpts.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Search the AIWiki knowledge base for pages matching a query.
pub struct AiwikiSearchTool;

/// Build a "not available" response when AIWiki is not initialized or disabled.
fn not_available() -> ToolOutput {
    ToolOutput {
        content: "AIWiki is not available. It may be disabled or not yet initialized. \
                  Run `/aiwiki init` to initialize AIWiki, or `/aiwiki on` to enable it."
            .to_string(),
        metadata: Some(json!({
            "error": "aiwiki_not_available",
            "enabled": false
        })),
    }
}

#[async_trait::async_trait]
impl Tool for AiwikiSearchTool {
    fn name(&self) -> &'static str {
        "aiwiki_search"
    }

    fn description(&self) -> &'static str {
        "Search the AIWiki knowledge base for pages matching a query. \
         Returns matching pages with titles, paths, and excerpts. \
         Use this to find relevant information from ingested documents. \
         The AIWiki must be initialized with `/aiwiki init` first."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query — keywords or phrase to find in the wiki"
                },
                "page_type": {
                    "type": "string",
                    "description": "Filter by page type: entities, concepts, sources, analyses",
                    "enum": ["entities", "concepts", "sources", "analyses"]
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum results to return (default: 10, max: 50)",
                    "minimum": 1,
                    "maximum": 50
                }
            },
            "required": ["query"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "aiwiki:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // Check if AIWiki exists and is enabled
        if !aiwiki::Aiwiki::exists(&ctx.working_dir) {
            return Ok(not_available());
        }

        let wiki = match aiwiki::Aiwiki::new(&ctx.working_dir).await {
            Ok(w) => w,
            Err(_) => return Ok(not_available()),
        };

        if !wiki.config.enabled {
            return Ok(not_available());
        }

        let query_str = input["query"]
            .as_str()
            .context("Missing required 'query' parameter")?;

        let page_type_filter = input["page_type"].as_str().map(String::from);
        let max_results = input["max_results"]
            .as_u64()
            .map(|n| n.min(50) as usize)
            .unwrap_or(10);

        // Perform the search
        let results = aiwiki::search_wiki(&wiki, query_str, page_type_filter).await?;

        if results.is_empty() {
            return Ok(ToolOutput {
                content: format!("No results found for query: '{}'", query_str),
                metadata: Some(json!({
                    "query": query_str,
                    "results_count": 0,
                    "enabled": true
                })),
            });
        }

        // Build output
        let mut output = format!("## AIWiki Search Results: '{}'\n\n", query_str);
        let total_results = results.len();
        let limited_results: Vec<_> = results.into_iter().take(max_results).collect();

        for (idx, result) in limited_results.iter().enumerate() {
            output.push_str(&format!(
                "{}. **{}**\n   Path: `{}`\n   {}\n\n",
                idx + 1,
                result.title,
                result.path,
                result.excerpt
            ));
        }

        if limited_results.len() < total_results {
            output.push_str(&format!(
                "_... and {} more results_\n",
                total_results - limited_results.len()
            ));
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "query": query_str,
                "results_count": limited_results.len(),
                "total_matches": total_results,
                "enabled": true
            })),
        })
    }
}
