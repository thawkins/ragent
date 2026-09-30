//! Inline tests for `twelvedata.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

// Fixture quotes use exactly representable decimal values.
#![allow(clippy::float_cmp)]
use super::*;

#[test]
fn test_normalize_symbol_routes_lse_suffix() {
    assert_eq!(
        normalize_symbol("LSEG.l"),
        ("LSEG".to_string(), Some("LSE"))
    );
    assert_eq!(
        normalize_symbol("LSEG.L"),
        ("LSEG".to_string(), Some("LSE"))
    );
    assert_eq!(normalize_symbol("MSFT"), ("MSFT".to_string(), None));
    assert_eq!(normalize_symbol("BRK.B"), ("BRK.B".to_string(), None));
}

#[test]
fn test_url_includes_exchange_for_lse_symbol() {
    let provider = TwelveDataProvider::new("demo", None, 0).unwrap();
    let url = provider.url(
        "quote",
        &[
            ("symbol", "LSEG".to_string()),
            ("exchange", "LSE".to_string()),
        ],
    );
    assert!(
        url.contains("symbol=LSEG"),
        "url should contain stripped symbol: {}",
        url
    );
    assert!(
        url.contains("exchange=LSE"),
        "url should contain LSE exchange: {}",
        url
    );
    assert!(
        !url.contains("symbol=LSEG.L"),
        "url should not contain .L suffix: {}",
        url
    );
}

#[test]
fn test_map_interval_defaults_to_daily() {
    assert_eq!(map_interval("1d"), "1day");
    assert_eq!(map_interval(""), "1day");
    assert_eq!(map_interval("1wk"), "1week");
}

#[test]
fn test_map_outputsize_matches_periods() {
    assert_eq!(map_outputsize("1w"), 7);
    assert_eq!(map_outputsize("1mo"), 30);
    assert_eq!(map_outputsize("max"), 5000);
}

#[test]
fn test_parse_timestamp_handles_date_and_datetime() {
    let d = parse_twelvedata_timestamp("2024-09-10").unwrap();
    assert_eq!(d.format("%Y-%m-%d").to_string(), "2024-09-10");

    let dt = parse_twelvedata_timestamp("2024-09-10 14:30:00").unwrap();
    assert_eq!(
        dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        "2024-09-10 14:30:00"
    );
}

#[test]
fn test_parse_fundamentals_from_statistics() {
    let parsed: Value = serde_json::from_str(SAMPLE_STATISTICS_JSON).unwrap();
    let fundamentals = parse_statistics_response(&parsed, "AAPL").unwrap();
    assert_eq!(fundamentals.symbol, "AAPL");
    assert_eq!(fundamentals.name.as_deref(), Some("Apple Inc"));
    assert_eq!(fundamentals.market_cap, Some(2_546_807_865_344));
    assert!((fundamentals.trailing_pe.unwrap() - 30.162_493).abs() < 1e-6);
    assert!((fundamentals.forward_pe.unwrap() - 26.982_489).abs() < 1e-6);
    assert!((fundamentals.eps.unwrap() - 5.108).abs() < 1e-6);
    assert!((fundamentals.dividend_yield.unwrap() - 0.0057).abs() < 1e-6);
    assert!((fundamentals.fifty_two_week_high.unwrap() - 157.26).abs() < 1e-6);
    assert!((fundamentals.fifty_two_week_low.unwrap() - 103.1).abs() < 1e-6);
}
#[test]
fn test_parse_fundamentals_missing_meta_symbol_returns_not_found() {
    let parsed: Value = serde_json::from_str(r#"{"statistics": {}}"#).unwrap();
    let err = parse_statistics_response(&parsed, "UNKNOWN").unwrap_err();
    assert!(err.is_symbol_not_found());
}

// Test helper that bypasses the network and parses a TwelveData quote JSON
// object using the same logic as `quote()`.
fn parse_quote_for_test(parsed: &Value, symbol: &str) -> FinanceResult<Quote> {
    let get_str = |k: &str| parsed.get(k).and_then(|v| v.as_str()).map(String::from);
    let get_f64 = |k: &str| {
        parsed
            .get(k)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    let get_u64 = |k: &str| {
        parsed
            .get(k)
            .and_then(|v| v.as_str())
            .and_then(|s| s.replace(';', "").parse::<u64>().ok())
            .unwrap_or(0)
    };

    let timestamp = parsed
        .get("datetime")
        .or_else(|| parsed.get("timestamp"))
        .and_then(|v| v.as_str())
        .and_then(parse_twelvedata_timestamp)
        .unwrap_or_else(Utc::now);

    let price = parsed
        .get("price")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|p| *p > 0.0)
        .or_else(|| {
            parsed
                .get("close")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|p| *p > 0.0)
        })
        .or_else(|| {
            parsed
                .get("previous_close")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|p| *p > 0.0)
        })
        .unwrap_or(0.0);

    Ok(Quote {
        symbol: get_str("symbol").unwrap_or_else(|| symbol.to_ascii_uppercase()),
        price,
        open: get_f64("open"),
        high: get_f64("high"),
        low: get_f64("low"),
        close: get_f64("previous_close"),
        volume: get_u64("volume"),
        change: get_f64("change"),
        change_percent: get_f64("percent_change"),
        currency: get_str("currency").unwrap_or_else(|| "USD".to_string()),
        market_state: get_str("type").unwrap_or_else(|| {
            parsed
                .get("market_state")
                .and_then(|v| v.as_str())
                .unwrap_or("REGULAR")
                .to_string()
        }),
        timestamp,
    })
}

#[test]
fn test_quote_price_falls_back_to_close_when_price_missing() {
    // Simulates the free-tier TwelveData response where `price` is absent
    // but OHLCV and previous_close are populated.
    let parsed: Value = serde_json::from_str(
        r#"{
              "symbol": "MSFT",
              "open": "481.54",
              "high": "484.27",
              "low": "477.15",
              "close": "480.35",
              "previous_close": "479.07",
              "volume": "23714829",
              "change": "1.28",
              "percent_change": "0.27",
              "currency": "USD",
              "type": "Common Stock",
              "datetime": "2026-08-18 16:00:00"
          }"#,
    )
    .unwrap();

    let quote = parse_quote_for_test(&parsed, "MSFT").unwrap();

    assert_eq!(quote.symbol, "MSFT");
    assert!(
        (quote.price - 480.35).abs() < 1e-6,
        "price should fall back to close: got {}",
        quote.price
    );
    assert_eq!(quote.open, 481.54);
    assert_eq!(quote.high, 484.27);
    assert_eq!(quote.low, 477.15);
    assert_eq!(quote.close, 479.07);
    assert_eq!(quote.volume, 23_714_829);
}

#[test]
fn test_quote_price_prefers_last_price_field() {
    let parsed: Value = serde_json::from_str(
        r#"{
              "symbol": "LSEG",
              "price": "8478.0",
              "open": "8436.0",
              "high": "8608.0",
              "low": "8380.0",
              "previous_close": "8450.0",
              "volume": "1490601",
              "change": "28.0",
              "percent_change": "0.33",
              "currency": "GBp",
              "type": "Common Stock"
          }"#,
    )
    .unwrap();

    let quote = parse_quote_for_test(&parsed, "LSEG.L").unwrap();
    assert_eq!(quote.symbol, "LSEG");
    assert_eq!(quote.price, 8478.0);
    assert_eq!(quote.close, 8450.0);
}

#[test]
fn test_quote_price_falls_back_to_previous_close_when_no_close() {
    let parsed: Value = serde_json::from_str(
        r#"{
              "symbol": "YHOO",
              "open": "655.5",
              "high": "670.25",
              "low": "648.75",
              "previous_close": "672.875",
              "volume": "70",
              "change": "-19.75",
              "percent_change": "-2.94",
              "currency": "GBp",
              "type": "Common Stock"
          }"#,
    )
    .unwrap();

    let quote = parse_quote_for_test(&parsed, "YHOO").unwrap();

    assert_eq!(quote.symbol, "YHOO");
    assert!((quote.price - 672.875).abs() < 1e-6);
}

const SAMPLE_STATISTICS_JSON: &str = r#"{
    "meta": {
        "symbol": "AAPL",
        "name": "Apple Inc",
        "currency": "USD",
        "exchange": "NASDAQ",
        "mic_code": "XNAS",
        "exchange_timezone": "America/New_York"
    },
    "statistics": {
        "valuations_metrics": {
            "market_capitalization": 2546807865344,
            "enterprise_value": 2620597731328,
            "trailing_pe": 30.162493,
            "forward_pe": 26.982489,
            "peg_ratio": 1.4,
            "price_to_sales_ttm": 7.336227,
            "price_to_book_mrq": 39.68831,
            "enterprise_to_revenue": 7.549,
            "enterprise_to_ebitda": 23.623
        },
        "financials": {
            "income_statement": {
                "diluted_eps_ttm": 5.108
            }
        },
        "stock_price_summary": {
            "fifty_two_week_low": 103.1,
            "fifty_two_week_high": 157.26,
            "fifty_two_week_change": 0.375625,
            "beta": 1.201965,
            "day_50_ma": 148.96686,
            "day_200_ma": 134.42506
        },
        "dividends_and_splits": {
            "forward_annual_dividend_yield": 0.0057
        }
    }
}"#;
