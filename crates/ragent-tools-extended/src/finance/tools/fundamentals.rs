//! `stock_fundamentals` tool — key metrics for a company.

use crate::finance::tools::with_yahoo_fallback;
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
    fn name(&self) -> &'static str {
        "stock_fundamentals"
    }

    fn description(&self) -> &'static str {
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

    fn permission_category(&self) -> &'static str {
        "network:fetch"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let req: StockFundamentalsInput = serde_json::from_value(input)?;
        let symbol = req.symbol.to_ascii_uppercase();
        let provider_name =
            crate::finance::default_provider(ctx.config.as_ref().map(|c| &c.finance))
                .name()
                .to_string();

        crate::finance::tools::log_provider_choice(ctx, self.name(), &provider_name);

        let fundamentals = with_yahoo_fallback(ctx, |p| {
            let symbol = symbol.clone();
            async move { p.fundamentals(&symbol).await }
        })
        .await?;

        Ok(ToolOutput {
            content: serde_json::to_string_pretty(&fundamentals)?,
            metadata: Some(json!({ "provider": provider_name })),
        })
    }
}
