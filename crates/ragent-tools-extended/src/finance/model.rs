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
    /// The ticker symbol.
    pub symbol: String,
    /// The latest traded price.
    pub price: f64,
    /// The opening price for the period.
    pub open: f64,
    /// The highest traded price for the period.
    pub high: f64,
    /// The lowest traded price for the period.
    pub low: f64,
    /// The closing price for the period.
    pub close: f64,
    /// The traded volume for the period.
    pub volume: u64,
    /// The absolute price change from the previous close.
    pub change: f64,
    /// The percentage price change from the previous close.
    pub change_percent: f64,
    /// The currency the quote is denominated in.
    pub currency: String,
    /// The market state (e.g. "REGULAR", "PRE", "POST", "CLOSED").
    pub market_state: String,
    /// When the quote was recorded.
    pub timestamp: DateTime<Utc>,
}

/// One open/high/low/close/volume bar for a chart or history series.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct OhlcvBar {
    /// The bar's time bucket.
    pub timestamp: DateTime<Utc>,
    /// The opening price of the bar.
    pub open: f64,
    /// The highest price of the bar.
    pub high: f64,
    /// The lowest price of the bar.
    pub low: f64,
    /// The closing price of the bar.
    pub close: f64,
    /// The traded volume of the bar.
    pub volume: u64,
}

/// Fundamental metrics for a company.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Fundamentals {
    /// The ticker symbol.
    pub symbol: String,
    /// The company name.
    pub name: Option<String>,
    /// The sector the company operates in.
    pub sector: Option<String>,
    /// The total market capitalisation.
    pub market_cap: Option<u64>,
    /// The trailing price-to-earnings ratio.
    pub trailing_pe: Option<f64>,
    /// The forward price-to-earnings ratio.
    pub forward_pe: Option<f64>,
    /// The earnings per share.
    pub eps: Option<f64>,
    /// The dividend yield as a percentage.
    pub dividend_yield: Option<f64>,
    /// The 52-week high price.
    pub fifty_two_week_high: Option<f64>,
    /// The 52-week low price.
    pub fifty_two_week_low: Option<f64>,
}

/// A current or snapshot exchange rate between two currencies.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CurrencyRate {
    /// The base (source) currency code.
    pub base: String,
    /// The quote (target) currency code.
    pub quote: String,
    /// The exchange rate (1 base = rate quote).
    pub rate: f64,
    /// When the rate was recorded.
    pub timestamp: DateTime<Utc>,
}

/// Classification of an option contract.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum OptionKind {
    /// A call option.
    #[default]
    Call,
    /// A put option.
    Put,
}

/// A single option contract in an options chain.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct OptionContract {
    /// The strike price.
    pub strike: f64,
    /// The expiration date.
    pub expiration: NaiveDate,
    /// Whether the contract is a call or a put.
    pub kind: OptionKind,
    /// The last traded price.
    pub last_price: f64,
    /// The current bid price.
    pub bid: f64,
    /// The current ask price.
    pub ask: f64,
    /// The traded volume.
    pub volume: u64,
    /// The open interest.
    pub open_interest: u64,
    /// The implied volatility.
    pub implied_volatility: Option<f64>,
}

/// A search result mapping a company or asset name to a ticker symbol.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SearchResult {
    /// The ticker symbol.
    pub symbol: String,
    /// The company or asset name.
    pub name: Option<String>,
    /// The exchange the symbol trades on.
    pub exchange: Option<String>,
    /// The asset class (e.g. equity, etf, crypto).
    pub asset_class: Option<String>,
}

/// Analyst recommendation counts for a single reporting period.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
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
