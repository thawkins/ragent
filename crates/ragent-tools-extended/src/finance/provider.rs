//! Provider trait for the finance toolset.
//!
//! All concrete adapters (free Yahoo, Alpha Vantage, etc.) implement this
//! trait so tools can operate on a normalized, provider-agnostic interface.

use crate::finance::{
    CurrencyRate, FinanceResult, Fundamentals, OhlcvBar, OptionContract, Quote,
    RecommendationPeriod, SearchResult,
};

/// Abstraction over any stocks/currency data provider.
#[async_trait::async_trait]
pub trait FinanceProvider: Send + Sync + std::fmt::Debug {
    /// Provider name, used for logging and cache keys.
    fn name(&self) -> &str;

    /// Whether this provider is configured and able to make requests.
    fn is_available(&self) -> bool;

    /// Latest quote for a single ticker.
    async fn quote(&self, symbol: &str) -> FinanceResult<Quote>;

    /// Historical OHLCV bars for a single ticker.
    async fn history(
        &self,
        symbol: &str,
        interval: &str,
        period: &str,
    ) -> FinanceResult<Vec<OhlcvBar>>;

    /// Fundamental metrics for a single ticker.
    async fn fundamentals(&self, symbol: &str) -> FinanceResult<Fundamentals>;

    /// Current exchange rate between two currencies.
    async fn currency_rate(&self, base: &str, quote: &str) -> FinanceResult<CurrencyRate>;

    /// Historical exchange-rate bars for a currency pair.
    async fn currency_history(
        &self,
        base: &str,
        quote: &str,
        interval: &str,
        period: &str,
    ) -> FinanceResult<Vec<OhlcvBar>>;

    /// Search for symbols matching a company or asset name fragment.
    async fn search(&self, query: &str) -> FinanceResult<Vec<SearchResult>>;

    /// Options chain for a single ticker, optionally filtered by expiration.
    async fn options(
        &self,
        symbol: &str,
        expiration: Option<&str>,
    ) -> FinanceResult<Vec<OptionContract>>;

    /// Analyst recommendation trend counts for a single ticker.
    async fn recommendations(&self, symbol: &str) -> FinanceResult<Vec<RecommendationPeriod>>;
}
