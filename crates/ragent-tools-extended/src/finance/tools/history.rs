//! `stock_history` tool — historical OHLCV bars for a ticker.

use crate::finance::default_provider;
use crate::{Tool, ToolContext, ToolOutput};
use serde_json::{Value, json};

/// Input for `stock_history`.
#[derive(Debug, serde::Deserialize)]
struct StockHistoryInput {
    symbol: String,
    #[serde(default = "default_interval")]
    interval: String,
    #[serde(default = "default_period")]
    period: String,
}

fn default_interval() -> String {
    "1d".to_string()
}

fn default_period() -> String {
    "1mo".to_string()
}

/// Tool that fetches historical stock prices.
pub struct StockHistoryTool;

#[async_trait::async_trait]
impl Tool for StockHistoryTool {
    fn name(&self) -> &str {
        "stock_history"
    }

    fn description(&self) -> &str {
        "Fetch historical OHLCV bars for a ticker symbol."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "symbol": { "type": "string", "description": "Ticker symbol" },
                "interval": { "type": "string", "description": "Candle interval: 1d, 1wk, 1mo", "default": "1d" },                  "period": { "type": "string", "description": "Lookback period: 1d, 5d, 1w, 1wk, 1mo, 3mo, 6mo, 1y, 5y, max. Short periods round up to the smallest supported Yahoo range (1 month).", "default": "1mo" }
            },
            "required": ["symbol"]
        })
    }

    fn permission_category(&self) -> &str {
        "network:fetch"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let req: StockHistoryInput = serde_json::from_value(input)?;
        let provider = default_provider(ctx.config.as_ref().map(|c| &c.finance));
        let bars = provider
            .history(&req.symbol.to_ascii_uppercase(), &req.interval, &req.period)
            .await?;

        Ok(ToolOutput {
            content: serde_json::to_string_pretty(&bars)?,
            metadata: Some(json!({ "provider": provider.name(), "count": bars.len() })),
        })
    }
}
