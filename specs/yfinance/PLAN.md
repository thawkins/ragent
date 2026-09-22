# Implementation Plan: yfinance — Stocks and Currency Toolset

**Spec ID:** `yfinance`
**Spec status:** draft

## Overview

This plan implements the `yfinance` specification: a provider-agnostic stocks and
currency toolset with a free Yahoo Finance default adapter and a pluggable paid
provider adapter. The implementation lives in the `crates/ragent-tools-extended`
crate and exposes six new tools to agents.

The default Yahoo Finance adapter is built on the [`yfinance_rs`](https://crates.io/crates/yfinance_rs)
crate, which mirrors the Python `yfinance` API and wraps the same public
endpoints. A thin provider layer normalizes `yfinance_rs` responses into the
shared internal schema and handles caching, rate limiting, and logging.

## Architecture

```
crates/ragent-tools-extended/src/
├── finance/
│   ├── mod.rs                  # Module root, shared types, provider trait
│   ├── model.rs                # Normalized Quote, OhlcvBar, Fundamentals, etc.
│   ├── error.rs                # FinanceError, FinanceResult
│   ├── provider.rs             # FinanceProvider trait + default_provider()
│   ├── cache.rs                # In-memory quote cache (60s TTL)
│   ├── rate_limit.rs           # Provider cooldown/backoff state
│   ├── providers/
│   │   ├── mod.rs              # Re-exports
│   │   ├── yahoo.rs            # yfinance_rs wrapper (free, default)
│   │   └── paid.rs             # Paid-provider router + Alpha Vantage backend
│   └── tools/
│       ├── mod.rs              # Registers all 6 tools
│       ├── quote.rs            # stock_quote
│       ├── history.rs          # stock_history
│       ├── fundamentals.rs     # stock_fundamentals
│       ├── currency_rate.rs    # currency_rate
│       ├── currency_history.rs # currency_history
│       ├── search.rs           # stock_search
│       └── options.rs          # stock_options
└── lib.rs                      # + finance module, + registration in create_extended_registry()

crates/ragent-tools-extended/Cargo.toml  # + yfinance_rs, reqwest, serde_json, chrono
crates/ragent-config/src/config.rs       # + finance visibility switch
crates/ragent-config/src/finance.rs      # Optional: runtime paid-provider API key mapping
```

### Data flow

```
Agent invokes stock_quote / currency_rate / ...
        │
        ▼
Tool::execute(input, ctx)
        │
        ├─► finance::cache::cached_quote(symbol, ttl=60s)
        ├─► finance::provider::default_provider(config)
        │       ├── YahooFinanceProvider  (yfinance_rs, free, default)
        │       └── PaidProvider          (if api_key configured)
        │
        ▼
Normalized response → ToolOutput { content, metadata }
```

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Create `finance/` module structure with shared types (`Quote`, `OhlcvBar`, `Fundamentals`, `CurrencyRate`, `OptionContract`, `SearchResult`) | FR-008, NFR-001 | M | Critical | completed | — |
| T-002 | Implement `finance::error` — typed errors for symbol-not-found, rate-limit, provider-failure, parse-failure | FR-013, NFR-004 | S | Critical | completed | T-001 |
| T-003 | Implement `finance::provider` trait — `quote`, `history`, `fundamentals`, `currency_rate`, `currency_history`, `search`, `options` methods | FR-008, FR-009, FR-010, FR-011 | M | Critical | completed | T-001, T-002 |
| T-004 | Implement `finance::cache` — 60s TTL in-memory quote cache keyed by symbol+provider | FR-016 | S | Medium | completed | T-001 |
| T-005 | Implement `finance::rate_limit` — per-provider cooldown state, 429 backoff with 30s cap, jitter | FR-012, FR-015 | S | High | completed | T-002 |
| T-006 | Implement `finance::providers::yahoo` — `yfinance_rs` wrapper for quotes, history, fundamentals, FX, search, options | FR-001, FR-002, FR-003, FR-004, FR-005, FR-006, FR-007, FR-009 | L | Critical | completed | T-003, T-005 |
| T-007 | Implement `finance::providers::paid` — paid-provider router that disables the free adapter, with Alpha Vantage as the first concrete backend | FR-010, FR-011, FR-014 | L | High | completed | T-003 |
| T-008 | Implement `stock_quote` tool — schema, execute, normalize, render | FR-001, NFR-001 | S | Critical | completed | T-006 |
| T-009 | Implement `stock_history` tool — schema, execute, normalize, render | FR-002, NFR-001 | S | Critical | completed | T-006 |
| T-010 | Implement `stock_fundamentals` tool — schema, execute, normalize, render | FR-003, NFR-001 | S | Critical | completed | T-006 |
| T-011 | Implement `currency_rate` tool — schema, execute, normalize, render | FR-004, NFR-001 | S | Critical | completed | T-006 |
| T-012 | Implement `currency_history` tool — schema, execute, normalize, render | FR-005, NFR-001 | S | Critical | completed | T-006 |
| T-013 | Implement `stock_search` tool — schema, execute, normalize, render | FR-006, NFR-001 | S | Medium | completed | T-006 |
| T-014 | Implement `stock_options` tool — schema, execute, normalize, render | FR-007, NFR-001 | M | Medium | completed | T-006 |
| T-015 | Register all 6 tools in `create_extended_registry()` and add `pub mod finance;` to `lib.rs` | NFR-001 | S | Critical | completed | T-008–T-014 |
| T-016 | Add `finance` tool-visibility switch to `ToolVisibilityConfig`, raw config, `iter_switches()`, `tool_family_names()`, default `true`, serialize/merge | NFR-002 | M | High | completed | T-015 |
| T-017 | Add optional paid-provider config block to `ragent_config::Config` (provider name, api_key, base_url, rate-limit settings) | FR-010, FR-011, FR-014 | M | High | completed | T-003 |
| T-018 | Write unit tests for `finance::cache` TTL and eviction behavior | FR-016, NFR-003 | S | Medium | completed | T-004 |
| T-019 | Write unit tests for `finance::rate_limit` backoff and cooldown behavior | FR-012, FR-015, NFR-003 | S | High | completed | T-005 |
| T-020 | Write unit tests for `finance::providers::yahoo` normalization and `yfinance_rs` response mapping | FR-001–FR-007, NFR-003 | M | High | completed | T-006 |
| T-021 | Write unit tests for paid-provider routing logic (default vs configured, free adapter disabled) | FR-009, FR-010, FR-011, FR-014, NFR-003 | S | High | completed | T-007 |
| T-022 | Write integration test verifying all 6 finance tools are registered and report `network:fetch` permission category | NFR-001 | S | Critical | completed | T-015 |
| T-023 | Write integration test verifying `finance` visibility switch hides/shows all 6 tools via `effective_hidden_tools()` | NFR-002 | S | High | completed | T-016 |
| T-024 | Run `cargo test -p ragent-tools-extended`, `cargo test -p ragent-config`, `cargo clippy`, and `cargo fmt --check` to confirm no regressions | NFR-001, NFR-004, NFR-005 | S | Critical | completed | T-018–T-023 |
## Task Detail

### T-001 — Shared types

Define provider-agnostic types in `finance/model.rs`:

```rust
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

pub struct OhlcvBar {
    pub timestamp: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

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

pub struct CurrencyRate {
    pub base: String,
    pub quote: String,
    pub rate: f64,
    pub timestamp: DateTime<Utc>,
}

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

pub struct SearchResult {
    pub symbol: String,
    pub name: Option<String>,
    pub exchange: Option<String>,
    pub asset_class: Option<String>,
}
```

### T-003 — Provider trait

```rust
#[async_trait]
pub trait FinanceProvider: Send + Sync {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
    async fn quote(&self, symbol: &str) -> Result<Quote>;
    async fn history(&self, symbol: &str, interval: &str, period: &str) -> Result<Vec<OhlcvBar>>;
    async fn fundamentals(&self, symbol: &str) -> Result<Fundamentals>;
    async fn currency_rate(&self, base: &str, quote: &str) -> Result<CurrencyRate>;
    async fn currency_history(&self, base: &str, quote: &str, interval: &str, period: &str) -> Result<Vec<OhlcvBar>>;
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>>;
    async fn options(&self, symbol: &str, expiration: Option<&str>) -> Result<Vec<OptionContract>>;
}
```

### T-006 — Yahoo adapter

Use the `yfinance_rs` crate as the default provider. The adapter converts
`yfinance_rs` types (e.g., `Ticker`, `Quote`, historical bar data) into the
normalized `finance::model` types. Use `finance::rate_limit` for 429 handling
and `finance::cache` for quote caching.

### T-007 — Paid adapter

Implement a configurable `PaidProvider` that routes to a selected paid backend.
When a paid provider is configured, `default_provider()` must return the paid
provider and must not use the free Yahoo adapter for the same data category
(FR-014). The first concrete backend is Alpha Vantage for quotes, history, and
FX. Keep the interface identical so adding Polygon/Massive, Finnhub, or
Twelve Data later requires only a new backend module.

### T-016 — Visibility switch

Mirror the existing `office` / `github` switch pattern in
`crates/ragent-config/src/config.rs`. Add `finance: bool` (default `true`) and a
`finance` arm to `tool_family_names()` listing all 6 tool names.

### T-017 — Paid-provider configuration

Add an optional `finance` object to `Config`:

```rust
pub struct FinanceProviderConfig {
    pub provider: String,      // "yahoo" or "alpha_vantage"
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub requests_per_minute: Option<u32>,
}
```

Deserialize from `ragent.json` under a `finance` key. When `provider` is not
`yahoo` and `api_key` is present, route all finance data requests to the paid
backend and disable the free adapter.

## Risks

| Risk | Mitigation |
|------|------------|
| `yfinance_rs` crate API differs from Python `yfinance` | Pin to a known version, pin sample JSON fixtures in tests, and keep normalization in the adapter |
| Alpha Vantage free tier is 25 requests/day | Document the limit and allow multiple API keys |
| Provider routing config confusion | Default to `yahoo`; only use paid adapter when `provider` is explicitly non-yahoo and `api_key` is present |