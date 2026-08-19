//! Provider-agnostic data model for the finance toolset.
//!
//! These types are intentionally independent of any particular provider so
//! that the free Yahoo adapter and paid adapters can share the same output
//! schema.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// A single market quote for a ticker symbol.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Quote {
    pub symbol: String,
    pub price: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
    pub change: f64,
    pub change_percent: f64,
    pub currency: String,
    pub market_state: String,
    pub timestamp: DateTime<Utc>,
}

/// One open/high/low/close/volume bar for a chart or history series.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct OhlcvBar {
    pub timestamp: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

/// Fundamental metrics for a company.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Fundamentals {
    pub symbol: String,
    pub name: Option<String>,
    pub sector: Option<String>,
    pub market_cap: Option<u64>,
    pub trailing_pe: Option<f64>,
    pub forward_pe: Option<f64>,
    pub eps: Option<f64>,
    pub dividend_yield: Option<f64>,
    pub fifty_two_week_high: Option<f64>,
    pub fifty_two_week_low: Option<f64>,
}

/// A current or snapshot exchange rate between two currencies.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CurrencyRate {
    pub base: String,
    pub quote: String,
    pub rate: f64,
    pub timestamp: DateTime<Utc>,
}

/// Classification of an option contract.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum OptionKind {
    #[default]
    Call,
    Put,
}

/// A single option contract in an options chain.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct OptionContract {
    pub strike: f64,
    pub expiration: NaiveDate,
    pub kind: OptionKind,
    pub last_price: f64,
    pub bid: f64,
    pub ask: f64,
    pub volume: u64,
    pub open_interest: u64,
    pub implied_volatility: Option<f64>,
}

/// A search result mapping a company or asset name to a ticker symbol.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SearchResult {
    pub symbol: String,
    pub name: Option<String>,
    pub exchange: Option<String>,
    pub asset_class: Option<String>,
}

/// Analyst recommendation counts for a single reporting period.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RecommendationPeriod {
    /// Reporting period label, e.g. "0m" for the current month or "-1m".
    pub period: String,
    /// Count of "strong buy" recommendations.
    pub strong_buy: u32,
    /// Count of "buy" recommendations.
    pub buy: u32,
    /// Count of "hold" recommendations.
    pub hold: u32,
    /// Count of "sell" recommendations.
    pub sell: u32,
    /// Count of "strong sell" recommendations.
    pub strong_sell: u32,
}
