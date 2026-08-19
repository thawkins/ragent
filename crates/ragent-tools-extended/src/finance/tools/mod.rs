//! Finance tool implementations.

pub mod currency_history;
pub mod currency_rate;
pub mod fundamentals;
pub mod history;
pub mod options;
pub mod quote;
pub mod recommendations;
pub mod search;

use crate::event::Event;
use crate::finance::{FinanceError, default_provider, yahoo_fallback_provider};
use std::sync::Arc;

/// Emit a log-window notice naming the finance provider selected for a tool.
///
/// The TUI subscribes to [`Event::AgentNotice`] and surfaces it in the log
/// panel, so users can see which provider actually served a `stock_*` call.
pub fn log_provider_choice(ctx: &crate::ToolContext, tool_name: &str, provider_name: &str) {
    ctx.event_bus.publish(Event::AgentNotice {
        session_id: ctx.session_id.clone(),
        message: format!("{tool_name}: using finance provider '{provider_name}'"),
    });
}

/// Execute a finance call, falling back to the free Yahoo provider only when
/// explicitly enabled and the configured paid provider reports that the
/// operation is not implemented, does not exist, or is not available on the
/// current API plan (e.g., TwelveData `/statistics` requires a paid plan).
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
    let provider_name = provider.name().to_string();
    match f.clone()(provider.clone()).await {
        Ok(v) => Ok(v),
        Err(FinanceError::ProviderFailure { message, .. }) => {
            let lower = message.to_ascii_lowercase();
            if lower.contains("not implemented")
                || lower.contains("does not exist")
                || lower.contains("available exclusively with")
                || lower.contains("upgrade your api key")
                || lower.contains("not available on your plan")
            {
                if cfg.map(|c| c.yahoo_fallback_enabled()).unwrap_or(false) {
                    ctx.event_bus.publish(crate::event::Event::AgentNotice {
                        session_id: ctx.session_id.clone(),
                        message: format!(
                            "finance: falling back to yahoo after paid provider failure: {}",
                            message
                        ),
                    });
                    let yahoo = yahoo_fallback_provider(cfg);
                    f(yahoo).await
                } else {
                    ctx.event_bus.publish(crate::event::Event::AgentNotice {
                        session_id: ctx.session_id.clone(),
                        message: format!(
                            "finance: paid provider failure and yahoo fallback is disabled: {}",
                            message
                        ),
                    });
                    Err(FinanceError::ProviderFailure {
                        provider: provider_name,
                        message,
                    })
                }
            } else {
                Err(FinanceError::ProviderFailure {
                    provider: provider_name,
                    message,
                })
            }
        }
        Err(e) => Err(e),
    }
}
