//! Pure normalization helpers for the Yahoo Finance provider.
//!
//! These functions are separated from `YahooFinanceProvider` so they can be
//! unit-tested without making network calls. They cover:
//!
//! - interval and period string → `yfinance_rs` enum mapping,
//! - `paft` money/quantity decimal → `f64`/`u64` conversion,
//! - percent-change calculation,
//! - Yahoo error classification and mapping to `FinanceError`.

use crate::finance::FinanceError;
use paft_decimal::ToPrimitive;

/// Convert a contextual price amount to `f64`, defaulting to `0.0` when absent.
pub fn price_f64(amount: Option<&paft_money::PriceAmount>) -> f64 {
    amount
        .map(|a| decimal_to_f64(a.as_decimal()))
        .unwrap_or(0.0)
}

/// Convert a `paft` decimal to `f64`, defaulting to `0.0` when unrepresentable.
pub fn decimal_to_f64(d: &paft_decimal::Decimal) -> f64 {
    d.to_f64().unwrap_or(0.0)
}

/// Convert a `paft` decimal to `u64`, defaulting to `0` when unrepresentable.
pub fn u64_from_decimal(d: &paft_decimal::Decimal) -> u64 {
    d.to_f64().map(|v| v as u64).unwrap_or(0)
}

/// Convert a contextual quantity amount to `u64`, defaulting to `0` when absent.
pub fn volume_u64(volume: Option<&paft_money::QuantityAmount>) -> u64 {
    volume
        .map(|v| u64_from_decimal(v.as_decimal()))
        .unwrap_or(0)
}

/// Compute percent change from `last` over `previous`, or `0.0` if `previous` is not positive.
pub fn pct_change(
    last: Option<&paft_money::PriceAmount>,
    previous: Option<&paft_money::PriceAmount>,
) -> f64 {
    let last = price_f64(last);
    let prev = price_f64(previous);
    if prev > 0.0 {
        ((last - prev) / prev) * 100.0
    } else {
        0.0
    }
}

/// Map a caller-supplied interval string to a `yfinance_rs::Interval`.
///
/// Accepts `1d`/`d`, `1wk`/`wk`/`w`, and `1mo`/`mo`/`m` (case-insensitive).
/// Any other value falls back to `Interval::D1`.
pub fn parse_interval(interval: &str) -> yfinance_rs::Interval {
    match interval.to_ascii_lowercase().as_str() {
        "1d" | "d" => yfinance_rs::Interval::D1,
        "1wk" | "wk" | "w" => yfinance_rs::Interval::W1,
        "1mo" | "mo" | "m" => yfinance_rs::Interval::M1,
        _ => yfinance_rs::Interval::D1,
    }
}

/// Map a caller-supplied period string to a `yfinance_rs::Range`.
///
/// Accepts `1d`, `5d`, `1mo`, `3mo`, `6mo`, `1y`, `5y`, and `max`
/// (case-insensitive). Shorter periods round up to the smallest supported
/// Yahoo range (`M1`). Any other value falls back to `Range::M6`.
pub fn parse_range(period: &str) -> yfinance_rs::Range {
    let p = period.to_ascii_lowercase();
    match p.as_str() {
        "1mo" | "1m" => yfinance_rs::Range::M1,
        "3mo" | "3m" => yfinance_rs::Range::M3,
        "6mo" | "6m" => yfinance_rs::Range::M6,
        "1y" | "1yr" | "y" => yfinance_rs::Range::Y1,
        "5y" | "5yr" => yfinance_rs::Range::Y5,
        "max" | "all" => yfinance_rs::Range::Max,
        // Round up short lookbacks to the minimum supported range.
        "1d" | "5d" | "1w" | "1wk" | "2wk" | "2w" | "3wk" | "3w" | "4wk" | "4w" => {
            yfinance_rs::Range::M1
        }
        _ => yfinance_rs::Range::M6,
    }
}

/// Returns `true` if the Yahoo error is a 429 rate-limit response.
pub fn is_rate_limit(err: &yfinance_rs::YfError) -> bool {
    matches!(err, yfinance_rs::YfError::RateLimited { .. })
}

/// Map a `yfinance_rs::YfError` to the provider-agnostic `FinanceError`.
pub fn map_yf_error(provider: &str, err: yfinance_rs::YfError) -> FinanceError {
    match err {
        yfinance_rs::YfError::NotFound { .. } => FinanceError::SymbolNotFound {
            symbol: "unknown".to_string(),
        },
        yfinance_rs::YfError::RateLimited { .. } => FinanceError::RateLimit {
            provider: provider.to_string(),
            retry_after: Some(30),
        },
        yfinance_rs::YfError::MissingData(msg) | yfinance_rs::YfError::InvalidData(msg) => {
            FinanceError::ParseFailure {
                provider: provider.to_string(),
                detail: msg,
            }
        }
        _ => FinanceError::ProviderFailure {
            provider: provider.to_string(),
            message: err.to_string(),
        },
    }
}
