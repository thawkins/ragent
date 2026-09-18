# Tools — Finance

Stock quotes, historical bars, fundamentals, options, recommendations, and
currency rates. Free Yahoo Finance is the default provider; Alpha Vantage can
be configured in `ragent.json`. See `docs/howtos/finance.md` for provider
details.

| Tool | Description |
|------|-------------|
| `stock_quote` | Latest stock quote (price, volume, change). |
| `stock_history` | Historical OHLCV bars. |
| `stock_fundamentals` | Market cap, P/E, EPS, dividend yield, sector. |
| `stock_search` | Search ticker symbols by company name. |
| `stock_options` | Options chain (calls and puts). |
| `stock_recommendations` | Analyst recommendation trends. |
| `currency_rate` | Current exchange rate between two currencies. |
| `currency_history` | Historical exchange-rate OHLCV bars. |

**Visibility switch:** `finance`.

---

## stock_quote

Latest quote for a ticker (price, open, high, low, close, volume, change).

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `symbol` | string | yes | Ticker symbol | `"AAPL"` |

**Example:** `stock_quote symbol="AAPL"`

---

## stock_history

Historical OHLCV bars. Short periods round up to the smallest supported
Yahoo range (1 month).

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `symbol` | string | yes | Ticker symbol | `"MSFT"` |
| `period` | string | no | Lookback period: `1d`, `5d`, `1w`/`1wk`, `1mo`, `3mo`, `6mo`, `1y`, `5y`, `max` | `"3mo"` |
| `interval` | string | no | Candle interval: `1d`, `1wk`, `1mo` | `"1d"` |

**Example:** `stock_history symbol="MSFT" interval="1d" period="3mo"`

---

## stock_fundamentals

Market cap, P/E, EPS, dividend yield, 52-week range, sector.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `symbol` | string | yes | Ticker symbol | `"AAPL"` |

---

## stock_search

Search ticker symbols matching a company or asset name.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `query` | string | yes | Company or asset name | `"Microsoft"` |

---

## stock_options

Fetch the options chain (calls and puts).

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `symbol` | string | yes | Ticker symbol | `"AAPL"` |
| `expiration` | string | no | Expiration date | `"2026-03-20"` |

---

## stock_recommendations

Analyst recommendation trends (strong buy … strong sell counts by period).

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `symbol` | string | yes | Ticker symbol | `"AAPL"` |

---

## currency_rate

Current exchange rate between two currencies.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `base` | string | yes | Source currency code | `"USD"` |
| `quote` | string | yes | Target currency code | `"EUR"` |

**Example:** `currency_rate base="USD" quote="EUR"`

---

## currency_history

Historical exchange-rate OHLCV bars for a currency pair.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `base` / `quote` | string | yes | Currency pair | `"USD"` / `"EUR"` |
| `period` | string | no | Lookback period | `"3mo"` |
| `interval` | string | no | Candle interval: `1d`, `1wk`, `1mo` | `"1d"` |
