//! Concrete Yahoo Finance provider adapter built on `yfinance_rs`.

use crate::finance::{
    CurrencyRate, FinanceProvider, FinanceResult, Fundamentals, OhlcvBar, OptionContract,
    OptionKind, Quote, RateLimiter, RecommendationPeriod, SearchResult, wait_for_min_interval,
};
use chrono::{NaiveDate, Utc};
use paft_decimal::ToPrimitive;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Provider name used in cache keys and logging.
pub const PROVIDER_NAME: &str = "yahoo";

/// Optional per-provider request throttle.
#[derive(Debug)]
struct Throttle {
    min_interval: Duration,
    last_request: std::sync::Mutex<Instant>,
}

impl Throttle {
    /// Build a throttle from a requests-per-minute target.
    #[must_use]
    fn from_rpm(rpm: u32) -> Self {
        let rpm = rpm.max(1);
        let min_interval = Duration::from_secs_f64(60.0 / f64::from(rpm));
        Self {
            min_interval,
            last_request: std::sync::Mutex::new(Instant::now().checked_sub(min_interval).unwrap()),
        }
    }

    /// Async wait until at least `min_interval` has passed since the last request.
    async fn wait(&self) {
        let wait = {
            let last = *self.last_request.lock().expect("throttle lock poisoned");
            let elapsed = Instant::now().saturating_duration_since(last);
            if elapsed < self.min_interval {
                self.min_interval.checked_sub(elapsed).unwrap()
            } else {
                Duration::ZERO
            }
        };
        if wait > Duration::ZERO {
            tokio::time::sleep(wait).await;
        }
        *self.last_request.lock().expect("throttle lock poisoned") = Instant::now();
    }
}

/// Free Yahoo Finance provider wrapping the `yfinance_rs` crate.
#[derive(Debug)]
pub struct YahooFinanceProvider {
    client: Arc<yfinance_rs::YfClient>,
    rate_limiter: RateLimiter,
    throttle: Option<Throttle>,
}

impl YahooFinanceProvider {
    /// Create a new Yahoo provider backed by the given shared client.
    #[must_use]
    pub fn new(client: Arc<yfinance_rs::YfClient>) -> Self {
        Self {
            client,
            rate_limiter: RateLimiter::new(),
            throttle: None,
        }
    }

    /// Create a provider with the default `YfClient`.
    #[must_use]
    pub fn default_client() -> Self {
        Self::new(Arc::new(yfinance_rs::YfClient::default()))
    }

    /// Create a provider from the finance configuration, applying a custom
    /// User-Agent and an optional requests-per-minute throttle.
    ///
    /// # Errors
    ///
    /// Returns a [`FinanceError`] if the underlying `YfClient` cannot be built.
    pub fn from_config(
        config: &ragent_config::finance::FinanceProviderConfig,
    ) -> FinanceResult<Self> {
        let mut builder = yfinance_rs::YfClient::builder();
        if let Some(ua) = &config.user_agent {
            builder = builder.user_agent(ua.clone());
        }
        let throttle = config
            .requests_per_minute
            .filter(|r| *r > 0)
            .map(Throttle::from_rpm);
        let client = Arc::new(
            builder
                .build()
                .map_err(|e| normalize::map_yf_error(PROVIDER_NAME, e))?,
        );
        Ok(Self {
            client,
            rate_limiter: RateLimiter::new(),
            throttle,
        })
    }

    fn ticker(&self, symbol: &str) -> yfinance_rs::Ticker {
        yfinance_rs::Ticker::new(&self.client, symbol)
    }

    async fn apply_throttle(&self) {
        // Process-wide cross-provider throttle; prevents rapid fire calls
        // across Yahoo and Alpha Vantage from triggering rate limits.
        wait_for_min_interval(None).await;
        if let Some(throttle) = &self.throttle {
            throttle.wait().await;
        }
    }
}

#[async_trait::async_trait]
impl FinanceProvider for YahooFinanceProvider {
    fn name(&self) -> &str {
        PROVIDER_NAME
    }

    fn is_available(&self) -> bool {
        true
    }
    async fn quote(&self, symbol: &str) -> FinanceResult<Quote> {
        self.apply_throttle().await;
        self.rate_limiter.check(PROVIDER_NAME)?;
        let result = self.ticker(symbol).fast_info().await;
        match result {
            Ok(info) => {
                self.rate_limiter.record_success(PROVIDER_NAME);
                let s = &info.snapshot;
                let as_of = s.as_of.unwrap_or_else(Utc::now);
                Ok(Quote {
                    symbol: symbol.to_ascii_uppercase(),
                    price: price_f64(s.last.as_ref()),
                    open: price_f64(s.open.as_ref()),
                    high: price_f64(s.day_high.as_ref()),
                    low: price_f64(s.day_low.as_ref()),
                    close: price_f64(s.last.as_ref()),
                    volume: volume_u64(s.volume.as_ref()),
                    change: price_f64(s.last.as_ref()) - price_f64(s.previous_close.as_ref()),
                    change_percent: pct_change(s.last.as_ref(), s.previous_close.as_ref()),
                    currency: s.currency.code().to_string(),
                    market_state: s
                        .market_state
                        .as_ref()
                        .map(|m| m.to_string())
                        .unwrap_or_default(),
                    timestamp: as_of,
                })
            }
            Err(err) => {
                if is_rate_limit(&err) {
                    self.rate_limiter.record_rate_limit(PROVIDER_NAME, None);
                }
                Err(map_yf_error(PROVIDER_NAME, err))
            }
        }
    }
    async fn history(
        &self,
        symbol: &str,
        interval: &str,
        period: &str,
    ) -> FinanceResult<Vec<OhlcvBar>> {
        self.apply_throttle().await;
        self.rate_limiter.check(PROVIDER_NAME)?;
        let interval = parse_interval(interval);
        let range = parse_range(period);
        let result = self
            .ticker(symbol)
            .history(Some(range), Some(interval), false)
            .await;
        match result {
            Ok(candles) => {
                self.rate_limiter.record_success(PROVIDER_NAME);
                Ok(candles
                    .into_iter()
                    .map(|c| OhlcvBar {
                        timestamp: c.ts,
                        open: price_f64(Some(&c.ohlc.open)),
                        high: price_f64(Some(&c.ohlc.high)),
                        low: price_f64(Some(&c.ohlc.low)),
                        close: price_f64(Some(&c.ohlc.close)),
                        volume: c
                            .volume
                            .as_ref()
                            .map(|v| u64_from_decimal(v.as_decimal()))
                            .unwrap_or(0),
                    })
                    .collect())
            }
            Err(err) => {
                if is_rate_limit(&err) {
                    self.rate_limiter.record_rate_limit(PROVIDER_NAME, None);
                }
                Err(map_yf_error(PROVIDER_NAME, err))
            }
        }
    }
    async fn fundamentals(&self, symbol: &str) -> FinanceResult<Fundamentals> {
        self.apply_throttle().await;
        self.rate_limiter.check(PROVIDER_NAME)?;
        let result = self.ticker(symbol).info().await;
        match result {
            Ok(info) => {
                self.rate_limiter.record_success(PROVIDER_NAME);
                let ks = &info.key_statistics;
                let name = info.snapshot.name.clone();
                let sector = info.profile.as_ref().and_then(|p| match p {
                    yfinance_rs::Profile::Company(company) => company.sector.clone(),
                    yfinance_rs::Profile::Fund(_) => None,
                    _ => None,
                });
                Ok(Fundamentals {
                    symbol: symbol.to_ascii_uppercase(),
                    name,
                    sector,
                    market_cap: ks
                        .market_cap
                        .as_ref()
                        .map(|m| u64_from_decimal(&m.amount())),
                    trailing_pe: ks
                        .pe_trailing_twelve_months
                        .as_ref()
                        .and_then(|d| d.to_f64()),
                    forward_pe: None,
                    eps: ks
                        .eps_trailing_twelve_months
                        .as_ref()
                        .map(|p| decimal_to_f64(&p.amount())),
                    dividend_yield: ks.dividend_yield_trailing.as_ref().and_then(|d| d.to_f64()),
                    fifty_two_week_high: ks
                        .fifty_two_week_high
                        .as_ref()
                        .map(|p| decimal_to_f64(&p.amount())),
                    fifty_two_week_low: ks
                        .fifty_two_week_low
                        .as_ref()
                        .map(|p| decimal_to_f64(&p.amount())),
                })
            }
            Err(err) => {
                if is_rate_limit(&err) {
                    self.rate_limiter.record_rate_limit(PROVIDER_NAME, None);
                }
                Err(map_yf_error(PROVIDER_NAME, err))
            }
        }
    }
    async fn currency_rate(&self, base: &str, quote: &str) -> FinanceResult<CurrencyRate> {
        self.apply_throttle().await;
        let pair = format!(
            "{}{}=X",
            base.to_ascii_uppercase(),
            quote.to_ascii_uppercase()
        );
        self.rate_limiter.check(PROVIDER_NAME)?;
        let result = self.ticker(&pair).fast_info().await;
        match result {
            Ok(info) => {
                self.rate_limiter.record_success(PROVIDER_NAME);
                Ok(CurrencyRate {
                    base: base.to_ascii_uppercase(),
                    quote: quote.to_ascii_uppercase(),
                    rate: price_f64(info.snapshot.last.as_ref()),
                    timestamp: info.snapshot.as_of.unwrap_or_else(Utc::now),
                })
            }
            Err(err) => {
                if is_rate_limit(&err) {
                    self.rate_limiter.record_rate_limit(PROVIDER_NAME, None);
                }
                Err(map_yf_error(PROVIDER_NAME, err))
            }
        }
    }
    async fn currency_history(
        &self,
        base: &str,
        quote: &str,
        interval: &str,
        period: &str,
    ) -> FinanceResult<Vec<OhlcvBar>> {
        self.apply_throttle().await;
        let pair = format!(
            "{}{}=X",
            base.to_ascii_uppercase(),
            quote.to_ascii_uppercase()
        );
        self.history(&pair, interval, period).await
    }
    async fn search(&self, query: &str) -> FinanceResult<Vec<SearchResult>> {
        self.apply_throttle().await;
        self.rate_limiter.check(PROVIDER_NAME)?;
        let result = yfinance_rs::search(&self.client, query).await;
        match result {
            Ok(response) => {
                self.rate_limiter.record_success(PROVIDER_NAME);
                Ok(response
                    .results
                    .into_iter()
                    .map(|r| SearchResult {
                        symbol: r.instrument.symbol.to_string(),
                        name: r.name,
                        exchange: r.instrument.exchange.as_ref().map(|e| e.to_string()),
                        asset_class: Some(r.instrument.kind.code().to_string()),
                    })
                    .collect())
            }
            Err(err) => {
                if is_rate_limit(&err) {
                    self.rate_limiter.record_rate_limit(PROVIDER_NAME, None);
                }
                Err(map_yf_error(PROVIDER_NAME, err))
            }
        }
    }
    async fn options(
        &self,
        symbol: &str,
        expiration: Option<&str>,
    ) -> FinanceResult<Vec<OptionContract>> {
        self.apply_throttle().await;
        self.rate_limiter.check(PROVIDER_NAME)?;
        let date = expiration
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .map(|d| {
                d.and_hms_opt(0, 0, 0)
                    .unwrap_or_default()
                    .and_utc()
                    .timestamp()
            });
        let result = self.ticker(symbol).option_chain(date).await;
        match result {
            Ok(chain) => {
                self.rate_limiter.record_success(PROVIDER_NAME);
                Ok(chain
                    .contracts
                    .into_iter()
                    .map(|c| OptionContract {
                        strike: decimal_to_f64(&c.key.strike.amount()),
                        expiration: c.key.expiration_date,
                        kind: match c.key.side.as_str() {
                            "CALL" => OptionKind::Call,
                            _ => OptionKind::Put,
                        },
                        last_price: c.price.map(|p| price_f64(Some(&p))).unwrap_or(0.0),
                        bid: c.bid.map(|p| price_f64(Some(&p))).unwrap_or(0.0),
                        ask: c.ask.map(|p| price_f64(Some(&p))).unwrap_or(0.0),
                        volume: c.volume.unwrap_or(0),
                        open_interest: c.open_interest.unwrap_or(0),
                        implied_volatility: c
                            .implied_volatility
                            .as_ref()
                            .and_then(|v| v.as_decimal().to_f64()),
                    })
                    .collect())
            }
            Err(err) => {
                if is_rate_limit(&err) {
                    self.rate_limiter.record_rate_limit(PROVIDER_NAME, None);
                }
                Err(map_yf_error(PROVIDER_NAME, err))
            }
        }
    }
    async fn recommendations(&self, symbol: &str) -> FinanceResult<Vec<RecommendationPeriod>> {
        self.apply_throttle().await;
        self.rate_limiter.check(PROVIDER_NAME)?;
        let result = self.ticker(symbol).recommendations().await;
        match result {
            Ok(rows) => {
                self.rate_limiter.record_success(PROVIDER_NAME);
                Ok(rows
                    .into_iter()
                    .map(|r| RecommendationPeriod {
                        period: r.period.to_string(),
                        strong_buy: r.strong_buy.unwrap_or(0),
                        buy: r.buy.unwrap_or(0),
                        hold: r.hold.unwrap_or(0),
                        sell: r.sell.unwrap_or(0),
                        strong_sell: r.strong_sell.unwrap_or(0),
                    })
                    .collect())
            }
            Err(err) => {
                if is_rate_limit(&err) {
                    self.rate_limiter.record_rate_limit(PROVIDER_NAME, None);
                }
                Err(map_yf_error(PROVIDER_NAME, err))
            }
        }
    }
}

pub mod normalize;

use normalize::{
    decimal_to_f64, is_rate_limit, map_yf_error, parse_interval, parse_range, pct_change,
    price_f64, u64_from_decimal, volume_u64,
};
