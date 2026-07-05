# Research Source Publication Dates — Completion Report

**Date:** 2025-01-17
**Status:** ✅ COMPLETE
**Spec ref:** `specs/researchsystem/SPEC.md` (FR-011 enhancement)

## Request

> In the `/research` slash command, capture the date of publication of each
> web source and show that in the output `RESEARCH.md` and the references.
> Also, for each finding, provide a range of dates for the associated web
> sources so we can judge the relative age of the finding.

## Implementation

### 1. Date extraction — `crates/ragent-research/src/web_date.rs` (new, 390 lines)

`pub fn extract_published_at(html: &str) -> Option<DateTime<Utc>>` checks
each page in rough order of reliability:

1. **JSON-LD** — `datePublished` / `dateCreated` / `dateModified` inside
   `<script type="application/ld+json">` blocks (object or array form).
2. **Meta tags** — `article:published_time`, `article:published`,
   `og:article:published_time`, `og:published_time`, `pubdate`,
   `publishdate`, `publication_date`, `dc.date`, `dc.date.issued`,
   `sailthru.date`, `date`.
3. **`<time datetime="...">`** elements.
4. **Visible-date fallback** — scans the first lines of rendered text for
   `YYYY-MM-DD` (and `/` / `.` variants) or `Month D, YYYY` tokens.

All parsing is defensive: malformed input returns `None` rather than
panicking, so a single bad page never aborts a research run. Dates without
a time component are mapped to midnight UTC of that day.

**Tests:** 12 unit tests covering each path plus edge cases
(JSON-LD array, `<time>` element, `pubdate` meta name, bare ISO date,
slash/dot ISO variants, human-date fallback, malformed content, JSON-LD
priority over meta).

### 2. Capture at fetch time — `crates/ragent-agent/src/research_adapter.rs`

`AgentWebFetchTool::fetch` now performs a **best-effort supplementary
raw-HTML fetch** to populate `published_at`:

- Reuses the existing `webfetch` tool for the rendered text body (unchanged).
- Issues a second `GET` with a 20 s timeout, 5-redirect limit, and a
  ragent User-Agent.
- Only proceeds when `Content-Type` is `text/html` or `application/xhtml`.
- Reads at most the first **64 KB** of the body (the `<head>` metadata is
  at the top, so this is plenty and keeps the extra request cheap).
- Calls `ragent_research::extract_published_at` on the head chunk.
- Any failure (network error, non-HTML, missing date) silently leaves
  `published_at = None` — the research run is never aborted.

`WebFetchedPage` gains a `published_at: Option<DateTime<Utc>>` field,
propagated through `WebGatherer::gather` into `Source::Web.published_at`.

### 3. Data model — `crates/ragent-research/src/source.rs`

```rust
pub enum Source {
    Web {
        url: String,
        title: String,
        captured_at: DateTime<Utc>,
        #[serde(default)]
        published_at: Option<DateTime<Utc>>,   // NEW
        body_path: PathBuf,
        #[serde(default)]
        body: String,
    },
    // Local / Spec / Other unchanged
}
```

`#[serde(default)]` on `published_at` keeps older `RESEARCH.md` files
loadable — they simply show `—` for Published until re-gathered.

New accessor: `Source::published_at(&self) -> Option<DateTime<Utc>>`
(returns `None` for non-Web variants).

### 4. References Index — `crates/ragent-research/src/io.rs`

`ResearchIo::render_references_index` now emits a **Published** column
between **Title** and **Relevance**:

```text
| # | Type | Path/URL | Title | Published | Relevance | Captured |
|---|------|----------|-------|-----------|-----------|----------|
| 1 | web  | https://… | …     | 2024-03-22 | …         | …        |
| 2 | web  | https://… | …     | —          | …         | …        |
| 3 | local| src/lib.rs| …     | —          | …         | …        |
```

Dated web source → `YYYY-MM-DD`; undated web / local / spec / other → `—`.

### 5. Per-finding date range — `crates/ragent-research/src/document.rs`

`render_finding_sources` now appends a `**Source date range:**` line after
the bullet list of cited sources. Examples:

- All dated: `**Source date range:** 2022-03-10..2024-11-02 (3 of 4 cited web sources dated)`
- Single date: `**Source date range:** 2024-03-22 (1 of 1 cited web sources dated)`
- No dates: `**Source date range:** — (cited web sources did not expose a publication date)`
- No web cited: `**Source date range:** — (no web sources cited)`

Each bullet in the `**Sources:**` list also shows the per-source date when
present: `- [1] Title — https://… (published 2024-03-22)`.

The line is omitted only when the finding already contains a Sources
paragraph produced by the LLM itself (deduplication guard preserved).

New helper: `render_finding_date_range(indices, sources) -> Option<String>`.

### 6. Supporting files — `crates/ragent-research/src/document.rs`

`render_supporting_file` for `Source::Web` now writes:

```text
# Web source

- URL: https://…
- Title: …
- Published (UTC): 2024-03-22T00:00:00+00:00   (or —)
- Captured (UTC): 2025-01-17T…

```text
<page body>
```
```

### 7. Re-exports — `crates/ragent-research/src/lib.rs`

`pub use web_date::extract_published_at;` so `ragent-agent` can call it.

## Backward compatibility

- `published_at` is `Option` with `#[serde(default)]`; pre-existing
  `RESEARCH.md` files load unchanged (Published shows `—` until re-gathered).
- All test fakes (`OkFetch`, `FakeFetch`, etc.) were updated to construct
  `WebFetchedPage` / `Source::Web` with `published_at: None`, so no test
  fixture needed a real date.

## Verification

| Check | Result |
|-------|--------|
| `cargo check -p ragent-research -p ragent-agent` | ✅ |
| `cargo test -p ragent-research --lib` | ✅ 289 passed |
| `cargo test -p ragent-research --tests` | ✅ 289 lib + 1 e2e + 8 integration |
| `cargo test -p ragent-agent --lib` | ✅ 320 passed |

## Files touched

| File | Change |
|------|--------|
| `crates/ragent-research/src/web_date.rs` | **NEW** — date-extraction module (390 lines, 12 tests) |
| `crates/ragent-research/src/lib.rs` | re-export `extract_published_at` |
| `crates/ragent-research/src/source.rs` | `Source::Web.published_at` field + accessor |
| `crates/ragent-research/src/web_gatherer.rs` | `WebFetchedPage.published_at` field + propagation |
| `crates/ragent-research/src/io.rs` | References Index **Published** column |
| `crates/ragent-research/src/document.rs` | per-finding `**Source date range:**` + supporting-file Published line |
| `crates/ragent-agent/src/research_adapter.rs` | best-effort raw-HTML fetch + `extract_published_at` call |
| `crates/ragent-research/src/{engine,manager,session,state,verify}.rs` | test fakes updated for new field |
| `crates/ragent-research/benches/*.rs` | bench fakes updated for new field |
| `crates/ragent-research/tests/test_research_integration.rs` | test fakes updated for new field |

## Behavioural summary

- Every captured web source now carries an optional UTC publication date.
- The `## References Index` table shows it in a new **Published** column.
- Each numbered finding under `## Findings` gets a `**Source date range:**`
  line summarising the earliest–latest publication dates of its cited web
  sources, with an explicit note when dates are unavailable or absent.
- Local, spec, and other sources have no publication date and always
  render `—`, so the reader can distinguish "undated web" from "non-web"
  at a glance.