---
status: draft
audit:
  - { time: 1787099746, from: "none", to: "draft", actor: "system" }
---
# Specification: yfinance — Stocks and Currency Toolset

## Overview

This specification defines a Yahoo-Finance-derived stocks and currency toolset
for ragent. The default implementation exposes the free, unauthenticated data
surfaces that the Python `yfinance` library consumes (Yahoo Finance public
endpoints for stock quotes, historical bars, fundamentals, options chains, and
FX/currency pairs). The design is provider-pluggable so that a paid Yahoo
Finance API subscription or an alternative licensed provider can be swapped in
without changing the tool schemas that agents depend on.

The ragent implementation should use the rust yfinance_rs crate that provides the same api as the python library:

The tools are native Rust implementations in the `ragent-tools-extended` crate.
They share a provider-agnostic core (data model, normalization, caching, rate
limiting) and a Yahoo-free adapter that is the default provider.

## Background

### Yahoo Finance as the default provider

The `financeapi` research artifact found that Yahoo Finance has no official API,
but the community `yfinance` Python package reverse-engineers public endpoints and
delivers broad global stock, ETF, mutual fund, crypto, and FX data, 20+ years of
daily history, dividends/splits, options chains, and fundamental data without an
API key. It is widely used for personal research and learning but is legally and
reliability-constrained: Yahoo's Terms of Service prohibit automated scraping, the
endpoints change without notice, there is no SLA, and rate limits are opaque.

This specification treats `yfinance` as a convenient, free, default data source for
agentic research and prototyping, while explicitly supporting paid/official
providers for production use.

### Related Research

This spec was informed by [`financeapi`](../research/financeapi/RESEARCH.md).

## Goals

1. Give agents a small, predictable set of tools for stock and currency research.
2. Default to free Yahoo Finance endpoints so no API key is required for basic
   use.
3. Allow operators to upgrade to a paid provider or a licensed provider
   (Alpha Vantage, Polygon/Massive, Finnhub, etc.) by configuration only.
4. Keep all schemas provider-agnostic so switching providers does not break
   existing agent prompts.

## Non-Goals

- This feature is read-only. It does **not** submit orders, manage portfolios, or
  execute trades.
- It is **not** a real-time trading feed. Data is delayed or historical.
- It does **not** guarantee investment-grade accuracy or legal clearance for
  commercial redistribution.

## Requirements

### Functional Requirements

**FR-001** — The system shall provide a `stock_quote` tool that, given a ticker
symbol, returns the latest available price, open, high, low, close, volume,
change, and percent change.

**FR-002** — The system shall provide a `stock_history` tool that, given a ticker
symbol, an interval (`1d`, `1wk`, `1mo`), and an optional period (`1mo`, `3mo`,
`6mo`, `1y`, `5y`, `max`), returns a time series of OHLCV bars.

**FR-003** — The system shall provide a `stock_fundamentals` tool that, given a
symbol, returns key fundamental metrics such as market cap, trailing P/E, forward
P/E, EPS, dividend yield, fifty-two week high/low, and sector.

**FR-004** — The system shall provide a `currency_rate` tool that, given a source
currency code and target currency code, returns the current mid-market exchange
rate.

**FR-005** — The system shall provide a `currency_history` tool that, given two
currency codes, an interval, and an optional period, returns historical exchange
rates.

**FR-006** — The system shall provide a `stock_search` tool that, given a company
name or fragment, returns candidate ticker symbols, exchange names, and asset
classes.

**FR-007** — The system shall provide a `stock_options` tool that, given a ticker
symbol and an optional expiration date, returns the options chain (calls and
puts) with strike, last price, bid, ask, volume, and open interest.

**FR-008** — The system shall normalize all provider responses into a common
internal schema so tool outputs are identical regardless of the backing
provider.

**FR-009** — When the user has not configured a provider, the system shall use
the Yahoo Finance free endpoint adapter.

**FR-010** — When the user provides a paid-provider API key and configuration,
the system shall route requests to that provider instead of the free Yahoo
adapter.

**FR-011** — The system shall support at least one paid provider out of the box
in addition to the free Yahoo adapter.

### Event-Driven Requirements

**FR-012** — When a provider request returns a 429 status, the system shall
apply exponential backoff with a maximum total wait of 30 seconds before
returning a rate-limit error.

**FR-013** — When a provider request fails because the symbol is not found, the
system shall return a clear "symbol not found" error and shall not retry.

### State-Driven Requirements

**FR-014** — While a paid provider is configured, the system shall disable the
free adapter for the same data category.

**FR-015** — While the tool is in rate-limit cooldown for a provider, the system
shall reject additional requests for that provider with a cooldown error.

### Optional Requirements

**FR-016** — The system may cache quote responses for up to 60 seconds to
reduce repeated calls for the same symbol.

**FR-017** — The system may expose a `stock_news` tool that returns recent
headlines related to a symbol.

**FR-018** — The paid provider adapter may expose WebSocket streaming for quotes
if the provider supports it.

### Unwanted Requirements

**FR-019** — The system shall not store user portfolio holdings, balances, or
trading positions.

**FR-020** — The system shall not send orders or execute trades on behalf of
the user.

### Non-Functional Requirements

**NFR-001** — The tools shall use the existing ragent `Tool` trait and register
through `ragent-tools-extended::create_extended_registry()`.

**NFR-002** — The tools shall be hidden behind a `finance` visibility switch in
`ragent_config::ToolVisibilityConfig` that defaults to `true`.

**NFR-003** — The implementation shall include unit tests for normalization
logic, provider fallback logic, and rate-limit behavior.

**NFR-004** — The implementation shall not introduce `unwrap()` on user-facing
paths.

**NFR-005** — The implementation shall use `tracing` for structured logging at
`debug` and `warn` levels.

## Constraints

- Free Yahoo endpoints are reverse-engineered and may break without warning.
- Paid providers require operator-provided API keys and may impose separate
  exchange-license terms.
- All data is delayed unless the paid provider explicitly advertises real-time
  entitlements.

## Risks

| Risk | Mitigation |
|------|------------|
| Yahoo endpoint changes break the free adapter | Maintain a provider abstraction and document fallback configuration |
| Yahoo rate limits are opaque | Share a single `YfClient` across tools, add jittered backoff, a 60s quote cache, an optional `requests_per_minute` throttle, a configurable `user_agent`, and clear rate-limit errors |
| Paid provider schema drift | Normalize responses in the provider adapter; update only the adapter |
| Legal ambiguity around Yahoo scraping | Default to free but clearly label paid/licensed providers for production |
| UI clutter from new tools | Gate all tools behind a `finance` visibility switch |

## Operational Notes

The free Yahoo adapter shares one `YfClient` instance across all finance tool
invocations. This keeps cookie/crumb authentication, in-memory caching, and
rate-limit backoff state in one place and avoids a burst of fresh HTTP clients
when an agent calls several finance tools in quick succession.

The `finance` configuration block accepts optional provider tuning:

- `user_agent` — a custom `User-Agent` header sent to Yahoo Finance endpoints.
- `requests_per_minute` — a per-minute throttle applied to Yahoo Finance
  requests (paid providers are not throttled by this setting).

Example:

```jsonc
{
  "tool_visibility": { "finance": true },
  "finance": {
    "provider": "yahoo",
    "user_agent": "Mozilla/5.0 (compatible; MyRagentBot/1.0)",
    "requests_per_minute": 30
  }
}
```

## Future Work

- Add additional paid providers (Alpha Vantage, Polygon/Massive, Finnhub,
  Twelve Data).
- Add a `stock_screener` tool with filters for sector, market cap, P/E, and
  dividend yield.
- Add a `stock_dividends` tool for historical dividend and split events.
