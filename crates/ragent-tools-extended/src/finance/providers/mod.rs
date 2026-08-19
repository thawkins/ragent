//! Finance provider adapters.

pub mod paid;
pub mod yahoo;

pub use paid::{
    PaidProvider, default_provider, paid_provider_from_config, yahoo_fallback_provider,
};
pub use yahoo::YahooFinanceProvider;
