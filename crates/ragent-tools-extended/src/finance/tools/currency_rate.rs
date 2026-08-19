//! `currency_rate` tool — current exchange rate for a currency pair.

use crate::finance::default_provider;
use crate::{Tool, ToolContext, ToolOutput};
use serde_json::{Value, json};

/// Input for `currency_rate`.
#[derive(Debug, serde::Deserialize)]
struct CurrencyRateInput {
    base: String,
    quote: String,
}

/// Tool that fetches a current FX rate.
pub struct CurrencyRateTool;

#[async_trait::async_trait]
impl Tool for CurrencyRateTool {
    fn name(&self) -> &str {
        "currency_rate"
    }

    fn description(&self) -> &str {
        "Fetch the current exchange rate between two currencies."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "base": { "type": "string", "description": "Source currency code, e.g. USD" },
                "quote": { "type": "string", "description": "Target currency code, e.g. EUR" }
            },
            "required": ["base", "quote"]
        })
    }

    fn permission_category(&self) -> &str {
        "network:fetch"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let req: CurrencyRateInput = serde_json::from_value(input)?;
        let provider = default_provider(ctx.config.as_ref().map(|c| &c.finance));
        let rate = provider
            .currency_rate(
                &req.base.to_ascii_uppercase(),
                &req.quote.to_ascii_uppercase(),
            )
            .await?;

        Ok(ToolOutput {
            content: serde_json::to_string_pretty(&rate)?,
            metadata: Some(json!({ "provider": provider.name() })),
        })
    }
}
