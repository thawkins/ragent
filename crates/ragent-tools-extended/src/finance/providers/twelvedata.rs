//! Concrete TwelveData provider adapter for the finance toolset.
//!
//! Implements `FinanceProvider` for the TwelveData REST API. Supports quotes
//! and historical bars on the free/paid tier; endpoints that are not available
//! or not mapped fall back to the free Yahoo provider via the tool-layer
//! `with_yahoo_fallback` helper.

use crate::finance::{
    CurrencyRate, FinanceError, FinanceProvider, FinanceResult, Fundamentals, OhlcvBar,
    OptionContract, Quote, RecommendationPeriod, SearchResult, wait_for_min_interval,
};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value;

/// Provider name used in cache keys and logging.
pub const PROVIDER_NAME: &str = "twelvedata";

/// Default TwelveData API base URL.
const DEFAULT_BASE_URL: &str = "https://api.twelvedata.com";

/// Normalize a caller-supplied symbol for the TwelveData API.
///
/// Symbols with a `.L` suffix are routed to the London Stock Exchange by
/// stripping the suffix and returning the LSE exchange code. Other symbols
/// are passed through unchanged with no exchange override.
fn normalize_symbol(symbol: &str) -> (String, Option<&'static str>) {
    let upper = symbol.to_ascii_uppercase();
    if let Some(base) = upper.strip_suffix(".L") {
        (base.to_string(), Some("LSE"))
    } else {
        (upper, None)
    }
}

/// TwelveData paid provider adapter.
#[derive(Debug)]
pub struct TwelveDataProvider {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl TwelveDataProvider {
    /// Create a new TwelveData provider from an API key.
    ///
    /// # Errors
    ///
    /// Returns an error if the API key is empty.
    pub fn new(api_key: &str, base_url: Option<String>) -> FinanceResult<Self> {
        if api_key.is_empty() {
            return Err(FinanceError::ConfigError(
                "TwelveData API key is empty".to_string(),
            ));
        }
        Ok(Self {
            api_key: api_key.to_string(),
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            client: reqwest::Client::new(),
        })
    }

    /// Build a full URL for a TwelveData endpoint.
    fn url(&self, endpoint: &str, params: &[(&str, String)]) -> String {
        let mut url = format!("{}/{}?apikey={}", self.base_url, endpoint, self.api_key);
        for (k, v) in params {
            url.reserve(k.len() + v.len() + 2);
            url.push('&');
            url.push_str(k);
            url.push('=');
            url.push_str(v);
        }
        url
    }

    /// Detect common TwelveData error/rate-limit bodies.
    fn check_errors(&self, parsed: &Value) -> Option<FinanceError> {
        if let Some(status) = parsed.get("status").and_then(|v| v.as_str())
            && status.eq_ignore_ascii_case("error")
        {
            let message = parsed
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("TwelveData API error")
                .to_string();
            return Some(FinanceError::ProviderFailure {
                provider: PROVIDER_NAME.to_string(),
                message,
            });
        }
        if parsed.get("code").and_then(|v| v.as_i64()).is_some() && parsed.get("message").is_some()
        {
            let message = parsed
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("TwelveData API error")
                .to_string();
            let lower = message.to_ascii_lowercase();
            if lower.contains("rate limit") || lower.contains("too many") {
                return Some(FinanceError::RateLimit {
                    provider: PROVIDER_NAME.to_string(),
                    retry_after: Some(60),
                });
            }
            return Some(FinanceError::ProviderFailure {
                provider: PROVIDER_NAME.to_string(),
                message,
            });
        }
        None
    }

    /// Fetch a JSON response from a TwelveData endpoint.
    async fn json(&self, endpoint: &str, params: &[(&str, String)]) -> FinanceResult<Value> {
        // Process-wide cross-provider throttle; prevents rapid fire calls
        // across Yahoo, Alpha Vantage, and TwelveData from triggering rate limits.
        wait_for_min_interval(None).await;

        let url = self.url(endpoint, params);
        let response =
            self.client
                .get(&url)
                .send()
                .await
                .map_err(|e| FinanceError::ProviderFailure {
                    provider: PROVIDER_NAME.to_string(),
                    message: e.to_string(),
                })?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| FinanceError::ProviderFailure {
                provider: PROVIDER_NAME.to_string(),
                message: e.to_string(),
            })?;

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(FinanceError::RateLimit {
                provider: PROVIDER_NAME.to_string(),
                retry_after: Some(60),
            });
        }

        let parsed: Value =
            serde_json::from_str(&body).map_err(|e| FinanceError::ParseFailure {
                provider: PROVIDER_NAME.to_string(),
                detail: e.to_string(),
            })?;

        if let Some(err) = self.check_errors(&parsed) {
            return Err(err);
        }

        Ok(parsed)
    }
}

#[async_trait::async_trait]
impl FinanceProvider for TwelveDataProvider {
    fn name(&self) -> &str {
        PROVIDER_NAME
    }

    fn is_available(&self) -> bool {
        true
    }

    async fn quote(&self, symbol: &str) -> FinanceResult<Quote> {
        let (td_symbol, exchange) = normalize_symbol(symbol);
        let mut params = vec![("symbol", td_symbol)];
        if let Some(ex) = exchange {
            params.push(("exchange", ex.to_string()));
        }

        let parsed = self.json("quote", &params).await?;

        if parsed.get("symbol").is_none() {
            return Err(FinanceError::SymbolNotFound {
                symbol: symbol.to_string(),
            });
        }

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
                .and_then(|s| s.replace(',', "").parse::<u64>().ok())
                .unwrap_or(0)
        };

        let timestamp = parsed
            .get("datetime")
            .or_else(|| parsed.get("timestamp"))
            .and_then(|v| v.as_str())
            .and_then(parse_twelvedata_timestamp)
            .unwrap_or_else(Utc::now);

        // The TwelveData `/quote` endpoint sometimes omits the `price` field
        // on free-tier keys (it returns only OHLCV and previous close). Fall
        // back through close and previous_close so callers never see a 0.0
        // current price when other data is present.
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

    async fn history(
        &self,
        symbol: &str,
        interval: &str,
        period: &str,
    ) -> FinanceResult<Vec<OhlcvBar>> {
        let (td_symbol, exchange) = normalize_symbol(symbol);
        let td_interval = map_interval(interval);
        let outputsize = map_outputsize(period).to_string();

        let mut params = vec![
            ("symbol", td_symbol),
            ("interval", td_interval.to_string()),
            ("outputsize", outputsize),
            ("timezone", "UTC".to_string()),
        ];
        if let Some(ex) = exchange {
            params.push(("exchange", ex.to_string()));
        }

        let parsed = self.json("time_series", &params).await?;

        let values = parsed
            .get("values")
            .and_then(|v| v.as_array())
            .ok_or_else(|| FinanceError::ParseFailure {
                provider: PROVIDER_NAME.to_string(),
                detail: "missing time_series.values".to_string(),
            })?;

        if values.is_empty() {
            return Err(FinanceError::SymbolNotFound {
                symbol: symbol.to_string(),
            });
        }

        let mut bars: Vec<OhlcvBar> = values
            .iter()
            .filter_map(|value| {
                let datetime = value.get("datetime").and_then(|v| v.as_str())?;
                let timestamp = parse_twelvedata_timestamp(datetime)?;
                let get_f64 = |k: &str| {
                    value
                        .get(k)
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0)
                };
                let volume = value
                    .get("volume")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.replace(',', "").parse::<u64>().ok())
                    .unwrap_or(0);
                Some(OhlcvBar {
                    timestamp,
                    open: get_f64("open"),
                    high: get_f64("high"),
                    low: get_f64("low"),
                    close: get_f64("close"),
                    volume,
                })
            })
            .collect();

        bars.sort_by_key(|a| a.timestamp);

        if let Some(cutoff) = history_cutoff(period) {
            bars.retain(|bar| bar.timestamp >= cutoff);
        }

        Ok(bars)
    }

    async fn fundamentals(&self, symbol: &str) -> FinanceResult<Fundamentals> {
        let (td_symbol, exchange) = normalize_symbol(symbol);
        let mut params = vec![("symbol", td_symbol)];
        if let Some(ex) = exchange {
            params.push(("exchange", ex.to_string()));
        }

        let parsed = self.json("statistics", &params).await?;
        parse_statistics_response(&parsed, symbol)
    }

    async fn currency_rate(&self, _base: &str, _quote: &str) -> FinanceResult<CurrencyRate> {
        Err(FinanceError::ProviderFailure {
            provider: PROVIDER_NAME.to_string(),
            message: "currency_rate not implemented for TwelveData".to_string(),
        })
    }

    async fn currency_history(
        &self,
        _base: &str,
        _quote: &str,
        _interval: &str,
        _period: &str,
    ) -> FinanceResult<Vec<OhlcvBar>> {
        Err(FinanceError::ProviderFailure {
            provider: PROVIDER_NAME.to_string(),
            message: "currency_history not implemented for TwelveData".to_string(),
        })
    }

    async fn search(&self, _query: &str) -> FinanceResult<Vec<SearchResult>> {
        Err(FinanceError::ProviderFailure {
            provider: PROVIDER_NAME.to_string(),
            message: "search not implemented for TwelveData".to_string(),
        })
    }

    async fn options(
        &self,
        _symbol: &str,
        _expiration: Option<&str>,
    ) -> FinanceResult<Vec<OptionContract>> {
        Err(FinanceError::ProviderFailure {
            provider: PROVIDER_NAME.to_string(),
            message: "options not implemented for TwelveData".to_string(),
        })
    }

    async fn recommendations(&self, _symbol: &str) -> FinanceResult<Vec<RecommendationPeriod>> {
        Err(FinanceError::ProviderFailure {
            provider: PROVIDER_NAME.to_string(),
            message: "recommendations not implemented for TwelveData".to_string(),
        })
    }
}

/// Parse TwelveData date/datetime strings into UTC.
///
/// Handles both "YYYY-MM-DD" and "YYYY-MM-DD HH:MM:SS" formats. Time-only or
/// timezone-suffixed values are accepted best-effort.
fn parse_twelvedata_timestamp(value: &str) -> Option<DateTime<Utc>> {
    let value = value.trim();

    if let Ok(dt) = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.and_utc());
    }
    if let Ok(d) = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return d.and_hms_opt(0, 0, 0).map(|dt| dt.and_utc());
    }
    // Some endpoints return an ISO-ish value, e.g. "2024-01-15T10:30:00".
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// Map caller interval strings to TwelveData interval values.
fn map_interval(interval: &str) -> &'static str {
    match interval {
        "1m" => "1min",
        "5m" => "5min",
        "15m" => "15min",
        "30m" => "30min",
        "1h" => "1h",
        "1d" | "d" => "1day",
        "1wk" | "wk" | "w" | "1w" => "1week",
        "1mo" | "mo" => "1month",
        _ => "1day",
    }
}

/// Map caller period strings to a TwelveData `outputsize`.
fn map_outputsize(period: &str) -> usize {
    match period {
        "1d" => 24, // Hourly data if used; TwelveData ignores for daily.
        "1w" | "1wk" | "w" | "wk" => 7,
        "1mo" | "1m" => 30,
        "3mo" | "3m" => 90,
        "6mo" | "6m" => 180,
        "1y" | "y" | "1yr" => 365,
        "5y" | "5yr" => 365 * 5,
        "max" | "all" => 5000,
        _ => 30,
    }
}

/// Compute the earliest UTC timestamp to retain for a given history period.
fn history_cutoff(period: &str) -> Option<DateTime<Utc>> {
    let now = Utc::now();
    let days = match period {
        "1w" | "1wk" | "w" | "wk" => 7,
        "1mo" | "1m" => 30,
        "3mo" | "3m" => 90,
        "6mo" | "6m" => 180,
        "1y" | "y" | "1yr" => 365,
        "5y" | "5yr" => 365 * 5,
        "max" | "all" => return None,
        _ => 30,
    };
    Some(now - chrono::Duration::days(days))
}

/// Extract a nested string value from a JSON object.
fn nested_str<'v>(root: &'v Value, path: &[&str]) -> Option<&'v str> {
    let mut current = root;
    for key in path {
        current = current.get(key)?;
    }
    current.as_str()
}

/// Extract a nested numeric value as `f64` from a JSON object, accepting both
/// numbers and numeric strings.
fn nested_f64(root: &Value, path: &[&str]) -> Option<f64> {
    let mut current = root;
    for key in path {
        current = current.get(key)?;
    }
    current
        .as_f64()
        .or_else(|| current.as_str().and_then(|s| s.parse().ok()))
}

/// Extract a nested numeric value as `u64` from a JSON object, accepting both
/// numbers and numeric strings.
fn nested_u64(root: &Value, path: &[&str]) -> Option<u64> {
    let mut current = root;
    for key in path {
        current = current.get(key)?;
    }
    current
        .as_u64()
        .or_else(|| current.as_f64().map(|f| f as u64))
        .or_else(|| {
            current
                .as_str()
                .and_then(|s| s.replace(',', "").parse().ok())
        })
}

/// Parse a TwelveData `/statistics` response into the normalized `Fundamentals`
/// model.
///
/// The statistics endpoint provides valuation metrics, income statement data,
/// stock price summary, and dividend information. Sector is not included in
/// this response, so it is left as `None`.
fn parse_statistics_response(parsed: &Value, symbol: &str) -> FinanceResult<Fundamentals> {
    let meta_symbol = nested_str(parsed, &["meta", "symbol"]);
    if meta_symbol.is_none() {
        return Err(FinanceError::SymbolNotFound {
            symbol: symbol.to_string(),
        });
    }

    let name = nested_str(parsed, &["meta", "name"]).map(String::from);
    let market_cap = nested_u64(
        parsed,
        &["statistics", "valuations_metrics", "market_capitalization"],
    );
    let trailing_pe = nested_f64(parsed, &["statistics", "valuations_metrics", "trailing_pe"]);
    let forward_pe = nested_f64(parsed, &["statistics", "valuations_metrics", "forward_pe"]);
    let eps = nested_f64(
        parsed,
        &[
            "statistics",
            "financials",
            "income_statement",
            "diluted_eps_ttm",
        ],
    );
    let dividend_yield = nested_f64(
        parsed,
        &[
            "statistics",
            "dividends_and_splits",
            "forward_annual_dividend_yield",
        ],
    )
    .or_else(|| {
        nested_f64(
            parsed,
            &[
                "statistics",
                "dividends_and_splits",
                "trailing_annual_dividend_yield",
            ],
        )
    });
    let fifty_two_week_high = nested_f64(
        parsed,
        &["statistics", "stock_price_summary", "fifty_two_week_high"],
    );
    let fifty_two_week_low = nested_f64(
        parsed,
        &["statistics", "stock_price_summary", "fifty_two_week_low"],
    );

    Ok(Fundamentals {
        symbol: meta_symbol
            .map(String::from)
            .unwrap_or_else(|| symbol.to_ascii_uppercase()),
        name,
        sector: None,
        market_cap,
        trailing_pe,
        forward_pe,
        eps,
        dividend_yield,
        fifty_two_week_high,
        fifty_two_week_low,
    })
}

#[cfg(test)]
mod tests {
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
        let provider = TwelveDataProvider::new("demo", None).unwrap();
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
                .and_then(|s| s.replace(",", "").parse::<u64>().ok())
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
}
