//! `stock_fundamentals` tool — key metrics for a company.

use crate::finance::default_provider;
use crate::{Tool, ToolContext, ToolOutput};
use serde_json::{Value, json};

/// Input for `stock_fundamentals`.
#[derive(Debug, serde::Deserialize)]
struct StockFundamentalsInput {
    symbol: String,
}

/// Tool that fetches fundamental metrics.
pub struct StockFundamentalsTool;

#[async_trait::async_trait]
impl Tool for StockFundamentalsTool {
    fn name(&self) -> &str {
        "stock_fundamentals"
    }

    fn description(&self) -> &str {
        "Fetch fundamental data for a ticker (market cap, P/E, EPS, dividend yield, 52-week range, sector)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "symbol": { "type": "string", "description": "Ticker symbol" }
            },
            "required": ["symbol"]
        })
    }

    fn permission_category(&self) -> &str {
        "network:fetch"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let req: StockFundamentalsInput = serde_json::from_value(input)?;
        let provider = default_provider(ctx.config.as_ref().map(|c| &c.finance));
        let fundamentals = provider
            .fundamentals(&req.symbol.to_ascii_uppercase())
            .await?;

        Ok(ToolOutput {
            content: serde_json::to_string_pretty(&fundamentals)?,
            metadata: Some(json!({ "provider": provider.name() })),
        })
    }
}
