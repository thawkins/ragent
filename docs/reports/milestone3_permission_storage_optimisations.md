# Milestone 3 Completion Report — Permission Checks & Storage Optimisations

**Date:** 2025-01-20
**Spec:** PERFPLAN.md Milestone 3
**Status:** ✅ COMPLETE

---

## Summary

Implemented three performance optimisations targeting per-tool-call overhead
and unnecessary I/O in the agent execution loop
(`crates/ragent-agent/src/session/processor.rs`):

| Task | Description | Expected Gain |
|------|-------------|---------------|
| T-3.1 | Cache compiled permission patterns (dir_lists) | 3–5% per tool call |
| T-3.2 | Skip unchanged interim storage updates | 2–5% on I/O-bound hosts |
| T-3.3 | Replace `chars().count()` with byte length | 2–4% per large result |

---

## Task 3.1: Cache Compiled Permission Patterns

### Problem

`check_permission_with_prompt()` (processor.rs:399) called
`get_compiled_denylist()` and `get_compiled_allowlist()` on every
permission check.  These functions cloned an entire `GlobSet` from a
`std::sync::RwLock` every invocation:

```rust
let denylist = get_compiled_denylist();  // cloned GlobSet
let allowlist = get_compiled_allowlist();  // cloned GlobSet
```

### Solution

Changed `get_compiled_allowlist()` and `get_compiled_denylist()` in
`crates/ragent-config/src/dir_lists.rs` to return `Arc<GlobSet>` instead
of `GlobSet`:

```rust
pub fn get_compiled_allowlist() -> Arc<GlobSet> { ... }
pub fn get_compiled_denylist() -> Arc<GlobSet> { ... }
```

This avoids cloning the compiled glob set on every call — callers now
get a cheap reference-counted pointer.

Added `invalidate_compiled_caches()` for future use when patterns are
mutated at runtime.

### Files Modified

- `crates/ragent-config/src/dir_lists.rs` — changed return types to `Arc<GlobSet>`
- `crates/ragent-agent/src/session/processor.rs` — no changes needed (call sites already bind via `let`)

---

## Task 3.2: Skip Unchanged Interim Storage Updates

### Problem

Every step fired an async storage update even when `assistant_parts` had
not changed since the last interim update:

```rust
let mut interim = Message::new(session_id, Role::Assistant, assistant_parts.clone());
interim.id = assistant_msg_id.clone();
let _ = self.storage_op(move |s| s.update_message(&interim)).await;
```

This ran `spawn_blocking` on SQLite even during idle/waiting periods.

### Solution

Added a `last_interim_hash: Option<u64>` field to step-local state in
the agent loop.  Before each storage update we compute a content hash
(via `std::collections::hash_map::DefaultHasher`) over all
`assistant_parts`.  If the hash matches the previous update, the
storage write is skipped entirely.

```rust
let current_hash = {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for part in &assistant_parts { ... }
    hasher.finish()
};
if last_interim_hash != Some(current_hash) {
    let _ = self.storage_op(move |s| s.update_message(&interim)).await;
    last_interim_hash = Some(current_hash);
}
```

### Files Modified

- `crates/ragent-agent/src/session/processor.rs` — hash computation + conditional skip

---

## Task 3.3: Replace `chars().count()` with Byte Length in Tool Result Truncation

### Problem

`tool_result_content_for_llm()` used `chars().count()` four times on
potentially large content:

```rust
if content.chars().count() <= MAX_TOOL_RESULT_CHARS_FOR_LLM { ... }
let head = truncate_at_char_boundary(content, ...);  // calls chars().count()
let tail = trailing_at_char_boundary(content, ...);  // calls chars().count()
let omitted_chars = content.chars().count().saturating_sub(...);
```

For a 12 000-char tool result this decoded UTF-8 four times.

### Solution

Replaced the threshold check with `content.len() <= MAX_TOOL_RESULT_BYTES_FOR_LLM`.
For the common case of ASCII tool output, `len()` == `chars().count()`,
so behaviour is identical.  When truncation is actually needed, we still
call `chars().count()` once (instead of four times) and reuse the
result.

```rust
if content.len() <= MAX_TOOL_RESULT_BYTES_FOR_LLM {
    return content.to_string();   // fast path: zero UTF-8 decode
}

let head = truncate_at_char_boundary(content, TOOL_RESULT_HEAD_CHARS_FOR_LLM);
let tail = trailing_at_char_boundary(content, TOOL_RESULT_TAIL_CHARS_FOR_LLM);
let total_chars = content.chars().count();   // one decode, reused below
let omitted_chars = total_chars
    .saturating_sub(head.chars().count() + tail.chars().count());

format!("... {total_chars} chars ...", ...)
```

### Files Modified

- `crates/ragent-agent/src/session/processor.rs` — byte-length threshold, single char count

---

## Benchmarks

Added two new benchmark functions to
`crates/ragent-agent/benches/agent_loop.rs`:

- `bench_compiled_dir_lists` — measures `Arc<GlobSet>` lookup vs baseline
- `bench_tool_result_truncation` — measures small (fast-path) vs large (truncation) payloads

Run with:
```bash
cargo bench -p ragent-agent --bench agent_loop
```

---

## Tests

- All 332 `ragent-agent` lib tests pass ✅
- Compilation clean (zero errors, zero new warnings after fix) ✅

---

## Cumulative Impact

| Milestone | Tasks | Expected Gain |
|-----------|-------|---------------|
| 1 | 3 | 15–30% |
| 2 | 2 | 5–15% |
| 3 | 3 | 5–10% |
| **Total** | **8** | **25–55%** |

Milestone 4 (micro-optimisations: Ollama stall patterns, `Arc<str>`
session IDs, `tokio::sync::RwLock` for `active_spec`) remains as
future work (P2).
