# Implementation Plan: Web Source Relevance Scoring/Annotation in `/research create`

## Executive Summary

Add a deterministic, zero-LLM relevance note to every `Source::Web` by tracking which sub-query produced the hit and scoring the match between that query and the hit's title/snippet/URL. The relevance note will flow through `SourceBody` into the synthesis prompt and be displayed in the References Index, supporting files, and bibliography.

---

## 1. Data Model Changes

### 1.1 `crates/ragent-research/src/source.rs`

Add a `relevance` field to `Source::Web`, mirroring the existing field on `Source::Local` and `Source::Spec`.

```rust
pub enum Source {
    Web {
        url: String,
        title: String,
        captured_at: DateTime<Utc>,
        #[serde(default)]
        published_at: Option<DateTime<Utc>>,
        body_path: PathBuf,
        #[serde(default)]
        body: String,
        /// One-line relevance note describing how well this web source matches
        /// the research topic/query that found it.
        #[serde(default)]
        relevance: String,
    },
    // ...
}
```

Update `Source::relevance()` to return the new web field:

```rust
pub fn relevance(&self) -> Option<&str> {
    match self {
        Self::Local { relevance, .. } | Self::Spec { relevance, .. } => Some(relevance),
        Self::Web { relevance, .. } if !relevance.is_empty() => Some(relevance),
        _ => None,
    }
}
```

### 1.2 `crates/ragent-research/src/web_gatherer.rs`

Add `matched_query` to `WebSearchHit` so the gatherer can record which sub-query produced each hit:

```rust
pub struct WebSearchHit {
    pub url: String,
    pub title: String,
    pub snippet: String,
    /// The actual sub-query string that returned this hit. Used for
    /// deterministic relevance scoring.
    pub matched_query: String,
}
```

### 1.3 `crates/ragent-research/src/analysis.rs`

`SourceBody` already has a `relevance: String` field and `build_source_bodies` already maps `Source::relevance()`. No change is required in `SourceBody` itself; the web relevance will automatically propagate once `Source::Web::relevance` exists.

---

## 2. Relevance Computation Heuristic

Add a new public helper in `crates/ragent-research/src/web_gatherer.rs`:

```rust
/// Build a short, deterministic relevance note for a web hit.
///
/// The score is based on:
/// - Query term overlap in title and snippet (higher weight).
/// - Query term overlap in the URL path (lower weight).
/// - Search result rank (earlier hits are presumed more relevant).
///
/// No LLM calls are made. The output is a one-line human-readable string such as:
///   "High relevance (rank 1): title/snippet match for 'rust async'"
///   "Moderate relevance (rank 3): partial match in URL for 'tokio runtime'"
pub fn compute_web_relevance(hit: &WebSearchHit, rank: usize) -> String {
    let query_terms: Vec<&str> = hit
        .matched_query
        .to_lowercase()
        .split_whitespace()
        .filter(|t| t.len() > 2)
        .collect();

    let title_lower = hit.title.to_lowercase();
    let snippet_lower = hit.snippet.to_lowercase();
    let url_lower = hit.url.to_lowercase();

    let title_matches = count_term_matches(&query_terms, &title_lower);
    let snippet_matches = count_term_matches(&query_terms, &snippet_lower);
    let url_matches = count_term_matches(&query_terms, &url_lower);

    let text_score = title_matches * 3 + snippet_matches * 2;
    let url_score = url_matches;
    let rank_penalty = rank.min(10) as isize; // 0-based rank

    let total = (text_score as isize * 2).saturating_sub(rank_penalty) + (url_score as isize);

    let level = if total >= 12 {
        "High relevance"
    } else if total >= 6 {
        "Moderate relevance"
    } else {
        "Low relevance"
    };

    let matched_in = if title_matches + snippet_matches > 0 {
        "title/snippet"
    } else if url_matches > 0 {
        "URL"
    } else {
        "search result"
    };

    format!(
        "{level} (rank {}): {matched_in} match for '{}' (score {total})",
        rank + 1,
        hit.matched_query
    )
}

fn count_term_matches(terms: &[&str], text: &str) -> usize {
    terms.iter().filter(|t| text.contains(*t)).count()
}
```

Design rationale:

- **Deterministic**: same query/hit/rank always produces the same note.
- **Cheap**: only string lowercase and substring scans, no tokenization or LLM.
- **Explainable**: the note tells the user *why* it got that score.
- **Bounded**: output length is capped by the input query length.

---

## 3. Updates to `crates/ragent-research/src/web_gatherer.rs`

### 3.1 Track matched query per hit

In `gather_with_observer`, when collecting hits, store the query alongside each hit and populate `WebSearchHit::matched_query`:

```rust
while let Some((idx, result)) = results.next().await {
    let query = queries
        .get(idx)
        .cloned()
        .unwrap_or_else(|| topic.to_string());
    match result {
        Ok(hits) => {
            for hit in hits {
                let url_key = hit.url.to_lowercase();
                if seen_urls.insert(url_key) {
                    hits_by_url.push((query.clone(), hit));
                }
            }
        }
        // ...
    }
}
```

Change `hits_by_url` to carry a richer tuple so the query survives:

```rust
let mut hits_by_url: Vec<(String, WebSearchHit)> = Vec::new();
```

### 3.2 Compute and store relevance during fetch

When building `Source::Web` after a successful fetch, compute relevance from the hit and its search-ranking index:

```rust
let candidates: Vec<(usize, String, WebSearchHit)> = hits_by_url
    .into_iter()
    .take(max_results)
    .enumerate()
    .map(|(index, (query, mut hit))| {
        hit.matched_query = query.clone();
        (index, query, hit)
    })
    .collect();
```

Inside the fetch success branch:

```rust
let relevance = compute_web_relevance(&hit, index);
collected.push((
    index,
    Some(Source::Web {
        url: page.url,
        title,
        captured_at: Utc::now(),
        published_at: page.published_at,
        body_path,
        body,
        relevance,
    }),
));
```

### 3.3 Update `GatherResult` and helper constructors

`GatherResult` already stores `queries: Vec<String>` and `sources: Vec<Source>`. No structural change is needed.

Update `fetch_url_as_source` to set a default relevance note for the `--from-url` primary source:

```rust
let source = Source::Web {
    url: page.url.clone(),
    title,
    captured_at: chrono::Utc::now(),
    published_at: page.published_at,
    body_path: web_body_path(0),
    body,
    relevance: format!("Primary source from --from-url: {}", page.url),
};
```

---

## 4. Updates to `crates/ragent-research/src/analysis.rs` (Synthesis Prompt)

`render_sources_block` already emits:

```rust
Relevance: {rel}
```

where `rel` is `src.relevance`. Once `Source::Web` has a relevance note, it will appear automatically. However, the default formatting currently prints `—` when relevance is empty. For web sources it will now be non-empty, so the prompt will include it.

No code change is strictly required in `analysis.rs`, but verify that the prompt wording is useful. Optionally enhance the `Relevance:` line label to be clearer:

```rust
Relevance note: {rel}
```

This is a cosmetic prompt change and should be gated so existing tests that assert byte-identical prompt output are not broken. If tests rely on the exact `Relevance:` text, leave it unchanged.

---

## 5. Rendering Updates

### 5.1 `crates/ragent-research/src/io.rs`: `render_references_index`

Current code renders `"—"` for web sources. Change the relevance match arm to use `Source::relevance()`:

```rust
let relevance = source
    .relevance()
    .map(sanitize_inline)
    .unwrap_or_else(|| "—".to_string());
```

This single change fixes web sources and keeps local/spec behavior identical.

### 5.2 `crates/ragent-research/src/document.rs`: `render_supporting_file`

Add the relevance note to the web supporting-file header:

```rust
Source::Web {
    url,
    title,
    captured_at,
    published_at,
    body,
    relevance,
    ..
} => Some(format!(
    "# Web source\n\n\
     - URL: {url}\n\
     - Title: {title}\n\
     - Published (UTC): {published}\n\
     - Captured (UTC): {captured}\n\
     - Relevance: {relevance}\n\n\
     ```text\n{body}\n```\n",
    // ...
)),
```

### 5.3 `crates/ragent-research/src/document.rs`: `render_bibliography`

Add a relevance bullet for every source that has one:

```rust
if let Some(rel) = source.relevance() {
    if !rel.is_empty() {
        out.push_str(&format!("- **Relevance:** {rel}\n"));
    }
}
```

Place it after `**Type:**` and `**Path/URL:**` lines.

---

## 6. Tests to Add / Update

### 6.1 `crates/ragent-research/src/source.rs`

Update existing tests that construct `Source::Web` to include `relevance: ""` or a real note. Since the new field has `#[serde(default)]`, old serialized state still deserializes.

Add a test:

```rust
#[test]
fn web_relevance_is_returned_by_relevance_accessor() {
    let src = Source::Web {
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: dt(),
        published_at: None,
        body_path: PathBuf::from("sources/web-01.md"),
        body: "body".into(),
        relevance: "High relevance (rank 1): title match".into(),
    };
    assert_eq!(src.relevance(), Some("High relevance (rank 1): title match"));
}
```

### 6.2 `crates/ragent-research/src/web_gatherer.rs`

Update `FakeSearch`/`gatherer_with` tests to populate `matched_query` in hits. Existing tests that build `WebSearchHit { ..., snippet: "".into() }` will fail compilation once `matched_query` is added, so they must be updated.

Add new tests:

```rust
#[tokio::test]
async fn gather_computes_relevance_for_each_web_source() {
    // Set up hits with title/snippet containing query terms.
    // Assert each returned Source::Web has a non-empty relevance note
    // containing "relevance" and the query.
}

#[test]
fn compute_web_relevance_prefers_title_and_snippet_matches() {
    let hit = WebSearchHit {
        url: "https://example.com".into(),
        title: "Rust async patterns".into(),
        snippet: "async/await in Rust".into(),
        matched_query: "rust async".into(),
    };
    let note = compute_web_relevance(&hit, 0);
    assert!(note.contains("High relevance") || note.contains("Moderate relevance"));
    assert!(note.contains("rust async"));
}

#[test]
fn compute_web_relevance_falls_back_to_url_matches() {
    let hit = WebSearchHit {
        url: "https://example.com/rust-async".into(),
        title: "Something else".into(),
        snippet: " unrelated".into(),
        matched_query: "rust async".into(),
    };
    let note = compute_web_relevance(&hit, 5);
    assert!(note.contains("URL"));
}
```

### 6.3 `crates/ragent-research/src/io.rs`

Add:

```rust
#[test]
fn render_references_index_shows_web_relevance() {
    let sources = vec![Source::Web {
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: Utc::now(),
        published_at: None,
        body_path: PathBuf::from("sources/web-01.md"),
        body: "body".into(),
        relevance: "High relevance (rank 1): title match for 'rust async'".into(),
    }];
    let table = ResearchIo::render_references_index(&sources, Utc::now());
    assert!(table.contains("High relevance"));
}
```

### 6.4 `crates/ragent-research/src/document.rs`

Add tests for:

- `render_supporting_file` includes the relevance line for web sources.
- `render_bibliography` includes the relevance bullet.

### 6.5 `crates/ragent-research/tests/test_research_create_synthesis.rs`

Update the fake `WebSearchHit` constructions to include `matched_query`. Add an assertion that the generated `RESEARCH.md` References Index contains a relevance note (e.g. `"relevance"` or `"rank"`) when web sources are captured.

### 6.6 `crates/ragent-research/tests/test_research_integration.rs`

Update the `Source::Web` construction to include the new `relevance` field. Add an assertion that the written `RESEARCH.md` shows a relevance note in the References Index.

---

## 7. Backward Compatibility Strategy

| Surface | Strategy |
|---|---|
| `Source::Web` deserialization | New `relevance` field uses `#[serde(default)]`. Old `state.json` / frontmatter-style JSON loads with empty relevance. |
| `WebSearchHit` | Adding a public field is a breaking change for any external crate constructing it. Because `WebSearchHit` is `#[derive(Debug, Clone, PartialEq, Eq)]` (not `Default`), all constructors must be updated. This is acceptable because `ragent-research` is the only crate that constructs `WebSearchHit` in this workspace, and tests live inside the crate. |
| RESEARCH.md rendering | References Index already has a Relevance column. Old web rows displayed `"—"`; new rows display the computed note. No column layout change. |
| Supporting files | New `- Relevance:` line is additive. Existing readers that parse the YAML-ish header by key will tolerate the extra line; naive parsers that split on line count may shift, but the format is already self-describing. |
| Synthesis prompt | The `Relevance:` line already existed for local/spec sources. For web sources it was previously `"—"`; now it carries a note. This is a prompt-content change, not a breaking schema change. |

### Migration note for persisted research items

Items already on disk will have `Source::Web { relevance: "" }` after deserialization. They will display `"—"` in the References Index until re-gathered. If desired, add a small one-off utility or manager method to backfill relevance for existing items, but this is out of scope for the initial feature.

---

## 8. `--from-url` Path Impact

The `--from-url` flow in `crates/ragent-research/src/session.rs` calls `web.fetch_url_as_source(url).await` before the normal web-gathering phase.

Changes needed:

1. Update `fetch_url_as_source` in `web_gatherer.rs` to set `relevance` to a primary-source note (see section 3.3).
2. The `Source::Web` value returned is pushed into `sources` in `session.rs`. It will automatically be included in `build_source_bodies`, the References Index, and the supporting file.
3. If the derived topic from `--from-url` is later used as the web search topic, normal gathered sources will get their own computed relevance notes; the primary source retains its `--from-url` note, making it distinguishable in the References Index.

No other session logic needs to change.

---

## 9. Public API Changes

### New public API surface

1. **`Source::Web` gains a new public field**: `relevance: String`.
2. **`WebSearchHit` gains a new public field**: `matched_query: String`.
3. **New public helper function** (suggested): `web_gatherer::compute_web_relevance(hit: &WebSearchHit, rank: usize) -> String`.

### Existing public API that automatically changes behavior

- `Source::relevance()` now returns `Some(...)` for web sources.
- `build_source_bodies` now propagates web relevance into `SourceBody::relevance`.
- `ResearchIo::render_references_index` now renders web relevance instead of `"—"`.

### No breaking changes to

- `AnalysisEngine` trait.
- `SessionConfig`, `RunOutcome`, `GatherResult`.
- `ResearchDocument`, `AssembledDocument`.
- IO path helpers.

---

## 10. Suggested Implementation Order

1. **Add `relevance` to `Source::Web`** (`source.rs`) and update `Source::relevance()`.
2. **Add `matched_query` to `WebSearchHit`** (`web_gatherer.rs`).
3. **Add `compute_web_relevance`** (`web_gatherer.rs`) and wire it into the gather loop.
4. **Update `fetch_url_as_source`** to set a relevance note.
5. **Update `io.rs` `render_references_index`** to use `Source::relevance()`.
6. **Update `document.rs` `render_supporting_file` and `render_bibliography`** to display relevance.
7. **Fix all compile errors** in tests and existing `WebSearchHit` constructors.
8. **Add new tests** for relevance computation and rendering.
9. **Run `cargo check --workspace` and `cargo test -p ragent-research`**.
10. **Update `CHANGELOG.md`** with the feature (per `AGENTS.md`, before any push to remote).

---

## 11. Risk and Mitigation

| Risk | Mitigation |
|---|---|
| Relevance note length blows up the prompt | The helper formats a bounded, one-line string. If query is long, the note is still proportional to query length. Consider capping query length in the note or truncating to 120 chars. |
| Rank penalty makes later hits look irrelevant | Rank only subtracts a small constant; strong title matches still score high. |
| Non-English or stemming mismatches | Substring matching is simple and deterministic but not semantic. This is acceptable per the "no extra LLM calls" constraint. Future work could plug in an LLM-based relevance scorer behind a config flag. |
| Existing tests assert exact prompt bytes | `render_sources_block` already emitted `Relevance:`; only the value changes from `"—"` to a note. Tests that construct `Source::Web` must add the new field. |
| `WebSearchHit` public field addition breaks external consumers | Document in release notes. Within this workspace, only `ragent-research` constructs it. |

---

This plan keeps the change focused, deterministic, backward-compatible for persisted data, and aligned with the existing relevance machinery for local and spec sources.
