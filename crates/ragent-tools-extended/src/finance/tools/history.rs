//! `stock_history` tool — historical OHLCV bars for a ticker.

use crate::finance::{default_provider, yahoo_fallback_provider};
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
        let symbol = req.symbol.to_ascii_uppercase();
        let provider = default_provider(ctx.config.as_ref().map(|c| &c.finance));
        let provider_name = provider.name().to_string();

        crate::finance::tools::log_provider_choice(ctx, self.name(), &provider_name);

        let bars = match provider.history(&symbol, &req.interval, &req.period).await {
            Ok(b) => b,
            Err(crate::finance::FinanceError::ProviderFailure { message, .. })
                if message.to_ascii_lowercase().contains("symbol")
                    && message.to_ascii_lowercase().contains("missing or invalid") =>
            {
                let cfg = ctx.config.as_ref().map(|c| &c.finance);
                if cfg.map(|c| c.yahoo_fallback_enabled()).unwrap_or(false) {
                    ctx.event_bus.publish(crate::event::Event::AgentNotice {
                        session_id: ctx.session_id.clone(),
                        message: format!(
                            "stock_history: falling back to yahoo after symbol failure for {}",
                            symbol
                        ),
                    });
                    let yahoo = yahoo_fallback_provider(cfg);
                    yahoo.history(&symbol, &req.interval, &req.period).await?
                } else {
                    ctx.event_bus.publish(crate::event::Event::AgentNotice {
                        session_id: ctx.session_id.clone(),
                        message: format!(
                            "stock_history: paid provider symbol failure and yahoo fallback is disabled for {}",
                            symbol
                        ),
                    });
                    return Err(crate::finance::FinanceError::ProviderFailure {
                        provider: provider_name,
                        message,
                    }
                    .into());
                }
            }
            Err(e) => return Err(e.into()),
        };

        Ok(ToolOutput {
            content: serde_json::to_string_pretty(&bars)?,
            metadata: Some(json!({ "provider": provider_name, "count": bars.len() })),
        })
    }
}
