//! `stock_quote` tool — latest price and session data for a ticker.

use crate::finance::{QuoteCache, default_provider};
use crate::{Tool, ToolContext, ToolOutput};
use serde_json::{Value, json};
use std::sync::Arc;

/// Input for `stock_quote`.
#[derive(Debug, serde::Deserialize)]
struct StockQuoteInput {
    symbol: String,
}

/// Tool that fetches a current stock quote.
pub struct StockQuoteTool {
    cache: Arc<QuoteCache>,
}

impl StockQuoteTool {
    /// Create a tool with the default 60-second quote cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: Arc::new(QuoteCache::default_cache()),
        }
    }
}

impl Default for StockQuoteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for StockQuoteTool {
    fn name(&self) -> &str {
        "stock_quote"
    }

    fn description(&self) -> &str {
        "Fetch the latest stock quote for a ticker symbol (price, open, high, low, close, volume, change)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "Ticker symbol to look up, e.g. AAPL"
                }
            },
            "required": ["symbol"]
        })
    }

    fn permission_category(&self) -> &str {
        "network:fetch"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let req: StockQuoteInput = serde_json::from_value(input)?;
        let symbol = req.symbol.to_ascii_uppercase();
        let provider = default_provider(ctx.config.as_ref().map(|c| &c.finance));

        if let Some(cached) = self.cache.get(provider.name(), &symbol) {
            return Ok(ToolOutput {
                content: serde_json::to_string_pretty(&cached)?,
                metadata: Some(json!({ "provider": provider.name(), "cached": true })),
            });
        }

        let quote = provider.quote(&symbol).await?;
        self.cache.set(provider.name(), &symbol, quote.clone());

        Ok(ToolOutput {
            content: serde_json::to_string_pretty(&quote)?,
            metadata: Some(json!({ "provider": provider.name(), "cached": false })),
        })
    }
}
