//! Finance module for stocks and currency tools (yfinance).
//!
//! This module provides provider-agnostic data types, a provider trait,
//! caching, rate limiting, and concrete adapters for free Yahoo Finance
//! (via `yfinance_rs`) and optional paid providers.

pub mod cache;
pub mod error;
pub mod model;
pub mod provider;
pub mod providers;
pub mod rate_limit;
pub mod throttle;
pub mod tools;

pub use cache::{DEFAULT_QUOTE_TTL, QuoteCache};
pub use error::{FinanceError, FinanceResult};
pub use model::{
    CurrencyRate, Fundamentals, OhlcvBar, OptionContract, OptionKind, Quote, RecommendationPeriod,
    SearchResult,
};
pub use provider::FinanceProvider;
pub use providers::{
    PaidProvider, TwelveDataProvider, YahooFinanceProvider, default_provider,
    paid_provider_from_config, yahoo_fallback_provider,
};
pub use rate_limit::{MAX_BACKOFF_SECONDS, RateLimiter};
pub use throttle::wait_for_min_interval;
