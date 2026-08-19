//! `stock_quote` tool — latest price and session data for a ticker.

use crate::finance::{QuoteCache, default_provider, yahoo_fallback_provider};
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
        let provider_name = provider.name().to_string();

        crate::finance::tools::log_provider_choice(ctx, self.name(), &provider_name);

        if let Some(cached) = self.cache.get(&provider_name, &symbol) {
            return Ok(ToolOutput {
                content: serde_json::to_string_pretty(&cached)?,
                metadata: Some(json!({ "provider": provider_name, "cached": true })),
            });
        }

        let quote = match provider.quote(&symbol).await {
            Ok(q) => q,
            Err(crate::finance::FinanceError::ProviderFailure { message, .. })
                if message.to_ascii_lowercase().contains("symbol")
                    && message.to_ascii_lowercase().contains("missing or invalid") =>
            {
                let cfg = ctx.config.as_ref().map(|c| &c.finance);
                if cfg.map(|c| c.yahoo_fallback_enabled()).unwrap_or(false) {
                    ctx.event_bus.publish(crate::event::Event::AgentNotice {
                        session_id: ctx.session_id.clone(),
                        message: format!(
                            "stock_quote: falling back to yahoo after symbol failure for {}",
                            symbol
                        ),
                    });
                    let yahoo = yahoo_fallback_provider(cfg);
                    yahoo.quote(&symbol).await?
                } else {
                    ctx.event_bus.publish(crate::event::Event::AgentNotice {
                        session_id: ctx.session_id.clone(),
                        message: format!(
                            "stock_quote: paid provider symbol failure and yahoo fallback is disabled for {}",
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

        self.cache.set(&provider_name, &symbol, quote.clone());

        Ok(ToolOutput {
            content: serde_json::to_string_pretty(&quote)?,
            metadata: Some(json!({ "provider": provider_name, "cached": false })),
        })
    }
}
