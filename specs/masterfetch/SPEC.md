---
status: draft
audit:
  - { time: 1784753451, from: "none", to: "draft", actor: "system" }
---
# Specification: MasterFetch — Integrated Web-Access Tools from Hound

## Overview

[Hound / master-fetch](https://github.com/dondai1234/master-fetch) (PyPI package
`hound-mcp`) is a Python MCP server that gives AI agents full web research
capability from a single local process: fetching any URL with automatic anti-bot
escalation, keyless multi-engine web search, best-first site crawling, PDF
extraction with OCR, and page screenshots. It costs $0, requires no API keys,
and no accounts.

This specification ("**masterfetch**") defines the integration of Hound's six
web-access tools into ragent **as a set of native Rust tools** in the
`ragent-tools-extended` crate — **not** as an external MCP server. The tools will
be re-implemented in Rust using `reqwest` for HTTP transport, `readability-rs`
and `html2text` for content extraction (both already in the workspace), and
ragent's existing `Tool` trait, `ToolRegistry`, and permission/visibility
infrastructure.

The name "masterfetch" is the internal spec/feature identifier. The integrated
tools are named `mf_fetch`, `mf_crawl`, `mf_search`, `mf_screenshot`,
`mf_cache_clear`, and `mf_version` — prefixed `mf_` to distinguish them from
ragent's existing `webfetch`/`websearch` tools while remaining recognisable.

## Background

### Source codebase

The reference implementation lives at `~/Projects/master-fetch` and is
structured as a Python package (`src/master_fetch/`) exposed via an MCP server
(`server.py`). The relevant modules are:

```
src/master_fetch/
├── server.py          # MCP server: 6 tool defs + dispatch, ResponseModel, BulkResponseModel
├── fetcher.py         # HTTP fetcher (primp-based TLS impersonation), Response class
├── browser.py         # Patchright anti-detect browser (stealth engine, CF solver)
├── extractor.py       # Content extraction chain: trafilatura → markdownify → raw text
├── crawl.py           # Best-first same-domain crawl, content-adaptive extraction, sitemap
├── search.py          # Keyless web search: merge, dedup, rank, neural rerank, consensus
├── search_engines.py  # 8 keyless backend adapters (ddg, brave, mojeek, yahoo, yandex, ...)
├── search_metasearch.py  # Vendored ddgs metasearch backbone (MIT, attributed)
├── cache.py           # SQLite (WAL) content cache with TTL, keyed by URL+params
├── envelope.py        # Page-type, freshness, source-authority signals (v10 envelope)
├── focus.py           # BM25 query-focused content filtering (post-cache, no re-fetch)
├── metadata.py        # OpenGraph + JSON-LD + canonical metadata extraction
├── links.py           # Outgoing-link classification (citations/navigation/external)
├── security.py        # SSRF protection, URL/CSS/header/proxy validation, query sanitisation
├── robots.py          # robots.txt compliance with per-domain caching
├── sitemap.py         # sitemap.xml discovery + parsing for one-fetch crawl maps
├── pdf_extractor.py   # PDF → structured markdown (tables, ToC, CID-OCR fallback)
├── ocr.py             # Image-only PDF / image page OCR (pypdfium2 + rapidocr)
├── reddit.py          # Reddit URL rewriting (old.reddit.com, 7× smaller)
├── reranker.py        # Local ONNX neural reranker (ms-marco-MiniLM-L-6-v2)
└── actions.py         # Page interaction (click, fill, press, wait, scroll, wait_selector)
```

### The 6 tools

| Tool (Hound name) | Integrated name | One-liner |
|-------------------|-----------------|-----------|
| `mcp_smart_fetch` | `mf_fetch` | Fetch any URL or PDF. HTTP first, auto-escalates to stealthy browser if blocked. Bulk, PDFs (with OCR + quality score), `css_selector`, `focus`, `actions`, pagination. |
| `mcp_smart_crawl` | `mf_crawl` | Best-first same-domain crawl. Each page as markdown with `content_ok` + `page_type`. `discover_only`, `crawl_urls`, `focus`, sitemap mode, time + token caps. |
| `mcp_smart_search` | `mf_search` | Local keyless web search. 8 backends in parallel, merges + ranks with neural rerank + cross-backend consensus. `relevance_score` + `engines_consensus` per result. |
| `mcp_screenshot` | `mf_screenshot` | Capture a page as an image. For multimodal agents (canvas, image-of-text, visual layout). |
| `cache_clear` | `mf_cache_clear` | Clear the fetch cache. `all=true` wipes everything. |
| `version` | `mf_version` | Installed version + update status. |

### Key Hound design principles to preserve

1. **Every response is actionable** — `content_ok`, `next_action`, `summary`,
   `page_type`, `content_age_days`/`is_stale`, `source_type`/`is_official`,
   `relevance_score`, `fetch_relevance`. Agents branch on structured fields, not
   error text. Hard-blocks (404/bot/auth) return clean errors, not fake content.
2. **Query-focused extraction** — `focus="query"` returns only BM25-relevant
   blocks, cutting context 80%+ on long pages. Post-cache (no re-fetch).
3. **Content-adaptive extraction** — article/docs → main content; list/index →
   structured link list; JS shells → detected and reported honestly.
4. **Smart caching** — SQLite keyed by URL + extraction type + `css_selector` +
   `pages`. Bad content is never cached; size cap evicts oldest.
5. **Honest signals** — `content_ok=False` for JS shells / login walls; quality
   score for PDFs; `engine_blocked` for rate-limited search backends.

### ragent tool architecture

ragent tools implement the `Tool` trait defined in
`crates/ragent-tools-extended/src/lib.rs`:

```rust
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;
    fn permission_category(&self) -> &str;
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput>;
}
```

`ToolOutput` carries `content: String` (the formatted text report) and optional
`metadata: Option<Value>` (structured signals). Tools are registered in
`create_extended_registry()` (same file) and surfaced to agents via
`ToolRegistry::definitions()`. The agent crate's `create_default_registry()`
calls `register_extracted_extended_tools()` which adapts each extended tool with
`ExtractedExtendedToolAdapter`.

Tool visibility is controlled by `ToolVisibilityConfig` in
`crates/ragent-config/src/config.rs` with switches (`office`, `github`,
`gitlab`, `teams`, `agents`, `plan`, `codeindex`). The
`tool_family_names(switch)` function maps a switch name to the list of tool
names it controls. `effective_hidden_tools()` hides any tool whose family
switch is off.

Network tools (`webfetch`, `websearch`, `http_request`) use
`permission_category() -> "web"` and rely on `reqwest` with a shared
`USER_AGENT` constant. The existing `webfetch` already uses `readability-rs`
for article extraction and `html2text` as a fallback.

### Design constraints

1. **No MCP dependency.** Tools are compiled into the ragent binary. No `pip`,
   no Python, no Node.js, no stdio transport, no `mcp` package.
2. **No API keys for search.** Hound's search is keyless and local — it scrapes
     public search engines. The integrated `mf_search` must preserve this
     zero-config promise. (ragent's existing `websearch` uses Tavily + API key;
     `mf_search` is a keyless alternative.)
3. **Single binary.** All HTTP fetch, content extraction, crawl, search, and
     cache logic must be pure Rust with no runtime subprocess.
4. **Reuse ragent infra.** Tools use the existing `Tool` trait, `ToolRegistry`,
     `ToolContext`, `reqwest` workspace dependency, `readability-rs`,
     `html2text`, `url`, and permission/visibility system.
5. **Graceful degradation.** Hound's browser tier (Patchright/Playwright) cannot
     be ported to Rust. The integrated tools must run in HTTP-only mode and
     degrade gracefully — anti-bot-protected pages return honest
     `content_ok=false` with actionable `next_action` guidance rather than
     silently failing.
6. **No new heavy dependencies.** Prefer workspace crates already present
     (`reqwest`, `serde`, `serde_json`, `regex`, `html2text`, `readability-rs`,
     `url`, `chrono`, `tokio`, `async-trait`, `tracing`). The SQLite cache can
     reuse `ragent-storage` infrastructure or a lightweight `rusqlite` dependency
     if not already available.

## Requirements

### FR-001 — Native Rust re-implementation (ubiquitous)

The system **shall** provide all six Hound web-access tools (`mf_fetch`,
`mf_crawl`, `mf_search`, `mf_screenshot`, `mf_cache_clear`, `mf_version`) as
native Rust structs implementing the `ragent_tools_extended::Tool` trait.

> *Ubiquitous requirement — applies to every tool in the masterfetch set
> without exception.*

### FR-002 — `mf_fetch` URL fetching with content extraction (ubiquitous)

The `mf_fetch` tool **shall** accept a `url` parameter (and optionally a
`urls` array for parallel bulk fetch), fetch the page via HTTP GET using
`reqwest`, and extract the main content as markdown using `readability-rs`
with `html2text` as fallback — mirroring ragent's existing `webfetch`
extraction chain. The tool **shall** support an optional `css_selector`
parameter to narrow extraction scope, and an optional `format` parameter
(`markdown` | `html` | `text` | `raw`).

> *Ubiquitous requirement — `mf_fetch` is the primary tool and every agent
> web-research workflow starts with it.*

### FR-003 — `mf_fetch` response envelope signals (ubiquitous)

The `mf_fetch` tool **shall** return a `ToolOutput` whose `metadata` contains
the Hound v10 envelope signals: `content_ok` (bool), `page_type` (string),
`next_action` (string), `summary` (string), `source_type` (string),
`is_official` (bool), `content_age_days` (int), `is_stale` (bool),
`fetcher_used` (string), `status` (int), `is_truncated` (bool),
`next_offset` (int), and `url` (string, final after redirects). The `content`
field **shall** be the extracted text.

> *Ubiquitous requirement — the envelope is what makes every response
> actionable; it is computed for every fetch without exception.*

### FR-004 — Query-focused extraction (event-driven)

When the `mf_fetch` tool receives a `focus` parameter (a non-empty query
string), the system **shall** filter the extracted content to return only the
BM25-relevant blocks (paragraphs, headings, tables, lists) after extraction,
reducing context on long pages. The filtering **shall** operate post-extraction
on the already-fetched content so it never triggers a re-fetch. If no blocks
clear the relevance threshold, the closest blocks **shall** be returned so the
agent receives something to judge rather than an empty page.

> *Event-driven requirement — triggers when the agent supplies a `focus`
> query.*

### FR-005 — Pagination and content chunking (state-driven)

When the extracted content exceeds `max_content_chars` (default 40 000
characters, minimum 500), the `mf_fetch` tool **shall** truncate the content,
set `is_truncated=true`, and populate `next_offset` with the character offset
to resume from on the next call. The agent **shall** be able to pass
`offset=next_offset` to retrieve the next chunk without re-fetching.

> *State-driven requirement — triggers when the content length exceeds the
> configured maximum.*

### FR-006 — Metadata extraction (ubiquitous)

The `mf_fetch` tool **shall** extract structured metadata from the page's
OpenGraph meta tags, JSON-LD blocks, canonical link, and `<title>` tag. The
extracted metadata **shall** include: `title`, `description`, `site_name`,
`type`, `image`, `canonical`, `lang`, `published_time`, `modified_time`, and
`author`. This metadata **shall** be included in the `ToolOutput.metadata`
field under a `metadata` key.

> *Ubiquitous requirement — metadata is extracted for every HTML page to
> support citation and relevance judgement.*

### FR-007 — Outgoing link classification (optional)

When the `mf_fetch` tool receives an `include_links` parameter set to `true`,
the system **shall** classify the page's outgoing links into `citations`
(links inside main-content area), `navigation` (links inside nav/header/footer),
and `external` (off-domain links), and populate `metadata.links` with the
classified lists plus a `primary_source` hint derived from canonical/JSON-LD
metadata or a citation pointing at a known primary host.

> *Optional requirement — only populated when the agent explicitly requests
> links.*

### FR-008 — `mf_search` keyless multi-engine search (ubiquitous)

The `mf_search` tool **shall** accept a `query` parameter and perform a
keyless web search by querying multiple public search-engine backends in
parallel (at minimum DuckDuckGo and Brave; additional backends are
optional), merging, deduplicating by normalised URL, and ranking results. The
tool **shall** return results as a formatted text list with per-result
`relevance_score`, `fetch_relevance` (high/med/low), `engines_consensus`, and
`url` — not page content.

> *Ubiquitous requirement — `mf_search` always runs the multi-backend
> parallel query pipeline.*

### FR-009 — `mf_search` response signals (ubiquitous)

The `mf_search` tool **shall** return a `ToolOutput` whose `metadata` contains:
`query`, `results` (array of `{title, url, snippet, source, position,
relevance_score, fetch_relevance, engines_consensus}`), `total_results`,
`engines_used`, `engine_blocked`, `rerank_mode`, `cached`, `duration_ms`,
`fetch_hint`, `related_queries`, `summary`, `next_action`, and `error`. The
`content` field **shall** be a human-readable ranked result list.

> *Ubiquitous requirement — the search response always carries structured
> signals so the agent can branch.*

### FR-010 — `mf_search` filters (optional)

When the `mf_search` tool receives any of the optional filter parameters
(`site`, `exclude_sites`, `freshness`, `max_results`, `page`), the system
**shall** apply them to the search results: `site` restricts to one domain,
`exclude_sites` removes specified domains, `freshness` (day|week|month|year)
filters by time, `max_results` (1–50, default 6) caps the result count, and
`page` (0–10) paginates.

> *Optional requirement — filters are only applied when the agent supplies
> them.*

### FR-011 — `mf_crawl` best-first same-domain crawl (event-driven)

When the `mf_crawl` tool is invoked with a `url` parameter, the system
**shall** perform a best-first same-domain crawl starting from that URL. The
crawl **shall** score discovered URLs by focus relevance + content-likelihood
(docs/guide/api boosted, login/submit/cart penalised) + shallow depth, fetch
each page via the `mf_fetch` HTTP path, extract content, and return each page
as markdown with per-page `content_ok`, `page_type`, `summary`, and `status`.
The crawl **shall** respect caps: `max_pages` (default 10), `max_depth`
(default 2), `max_total_chars` (token budget), and `deadline_ms` (default
120 000 ms).

> *Event-driven requirement — triggers when the agent invokes `mf_crawl` with
> a start URL.*

### FR-012 — `mf_crawl` content-adaptive extraction (state-driven)

When the `mf_crawl` tool extracts a page, the system **shall** classify the
page type from its HTML and extract accordingly: article/docs pages → main
content via `readability-rs`; list/index pages → a structured
`* [title](url)` link list; JS shells → detected and reported honestly with
`content_ok=false`. The `page_type` field **shall** reflect the classification
(article, list, js_shell, fallback).

> *State-driven requirement — triggers based on the detected page type of
> each crawled page.*

### FR-013 — `mf_crawl` sitemap mode (optional)

When the `mf_crawl` tool receives a `sitemap` parameter set to `true` or
`"auto"`, the system **shall** attempt to discover and parse the site's
`sitemap.xml` in a single fetch, returning the full URL list with `lastmod`
dates without BFS crawling. When `sitemap="auto"`, the system **shall** use
the sitemap if present and fall back to BFS if not. When
`discover_only=true`, the system **shall** return the URL map only without
fetching page content.

> *Optional requirement — only activates when the agent requests sitemap or
> discover-only mode.*

### FR-014 — `mf_crawl` selective crawl (optional)

When the `mf_crawl` tool receives a `crawl_urls` parameter (an array of
URLs), the system **shall** fetch only that chosen subset without
re-discovering links. This enables a two-phase crawl: first
`discover_only=true` or `sitemap=true` to get the URL map, then `crawl_urls`
to fetch the relevant subset.

> *Optional requirement — only activates when the agent supplies
> `crawl_urls`.*

### FR-015 — `mf_screenshot` graceful degradation (state-driven)

When the `mf_screenshot` tool is invoked, the system **shall** return an
error message explaining that screenshot capture requires a headless browser
engine that is not available in the integrated Rust runtime. The error
**shall** be user-readable and **shall** suggest using `mf_fetch` instead for
text-based content extraction. The tool **shall** remain registered and
visible so agents know it exists but understand its limitation.

> *State-driven requirement — the tool is always in the "browser-unavailable"
> state in the integrated Rust runtime; it must degrade honestly.*

### FR-016 — `mf_cache_clear` cache management (event-driven)

When the `mf_cache_clear` tool is invoked, the system **shall** clear the
content cache. When the `all` parameter is `true`, all entries **shall** be
purged. When `all` is `false` (default), only expired entries **shall** be
purged. The tool **shall** return a count of purged entries.

> *Event-driven requirement — triggers when the agent explicitly requests a
> cache clear.*

### FR-017 — `mf_version` version info (ubiquitous)

The `mf_version` tool **shall** return the masterfetch integration version,
the ragent version, and a brief description of the integrated tool set. This
tool does not make network calls.

> *Ubiquitous requirement — always available, always returns version
> info.*

### FR-018 — SQLite content cache (ubiquitous)

The system **shall** provide a SQLite-backed content cache (WAL mode) keyed by
URL + extraction type + `css_selector` + `pages`, with a configurable TTL
(default 3600 seconds). `mf_fetch` and `mf_crawl` **shall** check the cache
before fetching and store successful results after fetching. Entries with bad
content (`content_ok=false`) **shall** never be cached. A size cap
**shall** evict the oldest entries so a long-lived agent's cache cannot grow
unbounded. `cache_ttl=0` **shall** force a fresh fetch bypassing the cache.

> *Ubiquitous requirement — the cache underlies every fetch and crawl
> operation.*

### FR-019 — SSRF protection (unwanted)

The system **shall not** fetch URLs targeting private/internal IP ranges
(127.0.0.0/8, 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16,
link-local, multicast, reserved), localhost hostnames, cloud metadata
endpoints (169.254.169.254, metadata.google.internal), or DNS rebinding
services (nip.io, sslip.io, xip.io). Blocked schemes (file, ftp, gopher,
data, javascript) **shall** be rejected. The system **shall** normalise
alternate IP notations (octal, hex, decimal, short-form) before checking.
URLs containing backslash characters **shall** be rejected (CVE-2025-0454
parser confusion).

> *Unwanted requirement — SSRF attacks and internal network access are
> explicitly forbidden.*

### FR-020 — Tool registration (ubiquitous)

The system **shall** register all six `mf_*` tools in
`create_extended_registry()` in `crates/ragent-tools-extended/src/lib.rs`,
each wrapped in `Arc::new(...)` and added after the existing tool
registrations. The tools **shall** be automatically surfaced to agents via
the existing `register_extracted_extended_tools()` →
`ExtractedExtendedToolAdapter` flow.

> *Ubiquitous requirement — registration is the prerequisite for tool
> availability.*

### FR-021 — Tool visibility switch (state-driven)

The system **shall** add a `masterfetch` boolean switch to
`ToolVisibilityConfig` in `crates/ragent-config/src/config.rs` (default:
`true`), with a corresponding field in `ToolVisibilitySpecified`, an entry in
`iter_switches()`, a `"masterfetch"` arm in `tool_family_names()` listing all
six `mf_*` tool names, handling in the custom `Serialize`/`Deserialize`
impls, the `Default` implementation, and the `merge()` overlay logic. When the
switch is `false`, all six tools **shall** be hidden from
`ToolRegistry::definitions()` via `effective_hidden_tools()`.

> *State-driven requirement — the visibility state of the `masterfetch`
> switch controls tool availability.*

### FR-022 — Permission category (ubiquitous)

The `mf_fetch`, `mf_crawl`, `mf_search`, and `mf_screenshot` tools **shall**
return `"web"` from `permission_category()`, consistent with ragent's
existing `webfetch` and `websearch` tools. The `mf_cache_clear` and
`mf_version` tools **shall** return `"system"` since they do not make
outbound network calls.

> *Ubiquitous requirement — all tools must participate in ragent's
> permission system.*

### FR-023 — No API keys required for search (unwanted)

The `mf_search` tool **shall not** require any API key, token, or account
configuration. The system **shall not** read `HOUND_*` or `TAVILY_API_KEY`
environment variables for `mf_search`. A missing network connection or
upstream search-engine outage **shall** produce a readable error with
`engine_blocked` reporting which backends failed, not a crash.

> *Unwanted requirement — credential-gating for search is explicitly
> forbidden; this preserves Hound's zero-config promise and differentiates
> `mf_search` from ragent's existing Tavily-based `websearch`.*

### FR-024 — Error handling (unwanted)

The masterfetch tools **shall not** panic on upstream HTTP errors, malformed
HTML, missing pages, empty search results, or cache failures. Each tool
**shall** return a `Result<ToolOutput>` with a user-readable error message or
a graceful "no data found" text response with honest signals
(`content_ok=false`, `error` field populated), matching the reference
implementation's catch-and-return-text pattern.

> *Unwanted requirement — panics and unhandled exceptions are explicitly
> forbidden.*

### FR-025 — Shared HTTP client and user agent (ubiquitous)

The masterfetch HTTP layer **shall** use a shared `reqwest::Client` with a
`User-Agent` header of the form `ragent/{version} (masterfetch)` and a
configurable request timeout (default 30 seconds), consistent with the
existing `webfetch` and `websearch` tools. The client **shall** follow
redirects (up to 5) and handle gzip/deflate decompression automatically via
reqwest's built-in features.

> *Ubiquitous requirement — applies to every HTTP call made by the
> masterfetch tools.*

### FR-026 — Output as text with structured metadata (ubiquitous)

Every masterfetch tool **shall** return its result as a `ToolOutput` whose
`content` is a human-readable text string (the formatted report or result
list) and whose `metadata` is an optional JSON object containing the
structured signals defined in FR-003 and FR-009. Tools **shall not** return
raw JSON as the `content` field, matching the reference implementation's
"formatted text strings — never return raw JSON" convention.

> *Ubiquitous requirement — defines the output contract for all six
> tools.*

### FR-027 — URL normalisation and deduplication (ubiquitous)

The system **shall** normalise URLs for deduplication by: lowercasing the
host, stripping default ports (80 for http, 443 for https), removing
trailing slashes on non-root paths, and stripping tracking query parameters
(utm_*, fbclid, gclid, ref, _ga, mc_cid, mc_eid). This applies to both
cache keys and crawl link deduplication.

> *Ubiquitous requirement — normalisation prevents duplicate cache entries
> and duplicate crawl pages.*

### FR-028 — robots.txt compliance (optional)

When the `mf_fetch` or `mf_crawl` tool receives a `respect_robots` parameter
set to `true`, the system **shall** fetch and parse the target domain's
`robots.txt`, cache the result per-domain (TTL 3600 seconds), and refuse to
fetch URLs disallowed by the robots policy. When `respect_robots` is `false`
(default), the system **shall** skip the robots check.

> *Optional requirement — only activated when the agent explicitly requests
> robots compliance.*

### FR-029 — Page-type detection (state-driven)

When the `mf_fetch` or `mf_crawl` tool processes an HTML page, the system
**shall** classify the page type from its HTML structure and set the
`page_type` field in the response metadata. The classification **shall**
include at minimum: `article`, `docs`, `list`, `forum`, `qa`, `js_shell`,
`auth_wall`, `paywall`, `redirect`, and `unknown`. The classification
**shall** drive the `next_action` suggestion (e.g. `list` pages suggest
fetching the linked URLs, `auth_wall` pages suggest switching sources).

> *State-driven requirement — triggers based on the structural
> characteristics of each fetched page.*

### FR-030 — Freshness and source-authority signals (ubiquitous)

The system **shall** compute freshness signals (`content_age_days`,
`is_stale` where stale = age > 365 days) from the page's published/modified
date metadata, preferring the modified date over the published date. The
system **shall** classify the source authority (`source_type`, `is_official`)
from the URL domain: government (.gov), education (.edu), GitHub,
vendor-docs (docs.*, developer.*), Q&A (stackoverflow, stackexchange),
forum (reddit, discourse), blog (medium, substack), news, ecommerce, or
unknown. `is_official` **shall** be `true` only on strong signals (gov, edu,
github, vendor docs).

> *Ubiquitous requirement — freshness and authority signals are computed for
> every fetched page to support agent trust judgement.*

## Non-functional requirements

### NFR-001 — Performance

Each masterfetch tool **shall** complete within 30 seconds for a typical
single-URL fetch, within 60 seconds for a 10-page crawl, and within 15
seconds for a search query. The search backends **shall** run in parallel
using `tokio::join!` or `futures::join_all` to minimise latency. Cache hits
**shall** return in under 100 ms.

### NFR-002 — Minimal new dependencies

The implementation **shall** prefer crates already present in the workspace
(`reqwest`, `serde`, `serde_json`, `regex`, `html2text`, `readability-rs`,
`url`, `chrono`, `tokio`, `async-trait`, `tracing`, `anyhow`, `thiserror`).
If a SQLite cache is required, the implementation **shall** reuse
`ragent-storage` infrastructure or add `rusqlite` only if no existing SQLite
dependency is available. No browser engine (Playwright, Chromium, Patchright)
or OCR engine (rapidocr, onnxruntime, pypdfium2) dependencies shall be
added.

### NFR-003 — Testability

Each module (HTTP client, content extractor, metadata extractor, link
classifier, page-type detector, freshness computer, URL normaliser,
SSRF validator, search engine adapter, cache, crawler, BM25 focus filter)
**shall** be a pure function or struct with injectable behaviour, enabling
unit tests without live network calls. Tool logic (report formatting,
signal computation, ranking) **shall** be extracted into testable pure
functions. HTTP-dependent tests **shall** be gated behind a `#[ignore]`
attribute or a network feature flag.

### NFR-004 — Documentation

Every public function and module **shall** carry a `///` doc comment
describing its purpose, per the project's documentation standards. The
`masterfetch` module **shall** have a `//!` module-level doc comment.
The `mf_*` tool descriptions **shall** be agent-optimised: concise but
information-dense, with decision guides, response-signal explanations, and
anti-pattern warnings — mirroring Hound's hand-crafted tool definitions.

## Scope

### In scope

- Re-implementing all six Hound tools (`mf_fetch`, `mf_crawl`, `mf_search`,
  `mf_screenshot`, `mf_cache_clear`, `mf_version`) in Rust inside
  `crates/ragent-tools-extended`.
- Shared HTTP client module with SSRF protection and URL validation.
- Content extraction chain (`readability-rs` → `html2text` → raw text).
- Metadata extraction (OpenGraph, JSON-LD, canonical, `<title>`).
- Outgoing link classification (citations/navigation/external/primary_source).
- Page-type detection (article/docs/list/forum/qa/js_shell/auth_wall/paywall).
- Freshness and source-authority envelope signals.
- BM25 query-focused content filtering.
- URL normalisation and deduplication.
- robots.txt compliance (optional, per-request).
- SQLite content cache with TTL and size cap.
- Best-first same-domain crawler with content-adaptive extraction and sitemap.
- Keyless multi-engine web search (DuckDuckGo + Brave minimum, extensible).
- Cross-engine consensus ranking and result merging.
- Tool registration in `create_extended_registry()`.
- `masterfetch` tool-visibility switch in `ragent-config`.
- Permission categories (`web` for network tools, `system` for cache/version).
- Unit tests for all pure-logic modules.
- Integration tests for tool registration and visibility.

### Out of scope

- Running hound-mcp as an MCP server (explicitly excluded by the feature
  request).
- Anti-detect browser escalation (Patchright/Playwright stealth engine) —
  cannot be ported to Rust; `mf_fetch` runs HTTP-only with graceful
  degradation.
- Cloudflare bypass / Turnstile solving — requires a browser engine.
- Screenshot capture (`mf_screenshot` returns an honest error explaining the
  browser-engine limitation).
- PDF extraction with OCR (pdfplumber, pypdfium2, rapidocr) — ragent already
  has `pdf_read` in the extended tool set; `mf_fetch` will detect PDFs and
  delegate to `pdf_read` or return a content-type hint rather than
  re-implementing PDF extraction.
- Neural reranker (ONNX cross-encoder) — `mf_search` falls back to
  cross-engine consensus + engine-position order, matching Hound's lean
  install behaviour.
- Page interaction actions (click, fill, press, scroll) — requires browser
  engine.
- Reddit URL rewriting (old.reddit.com optimisation) — may be a future
  enhancement.
- Hound CLI features (`hound -u`, `--doctor`, `--rollback`) — not applicable
  to integrated tools.
- Self-updating mechanism — ragent has its own release process.
- Search engine proxy support (`HOUND_SEARCH_PROXY`) — may be a future
  enhancement.