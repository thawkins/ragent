---
status: draft
---

# Manual Test Plan: yfinance — Stocks and Currency Toolset

## Scope

This document describes the manual tests to verify the `yfinance` stocks and
currency toolset in ragent. It covers the six tools, the free Yahoo Finance
default provider (via the `yfinance_rs` crate), the optional paid-provider
configuration, rate-limit behavior, and tool visibility.

## Prerequisites

1. Build ragent from source:
   - `cargo build --release`
2. Ensure `ragent` is on `PATH` or use the full path to `target/release/ragent`.
3. Have an internet connection. The free Yahoo adapter calls public endpoints.
4. (Optional) Obtain an Alpha Vantage API key from
   <https://www.alphavantage.co/support/#api-key> to test paid-provider routing.
5. Ensure no previous `ragent.json` blocks tool visibility. If `finance` is set
   disabled, re-enable it:
   ```json
   { "tool_visibility": { "finance": true } }
   ```

## Test Cases

### TC-001 — Tool visibility defaults to enabled

**Title:** Finance tools appear in the default tool list

**Preconditions:**
- ragent is built.
- No `ragent.json` disables the `finance` family.

**Steps:**
1. Start ragent:
   ```bash
   ragent --no-tui --log-level debug
   ```
2. At the prompt, type:
   ```text
   /tools
   ```
3. Press `Enter`.

**Test data:** None.

**Expected results:**
- The response lists the six finance tools: `stock_quote`, `stock_history`,
  `stock_fundamentals`, `currency_rate`, `currency_history`, `stock_search`, and
  `stock_options`.

### TC-002 — stock_quote returns data for a known symbol

**Title:** Retrieve a current stock quote via Yahoo Finance

**Preconditions:**
- ragent is running and finance tools are enabled.
- Internet access is available.

**Steps:**
1. In the ragent input box, type:
   ```text
   Use stock_quote to fetch the latest quote for AAPL
   ```
2. Press `Enter`.

**Test data:** Symbol = `AAPL`.

**Expected results:**
- The tool call is shown in the log panel as step 1.
- The result contains fields: `symbol`, `price`, `open`, `high`, `low`, `close`,
  `volume`, `change`, `change_percent`, `currency`, and `market_state`.
- All numeric fields are non-zero and `currency` is `USD`.

### TC-003 — stock_history returns daily bars

**Title:** Retrieve one month of daily history for a known symbol

**Preconditions:**
- ragent is running and finance tools are enabled.
- Internet access is available.

**Steps:**
1. In the ragent input box, type:
   ```text
   Use stock_history for MSFT with interval 1d and period 1mo
   ```
2. Press `Enter`.

**Test data:** Symbol = `MSFT`, interval = `1d`, period = `1mo`.

**Expected results:**
- The result contains at least 15 daily OHLCV bars.
- Each bar has `timestamp`, `open`, `high`, `low`, `close`, `volume`.
- Timestamps are in UTC.

### TC-004 — stock_fundamentals returns key metrics

**Title:** Retrieve fundamentals for a large-cap US stock

**Preconditions:**
- ragent is running and finance tools are enabled.
- Internet access is available.

**Steps:**
1. In the ragent input box, type:
   ```text
   Use stock_fundamentals for GOOGL
   ```
2. Press `Enter`.

**Test data:** Symbol = `GOOGL`.

**Expected results:**
- The result contains `symbol`, `name`, `sector`, `market_cap`, `trailing_pe`,
  `forward_pe`, `eps`, `dividend_yield`, `fifty_two_week_high`,
  `fifty_two_week_low`.
- `symbol` matches the input and `name` is non-empty.

### TC-005 — currency_rate returns an FX rate

**Title:** Retrieve the current USD to EUR exchange rate

**Preconditions:**
- ragent is running and finance tools are enabled.
- Internet access is available.

**Steps:**
1. In the ragent input box, type:
   ```text
   Use currency_rate from USD to EUR
   ```
2. Press `Enter`.

**Test data:** Base = `USD`, quote = `EUR`.

**Expected results:**
- The result contains `base`, `quote`, `rate`, and `timestamp`.
- `rate` is a positive number near 0.85–1.15.

### TC-006 — currency_history returns FX history

**Title:** Retrieve one month of daily USD/JPY history

**Preconditions:**
- ragent is running and finance tools are enabled.
- Internet access is available.

**Steps:**
1. In the ragent input box, type:
   ```text
   Use currency_history for USD to JPY with interval 1d and period 1mo
   ```
2. Press `Enter`.

**Test data:** Base = `USD`, quote = `JPY`, interval = `1d`, period = `1mo`.

**Expected results:**
- The result contains at least 15 daily bars.
- Each bar has `timestamp`, `open`, `high`, `low`, `close`, `volume`.

### TC-007 — stock_search resolves a company name to symbols

**Title:** Search for "Tesla" and verify symbols returned

**Preconditions:**
- ragent is running and finance tools are enabled.
- Internet access is available.

**Steps:**
1. In the ragent input box, type:
   ```text
   Use stock_search for Tesla
   ```
2. Press `Enter`.

**Test data:** Query = `Tesla`.

**Expected results:**
- The result contains one or more `SearchResult` entries.
- At least one entry has `symbol` equal to `TSLA`.

### TC-008 — stock_options returns an options chain

**Title:** Retrieve the options chain for a liquid stock

**Preconditions:**
- ragent is running and finance tools are enabled.
- Internet access is available.

**Steps:**
1. In the ragent input box, type:
   ```text
   Use stock_options for SPY
   ```
2. Press `Enter`.

**Test data:** Symbol = `SPY`, no expiration supplied.

**Expected results:**
- The result contains a list of call and put contracts.
- Each contract has `strike`, `expiration`, `kind`, `last_price`, `bid`, `ask`,
  `volume`, `open_interest`.
- At least one `Call` and one `Put` are present.

### TC-009 — Paid-provider routing

**Title:** Configure Alpha Vantage and verify routing for quotes

**Preconditions:**
- ragent is not running.
- A valid Alpha Vantage API key is available.

**Steps:**
1. Open the project root in a terminal.
2. Create or edit `.ragent/ragent.json`:
   ```bash
   mkdir -p .ragent
   ```
   Add the following JSON:
   ```json
   {
     "tool_visibility": { "finance": true },
     "finance": {
       "provider": "alpha_vantage",
       "api_key": "YOUR_ALPHA_VANTAGE_KEY"
     }
   }
   ```
3. Save the file.
4. Start ragent:
   ```bash
   ragent --no-tui --log-level debug
   ```
5. At the prompt, type:
   ```text
   Use stock_quote to fetch IBM
   ```
6. Press `Enter`.

**Test data:** Symbol = `IBM`, API key = your Alpha Vantage key.

**Expected results:**
- The log panel shows the tool is using the paid adapter (a debug message such
  as `finance provider=alpha_vantage`).
- The free Yahoo adapter is not used for this request.
- The quote is returned with `symbol=IBM` and a valid price.

### TC-010 — Rate-limit cooldown handling

**Title:** Rapid repeated requests for the same symbol do not exceed backoff

**Preconditions:**
- ragent is running.
- Finance tools are enabled.

**Steps:**
1. In the ragent input box, type:
   ```text
   Call stock_quote for NVDA three times in a row
   ```
2. Press `Enter`.

**Test data:** Symbol = `NVDA`.

**Expected results:**
- The first call fetches from the provider.
- The second and third calls return cached quote data within 60 seconds if the
  cache is implemented (FR-016).
- If the cache is not implemented, the tool still returns results without a
  429/rate-limit error because of the 30s backoff.

### TC-011 — Tool visibility can be disabled

**Title:** Disabling the finance family hides all finance tools

**Preconditions:**
- ragent is not running.

**Steps:**
1. Create or edit `.ragent/ragent.json`:
   ```json
   { "tool_visibility": { "finance": false } }
   ```
2. Save the file.
3. Start ragent:
   ```bash
   ragent --no-tui --log-level debug
   ```
4. At the prompt, type:
   ```text
   /tools
   ```
5. Press `Enter`.

**Test data:** None.

**Expected results:**
- The list does not contain `stock_quote`, `stock_history`,
  `stock_fundamentals`, `currency_rate`, `currency_history`, `stock_search`, or
  `stock_options`.

### TC-012 — Symbol not found error is clear

**Title:** An invalid symbol returns a readable "not found" message

**Preconditions:**
- ragent is running.
- Finance tools are enabled.
- Internet access is available.

**Steps:**
1. In the ragent input box, type:
   ```text
   Use stock_quote for INVALIDTICKER12345
   ```
2. Press `Enter`.

**Test data:** Symbol = `INVALIDTICKER12345`.

**Expected results:**
- The tool returns an error that contains the phrase "symbol not found" or
  "not found".
- The agent does not crash or retry indefinitely.

## Cleanup

1. If you created `.ragent/ragent.json` only for these tests and do not want to
   keep it, delete or revert the file.
2. Remove any Alpha Vantage API key from the file if it was stored there, or
   rotate the key.
3. Stop ragent by typing `/quit` or pressing `Ctrl+C`.

## Notes

- All numeric assertions should be treated as sanity checks, not investment
  advice.
- Yahoo Finance endpoints are public but reverse-engineered; a field may
  occasionally be missing or `null`. The test should accept missing optional
  fields as long as required fields are present and sensible.
- If a paid-provider test fails due to rate limits, wait 60 seconds and retry,
  or test with a different API key.
