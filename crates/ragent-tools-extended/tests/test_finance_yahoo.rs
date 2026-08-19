//! Unit tests for `finance::providers::yahoo` normalization and
//! `yfinance_rs` response mapping.
//!
//! These tests target the pure helper module exposed under
//! `finance::providers::yahoo::normalize` so they run without network calls.

use ragent_tools_extended::finance::FinanceError;
use ragent_tools_extended::finance::providers::yahoo::normalize::{
    decimal_to_f64, is_rate_limit, map_yf_error, parse_interval, parse_range, pct_change,
    price_f64, u64_from_decimal, volume_u64,
};
use std::str::FromStr;

fn price_amount(value: &str) -> paft_money::PriceAmount {
    paft_money::PriceAmount::new(paft_decimal::Decimal::from_str(value).unwrap())
}

fn quantity_amount(value: &str) -> paft_money::QuantityAmount {
    paft_money::QuantityAmount::from_decimal(paft_decimal::Decimal::from_str(value).unwrap())
        .expect("quantity should be non-negative")
}

#[test]
fn test_parse_interval_maps_common_inputs() {
    assert_eq!(parse_interval("1d"), yfinance_rs::Interval::D1);
    assert_eq!(parse_interval("d"), yfinance_rs::Interval::D1);
    assert_eq!(parse_interval("1D"), yfinance_rs::Interval::D1);
    assert_eq!(parse_interval("1wk"), yfinance_rs::Interval::W1);
    assert_eq!(parse_interval("wk"), yfinance_rs::Interval::W1);
    assert_eq!(parse_interval("w"), yfinance_rs::Interval::W1);
    assert_eq!(parse_interval("1mo"), yfinance_rs::Interval::M1);
    assert_eq!(parse_interval("mo"), yfinance_rs::Interval::M1);
    assert_eq!(parse_interval("m"), yfinance_rs::Interval::M1);
}

#[test]
fn test_parse_interval_defaults_to_daily_for_unknown() {
    assert_eq!(parse_interval("1h"), yfinance_rs::Interval::D1);
    assert_eq!(parse_interval(""), yfinance_rs::Interval::D1);
}

#[test]
fn test_parse_range_maps_common_periods() {
    assert_eq!(parse_range("1mo"), yfinance_rs::Range::M1);
    assert_eq!(parse_range("3mo"), yfinance_rs::Range::M3);
    assert_eq!(parse_range("6mo"), yfinance_rs::Range::M6);
    assert_eq!(parse_range("1y"), yfinance_rs::Range::Y1);
    assert_eq!(parse_range("5y"), yfinance_rs::Range::Y5);
    assert_eq!(parse_range("max"), yfinance_rs::Range::Max);
}

#[test]
fn test_parse_range_defaults_to_six_months_for_unknown() {
    assert_eq!(parse_range(""), yfinance_rs::Range::M6);
    assert_eq!(parse_range("unknown"), yfinance_rs::Range::M6);
}

#[test]
fn test_parse_range_rounds_up_short_lookbacks_to_one_month() {
    assert_eq!(parse_range("1d"), yfinance_rs::Range::M1);
    assert_eq!(parse_range("5d"), yfinance_rs::Range::M1);
    assert_eq!(parse_range("1wk"), yfinance_rs::Range::M1);
    assert_eq!(parse_range("1w"), yfinance_rs::Range::M1);
    assert_eq!(parse_range("2w"), yfinance_rs::Range::M1);
    assert_eq!(parse_range("4wk"), yfinance_rs::Range::M1);
}

#[test]
fn test_price_f64_converts_present_amount() {
    let amount = price_amount("150.50");
    assert_eq!(price_f64(Some(&amount)), 150.50);
}

#[test]
fn test_price_f64_defaults_to_zero_when_missing() {
    assert_eq!(price_f64(None), 0.0);
}

#[test]
fn test_volume_u64_converts_present_quantity() {
    let qty = quantity_amount("1000000");
    assert_eq!(volume_u64(Some(&qty)), 1_000_000);
}

#[test]
fn test_volume_u64_defaults_to_zero_when_missing() {
    assert_eq!(volume_u64(None), 0);
}

#[test]
fn test_decimal_to_f64_round_trips_simple_values() {
    let d = paft_decimal::Decimal::from_str("123.456").unwrap();
    assert!((decimal_to_f64(&d) - 123.456).abs() < f64::EPSILON * 100.0);
}

#[test]
fn test_u64_from_decimal_truncates_fractional_part() {
    let d = paft_decimal::Decimal::from_str("123.99").unwrap();
    assert_eq!(u64_from_decimal(&d), 123);
}

#[test]
fn test_pct_change_computes_positive_percent() {
    let last = price_amount("110.0");
    let prev = price_amount("100.0");
    assert!((pct_change(Some(&last), Some(&prev)) - 10.0).abs() < f64::EPSILON);
}

#[test]
fn test_pct_change_computes_negative_percent() {
    let last = price_amount("90.0");
    let prev = price_amount("100.0");
    assert!((pct_change(Some(&last), Some(&prev)) - (-10.0)).abs() < f64::EPSILON);
}

#[test]
fn test_pct_change_returns_zero_when_previous_is_zero() {
    let last = price_amount("110.0");
    let prev = price_amount("0.0");
    assert_eq!(pct_change(Some(&last), Some(&prev)), 0.0);
}

#[test]
fn test_pct_change_returns_zero_when_previous_is_missing() {
    let last = price_amount("110.0");
    assert_eq!(pct_change(Some(&last), None), 0.0);
}

#[test]
fn test_is_rate_limit_detects_rate_limited_error() {
    let err = yfinance_rs::YfError::RateLimited {
        url: "https://finance.yahoo.com".to_string(),
    };
    assert!(is_rate_limit(&err));
}

#[test]
fn test_is_rate_limit_false_for_other_errors() {
    let err = yfinance_rs::YfError::NotFound {
        url: "https://finance.yahoo.com".to_string(),
    };
    assert!(!is_rate_limit(&err));
}

#[test]
fn test_map_yf_error_not_found_maps_to_symbol_not_found() {
    let err = yfinance_rs::YfError::NotFound {
        url: "https://finance.yahoo.com".to_string(),
    };
    let mapped = map_yf_error("yahoo", err);
    assert!(matches!(mapped, FinanceError::SymbolNotFound { symbol } if symbol == "unknown"));
}

#[test]
fn test_map_yf_error_rate_limited_maps_to_rate_limit() {
    let err = yfinance_rs::YfError::RateLimited {
        url: "https://finance.yahoo.com".to_string(),
    };
    let mapped = map_yf_error("yahoo", err);
    assert!(
        matches!(mapped, FinanceError::RateLimit { ref provider, retry_after } if provider == "yahoo" && retry_after == Some(30))
    );
    assert!(
        mapped
            .to_string()
            .contains("try again later or configure a paid provider"),
        "rate-limit error should suggest a paid provider fallback"
    );
}

#[test]
fn test_map_yf_error_missing_data_maps_to_parse_failure() {
    let err = yfinance_rs::YfError::MissingData("regularMarketPrice".to_string());
    let mapped = map_yf_error("yahoo", err);
    assert!(
        matches!(mapped, FinanceError::ParseFailure { provider, detail } if provider == "yahoo" && detail == "regularMarketPrice")
    );
}

#[test]
fn test_map_yf_error_invalid_data_maps_to_parse_failure() {
    let err = yfinance_rs::YfError::InvalidData("unrecognized field".to_string());
    let mapped = map_yf_error("yahoo", err);
    assert!(
        matches!(mapped, FinanceError::ParseFailure { provider, detail } if provider == "yahoo" && detail == "unrecognized field")
    );
}

#[test]
fn test_map_yf_error_server_error_maps_to_provider_failure() {
    let err = yfinance_rs::YfError::ServerError {
        status: 503,
        url: "https://finance.yahoo.com".to_string(),
    };
    let mapped = map_yf_error("yahoo", err);
    assert!(
        matches!(mapped, FinanceError::ProviderFailure { provider, message } if provider == "yahoo" && message.contains("503"))
    );
}

#[test]
fn test_map_yf_error_preserves_provider_name() {
    let err = yfinance_rs::YfError::RateLimited {
        url: "https://finance.yahoo.com".to_string(),
    };
    let mapped = map_yf_error("alpha_vantage", err);
    assert!(
        matches!(mapped, FinanceError::RateLimit { provider, .. } if provider == "alpha_vantage")
    );
}

#[test]
fn test_price_f64_handles_zero() {
    let amount = price_amount("0.0");
    assert_eq!(price_f64(Some(&amount)), 0.0);
}

#[test]
fn test_volume_u64_handles_zero() {
    let qty = quantity_amount("0");
    assert_eq!(volume_u64(Some(&qty)), 0);
}
