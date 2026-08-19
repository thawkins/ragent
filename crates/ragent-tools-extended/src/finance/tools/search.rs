//! `stock_search` tool — find tickers by company or asset name.

use crate::finance::default_provider;
use crate::{Tool, ToolContext, ToolOutput};
use serde_json::{Value, json};

/// Input for `stock_search`.
#[derive(Debug, serde::Deserialize)]
struct StockSearchInput {
    query: String,
}

/// Tool that searches for ticker symbols.
pub struct StockSearchTool;

#[async_trait::async_trait]
impl Tool for StockSearchTool {
    fn name(&self) -> &str {
        "stock_search"
    }

    fn description(&self) -> &str {
        "Search for ticker symbols matching a company or asset name."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Company or asset name to search for" }
            },
            "required": ["query"]
        })
    }

    fn permission_category(&self) -> &str {
        "network:fetch"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let req: StockSearchInput = serde_json::from_value(input)?;
        let provider = default_provider(ctx.config.as_ref().map(|c| &c.finance));
        let results = provider.search(&req.query).await?;

        Ok(ToolOutput {
            content: serde_json::to_string_pretty(&results)?,
            metadata: Some(json!({ "provider": provider.name(), "count": results.len() })),
        })
    }
}
