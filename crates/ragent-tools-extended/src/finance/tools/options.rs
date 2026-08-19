//! `stock_options` tool — options chain for a ticker.

use crate::finance::default_provider;
use crate::{Tool, ToolContext, ToolOutput};
use serde_json::{Value, json};

/// Input for `stock_options`.
#[derive(Debug, serde::Deserialize)]
struct StockOptionsInput {
    symbol: String,
    expiration: Option<String>,
}

/// Tool that fetches an options chain.
pub struct StockOptionsTool;

#[async_trait::async_trait]
impl Tool for StockOptionsTool {
    fn name(&self) -> &str {
        "stock_options"
    }

    fn description(&self) -> &str {
        "Fetch the options chain (calls and puts) for a ticker symbol."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "symbol": { "type": "string", "description": "Ticker symbol" },
                "expiration": { "type": "string", "description": "Optional expiration date YYYY-MM-DD" }
            },
            "required": ["symbol"]
        })
    }

    fn permission_category(&self) -> &str {
        "network:fetch"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let req: StockOptionsInput = serde_json::from_value(input)?;
        let provider = default_provider(ctx.config.as_ref().map(|c| &c.finance));
        let expiration = req.expiration.as_deref();
        let contracts = provider
            .options(&req.symbol.to_ascii_uppercase(), expiration)
            .await?;

        Ok(ToolOutput {
            content: serde_json::to_string_pretty(&contracts)?,
            metadata: Some(json!({ "provider": provider.name(), "count": contracts.len() })),
        })
    }
}
