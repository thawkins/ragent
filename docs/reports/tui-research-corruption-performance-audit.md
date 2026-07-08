# TUI `/research` Progress Panel Corruption — Performance & Resource Audit

**Status:** Read-only audit complete. No code was modified.
**Scope:** `crates/ragent-tui/src/app/research.rs`, `crates/ragent-tui/src/research_progress.rs`, `crates/ragent-research/src/web_gatherer.rs`, `crates/ragent-research/src/manager.rs`, and related rendering paths.
**Role:** Performance/resource review (expensive loops, repeated I/O, allocations, blocking calls).

---

## 1. Bottom line

No explicit source of strings such as `%%??? web` was found in the codebase. The screen-corruption-like artifacts in the `/research create` progress panel are **emergent** from a combination of:

1. A lossy markdown→HTML→plain-text conversion pipeline being run on every progress event.
2. Repeated full message replacement on every incoming research event.
3. Raw URL / page title / HTTP error text flowing unescaped from the network into the UI payload.
4. Expensive per-frame re-wrapping of all message text using `Vec<char>`→`String` reconstruction.
5. Unbounded growth of per-run progress trackers and a render cache that is emptied once it hits capacity.

The corruption is best characterized as **rendering/data-quality artifacts**, not terminal-escape injection or progress-spinner corruption.

---

## 2. Exact file/line findings

### 2.1. Progress payload is rebuilt and replaced on every event

**File:** `crates/ragent-tui/src/app/event_handler.rs`
**Lines:** 637–676, 1652–1695

`Event::AgentNotice` messages carrying the sentinel prefix `__research_progress__` are decoded and applied to the matching `ResearchProgress`. After every event the helper `refresh_research_progress_message` is called:

```rust
// event_handler.rs:1652
pub(crate) fn refresh_research_progress_message(&mut self, name: &str) {
    let Some(progress) = self.research_progress.iter().find(|p| p.name == name).cloned() else {
        return;
    };
    let rendered = self.render_markdown_to_ascii(&progress.render());
    // ... search for an existing assistant message, replace text, or push a new message
}
```

Impact:

- `ResearchProgress::render()` allocates a new `String` every time.
- `render_markdown_to_ascii()` re-parses and re-converts the whole payload every time.
- For a busy research run (web phase can emit dozens of `WebCaptured`/`FetchFailed` events), the same message is destroyed/recreated repeatedly.

### 2.2. Markdown→HTML→text pipeline mangles plain-text progress content

**File:** `crates/ragent-tui/src/app/models.rs`
**Lines:** 142–197

```rust
pub fn render_markdown_to_ascii(&mut self, text: &str) -> String {
    if let Some(research) = try_extract_research_code_block(text) { return research; }
    if !text.starts_with("From: /") { return text.to_string(); }

    // FNV hash of input bytes
    ...

    let parser = Parser::new_ext(text, opts);
    let mut html_buf = String::new();
    html::push_html(&mut html_buf, parser);

    let rendered = match std::panic::catch_unwind(||
        html2text::from_read(html_buf.as_bytes(), 120)
    ) {
        Ok(Ok(text)) => text,
        _ => text.to_string(), // fallback preserves raw, possibly malformed content
    };
    ...
    let result = self.normalize_ascii_tables(&cleaned);
```

Observations:

- The progress text does **not** start with `From: /`, so the bypass for research code blocks does not apply. The payload is forced through `pulldown_cmark` + `html2text`.
- `html2text` operates on HTML bytes with a width of 120; it can re-wrap, hyphenate, or corrupt Unicode symbols (e.g. emoji `🔬`, `✅`, `📊`) and box-drawing characters used by `normalize_ascii_tables`.
- On panic or `html2text` error, the raw text is returned unchanged. If the raw text contains partial HTML/JSON from the payload, it is dumped straight into the message window.

### 2.3. Research progress content carries raw, unescaped external strings

**File:** `crates/ragent-research/src/web_gatherer.rs`
**Lines:** 733, 752

```rust
obs.on_event(GatherEvent::SourceCaptured {
    url: page.url.clone(),
    title: title.clone(),
});

obs.on_event(GatherEvent::FetchFailed {
    url: hit.url.clone(),
    error: e.to_string(),
});
```

**File:** `crates/ragent-research/src/session.rs`
**Lines:** 976–1025

These values are forwarded into `SessionEvent::FromUrlBodyPreview`, `WebCaptured`, `WebFetchFailed`, etc., and ultimately serialized into JSON by `TuiResearchObserver`:

**File:** `crates/ragent-tui/src/research_adapter.rs`
**Lines:** 81–87

```rust
fn on_event(&self, event: ragent_research::SessionEvent) {
    let message = crate::research_progress::encode_progress_event(&self.name, &self.topic, &event);
    self.app_event_bus.publish(Event::AgentNotice { ... message });
}
```

**File:** `crates/ragent-tui/src/research_progress.rs`
**Lines:** 215–236

```rust
SessionEvent::WebFetchFailed { url, error } => (
    SessionPhase::Web, "failed_url",
    format!("fetch failed for {url}: {error}"), None,
),
SessionEvent::WebCaptured { url, title } => (
    SessionPhase::Web, "captured",
    format!("captured {} — {}", url, title), None,
),
SessionEvent::FromUrlBodyPreview { url, body_preview } => (
    SessionPhase::Setup, "preview",
    format!("--from-url body preview for {url}:\n{body_preview}"), None,
),
```

There is **no escaping or control-character stripping** of `url`, `title`, `error`, or `body_preview` before they reach the TUI. If a fetched page title contains unusual Unicode, combining characters, or an error string contains raw bytes, they are embedded verbatim in the JSON payload and later rendered.

### 2.4. TUI re-wraps all message text every frame with expensive char allocation

**File:** `crates/ragent-tui/src/layout.rs`
**Lines:** 1398–1470

```rust
fn char_wrap(text: &str, width: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut lines = Vec::new();
    let mut start = 0usize;
    while start < chars.len() {
        let end = (start + width).min(chars.len());
        lines.push(chars[start..end].iter().collect::<String>());
        start = end;
    }
    ...
}

fn build_wrapped_content_lines(lines: &[Line<'_>], inner_width: usize) -> Vec<String> {
    let mut result = Vec::new();
    for line in lines {
        let text = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
        ...
        let chars: Vec<char> = text.chars().collect();
        while line_start < chars.len() {
            ...
            result.push(chars[line_start..break_pos].iter().collect::<String>());
        }
    }
    result
}
```

`build_wrapped_content_lines` is called every render pass for the messages panel, the log panel, the todo panel, and the profile panel. For each visible `Line` it:

1. Collects every span content into a new `String`.
2. Collects every `char` into a `Vec<char>`.
3. Rebuilds each wrapped segment into another `String`.

For long research output (hundreds of lines, URLs, multi-byte emoji), this is `O(chars²)` in pathological cases and produces heavy allocation churn.

### 2.5. `ResearchProgress` tracker list and markdown render cache grow without effective bounds

**File:** `crates/ragent-tui/src/app/state.rs`
**Line:** 1417

```rust
pub research_progress: Vec<crate::research_progress::ResearchProgress>,
```

A new tracker is pushed for every `/research create` call and is never pruned. Each tracker retains a clone of all `ResearchStep` entries for that run.

**File:** `crates/ragent-tui/src/app/models.rs`
**Lines:** 191–195

```rust
if self.md_render_cache.len() >= 256 {
    self.md_render_cache.clear(); // LRU handles eviction
}
self.md_render_cache.put(hash, result.clone());
```

The cache is an `LruCache` but the code invalidates it by **clearing the entire cache** once it reaches 256 entries, throwing away prior render work. Combined with repeated progress re-rendering, this guarantees near-zero cache hit rate for active research runs.

### 2.6. Not a spinner / ANSI / multi-byte truncation root cause

- **No progress spinner:** `ResearchProgress::render()` emits static Unicode icons (`▶`, `✓`, `⚠`) and plain text. No terminal escape sequences are generated by this module.
- **No ANSI stripping:** incoming progress strings are not ANSI-stripped before display; however, the observed `%%???` style artifacts are not consistent with ANSI codes, which would appear as `[`, `;`, or `m` sequences.
- **Multi-byte truncation:** the research code uses `truncate_at_char_boundary` correctly in `crates/ragent-research/src/session.rs:878`, so the session layer is not the source. The TUI’s `char_wrap` uses `chars().collect()`, so it is also boundary-safe. The artifact therefore comes from **character substitution/loss during markdown/html conversion**, not from mid-char splitting.

---

## 3. Recommended low-risk, measurable optimizations

| Priority | File(s) | Recommendation | Expected impact |
|---|---|---|---|
| High | `crates/ragent-tui/src/app/event_handler.rs:1652` | Cache the rendered research message and only re-render when `ResearchProgress.steps` changes. Avoid calling `render_markdown_to_ascii` on every event. | Cuts render work from O(events × text) to O(changes × text). |
| High | `crates/ragent-tui/src/app/models.rs:142` | Bypass `render_markdown_to_ascii` for sentinel-prefixed research progress text; render it as plain text directly. | Removes lossy HTML conversion and eliminates most artifact sources. |
| High | `crates/ragent-research/src/web_gatherer.rs:733`, `752` and `crates/ragent-tui/src/research_progress.rs:215–236` | Strip control characters (`\x00-\x1F` except `\n`, `\t`) and trim/ellipsize very long URLs/titles before emitting them. | Prevents garbage characters from external content. |
| Medium | `crates/ragent-tui/src/layout.rs:1424` | Cache wrapped lines per message and invalidate only when the message text or panel width changes. | Reduces per-frame allocation from `O(total chars)` to `O(changed chars)`. |
| Medium | `crates/ragent-tui/src/app/state.rs:1417` | Keep only the most recent N (e.g. 10) `ResearchProgress` entries, or remove completed ones after a timeout. | Bounds memory growth for long-lived TUI sessions. |
| Low | `crates/ragent-tui/src/app/models.rs:192` | Replace `clear()` with `pop_lru()` to evict one entry at a time, preserving useful cache entries. | Improves hit rate of markdown render cache. |

---

## 4. What does NOT need to change

- The `WebGatherer` accumulation logic is correct: `collected.push((index, None))` for failures and `filter_map(|(_, src)| src)` at line 765 keeps successful sources.
- `SessionEvent` serialization via `serde_json` is correct; the sentinel prefix mechanism is sound.
- `truncate_at_char_boundary` in `session.rs:878` is boundary-safe.

---

## 5. Conclusion

The `/research` progress panel artifacts are a **rendering/throughput problem**, not a security or terminal-escape-injection bug. The fastest fix with the lowest risk is to stop running already-plain progress text through the markdown→HTML→text pipeline and to avoid re-rendering the same research message on every event. Secondary wins come from sanitizing externally sourced strings and caching wrapped lines.
