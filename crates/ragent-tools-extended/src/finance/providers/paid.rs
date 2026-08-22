//! Paid-provider router for the finance toolset.
//!
//! Currently implements Alpha Vantage as the first concrete paid backend.
//! When a paid provider is configured, this router disables the free Yahoo
//! adapter for the same data category.
//!
//! The free Yahoo adapter is cached per effective configuration (User-Agent
//! and requests-per-minute) so that all finance tools share a single
//! `YfClient`, cookie/crumb state, rate-limit backoff, and throttle.

use crate::finance::providers::{twelvedata::TwelveDataProvider, yahoo::YahooFinanceProvider};
use crate::finance::{
    CurrencyRate, FinanceError, FinanceProvider, FinanceResult, Fundamentals, OhlcvBar,
    OptionContract, Quote, RecommendationPeriod, SearchResult, wait_for_min_interval,
};
use ragent_config::finance::FinanceProviderConfig;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Global cache of Yahoo providers keyed by their effective configuration.
///
/// This avoids creating a fresh `YfClient` on every finance tool call, which
/// previously caused a burst of authentication requests and made Yahoo rate-limit
/// the session immediately.
static YAHOO_PROVIDERS: Mutex<Option<HashMap<String, Arc<YahooFinanceProvider>>>> =
    Mutex::new(None);

/// Build a cache key from the Yahoo-relevant parts of the finance config.
fn yahoo_provider_key(config: Option<&FinanceProviderConfig>) -> String {
    if let Some(c) = config {
        format!(
            "ua={};rpm={}",
            c.user_agent.as_deref().unwrap_or("default"),
            c.requests_per_minute.unwrap_or(0)
        )
    } else {
        "ua=default;rpm=0".to_string()
    }
}

/// Return a cached or newly created Yahoo provider for the given configuration.
///
/// The cache lookup and insertion are performed atomically under a single
/// lock acquisition so that two concurrent calls with the same key cannot both
/// miss the cache and create distinct providers (which would break
/// `Arc::ptr_eq` guarantees relied on by callers).
fn get_or_create_yahoo_provider(
    config: Option<&FinanceProviderConfig>,
) -> Arc<dyn FinanceProvider> {
    let key = yahoo_provider_key(config);

    let mut guard = YAHOO_PROVIDERS
        .lock()
        .expect("yahoo provider cache poisoned");
    let map = guard.get_or_insert_with(HashMap::new);
    if let Some(provider) = map.get(&key) {
        return provider.clone();
    }

    let provider: Arc<YahooFinanceProvider> = if let Some(cfg) = config {
        Arc::new(
            YahooFinanceProvider::from_config(cfg)
                .unwrap_or_else(|_| YahooFinanceProvider::default_client()),
        )
    } else {
        Arc::new(YahooFinanceProvider::default_client())
    };
    map.insert(key, provider.clone());
    provider
}

/// Paid provider configuration and router.
#[derive(Debug)]
pub struct PaidProvider {
    name: String,
    api_key: String,
    base_url: Option<String>,
    client: reqwest::Client,
}

impl PaidProvider {
    /// Create a new paid provider from a provider name and API key.
    ///
    /// # Errors
    ///
    /// Returns an error if `provider` is empty or `api_key` is empty.
    pub fn new(provider: &str, api_key: &str, base_url: Option<String>) -> FinanceResult<Self> {
        if provider.is_empty() {
            return Err(FinanceError::ConfigError(
                "paid provider name is empty".to_string(),
            ));
        }
        if api_key.is_empty() {
            return Err(FinanceError::ConfigError(
                "paid provider API key is empty".to_string(),
            ));
        }
        Ok(Self {
            name: provider.to_string(),
            api_key: api_key.to_string(),
            base_url,
            client: reqwest::Client::new(),
        })
    }

    fn alpha_vantage_url(&self, function: &str, params: &[(&str, String)]) -> String {
        let base = self
            .base_url
            .as_deref()
            .unwrap_or("https://www.alphavantage.co/query");
        let mut url = format!("{}?function={}&apikey={}", base, function, self.api_key);
        for (k, v) in params {
            url.reserve(k.len() + v.len() + 2);
            url.push('&');
            url.push_str(k);
            url.push('=');
            url.push_str(v);
        }
        url
    }

    /// Check an Alpha Vantage response body for rate-limit / error messages.
    fn check_alpha_vantage_errors(&self, parsed: &serde_json::Value) -> Option<FinanceError> {
        let rate_limit_phrases = [
            "per minute",
            "per second",
            "rate limit",
            "spreading out",
            "too many",
            "25 requests per day",
        ];
        if let Some(info) = parsed.get("Information").and_then(|v| v.as_str()) {
            let lower = info.to_ascii_lowercase();
            if rate_limit_phrases.iter().any(|p| lower.contains(p)) {
                return Some(FinanceError::RateLimit {
                    provider: self.name.clone(),
                    retry_after: Some(60),
                });
            }
            return Some(FinanceError::ProviderFailure {
                provider: self.name.clone(),
                message: info.to_string(),
            });
        }
        if parsed.get("Note").is_some() {
            return Some(FinanceError::RateLimit {
                provider: self.name.clone(),
                retry_after: Some(60),
            });
        }
        if let Some(err) = parsed.get("Error Message").and_then(|v| v.as_str()) {
            return Some(FinanceError::ProviderFailure {
                provider: self.name.clone(),
                message: err.to_string(),
            });
        }
        None
    }

    /// Fetch a JSON response from Alpha Vantage and surface common error bodies.
    async fn alpha_vantage_json(
        &self,
        function: &str,
        params: &[(&str, String)],
    ) -> FinanceResult<serde_json::Value> {
        // Process-wide cross-provider throttle; prevents rapid fire calls
        // across Yahoo and Alpha Vantage from triggering rate limits.
        wait_for_min_interval(None).await;

        let url = self.alpha_vantage_url(function, params);
        let response =
            self.client
                .get(&url)
                .send()
                .await
                .map_err(|e| FinanceError::ProviderFailure {
                    provider: self.name.clone(),
                    message: e.to_string(),
                })?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| FinanceError::ProviderFailure {
                provider: self.name.clone(),
                message: e.to_string(),
            })?;
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(FinanceError::RateLimit {
                provider: self.name.clone(),
                retry_after: Some(60),
            });
        }
        let parsed: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| FinanceError::ParseFailure {
                provider: self.name.clone(),
                detail: e.to_string(),
            })?;
        if let Some(err) = self.check_alpha_vantage_errors(&parsed) {
            return Err(err);
        }
        Ok(parsed)
    }
}

#[async_trait::async_trait]
impl FinanceProvider for PaidProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_available(&self) -> bool {
        true
    }

    async fn quote(&self, symbol: &str) -> FinanceResult<Quote> {
        if self.name.eq_ignore_ascii_case("alpha_vantage") {
            // Alpha Vantage quote endpoint: GLOBAL_QUOTE
            let parsed = self
                .alpha_vantage_json("GLOBAL_QUOTE", &[("symbol", symbol.to_ascii_uppercase())])
                .await?;
            let quote = parsed
                .get("Global Quote")
                .ok_or_else(|| FinanceError::ParseFailure {
                    provider: self.name.clone(),
                    detail: "missing Global Quote".to_string(),
                })?;
            if quote.is_object() && quote.as_object().map(|o| o.is_empty()).unwrap_or(true) {
                return Err(FinanceError::SymbolNotFound {
                    symbol: symbol.to_string(),
                });
            }
            let get_str = |k: &str| quote.get(k).and_then(|v| v.as_str()).map(String::from);
            let get_f64 = |k: &str| {
                quote
                    .get(k)
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0)
            };
            let get_pct = |k: &str| {
                quote
                    .get(k)
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim_end_matches('%').trim())
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0)
            };
            return Ok(Quote {
                symbol: get_str("01. symbol").unwrap_or_else(|| symbol.to_ascii_uppercase()),
                price: get_f64("05. price"),
                open: get_f64("02. open"),
                high: get_f64("03. high"),
                low: get_f64("04. low"),
                close: get_f64("05. price"),
                volume: get_str("06. volume")
                    .and_then(|s| s.replace(",", "").parse::<u64>().ok())
                    .unwrap_or(0),
                change: get_f64("09. change"),
                change_percent: get_pct("10. change percent"),
                currency: "USD".to_string(),
                market_state: "REGULAR".to_string(),
                timestamp: get_str("07. latest trading day")
                    .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
                    .and_then(|d| d.and_hms_opt(0, 0, 0))
                    .map(|dt| dt.and_utc())
                    .unwrap_or_else(chrono::Utc::now),
            });
        }
        Err(FinanceError::ProviderFailure {
            provider: self.name.clone(),
            message: format!("paid provider '{}' not implemented", self.name),
        })
    }

    async fn history(
        &self,
        symbol: &str,
        _interval: &str,
        period: &str,
    ) -> FinanceResult<Vec<OhlcvBar>> {
        if self.name.eq_ignore_ascii_case("alpha_vantage") {
            // Alpha Vantage daily endpoint: compact output gives ~100 latest days.
            let parsed = self
                .alpha_vantage_json(
                    "TIME_SERIES_DAILY",
                    &[
                        ("symbol", symbol.to_ascii_uppercase()),
                        ("outputsize", "compact".to_string()),
                    ],
                )
                .await?;
            let series_key = "Time Series (Daily)";
            let series = parsed
                .get(series_key)
                .ok_or_else(|| FinanceError::ParseFailure {
                    provider: self.name.clone(),
                    detail: format!("missing {}", series_key),
                })?;
            let series = series
                .as_object()
                .ok_or_else(|| FinanceError::ParseFailure {
                    provider: self.name.clone(),
                    detail: format!("{} is not an object", series_key),
                })?;
            if series.is_empty() {
                return Err(FinanceError::SymbolNotFound {
                    symbol: symbol.to_string(),
                });
            }

            let mut bars: Vec<OhlcvBar> = series
                .iter()
                .filter_map(|(date_str, value)| {
                    let timestamp = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                        .ok()
                        .and_then(|d| d.and_hms_opt(0, 0, 0))
                        .map(|dt| dt.and_utc())?;
                    let get_f64 = |k: &str| {
                        value
                            .get(k)
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse::<f64>().ok())
                            .unwrap_or(0.0)
                    };
                    let volume = value
                        .get("5. volume")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.replace(",", "").parse::<u64>().ok())
                        .unwrap_or(0);
                    Some(OhlcvBar {
                        timestamp,
                        open: get_f64("1. open"),
                        high: get_f64("2. high"),
                        low: get_f64("3. low"),
                        close: get_f64("4. close"),
                        volume,
                    })
                })
                .collect();

            bars.sort_by_key(|a| a.timestamp);

            let cutoff = history_cutoff(period);
            if let Some(since) = cutoff {
                bars.retain(|bar| bar.timestamp >= since);
            }

            return Ok(bars);
        }
        Err(FinanceError::ProviderFailure {
            provider: self.name.clone(),
            message: "history not implemented for paid provider".to_string(),
        })
    }

    async fn fundamentals(&self, symbol: &str) -> FinanceResult<Fundamentals> {
        let _ = symbol;
        Err(FinanceError::ProviderFailure {
            provider: self.name.clone(),
            message: "fundamentals not implemented for paid provider".to_string(),
        })
    }

    async fn currency_rate(&self, base: &str, quote: &str) -> FinanceResult<CurrencyRate> {
        let _ = (base, quote);
        Err(FinanceError::ProviderFailure {
            provider: self.name.clone(),
            message: "currency_rate not implemented for paid provider".to_string(),
        })
    }

    async fn currency_history(
        &self,
        base: &str,
        quote: &str,
        interval: &str,
        period: &str,
    ) -> FinanceResult<Vec<OhlcvBar>> {
        let _ = (base, quote, interval, period);
        Err(FinanceError::ProviderFailure {
            provider: self.name.clone(),
            message: "currency_history not implemented for paid provider".to_string(),
        })
    }

    async fn search(&self, query: &str) -> FinanceResult<Vec<SearchResult>> {
        let _ = query;
        Err(FinanceError::ProviderFailure {
            provider: self.name.clone(),
            message: "search not implemented for paid provider".to_string(),
        })
    }

    async fn options(
        &self,
        symbol: &str,
        expiration: Option<&str>,
    ) -> FinanceResult<Vec<OptionContract>> {
        let _ = (symbol, expiration);
        Err(FinanceError::ProviderFailure {
            provider: self.name.clone(),
            message: "options not implemented for paid provider".to_string(),
        })
    }

    async fn recommendations(&self, symbol: &str) -> FinanceResult<Vec<RecommendationPeriod>> {
        if self.name.eq_ignore_ascii_case("alpha_vantage") {
            let parsed = self
                .alpha_vantage_json(
                    "RECOMMENDATION_TRENDS",
                    &[("symbol", symbol.to_ascii_uppercase())],
                )
                .await?;
            let trend = parsed
                .get("recommendationTrends")
                .and_then(|v| v.get("trend"))
                .and_then(|v| v.as_array())
                .ok_or_else(|| FinanceError::ParseFailure {
                    provider: self.name.clone(),
                    detail: "missing recommendationTrends.trend".to_string(),
                })?;
            if trend.is_empty() {
                return Err(FinanceError::SymbolNotFound {
                    symbol: symbol.to_string(),
                });
            }
            let mut periods = Vec::with_capacity(trend.len());
            for item in trend {
                let get_u32 = |k: &str| {
                    item.get(k)
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<u32>().ok())
                        .unwrap_or(0)
                };
                periods.push(RecommendationPeriod {
                    period: item
                        .get("period")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    strong_buy: get_u32("strongBuy"),
                    buy: get_u32("buy"),
                    hold: get_u32("hold"),
                    sell: get_u32("sell"),
                    strong_sell: get_u32("strongSell"),
                });
            }
            return Ok(periods);
        }
        Err(FinanceError::ProviderFailure {
            provider: self.name.clone(),
            message: "recommendations not implemented for paid provider".to_string(),
        })
    }
}

/// Compute the earliest UTC timestamp to retain for a given history period.
/// Returns `None` for `max` (keep everything). Alpha Vantage compact daily
/// output is bounded, so this is a best-effort filter on the returned data.
fn history_cutoff(period: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let now = chrono::Utc::now();
    let days = match period {
        "1w" => 7,
        "1mo" => 30,
        "3mo" => 90,
        "6mo" => 180,
        "1y" => 365,
        "5y" => 365 * 5,
        "max" => return None,
        _ => 30,
    };
    Some(now - chrono::Duration::days(days))
}

/// Create the appropriate paid provider from a `FinanceProviderConfig`.
pub fn paid_provider_from_config(
    config: &ragent_config::finance::FinanceProviderConfig,
) -> FinanceResult<Arc<dyn FinanceProvider>> {
    match config.provider.as_str() {
        "alpha_vantage" => {
            let api_key = config.api_key.as_ref().ok_or_else(|| {
                FinanceError::ConfigError("Alpha Vantage API key missing".to_string())
            })?;
            Ok(Arc::new(PaidProvider::new(
                "alpha_vantage",
                api_key,
                config.base_url.clone(),
            )?))
        }
        "twelvedata" => {
            let api_key = config.api_key.as_ref().ok_or_else(|| {
                FinanceError::ConfigError("TwelveData API key missing".to_string())
            })?;
            Ok(Arc::new(TwelveDataProvider::new(
                api_key,
                config.base_url.clone(),
            )?))
        }
        _ => Err(FinanceError::ConfigError(format!(
            "paid provider '{}' is not supported",
            config.provider
        ))),
    }
}

/// Return the default provider based on configuration.
///
/// If a paid provider is configured and available, it is returned and the free
/// Yahoo adapter is disabled. Otherwise a cached free Yahoo adapter is used so
/// that all finance tools share one underlying `YfClient` and rate-limit state.
pub fn default_provider(
    config: Option<&ragent_config::finance::FinanceProviderConfig>,
) -> Arc<dyn FinanceProvider> {
    if let Some(cfg) = config
        && cfg.is_paid_provider_configured()
    {
        match paid_provider_from_config(cfg) {
            Ok(provider) => return provider,
            Err(err) => {
                tracing::warn!(
                    provider = %cfg.provider,
                    error = %err,
                    "configured paid finance provider could not be constructed; falling back to yahoo"
                );
            }
        }
    }
    get_or_create_yahoo_provider(config)
}

/// Return a cached free Yahoo provider for use as a fallback.
///
/// Used when the configured paid provider does not implement a specific endpoint
/// (e.g., Alpha Vantage does not support analyst recommendations), so the tool
/// can still return data from the free Yahoo Finance adapter.
pub fn yahoo_fallback_provider(
    config: Option<&ragent_config::finance::FinanceProviderConfig>,
) -> Arc<dyn FinanceProvider> {
    get_or_create_yahoo_provider(config)
}
