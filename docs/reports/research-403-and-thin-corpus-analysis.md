# Research System: 403 Fetch Failures and Thin Topic Corpus

## Summary

Research runs currently produce many HTTP 403 fetch failures and very little usable topic content. The root cause is not broken accumulation logic, but **aggressive concurrent page fetching (10 in parallel) with no retry/backoff and a static, easily-blocked user agent**, combined with a **search-result cap of 20 hiding behind a request for 100 results**. Recent changes switched the gatherer from sequential to concurrent fetching without adding retries, which amplified the 403 problem.

The final `RESEARCH.md` is thin because the source corpus is mostly composed of failed fetches, truncated bodies, and low-quality `html2text` fallbacks rather than useful article text.

---

## 1. Web Gatherer: No Retries and High Concurrency

**File:** `crates/ragent-research/src/web_gatherer.rs`

- **Line 63:** `DEFAULT_FETCH_CONCURRENCY = 10` — up to 10 page fetches run in parallel by default.
- **Lines 711–757:** The fetch loop uses `buffer_unordered(fetch_concurrency)`. On any error it emits `GatherEvent::FetchFailed`, logs a warning, and pushes `(index, None)` into `collected`.
- **Line 763:** `collected.into_iter().filter_map(|(_, src)| src)` silently drops every failed fetch.

**Problem:** There is no retry, no exponential backoff, and no distinction between transient errors (429, 503) and hard blocks (403, 404). A burst of 10 concurrent requests with the `ragent/0.1` user agent is the exact pattern that Cloudflare, news sites, and publishers rate-limit or block.

---

## 2. `webfetch` Tool Treats 403/429 as Fatal

**File:** `crates/ragent-tools-extended/src/webfetch.rs`

- **Line 19:** `USER_AGENT = "ragent/0.1 (https://github.com/thawkins/ragent)"` — a single static user agent.
- **Lines 136–143:** Any non-success status immediately `bail!`:
  ```rust
  bail!("HTTP {} fetching {}: {}", status, url, ...);
  ```
  403 and 429 are treated the same as 404 or 500: immediate failure with no retry.
- **Lines 153–164:** On success, `readability-rs` extracts article text. If the extracted text is shorter than `MIN_READABILITY_TEXT_LEN = 500` (**line 41**), the tool falls back to `html2text`, which often produces noisy text for interstitial/paywall pages.

**Problem:** Many real pages are rejected at the HTTP layer, and pages that return 200 but contain thin/interstitial text are kept as low-quality sources.

---

## 3. Requested Result Budget Mismatch

**File:** `crates/ragent-tools-extended/src/websearch.rs`

- **Line 18:** `MAX_NUM_RESULTS = 20` — hard cap per Tavily call.
- **Lines 74–77:** `num_results` is clamped to `MAX_NUM_RESULTS`.

**File:** `crates/ragent-research/src/session.rs`

- **Line 116:** `max_web_results: DEFAULT_MAX_WEB_RESULTS` where `DEFAULT_MAX_WEB_RESULTS = 100` (`web_gatherer.rs:55`).

**Problem:** The session asks for up to 100 results, but each sub-query can return at most 20. The gatherer still builds a 100-slot fetch queue, so the concurrency budget is wasted and the perceived yield is low.

---

## 4. Accumulation Logic Is Intact But Fragile

**File:** `crates/ragent-research/src/session.rs`

- **Lines 1006–1012:** Web phase calls `gather_with_observer(&topic, config.max_web_results, ...)`.
- **Lines 1019–1023:** On success it extends `web_queries` and `sources`; on error it emits `WebSearchFailed` and continues with an empty web corpus.
- **Lines 1093–1107:** Spec phase only re-emits `Source::Spec` entries already added during local gathering. Because `disable_specs: true` is the default (**line 119**) and the local gatherer receives `skip_specs: config.disable_specs` (**line 1047**), no spec sources are gathered unless the user passes `--use-specs`.

**Problem:** Sources are not being dropped by the accumulator; the accumulator receives very few usable sources to begin with.

---

## 5. TUI Progress Rendering: Failures Are Visible, Quality Is Not

**File:** `crates/ragent-tui/src/research_progress.rs`

- **Lines 87–105:** `ResearchProgress::apply` appends each `SourceCaptured` and `WebFetchFailed` as a separate log line (it only updates in place for `Started → Done` transitions).
- **Lines 194–198:** `WebCaptured` is rendered as `captured {url} — {title}` with no body-length or quality metric.
- **Lines 188–193:** `WebFetchFailed` is rendered as `fetch failed for {url}: {error}`.

**File:** `crates/ragent-tui/src/app/event_handler.rs`

- **Lines 637–675:** Research events are decoded and pushed to the log panel. Errors set the status bar to a warning.

**Problem:** The TUI shows every 403 failure prominently but gives no signal about *why* the final document is thin. It does not display body size, readability fallback usage, or the success/failure ratio.

---

## 6. Recent Git Diff Changes Did Not Break Accumulation

The recent diffs are:

- `web_gatherer.rs`: Added `DEFAULT_FETCH_CONCURRENCY` and switched to `buffer_unordered` concurrent fetching (previously sequential).
- `session.rs`: Added `fetch_concurrency` to `SessionConfig`, `FromUrlBodyPreview` event, and topic-derivation helpers.
- `engine.rs`: Added forwarding of `QueriesDecomposed` and `SourceCaptured` events.
- `research_progress.rs` / `app/research.rs`: Added per-run progress trackers and `FromUrlBodyPreview` handling.

None of these drop sources. The regression in perceived quality comes from switching to 10-way concurrent fetches **without adding retries or backoff**.

---

## Recommended Improvements

| Priority | File(s) | Recommended Change |
|---|---|---|
| High | `crates/ragent-research/src/web_gatherer.rs:711–757` | Add retry loop with exponential backoff for transient HTTP errors. Retry 429/503, optionally retry 403 once after a short delay. |
| High | `crates/ragent-tools-extended/src/webfetch.rs:136–143` | Move 403/429 handling into a retry/backoff path instead of immediate `bail!`. |
| Medium | `crates/ragent-tools-extended/src/webfetch.rs:19` | Rotate or randomize user agents and add common browser headers to reduce bot blocking. |
| Medium | `crates/ragent-research/src/session.rs:116` | Cap `max_web_results` to the actual search-provider maximum (20 for Tavily), or issue paginated searches if supported. |
| Medium | `crates/ragent-research/src/web_gatherer.rs:720–744` | Drop captured sources with empty or very short bodies so empty 200 responses do not pollute the corpus. |
| Medium | `crates/ragent-research/src/session.rs:1002–1032` | Emit a stronger warning or short-circuit when web gathering returns zero sources. |
| Low | `crates/ragent-tui/src/research_progress.rs:194–198` | Include body length in `WebCaptured` progress detail. |
| Low | `crates/ragent-tui/src/app/event_handler.rs:637–675` | Summarize the web phase with a `X captured / Y failed / Z skipped` tally. |

---

## Bottom Line

The 403 storm and thin corpus are caused by **burst concurrent fetching with no retry/backoff and a static user agent**, compounded by a **search-result cap of 20 behind a request for 100**. The source accumulation logic is intact, but it is accumulating mostly failures and truncated bodies. Adding retries, backoff, user-agent rotation, and body-quality filtering will meaningfully increase the usable corpus.
