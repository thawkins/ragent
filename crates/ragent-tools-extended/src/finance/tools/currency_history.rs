//! `currency_history` tool — historical exchange-rate bars.

use crate::finance::default_provider;
use crate::{Tool, ToolContext, ToolOutput};
use serde_json::{Value, json};

/// Input for `currency_history`.
#[derive(Debug, serde::Deserialize)]
struct CurrencyHistoryInput {
    base: String,
    quote: String,
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

/// Tool that fetches historical FX bars.
pub struct CurrencyHistoryTool;

#[async_trait::async_trait]
impl Tool for CurrencyHistoryTool {
    fn name(&self) -> &'static str {
        "currency_history"
    }

    fn description(&self) -> &'static str {
        "Fetch historical exchange-rate OHLCV bars for a currency pair."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "base": { "type": "string", "description": "Source currency code" },
                "quote": { "type": "string", "description": "Target currency code" },
                "interval": { "type": "string", "description": "Candle interval: 1d, 1wk, 1mo", "default": "1d" },
                "period": { "type": "string", "description": "Lookback period", "default": "1mo" }
            },
            "required": ["base", "quote"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "network:fetch"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let req: CurrencyHistoryInput = serde_json::from_value(input)?;
        let provider = default_provider(ctx.config.as_ref().map(|c| &c.finance));
        let bars = provider
            .currency_history(
                &req.base.to_ascii_uppercase(),
                &req.quote.to_ascii_uppercase(),
                &req.interval,
                &req.period,
            )
            .await?;

        Ok(ToolOutput {
            content: serde_json::to_string_pretty(&bars)?,
            metadata: Some(json!({ "provider": provider.name(), "count": bars.len() })),
        })
    }
}
