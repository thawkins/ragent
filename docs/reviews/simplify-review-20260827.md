# /simplify Review — 2026-08-27

Scope: Rust source files changed in the last 3 commits (`git diff --name-only HEAD~3`),
reviewed for substantive code-quality issues (performance, dead code, duplication,
complexity, error handling). Style/formatting nits are excluded.

Mode: default (safe/straightforward fixes applied). Verified with `cargo fmt --check`,
`cargo check`, and `cargo clippy` (all pass; the only warnings are pre-existing in
vendored `pdf-extract` and an external future-incompat crate, unrelated to these edits).

---

## Fixes applied

### 1. `crates/ragent-agent/src/session/mod.rs` — `remove_session_state` was a silent no-op (bug, R-3)

**Problem:** `session_state_cache` and `remove_session_state` each declared their own
function-local `static CACHE`. Function-local statics are distinct items per
declaration site, so `remove_session_state` referenced a *different* empty map and
never removed anything — the comment above it even warned about this exact hazard,
yet the code did it anyway. The global session-state cache grew unbounded whenever a
session was archived or a sub-agent completed (R-3 violation).

**Fix:** Hoisted a single module-scope `static SESSION_STATE_CACHE: OnceLock<Mutex<HashMap<...>>>`
and both methods now reference it, so eviction works.

### 2. `crates/ragent-agent/src/orchestrator/coordinator.rs` — dead `JobEntry._id` field

**Problem:** `JobEntry._id` was written (in `start_job_async`) but never read; it was
underscored to silence the lint. The job id is already the `DashMap` key.

**Fix:** Removed the field and its write site. Also collapsed a `match router.send().await {
Ok(resp) => Ok((agent_id, resp)), Err(e) => Err(e) }` into `.map(|resp| (agent_id, resp))`.

### 3. `crates/ragent-tools-core/src/bash.rs` — allocation in hot `is_safe_command` path

**Problem:** `trimmed.starts_with(&format!("{safe} "))` allocated a fresh `String` per
allowlist entry per command validation (up to one allocation per safe-command per
invocation on every bash call).

**Fix:** Replaced with a non-allocating `strip_prefix(safe).is_some_and(|r| r.starts_with(' '))`.

### 4. `crates/ragent-tools-core/src/askpass.rs` — duplicated `safe_session_id`

**Problem:** `askpass.rs` re-implemented `bash::safe_session_id` character-for-character,
even though its own doc comment said it "re-uses the bash module's sanitizer."

**Fix:** Deleted the local copy and call `crate::bash::safe_session_id(session_id)`.

### 5. `crates/ragent-tui/src/layout_statusbar.rs` — identical `Compact`/`Minimal` match arms

**Problem:** Both arms computed the exact same percentage string; they would drift if
only one were edited.

**Fix:** Collapsed to `ResponsiveMode::Compact | ResponsiveMode::Minimal => { ... }`.

### 6. `crates/ragent-config/src/bash_lists.rs` — `to_string()` allocations for `contains`

**Problem:** `g.allowlist.contains(&entry.to_string())` and the denylist equivalent
allocated a `String` just to look up an existing element.

**Fix:** Replaced with `g.allowlist.iter().any(|e| e == entry)` (non-allocating).

### 7. `crates/ragent-tools-core/src/read.rs` — `.expect()` on cache locks

**Problem:** `read_cache().lock().expect("cache poisoned")` would panic a tool call if
the cache mutex were poisoned. `cached_read` already returns `Result`.

**Fix:** `.map_err(|e| anyhow::anyhow!("cache poisoned: {e}"))?` so a lock failure is a
recoverable tool error.

### 8. `crates/ragent-tui/src/app/event_handler.rs` — production `.expect("just pushed")`

**Problem:** `self.research_progress.last_mut().expect("just pushed")` relied on the
immediately-preceding `push`; a race would panic the TUI.

**Fix:** Guarded with a `let Some(last) = ... else { error!; return }` that logs instead
of panicking.

### 9. `crates/ragent-tui/src/layout.rs` — production `.unwrap()` on link/image state

**Problem:** `link_state.as_mut().unwrap().1` / `image_state.as_mut().unwrap().1` assumed
the `Option` was populated; a logic regression would crash the renderer.

**Fix:** Replaced with `if let Some(ls) = link_state.as_mut()` / `if let Some(is) = image_state.as_mut()`.

### 10. `crates/ragent-tui/src/app/cron.rs` — `.expect()` on runtime parse + double `get_task`

**Problem:** (a) `parse_from_rfc3339(...).expect("valid far-future timestamp")` sat in a
production path; (b) the completion monitor called `agent_manager.get_task(&task_id)`
twice for the same task (once to check `done`, once to read the result).

**Fix:** (a) replaced the `.expect` with an `if let Ok(far_future)` guard; (b) kept the
first `get_task` result, matched on `&task`, and reused the borrowed entry in the
stateful branch — removing the redundant second query.

### 11. `crates/ragent-tui/src/app/session_ops.rs` — `parse_refs` computed twice

**Problem:** `parse_refs(&text)` was called once to test emptiness and a second time to
extract names — doubling tokenising work on every user send.

**Fix:** Parse once into `refs`, derive `has_refs` from it, and reuse for the name list.

### 12. `crates/ragent-tui/src/app/helpers.rs` — duplicated tail-8 helpers

**Problem:** `short_session_id` and `short_run_id` shared identical "last 8 chars" logic.

**Fix:** Extracted a private `tail8(s: &str)` and have both call it.

---

## Findings reviewed but not applied (default mode)

These were larger refactors deferred per the default (safe-only) mode. **All items
below have since been implemented** (2026-08-27) — see
`docs/reviews/simplify-review-20260827-applied.md` for the details of each change
and its verification:

- **`bash.rs:1193-1477`** — `execute` was a ~285-line function mixing validation, command
  build, output truncation, and formatting. Extracted `build_shell_command` and
  `truncate_output`.
- **`bash.rs` shell-type match** — duplicated between `spawn_background_shell` and `execute`
  is now a single `build_shell_command` helper shared by both.
- **`activity_log.rs` / `edit_log.rs` / `yolo.rs`** — three near-identical toggle modules
  deduplicated via a new shared `RuntimeFlag` helper in `ragent-config`.
- **`bg.rs`** — ten identical `lock().expect("background inner lock poisoned")` sites
  replaced with recoverable lock handling; the O(n²) `replace_range` trim in
  `append_with_cap` switched to `drain(..overflow)`.
- **`bash_lists.rs`** — poison handling now `tracing::warn!`s on the read paths that
  previously returned an empty list silently.
- **`coordinator.rs`** — `active_jobs` leak on the empty-match early-return fixed with an
  `ActiveJobsGuard` (matching the async path).
- **`task/mod.rs`** — `suspend_task`/`resume_task` stub docs tightened; model-override
  resolution deduplicated into a shared `apply_model_override` helper.
- **`research/`** — `digest.rs` now lowercases each source body once; a shared
  `polarity::cited_indices` helper replaces the duplicated extraction in `verify.rs` /
  `synthesis.rs` / `cite_checker.rs`.
- **`session/loop_steps.rs`** — duplicated `spawn_blocking` + error-swallow pattern
  extracted into `spawn_blocking_section`; the `generic_openai`/`azure_foundry` base_url
  arms collapsed into a shared `resolve_api_base` helper.

---

## Verification

- `cargo fmt --check` — passes
- `cargo check -p ragent-agent -p ragent-tools-core -p ragent-config -p ragent-tui` — passes
- `cargo clippy` (same crates) — no new warnings from these changes

## Applied-follow-up verification (2026-08-27)

After implementing the deferred findings (see above):

- `cargo fmt --check` — passes
- `cargo check -p ragent-config -p ragent-tools-core -p ragent-agent -p ragent-research` — passes
- `cargo clippy` (same crates) — no new warnings (the only warnings are the pre-existing
  vendored `pdf-extract` `NAN` deprecation and the external future-incompat crate)
- `cargo test -p ragent-config` (activity_log + yolo persistence) — passes
- `cargo test -p ragent-tools-core` — passes
- `cargo test -p ragent-agent --lib` (orchestrator + task) — passes
- `cargo test -p ragent-research --lib` (606 tests) — passes
