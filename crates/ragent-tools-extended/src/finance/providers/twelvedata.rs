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
    /// Configured minimum seconds between finance API calls; `0` means "use the
    /// built-in default" (see [`wait_for_min_interval`]). ANTIPAT M5.9 / 3.4.
    min_call_interval_seconds: u64,
}

impl TwelveDataProvider {
    /// Create a new TwelveData provider from an API key.
    ///
    /// `min_call_interval_seconds` is the configured global throttle interval;
    /// pass `0` to fall back to the built-in default (ANTIPAT M5.9 / 3.4).
    ///
    /// # Errors
    ///
    /// Returns an error if the API key is empty.
    pub fn new(
        api_key: &str,
        base_url: Option<String>,
        min_call_interval_seconds: u64,
    ) -> FinanceResult<Self> {
        if api_key.is_empty() {
            return Err(FinanceError::ConfigError(
                "TwelveData API key is empty".to_string(),
            ));
        }
        let base_url = base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        // SEC-tools-extended-008 (SECTASKS T-066): the config-supplied base URL
        // was used verbatim as the API endpoint for a key-bearing request.
        if let Err(e) = crate::masterfetch::security::validate_url(&base_url) {
            return Err(FinanceError::ConfigError(format!(
                "TwelveData base_url rejected: {e}"
            )));
        }
        // ANTIPAT M5.9 / 3.2: a bare `Client::new()` had the reqwest default
        // (no timeout), so a hung finance endpoint blocked indefinitely.
        let client = crate::masterfetch::http::build_default_client().map_err(|e| {
            FinanceError::ConfigError(format!("failed to build finance HTTP client: {e}"))
        })?;
        Ok(Self {
            api_key: api_key.to_string(),
            base_url,
            client,
            min_call_interval_seconds,
        })
    }

    /// Build the throttle config to pass to [`wait_for_min_interval`].
    ///
    /// `None` (no explicit interval configured) yields the documented default.
    fn min_call_interval_config(&self) -> Option<ragent_config::finance::FinanceProviderConfig> {
        if self.min_call_interval_seconds == 0 {
            return None;
        }
        Some(ragent_config::finance::FinanceProviderConfig {
            min_call_interval_seconds: self.min_call_interval_seconds,
            ..Default::default()
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
        // The config-derived interval is threaded through so
        // `finance.min_call_interval_seconds` is honoured (ANTIPAT M5.9 / 3.4).
        wait_for_min_interval(self.min_call_interval_config().as_ref()).await;

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
        // ANTIPAT 4.1: bound the provider response body.
        let body = crate::masterfetch::http::read_body_capped(
            response,
            crate::masterfetch::http::MAX_RESPONSE_BODY_BYTES,
        )
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
#[path = "../tests/inline/twelvedata_tests.rs"]
mod tests;
