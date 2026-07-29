# /simplify Review Report — Last 3 Commits

**Date:** 2025-07-24
**Scope:** Files changed in the last 3 commits (HEAD~3..HEAD)
**Reviewer:** RAgent `/simplify` skill

---

## Issues Fixed

### 1. Memory leak — `merged_prices()` (`crates/ragent-agent/src/cost/mod.rs:132`)

`Box::leak(entry.model.clone().into_boxed_str())` leaked every user-supplied
price override model string. Called once per `process_user_message` turn, so
leaks accumulated over long sessions with configured price overrides.

**Fix:** Changed `PriceTable` from `HashMap<&'static str, (f64, f64)>` to
`HashMap<String, (f64, f64)>`, and `UsageRecord::model_id` from `&'static str`
to `String`. Removed the `Box::leak` entirely — overrides are now inserted as
owned `String`s. Updated all built-in `table.insert(...)` call sites and tests
to use `.to_string()` / `.into()`.

### 2. Memory leak — `publish_run_cost_summary` closure (`crates/ragent-agent/src/session/processor.rs:541`)

`Box::leak(model_id.clone().into_boxed_str())` leaked the model ID string on
every agent run (once per user message). Over a long session with many turns
this leaks unbounded `&'static str` allocations.

**Fix:** Removed the `Box::leak` entirely. Since `UsageRecord::model_id` is
now `String`, the closure simply passes `model_id.clone()`.

### 3. Corrupted Unicode in comment (`crates/ragent-server/src/sse.rs:357`)

The `// ── Public API ──` separator comment had U+FFFD replacement characters
(corrupted bytes) mixed into the box-drawing dashes — likely from a bad
edit/merge operation.

**Fix:** Restored the line to clean box-drawing characters matching the
original from `HEAD~3`.

---

## Issues Noted but Not Fixed

### 4. Duplicated worst-status logic (`crates/ragent-agent/src/dry_run.rs:193–215`)

`compute_verdict()` and `section_status_from_items()` have near-identical
loop-and-match logic for computing the worst status from a list. This is a
new file but the duplication is minor (two 10-line functions operating on
different types — `ReadinessVerdict` vs `ReadinessStatus`). Extracting a
shared generic helper would add complexity without clear benefit since the
types differ. Left as-is.

---

## Verification

- `cargo check` — all crates pass
- `cargo clippy` — no warnings
- `cargo fmt --check` — clean
- `cargo test -p ragent-agent` (lib + `test_run_cost_summary`) — all pass
- `cargo test -p ragent-server` — all pass
- `cargo test -p ragent-research` — all pass