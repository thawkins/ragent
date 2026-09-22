# Implementation Plan: MasterFetch — Integrated Web-Access Tools from Hound

**Spec ID:** `masterfetch`
**Spec status:** draft

## Overview

This plan implements the masterfetch specification
(`specs/masterfetch/SPEC.md`): the integration of six Hound web-access tools
into ragent as native Rust tools in the `crates/ragent-tools-extended` crate.
The work involves:

1. A shared HTTP client module with SSRF protection and URL validation.
2. A content extraction chain (`readability-rs` → `html2text` → raw text).
3. A metadata extractor (OpenGraph, JSON-LD, canonical, `<title>`).
4. An outgoing-link classifier (citations/navigation/external/primary_source).
5. A page-type detector (article/docs/list/forum/qa/js_shell/auth_wall/paywall).
6. A freshness and source-authority envelope signal computer.
7. A BM25 query-focused content filter.
8. A URL normaliser and dedup utility.
9. A robots.txt compliance checker (optional, per-request).
10. A SQLite content cache with TTL and size cap.
11. A best-first same-domain crawler with content-adaptive extraction + sitemap.
12. A keyless multi-engine web search (DuckDuckGo + Brave minimum).
13. Six tool structs implementing the `Tool` trait.
14. Registration in `create_extended_registry()`.
15. A `masterfetch` tool-visibility switch in `ragent-config`.
16. Unit and integration tests.

No MCP server, Python, Node.js, or browser engine is involved. The tools run
in HTTP-only mode with graceful degradation for anti-bot-protected pages.

## Architecture

```
crates/ragent-tools-extended/src/
├── masterfetch/
│   ├── mod.rs              # Module root, re-exports, shared types
│   ├── http.rs             # Shared reqwest::Client, USER_AGENT, SSRF guard
│   ├── security.rs         # URL validation, SSRF protection, IP normalisation
│   ├── cache.rs            # SQLite content cache (WAL, TTL, size cap)
│   ├── extractor.rs        # Content extraction chain (readability → html2text → raw)
│   ├── metadata.rs         # OpenGraph + JSON-LD + canonical metadata extraction
│   ├── links.rs            # Outgoing-link classification (citations/nav/external)
│   ├── envelope.rs         # Page-type detection, freshness, source-authority signals
│   ├── focus.rs            # BM25 query-focused content filtering
│   ├── urlnorm.rs          # URL normalisation + dedup + tracking-param stripping
│   ├── robots.rs           # robots.txt fetch + parse + per-domain cache
│   ├── search/
│   │   ├── mod.rs          # Search orchestration: parallel backends, merge, rank
│   │   ├── engine.rs       # SearchEngine trait + RawResult + EngineReport
│   │   ├── duckduckgo.rs   # DuckDuckGo keyless backend (HTML scraping)
│   │   ├── brave.rs        # Brave keyless backend (HTML scraping)
│   │   └── consensus.rs    # Cross-engine consensus boost + relevance scoring
│   ├── crawl/
│   │   ├── mod.rs          # Crawl orchestration: best-first priority queue
│   │   ├── sitemap.rs      # sitemap.xml discovery + parsing
│   │   └── classify.rs     # Content-adaptive page classification + extraction
│   └── tools/
│       ├── mod.rs          # Registers all 6 tools
│       ├── fetch.rs        # mf_fetch
│       ├── crawl_tool.rs   # mf_crawl  (avoids clash with crawl/ module)
│       ├── search_tool.rs  # mf_search (avoids clash with search/ module)
│       ├── screenshot.rs   # mf_screenshot (graceful degradation)
│       ├── cache_clear.rs  # mf_cache_clear
│       └── version.rs      # mf_version
└── lib.rs                  # + masterfetch module, + registration in create_extended_registry()

crates/ragent-config/src/config.rs  # + masterfetch visibility switch
```

### Data flow

```
Agent invokes mf_<tool>
        │
        ▼
Tool::execute(input, ctx)
        │
        ├─► masterfetch::security::validate_url  ──►  SSRF check, scheme check
        ├─► masterfetch::cache::get_cached       ──►  SQLite (WAL)
        │   (cache hit → return cached content + envelope)
        │
        ├─► masterfetch::http::get_with_retry    ──►  reqwest::Client ──►  target URL
        │
        ├─► masterfetch::extractor::extract      ──►  readability-rs → html2text → raw
        ├─► masterfetch::metadata::extract       ──►  OpenGraph + JSON-LD + canonical
        ├─► masterfetch::links::extract_links    ──►  citations / navigation / external
        ├─► masterfetch::envelope::compute       ──►  page_type, freshness, source_type
        ├─► masterfetch::focus::focus_content    ──►  BM25 filtering (if focus= provided)
        │
        ▼
Formatted text report + structured metadata → ToolOutput { content, metadata }
        │
        └─► masterfetch::cache::set_cached  ──►  SQLite (WAL)
```

```
Agent invokes mf_search
        │
        ▼
SearchTool::execute(input, ctx)
        │
        ├─► masterfetch::search::duckduckgo::search  ──┐
        ├─► masterfetch::search::brave::search       ──┤  parallel (tokio::join!)
        │                                               │
        ▼                                               │
masterfetch::search::consensus::merge_and_rank  ◄──────┘
        │  (dedup by normalised URL, consensus boost, relevance scoring)
        ▼
Formatted ranked result list + metadata → ToolOutput { content, metadata }
```

### Search-only data flow

```
Agent invokes mf_crawl
        │
        ▼
CrawlTool::execute(input, ctx)
        │
        ├─► (if sitemap=true) masterfetch::crawl::sitemap::discover  ──►  URL map
        │
        ├─► Best-first priority queue (score by focus + content-likelihood + depth)
        │   │
        │   ▼  (for each URL, up to max_pages)
        │   masterfetch::crawl::classify::classify_and_extract
        │   │   ├─► article/docs → readability-rs main content
        │   │   ├─► list/index  → structured link list
        │   │   └─► js_shell    → content_ok=false, honest report
        │   │
        │   └─► Per-page: content_ok, page_type, summary, status
        │
        ▼
Crawl report (all pages) + metadata → ToolOutput { content, metadata }
```

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Create `masterfetch/` module structure with shared types (`FetchResult`, `SearchResult`, `CrawlPage`, `PageType`, `SourceType`, `EnvelopeSignals`) | FR-001, NFR-004 | M | Critical | completed | — |
| T-002 | Implement `masterfetch::security` — URL validation with SSRF protection (private IPs, localhost, cloud metadata, DNS rebinding, blocked schemes, backslash rejection, alternate IP notation normalisation) | FR-019, NFR-003 | L | Critical | completed | T-001 |
| T-003 | Implement `masterfetch::http` — shared `reqwest::Client` with `User-Agent`, 30s timeout, redirect policy (max 5), gzip/deflate support | FR-025, NFR-002 | S | Critical | completed | T-001 |
| T-004 | Implement `masterfetch::urlnorm` — URL normalisation (lowercase host, strip default ports, strip trailing slashes, strip tracking params) | FR-027, NFR-003 | S | High | completed | T-001 |
| T-005 | Implement `masterfetch::extractor` — content extraction chain: `readability-rs` (primary) → `html2text` (fallback) → raw text (last resort), with `css_selector` narrowing and `format` parameter (markdown/html/text/raw) | FR-002, NFR-003 | M | Critical | completed | T-001 |
| T-006 | Implement `masterfetch::metadata` — OpenGraph meta tag extraction, JSON-LD block parsing, canonical URL, `<title>` fallback, `html lang` attribute | FR-006, NFR-003 | M | High | completed | T-001 |
| T-007 | Implement `masterfetch::links` — outgoing-link classification: citations (main-content area), navigation (nav/header/footer), external (off-domain), `primary_source` hint from canonical/JSON-LD or known primary hosts | FR-007, NFR-003 | M | Medium | completed | T-001 |
| T-008 | Implement `masterfetch::envelope` — page-type detection (article/docs/list/forum/qa/js_shell/auth_wall/paywall/redirect/unknown), source-authority classification (gov/edu/github/docs-site/qa/forum/blog/news/ecommerce/unknown), freshness computation (content_age_days, is_stale) | FR-003, FR-029, FR-030, NFR-003 | L | High | completed | T-006 |
| T-009 | Implement `masterfetch::focus` — BM25 query-focused content filtering: split into blocks, score by BM25 (k1=1.5, b=0.75, always-positive IDF), keep blocks above threshold, preserve preceding heading, fallback to top-N closest blocks | FR-004, NFR-003 | M | High | completed | T-001 |
| T-010 | Implement `masterfetch::robots` — robots.txt fetch + parse with per-domain cache (TTL 3600s), `is_allowed(url)` check | FR-028, NFR-003 | M | Low | completed | T-003 |
| T-011 | Implement `masterfetch::cache` — SQLite WAL-mode content cache keyed by URL+extraction_type+css_selector+pages, TTL expiry, size cap with oldest-eviction, `get_cached` / `set_cached` / `clear_expired` / `clear_all` | FR-018, NFR-002 | L | High | completed | T-004 |
| T-012 | Implement `masterfetch::search::engine` — `SearchEngine` trait, `RawResult` struct, `EngineReport` struct, URL normalisation for dedup | FR-008, NFR-003 | M | Critical | completed | T-004 |
| T-013 | Implement `masterfetch::search::duckduckgo` — DuckDuckGo keyless backend: HTTP scrape of HTML results page, parse results (title, url, snippet), handle rate-limiting (202/429) | FR-008, NFR-003 | M | Critical | completed | T-003, T-012 |
| T-014 | Implement `masterfetch::search::brave` — Brave keyless backend: HTTP scrape of HTML results page, parse results | FR-008, NFR-003 | M | High | completed | T-003, T-012 |
| T-015 | Implement `masterfetch::search::consensus` — merge + dedup by normalised URL, cross-engine consensus boost (distinct index families), relevance scoring (0–1), `fetch_relevance` tier (high/med/low), `related_queries` mining from titles+snippets, `fetch_hint` computation | FR-008, FR-009, NFR-003 | M | High | completed | T-012, T-013, T-014 |
| T-016 | Implement `masterfetch::search::mod` — search orchestration: run all enabled backends in parallel via `futures::join_all`, collect results + engine reports, pass to consensus merge, apply filters (site, exclude_sites, freshness, max_results, page), 5-min search cache | FR-008, FR-009, FR-010, NFR-001 | M | High | completed | T-015 |
| T-017 | Implement `masterfetch::crawl::sitemap` — sitemap.xml discovery (robots.txt Sitemap: directives → conventional /sitemap.xml + /sitemap_index.xml), XML parsing (urlset + sitemapindex), gzip detection, recursion cap, URL list with lastmod | FR-013, NFR-003 | M | Medium | completed | T-003 |
| T-018 | Implement `masterfetch::crawl::classify` — content-adaptive page classification from HTML: detect article/docs (readability), list/index (link density), js_shell (low text + high body bytes), and extract accordingly | FR-012, FR-029, NFR-003 | M | High | completed | T-005, T-008 |
| T-019 | Implement `masterfetch::crawl::mod` — best-first crawl orchestration: priority queue scored by focus relevance + content-likelihood (boost docs/guide/api, penalise login/submit/cart) + shallow depth, URL normalisation + dedup, same-domain scoping, caps (max_pages, max_depth, max_total_chars, deadline_ms), `discover_only` and `crawl_urls` modes | FR-011, FR-013, FR-014, NFR-001 | L | High | completed | T-005, T-011, T-017, T-018 |
| T-020 | Implement `mf_fetch` tool — validate URL (SSRF), check cache, HTTP fetch, content extraction, metadata extraction, link classification (if include_links), envelope signals, focus filtering (if focus), pagination/chunking, cache store, format response as text + metadata | FR-002, FR-003, FR-004, FR-005, FR-006, FR-007, FR-019, FR-022, FR-025, FR-026, FR-028, FR-029, FR-030 | L | Critical | completed | T-002, T-003, T-005, T-006, T-007, T-008, T-009, T-010, T-011 |
| T-021 | Implement `mf_search` tool — validate query, run parallel backends via search orchestration, apply filters, format ranked result list as text, populate metadata with signals | FR-008, FR-009, FR-010, FR-022, FR-023, FR-025, FR-026 | M | Critical | completed | T-016 |
| T-022 | Implement `mf_crawl` tool — validate start URL, sitemap or BFS discovery, best-first crawl with content-adaptive extraction, per-page signals, caps enforcement, format crawl report as text | FR-011, FR-012, FR-013, FR-014, FR-022, FR-025, FR-026, FR-028 | L | High | completed | T-019 |
| T-023 | Implement `mf_screenshot` tool — return honest error: browser engine not available in integrated Rust runtime, suggest `mf_fetch` for text content | FR-015, FR-022, FR-026 | S | Medium | completed | T-001 |
| T-024 | Implement `mf_cache_clear` tool — clear expired or all cache entries, return purged count | FR-016, FR-022, FR-026 | S | Medium | completed | T-011 |
| T-025 | Implement `mf_version` tool — return masterfetch integration version + ragent version + tool set description | FR-017, FR-022, FR-026 | S | Low | completed | T-001 |
| T-026 | Register all six `mf_*` tools in `create_extended_registry()` and add `pub mod masterfetch` to `lib.rs` | FR-020 | S | Critical | completed | T-020–T-025 |
| T-027 | Add `masterfetch` tool-visibility switch to `ToolVisibilityConfig`, `ToolVisibilitySpecified`, `iter_switches()`, `tool_family_names()`, `Default` (true), `Serialize`/`Deserialize`, and `merge()` | FR-021 | M | High | completed | T-026 |
| T-028 | Write unit tests for `masterfetch::security` — every SSRF bypass vector (private IPs, alternate notations, localhost, cloud metadata, DNS rebinding, backslash, blocked schemes, bracketed hosts, IPv4-mapped IPv6) | FR-019, NFR-003 | M | Critical | completed | T-002 |
| T-029 | Write unit tests for `masterfetch::urlnorm` — host lowercasing, port stripping, trailing slash, tracking-param stripping, idempotency | FR-027, NFR-003 | S | High | completed | T-004 |
| T-030 | Write unit tests for `masterfetch::extractor` — readability extraction, html2text fallback, css_selector narrowing, format variants (markdown/html/text/raw), empty/malformed HTML | FR-002, NFR-003 | M | High | completed | T-005 |
| T-031 | Write unit tests for `masterfetch::metadata` — OpenGraph extraction, JSON-LD parsing, canonical URL, `<title>` fallback, missing metadata, both attribute orders | FR-006, NFR-003 | M | High | completed | T-006 |
| T-032 | Write unit tests for `masterfetch::links` — citation classification, navigation classification, external classification, primary_source hint, empty page, malformed HTML | FR-007, NFR-003 | M | Medium | completed | T-007 |
| T-033 | Write unit tests for `masterfetch::envelope` — each page type (article/docs/list/forum/qa/js_shell/auth_wall/paywall), source-type classification, freshness computation (modified > published, future date → -1, stale threshold) | FR-003, FR-029, FR-030, NFR-003 | L | High | completed | T-008 |
| T-034 | Write unit tests for `masterfetch::focus` — BM25 scoring, threshold filtering, heading preservation, fallback to top-N, empty query (no-op), single block (no-op) | FR-004, NFR-003 | M | High | completed | T-009 |
| T-035 | Write unit tests for `masterfetch::cache` — get/set round-trip, TTL expiry, size cap eviction, `clear_expired` vs `clear_all`, WAL mode, bad content not cached | FR-018, NFR-003 | M | High | completed | T-011 |
| T-036 | Write unit tests for `masterfetch::search::consensus` — URL dedup, consensus boost (same URL from multiple engines), relevance scoring, tier derivation, related-query mining, fetch hint | FR-008, FR-009, NFR-003 | M | High | completed | T-015 |
| T-037 | Write integration test verifying all six `mf_*` tools are registered in `create_extended_registry()` | FR-020, NFR-003 | S | Critical | completed | T-026 |
| T-038 | Write integration test verifying `masterfetch` visibility switch hides/shows all six tools via `effective_hidden_tools()` | FR-021, NFR-003 | S | High | completed | T-027 |
| T-039 | Write integration test verifying network tools return `"web"` and cache/version tools return `"system"` from `permission_category()` | FR-022, NFR-003 | S | High | completed | T-026 |
| T-040 | Run `cargo test -p ragent-tools-extended`, `cargo test -p ragent-config`, `cargo clippy`, and `cargo fmt --check` to confirm no regressions | NFR-001, NFR-002 | S | Critical | completed | T-028–T-039 |
## Task detail

### T-001 — Module structure and shared types

Create `crates/ragent-tools-extended/src/masterfetch/mod.rs`. Define the shared
types that mirror Hound's `ResponseModel`, `SearchResult`, and `CrawlPage`:

```rust
pub enum PageType {
    Article, Docs, List, Forum, Qa, JsShell,
    AuthWall, Paywall, Redirect, Image, Json, Unknown,
}

pub enum SourceType {
    VendorDocs, OfficialDocs, News, Blog, Forum, Qa,
    Gov, Edu, Github, DocsSite, Ecommerce, Unknown,
}

pub struct EnvelopeSignals {
    pub page_type: PageType,
    pub source_type: SourceType,
    pub is_official: bool,
    pub content_age_days: i64,   // -1 = no date recoverable
    pub is_stale: bool,
    pub content_ok: bool,
    pub next_action: String,
    pub summary: String,
}

pub struct FetchResult {
    pub url: String,
    pub status: u16,
    pub content: String,
    pub content_type: String,
    pub total_size_bytes: usize,
    pub total_extracted_chars: usize,
    pub is_truncated: bool,
    pub next_offset: usize,
    pub fetcher_used: String,    // "http" or "cache"
    pub cached: bool,
    pub duration_ms: u64,
    pub metadata: PageMetadata,
    pub envelope: EnvelopeSignals,
    pub error: String,
}

pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: String,
    pub position: usize,
    pub relevance_score: f64,
    pub fetch_relevance: String,   // high/med/low
    pub engines_consensus: String,
}
```

Add `pub mod masterfetch;` to `crates/ragent-tools-extended/src/lib.rs`.

### T-002 — Security module

`masterfetch/security.rs` ports Hound's `security.py`. The `validate_url`
function:

1. Rejects empty/non-string URLs and oversized URLs (> 8192 chars).
2. Rejects backslash characters (CVE-2025-0454 parser confusion).
3. Parses the URL and rejects blocked schemes (file, ftp, gopher, data,
   javascript, vbscript, about, chrome).
4. Accepts only http and https schemes.
5. Validates bracketed hosts are valid IPv6 (CVE-2024-11168).
6. Resolves the hostname and checks against private network ranges
   (127.0.0.0/8, 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16,
   0.0.0.0/8, 224.0.0.0/4, 240.0.0.0/4, ::1/128, fc00::/7, fe80::/10).
7. Rejects localhost, metadata.google.internal, 169.254.169.254.
8. Rejects DNS rebinding suffixes (.nip.io, .sslip.io, .xip.io, .nip.name,
   .1u.ms).
9. Normalises alternate IP notations (octal, hex, decimal, short-form) before
   checking.

Uses the `url` crate for parsing and standard library / `std::net::IpAddr` for
IP range checks. No DNS resolution is performed (the check is against the
hostname/IP literal, matching Hound's approach).

### T-003 — HTTP client

`masterfetch/http.rs` builds a shared `reqwest::Client`:

```rust
pub fn build_client(timeout_secs: u64) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .redirect(Policy::limited(5))
        .user_agent(USER_AGENT)
        .gzip(true)
        .deflate(true)
        .build()
}
```

`USER_AGENT` is `"ragent/{version} (masterfetch)"`. The client is shared across
all `mf_*` tools to reuse connection pooling.

### T-005 — Content extractor

`masterfetch/extractor.rs` implements the extraction chain:

1. If `format=raw`, return the HTML body unchanged.
2. If the content-type is HTML and `format != raw`:
   a. Try `readability::extract()` for article text (already used by
      `webfetch`). If text length ≥ 500 chars, use it.
   b. If readability fails or produces short text, fall back to
      `html2text::from_read()` (already in workspace).
   c. If `css_selector` is provided, narrow the HTML before extraction using
      a simple HTML parser (the `scraper` crate if available, or regex-based
      extraction as a fallback).
3. If `format=text`, strip all HTML tags.
4. If `format=html`, return the cleaned HTML (noise tags stripped).

This mirrors Hound's `extractor.py` chain (trafilatura → markdownify → raw
text) but uses ragent's existing extraction crates.

### T-008 — Envelope signals

`masterfetch/envelope.rs` ports Hound's `envelope.py`:

- **Page-type detection**: structural markers in raw HTML (forum markers like
  phpbb/discourse, Q&A markers like stackoverflow/stackexchange, docs markers
  like mkdocs/docusaurus/readthedocs, paywall markers like
  "subscribe to continue"). Error-derived signals (js_shell, auth_wall)
  override structural signals.
- **Source-authority classification**: domain-based, using Hound's
  `_NEWS_DOMAINS`, `_QA_DOMAINS`, `_GITHUB_DOMAINS` sets + gov/edu/docs-site
  prefix detection.
- **Freshness**: parse dates from metadata (modified_time preferred over
  published_time), compute `content_age_days` and `is_stale` (age > 365).
  Returns (-1, false) when no date is recoverable or date is in the future.

### T-009 — BM25 focus filter

`masterfetch/focus.rs` ports Hound's `focus.py`:

- Split markdown into blocks separated by blank lines.
- Tokenise blocks and query into lowercase tokens (len ≥ 2).
- Compute BM25 scores (k1=1.5, b=0.75) with always-positive IDF
  (`log((n - d + 0.5) / (d + 0.5) + 1)`).
- Keep blocks scoring above threshold (1.0).
- Preserve the heading immediately preceding a kept block.
- If nothing clears the threshold, keep the top-5 closest blocks.
- No-op if query is empty, text has ≤ 1 block, or query yields no usable terms.

### T-011 — SQLite cache

`masterfetch/cache.rs` ports Hound's `cache.py`:

- SQLite in WAL mode with a `cache` table keyed by a SHA-256 hash of
  `url|extraction_type|css_selector|pages|source`.
- Columns: `key`, `url`, `extraction_type`, `content`, `status`,
  `fetched_at` (unix timestamp), `ttl`, `content_type`, `total_size_bytes`,
  `envelope` (JSON).
- `get_cached`: returns content if within TTL, else None.
- `set_cached`: inserts/updates, but only if `content_ok=true` (bad content
  never cached).
- Size cap (default 10 000 entries): evict oldest by `fetched_at` when cap
  exceeded.
- `clear_expired`: deletes entries past TTL.
- `clear_all`: deletes all entries.
- Cache directory: `~/.ragent/masterfetch_cache/` (reuses ragent's home
  directory pattern).

If `rusqlite` is not in the workspace, check if `ragent-storage` exposes a
reusable SQLite connection. If not, add `rusqlite` as a dependency (it is a
common, lightweight, pure-Rust SQLite binding).

### T-013 / T-014 — Search backends

Each search backend implements a `SearchEngine` trait:

```rust
#[async_trait]
pub trait SearchEngine: Send + Sync {
    fn name(&self) -> &str;
    fn index_family(&self) -> &str;
    async fn search(&self, query: &str, max_results: usize) -> Result<EngineResult>;
}
```

`EngineResult` contains `results: Vec<RawResult>` and `report: EngineReport`
(ok/blocked/error).

**DuckDuckGo** (`duckduckgo.rs`): fetches `https://html.duckduckgo.com/html/`
with the query as a POST form field, parses the HTML results page with regex
or a simple HTML parser to extract result blocks (`result__a` title+href,
`result__snippet` text). Handles HTTP 202 soft rate-limit.

**Brave** (`brave.rs`): fetches `https://search.brave.com/search?q=...`, parses
the HTML results page. May require a realistic User-Agent and Accept headers
to avoid immediate blocking.

Both backends are keyless and rely on HTML scraping. They are inherently
fragile (search engines change their HTML), so the parsers must be forgiving
and return empty results (not errors) on parse failures.

### T-019 — Crawl orchestration

`masterfetch/crawl/mod.rs` implements the best-first crawl:

1. Normalise the start URL, extract the root domain.
2. If `sitemap=true` or `sitemap="auto"`: call
   `sitemap::discover_sitemap()` to get the URL list. If `discover_only=true`,
   return the URL map only.
3. If `crawl_urls` is provided: fetch only those URLs (second-phase selective
   crawl).
4. Otherwise: BFS/best-first from the start URL.
5. Best-first scoring: `score = focus_relevance + content_likelihood -
   depth_penalty`. Content-likelihood boosts docs/guide/api/reference/article/
   blog paths and penalises login/submit/register/cart/admin paths.
6. For each URL (up to `max_pages`): fetch via the `mf_fetch` HTTP path,
   classify page type, extract content, compute envelope signals.
7. Extract same-domain links from each page, normalise, dedup, add to the
   priority queue.
8. Stop when `max_pages`, `max_depth`, `max_total_chars`, or `deadline_ms` is
   hit. Return partial results with `truncated_by_time` or `truncated_by_budget`
   flag.

### T-026 — Registration

In `create_extended_registry()`, after the existing registrations, add:

```rust
registry.register(Arc::new(masterfetch::tools::fetch::MfFetchTool));
registry.register(Arc::new(masterfetch::tools::crawl_tool::MfCrawlTool));
registry.register(Arc::new(masterfetch::tools::search_tool::MfSearchTool));
registry.register(Arc::new(masterfetch::tools::screenshot::MfScreenshotTool));
registry.register(Arc::new(masterfetch::tools::cache_clear::MfCacheClearTool));
registry.register(Arc::new(masterfetch::tools::version::MfVersionTool));
```

### T-027 — Visibility switch

Add a `masterfetch: bool` field (default `true`) to `ToolVisibilityConfig` and
a `masterfetch: Option<bool>` to the raw deserialiser. Add `"masterfetch"` to
`iter_switches()`. Add a `"masterfetch"` arm to `tool_family_names()` listing
all six `mf_*` names:

```rust
"masterfetch" => Some(&[
    "mf_fetch",
    "mf_crawl",
    "mf_search",
    "mf_screenshot",
    "mf_cache_clear",
    "mf_version",
]),
```

Update `Serialize` (increment count), `Default`, `Deserialize`, and the
`merge()` overlay logic (propagate `specified.masterfetch` flag).

## Risks

| Risk | Mitigation |
|------|------------|
| Search engine HTML scraping is fragile (SERP markup changes) | Parsers are forgiving: return empty results on parse failure, not errors. Backends are circuit-broken. Multiple backends provide redundancy. |
| DuckDuckGo / Brave rate-limit or block the ragent IP | Multiple backends run in parallel; a blocked backend is reported in `engine_blocked` and the others carry. Future: add more backends (mojeek, yandex). |
| No anti-bot bypass means `mf_fetch` fails on Cloudflare-protected sites | Graceful degradation: `content_ok=false` with `next_action` suggesting alternative sources. This is an explicit design constraint (FR-015, out-of-scope). |
| `readability-rs` extraction quality differs from trafilatura | `readability-rs` is already used by ragent's `webfetch` with good results. `html2text` fallback covers edge cases. |
| SQLite dependency (`rusqlite`) may not be in workspace | Check `ragent-storage` for existing SQLite usage. If unavailable, add `rusqlite` (lightweight, widely used, pure-Rust binding). |
| BM25 focus filter performance on very large pages | Block-based processing is O(n) in the number of blocks. Pages are already chunked to 40K chars by `max_content_chars`. |
| `masterfetch` switch default `true` changes existing user config | `true` matches codeindex precedent; tools are hidden only if explicitly disabled. |
| Name collision: `crawl/` module vs `mf_crawl` tool, `search/` module vs `mf_search` tool | Tool files are `crawl_tool.rs` and `search_tool.rs`; modules are `crawl/` and `search/`. |
| robots.txt fetch adds latency to every `respect_robots=true` request | Per-domain cache (TTL 3600s) means at most one robots.txt fetch per domain per hour. Default is `respect_robots=false`. |