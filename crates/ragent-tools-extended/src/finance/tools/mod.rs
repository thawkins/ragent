//! Finance tool implementations.

pub mod currency_history;
pub mod currency_rate;
pub mod fundamentals;
pub mod history;
pub mod options;
pub mod quote;
pub mod recommendations;
pub mod search;

use crate::finance::{FinanceError, Quote, default_provider, yahoo_fallback_provider};
use std::sync::Arc;

/// Build the provider for the current tool context.
#[allow(dead_code)]
fn finance_provider(ctx: &crate::ToolContext) -> Arc<dyn crate::finance::FinanceProvider> {
    let cfg = ctx.config.as_ref().map(|c| &c.finance);
    default_provider(cfg)
}

/// Convert a result to a JSON-pretty string for the tool output content.
#[allow(dead_code)]
fn to_json_content<T: serde::Serialize>(value: &T) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(value)?)
}

/// Execute a finance call, falling back to the free Yahoo provider when the
/// configured paid provider reports that the operation is not implemented or
/// does not exist (e.g., Alpha Vantage lacks recommendations, fundamentals,
/// search, options, and currency endpoints on the free tier).
async fn with_yahoo_fallback<T, F, Fut>(
    ctx: &crate::ToolContext,
    f: F,
) -> crate::finance::FinanceResult<T>
where
    F: Fn(Arc<dyn crate::finance::FinanceProvider>) -> Fut + Clone + Send,
    Fut: std::future::Future<Output = crate::finance::FinanceResult<T>> + Send,
    T: Send,
{
    let cfg = ctx.config.as_ref().map(|c| &c.finance);
    let provider = default_provider(cfg);
    match f.clone()(provider).await {
        Ok(v) => Ok(v),
        Err(FinanceError::ProviderFailure { message, .. })
            if message.to_ascii_lowercase().contains("not implemented")
                || message.to_ascii_lowercase().contains("does not exist") =>
        {
            let yahoo = yahoo_fallback_provider(cfg);
            f(yahoo).await
        }
        Err(e) => Err(e),
    }
}

#[allow(dead_code)]
const _: fn(&crate::ToolContext) -> Arc<dyn crate::finance::FinanceProvider> = finance_provider;

#[allow(dead_code)]
const _: fn(&Quote) -> anyhow::Result<String> = to_json_content::<Quote>;
