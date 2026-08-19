//! `stock_recommendations` tool — analyst recommendation trends for a ticker.

use crate::finance::tools::with_yahoo_fallback;
use crate::{Tool, ToolContext, ToolOutput};
use serde_json::{Value, json};

/// Input for `stock_recommendations`.
#[derive(Debug, serde::Deserialize)]
struct StockRecommendationsInput {
    symbol: String,
}

/// Tool that fetches analyst recommendation trends.
///
/// Returns counts of strong buy, buy, hold, sell, and strong sell ratings
/// over recent reporting periods for the requested ticker.
pub struct StockRecommendationsTool;

#[async_trait::async_trait]
impl Tool for StockRecommendationsTool {
    fn name(&self) -> &str {
        "stock_recommendations"
    }

    fn description(&self) -> &str {
        "Fetch analyst recommendation trends for a ticker symbol (strong buy, buy, hold, sell, strong sell counts by period)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "symbol": { "type": "string", "description": "Ticker symbol to look up, e.g. AAPL" }
            },
            "required": ["symbol"]
        })
    }

    fn permission_category(&self) -> &str {
        "network:fetch"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let req: StockRecommendationsInput = serde_json::from_value(input)?;
        let symbol = req.symbol.to_ascii_uppercase();
        let provider_name =
            crate::finance::default_provider(ctx.config.as_ref().map(|c| &c.finance))
                .name()
                .to_string();

        crate::finance::tools::log_provider_choice(ctx, self.name(), &provider_name);

        let periods = with_yahoo_fallback(ctx, |p| {
            let symbol = symbol.clone();
            async move { p.recommendations(&symbol).await }
        })
        .await?;

        Ok(ToolOutput {
            content: serde_json::to_string_pretty(&periods)?,
            metadata: Some(json!({ "provider": provider_name, "count": periods.len() })),
        })
    }
}
