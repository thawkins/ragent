# Code Quality Review — Recently Changed Files

**Date:** 2026-08-26
**Scope:** Files changed in the last 3 commits (HEAD~3), focused on Rust source files.
**Method:** 6 parallel `explore` sub-agents reviewed ~50 source files across all major crates. Findings were triaged; safe/straightforward fixes were applied directly. Larger refactors are documented as recommendations.

---

## Summary

| Category | Issues Found | Fixes Applied |
|----------|-------------|---------------|
| Error Handling | 12 | 4 |
| Performance | 10 | 4 |
| Duplication | 15 | 3 |
| Dead Code | 6 | 2 |
| Complexity | 5 | 0 (recommendations only) |

**Applied fixes:** 13 changes across 9 files. All compile (`cargo check`), all affected crate tests pass, and `cargo fmt --check` passes.

---

## Fixes Applied

### 1. Panic risk: `.unwrap()` on `session_id` in `/name` command (P0)

**File:** `crates/ragent-tui/src/app/slash.rs`
**Category:** Error handling
**Problem:** The `/name` slash command checked `self.session_id.is_none()` then called `self.session_id.clone().unwrap()` a few lines later — a TOCTOU panic risk on a user-facing path, violating the project's "no `.unwrap()` on user-facing paths" rule.
**Fix:** Replaced with `let Some(session_id) = self.session_id.clone() else { ... return; }` — eliminates the unwrap and the prior redundant `is_none()` check.

### 2. Silent error swallowing in `is_interrupted_locked` (HIGH)

**File:** `crates/ragent-storage/src/activity_log.rs`
**Category:** Error handling
**Problem:** `is_interrupted_locked` used `.optional().ok().flatten()` which silently converted genuine storage failures (locked DB, I/O error, corruption) into `Ok(None) → false`, allowing appends to proceed into runs whose real state is unknown. This violates the "No silent error swallowing" rule.
**Fix:** Changed signature to return `Result<bool>`, propagate errors via `?`, and updated all 3 call sites to use `?`.

### 3. Dead code: unused `_active_tool_call_name` allocation (MEDIUM)

**File:** `crates/ragent-llm/src/providers/bedrock.rs`
**Category:** Dead code / performance
**Problem:** `let _active_tool_call_name = String::new();` was allocated on every Bedrock Converse chat call but never read or assigned — an unused stub that allocates an empty string per request.
**Fix:** Removed the dead binding.

### 4. Redundant double-collection in `build_cross_locus_reconcile` (MEDIUM)

**File:** `crates/ragent-research/src/reconcile.rs`
**Category:** Performance
**Problem:** `.collect::<Vec<_>>().into_iter().collect()` created a `Vec<usize>`, consumed it into an iterator, and re-collected into an identical `Vec<usize>` — doubling the allocation for no reason.
**Fix:** Replaced with a single `.collect()`.

### 5. `pruned_tool_results` underflow-safe rewrite (MEDIUM)

**File:** `crates/ragent-types/src/activity.rs`
**Category:** Complexity / correctness
**Problem:** The `total - keep_last` subtraction was safe only because of the `>= total` guard — fragile and the kind of code that breaks if the guard changes.
**Fix:** Rewrote as `let start = self.tool_results.len().saturating_sub(keep_last); &self.tool_results[start..]` — no guard needed, no underflow path.

### 6. Triple `match &src` destructure in `fetch_from_url_seeds` (LOW)

**File:** `crates/ragent-research/src/session.rs`
**Category:** Duplication
**Problem:** Three consecutive `match &src` expressions extracted `url`, `title`, `body` separately from the same `Source::Web` variant, destructuring the same enum three times with three clones.
**Fix:** Consolidated into a single match returning a tuple `(src_url, src_title, src_body)`.

### 7. `bg.rs` reader tasks silently swallow I/O errors (LOW)

**File:** `crates/ragent-tools-core/src/bg.rs`
**Category:** Error handling
**Problem:** `while let Ok(Some(line)) = lines.next_line().await` silently discarded `Err` from the stdout/stderr readers — a genuine read error would terminate the reader with no diagnostic.
**Fix:** Replaced both reader loops with explicit `match` that logs `Err(e)` via `debug!` before breaking.

### 8. `image_dimensions_or_placeholder` duplicate URL-prefix check (P3)

**File:** `crates/ragent-tui/src/app/helpers.rs`
**Category:** Duplication
**Problem:** `src.starts_with("http://") || src.starts_with("https://")` was evaluated twice in 6 lines.
**Fix:** Computed `let is_url = ...` once and reused it for both `resolved` and `dims`.

### 9. `unreachable!()` on user-facing paths in 3 providers (MEDIUM)

**Files:** `crates/ragent-llm/src/providers/openai.rs`, `copilot.rs`, `huggingface.rs`
**Category:** Error handling
**Problem:** Inside `tool_uses` mapping closures, `_ => unreachable!()` would panic the process if the filter predicate and match arm ever drifted apart (e.g. a new `ContentPart` variant is added). This violates the "no `unwrap`/`expect` on user-facing paths" rule.
**Fix:** Replaced all 3 with `filter_map` + `_ => None`, which skips non-matching items gracefully instead of panicking.

### 10. `summarize_subject` silently swallows all errors (LOW)

**File:** `crates/ragent-research/src/analysis.rs`
**Category:** Error handling
**Problem:** Used `.ok()?` at 3 failure points (provider lookup, client creation, chat request) and discarded stream errors — no diagnostic trail for why the LLM summarizer failed.
**Fix:** Added `tracing::warn!` / `tracing::debug!` at each failure point (client creation, chat request, stream error, parse failure) before returning `None`.

### 11. `verify.rs` `supported_by` O(n×m) linear scan → HashSet (MEDIUM)

**File:** `crates/ragent-research/src/verify.rs`
**Category:** Performance
**Problem:** `s_words` was a sorted `Vec<String>` and `s_words.contains(w)` did a linear scan for each word — O(n×m) per finding×source pair.
**Fix:** Convert `s_words` to a `HashSet<String>` for O(1) lookup. Added `use std::collections::HashSet`.

### 12. `synthesis.rs` `build_haystack` clones 3 Vecs (MEDIUM)

**File:** `crates/ragent-research/src/synthesis.rs`
**Category:** Performance
**Problem:** `build_haystack` cloned `findings`, `top_implications`, and `open_questions` (`Vec<String>`) solely to move them into a `parts` vec and join — 3 heap allocations for a search string.
**Fix:** Rewrote to iterate by reference with `push_str` into a single pre-allocated `String`, eliminating all 3 clones.

### 13. `try_update_event` returns less informative `attempted` string (LOW)

**File:** `crates/ragent-storage/src/activity_log.rs`
**Category:** Complexity / correctness
**Problem:** Built a rich `attempted` string (with the JSON kind tag) for the audit event, but returned `AppendError::MutationRejected { attempted: "update".to_string() }` — a different, less informative value.
**Fix:** The rich `attempted` string is now cloned into the returned error (when the event exists), so the error carries the same detail as the audit log. The fallback "update" string is only used when the target event doesn't exist.

---

## Recommendations (Not Applied — Larger Refactors)

These are higher-impact but require more extensive changes. They are documented for follow-up.

### HIGH — Duplicated `AbortOnDrop` struct

**Files:** `crates/ragent-agent/src/session/loop_steps.rs:941`, `processor.rs:646`
The identical `AbortOnDrop(JoinHandle)` + `Drop` impl is defined in both files.
**Fix:** Define once in a shared `crate::session::util` module and import.

### HIGH — Triplicated Anthropic SSE stream parser

**Files:** `crates/ragent-llm/src/providers/anthropic.rs`, `bedrock.rs`, `azure_resource.rs`
~140 lines of SSE event-stream parsing are copy-pasted verbatim across 3 providers.
**Fix:** Extract a shared `anthropic_sse_stream()` function.

### HIGH — Triplicated OpenAI message builder

**Files:** `crates/ragent-llm/src/providers/openai.rs`, `copilot.rs`, `huggingface.rs`
The `build_request_body` message construction is nearly identical across 3 providers.
**Fix:** Extract `build_openai_compatible_messages()` helper.

### HIGH — Duplicated error-classification functions

**File:** `crates/ragent-agent/src/team/manager.rs:54-86`
`is_token_overflow_error` and `is_permanent_api_error` duplicate canonical implementations in `crate::session::history`.
**Fix:** Import from `crate::session::history` instead of redefining.

### MEDIUM — `is_safe_command()` is a no-op log in bash.rs

**File:** `crates/ragent-tools-core/src/bash.rs:955, 1150`
`if is_safe_command(command) { tracing::info!(...) }` logs but does nothing else — the banned/denied checks run unconditionally regardless. The call is a no-op side effect.
**Fix:** Either wire it to short-circuit (auto-approve) or remove the dead check.

### MEDIUM — `citation_re()` / `CITATION_RE` duplicated

**Files:** `crates/ragent-research/src/synthesis.rs:28`, `verify.rs:16`
Identical `OnceLock<Regex>` + `citation_re()` helper defined in both files.
**Fix:** Extract into a shared `pub(crate)` helper.

### MEDIUM — `find_tool_call_pair` / `find_checkpoint` / `run_status` full-table scans

**File:** `crates/ragent-storage/src/activity_log.rs:557, 718, 744`
All three call `read_run()` which loads every event for the run into a `Vec` then iterates linearly.
**Fix:** Use targeted SQL queries (`WHERE` + `ORDER BY seq DESC LIMIT 1`) instead of materializing the full log.

### MEDIUM — Dead `rebuilding` field and `AppendError::RunRebuilding` variant

**File:** `crates/ragent-storage/src/activity_log.rs:52, 119`
The `rebuilding: Mutex<HashSet<String>>` field is `#[allow(dead_code)]` and never read. `AppendError::RunRebuilding` is never constructed.
**Fix:** Remove both (YAGNI) or implement the intended concurrency guard.

### MEDIUM — `block_in_place` + `block_on` on the TUI runtime

**File:** `crates/ragent-tui/src/app/slash.rs:6283, 6485, 6497`
`tokio::task::block_in_place` is used in slash-command handlers — panics on `current_thread` runtime and freezes the UI.
**Fix:** Spawn async work with `tokio::spawn` + event-bus delivery (same pattern as `/research`).

### MEDIUM — `copilot.rs` `hash_token` uses unstable `DefaultHasher`

**File:** `crates/ragent-llm/src/providers/copilot.rs:872`
`DefaultHasher` (SipHash-1-3) is not stable across Rust releases but is used as a persistent cache key.
**Fix:** Use a deterministic hash (xxhash, aHash with fixed seed, or SHA-256 prefix).

### LOW — `handle_event` (1640 lines) and `execute_slash_command_inner` (7980 lines)

**Files:** `crates/ragent-tui/src/app/event_handler.rs:171`, `slash.rs:1673`
These are the two largest functions in the codebase and are beyond reasonable cognitive complexity.
**Fix:** Extract each match arm into a named helper method. Large but mechanical; do incrementally.

### LOW — Duplicated status-string truncation logic

**Files:** `crates/ragent-tui/src/app/event_handler.rs:934, helpers.rs:91, event_handler.rs:1451`
"Truncate at char boundary to N then append ellipsis" reimplemented 3 times.
**Fix:** Add `truncate_at_char_boundary(s: &str, max: usize) -> String` helper.

### LOW — Duplicated `&id[..8.min(id.len())]` slice

**File:** `crates/ragent-tui/src/app/event_handler.rs` (9+ occurrences)
**Fix:** Add `fn short_id(id: &str) -> &str { &id[..8.min(id.len())] }`.

### LOW — SSE JSON parse failures silently discarded across all providers

**Files:** All provider SSE parsers (`anthropic.rs:464`, `openai.rs:463`, etc.)
`Err(_) => continue` discards malformed SSE data with no tracing.
**Fix:** Add `tracing::debug!` on the `Err` branch.