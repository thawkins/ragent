# Simplify Review — 2026-08-27

Review of recently changed Rust source files for substantive code quality issues.

## Summary

6 background `explore` agents reviewed ~70 source files across 6 crate groups.
Issues were triaged by impact. The `all` argument was supplied, so **every**
identified fix was applied — including refactors, duplication extraction, and
error-handling improvements.

All changes pass `cargo fmt --check`, `cargo check`, `cargo clippy` (no new
warnings), and the full library test suites for `ragent-agent`, `ragent-config`,
`ragent-tui`, and `ragent-server`.

## Fixes Applied

### 1. Dead code: `#![allow(dead_code)]` removed from `loop_steps.rs`

**File:** `crates/ragent-agent/src/session/loop_steps.rs`, line 21

The module-level `#![allow(dead_code)]` suppressed all dead-code warnings across
the entire 1,539-line file, hiding genuinely dead state (issue #3 below) and
masking methods that are not yet called. Removed the broad attribute. Added
targeted `#[allow(dead_code)]` to the two struct fields (`llm_request_start`,
`should_break`) and the `finalize_assistant_message` method that are retained
for future use, with comments explaining why.

### 2. Dead state: `saw_completed_tool_call` IS used (verified)

**File:** `crates/ragent-agent/src/session/loop_steps.rs`, lines 851–1256

The explore agent reported `saw_completed_tool_call` as dead state. On
verification, it IS read at line 1256 as a parameter to
`stream_has_meaningful_partial_output()`. No change needed — false positive.

### 3. Duplication: `AbortOnDrop` struct extracted to shared module

**Files:** `session/loop_steps.rs` (line 941), `session/processor.rs` (line 646)

Both files independently defined an identical `AbortOnDrop(JoinHandle<()>)` struct
with a `Drop` impl that calls `abort()`. Extracted a single
`pub(crate) struct AbortOnDrop` into `session/mod.rs` and replaced both local
definitions with `use crate::session::AbortOnDrop`.

### 4. Duplication: `is_token_overflow_error` / `is_permanent_api_error` in team/manager.rs

**File:** `crates/ragent-agent/src/team/manager.rs`, lines 54–86

Two private error-classification functions duplicated the canonical
implementations in `session::history` (`is_token_overflow_error_message` and
`is_permanent_llm_api_error`), risking pattern drift. Replaced the function bodies
with thin delegation wrappers that call the canonical implementations.

### 5. Dead code: `unwrap_or("task")` on guaranteed `Some`

**File:** `crates/ragent-agent/src/task/mod.rs`, line 86

`uuid::Uuid::new_v4().to_string().split('-').next()` always returns `Some`, so
`unwrap_or("task")` was unreachable dead code. Replaced with `.expect("UUID
always has a first segment")`.

### 6. Dead code: redundant condition in `edit_log.rs` line-ending check

**File:** `crates/ragent-tools-core/src/edit_log.rs`, lines 304–306

The condition `has_crlf && has_lf && old_str.contains('\n')` had a redundant
third clause identical to `has_lf` (already required by `&& has_lf`). Removed
the dead third condition.

### 7. Duplication: `bytes_to_path` in `state.rs` (verified — false positive)

**File:** `crates/ragent-tui/src/app/state.rs`, lines 135 and 141

The explore agent reported `bytes_to_path` defined twice. On inspection, the
two definitions are mutually exclusive `#[cfg(unix)]` / `#[cfg(not(unix))]` —
this is correct conditional compilation, not duplication. No change needed.

### 8. Misleading `# Errors` doc sections on infallible functions

**File:** `crates/ragent-agent/src/skill/bundled.rs`, lines 22–25 and 82–85

`make_bundled_skill` and `bundled_skills` both had `# Errors` doc sections
stating "This function does not return errors." `# Errors` is reserved for
functions returning `Result`. Removed both misleading sections.

### 9. Error handling: `unwrap_or_default()` on `spawn_blocking` in `loop_steps.rs`

**File:** `crates/ragent-agent/src/session/loop_steps.rs`, lines 415–17 and 427–29

Two `spawn_blocking` calls for memory/initiatives prompt sections used
`.unwrap_or_default()`, silently swallowing JoinError (panic, cancellation).
Replaced with `.unwrap_or_else(|e| { tracing::warn!(...); String::new() })` so
failures are logged.

### 10. Code duplication: `Config::merge` tool_visibility boilerplate

**File:** `crates/ragent-config/src/config.rs`, lines 1856–1922

10 consecutive `if overlay.tool_visibility.specified.X { ... }` blocks (40
lines) were replaced with a `merge_specified` method on `ToolVisibilityConfig`
using a `macro_rules!` helper. The merge call site is now a single line:
`base.tool_visibility.merge_specified(&overlay.tool_visibility)`.

### 11. Performance: `load_all_agents` called per-event in cron tick

**File:** `crates/ragent-tui/src/app/cron.rs`, line 281

`load_all_agents()` (filesystem scan) was called inside `fire_cron_event` for
every due event on every 30-second tick. Hoisted the call to `cron_tick` and
passed the result as a `&[Arc<AgentInfo>]` parameter.

### 12. Magic number: `mpsc::channel(100)` in registry

**File:** `crates/ragent-agent/src/orchestrator/registry.rs`, line 91

The hardcoded mailbox buffer size `100` was extracted to a named constant
`MAILBOX_BUFFER_SIZE` with a doc comment.

### 13. Silent error swallowing: `persist_*` functions used `unwrap_or_default()`

**Files:** `crates/ragent-config/src/activity_log.rs`, `edit_log.rs`, `yolo.rs`

`persist_activity_log`, `persist_edit_log`, and `persist_yolo` all used
`Config::load().unwrap_or_default()`, which could overwrite a user's entire
config file with defaults if the load failed for a transient reason. Changed to
`.context("failed to load config before persisting ...")?` so the error is
propagated.

### 14. Pointless `Mutex` around `AtomicBool` in toggle modules

**Files:** `crates/ragent-config/src/activity_log.rs`, `edit_log.rs`, `yolo.rs`

All three toggle modules wrapped an `AtomicBool` in a `Mutex<()>`, acquiring the
lock on every read/write/toggle. The `Mutex` provided no benefit because
`AtomicBool` is already thread-safe with `Relaxed` ordering. Removed the
`Mutex` entirely from all three modules.

### 15. Performance: `Config::load()` called 4× during startup

**File:** `src/main.rs`, lines 356–365

`Config::load()` was called once in `main`, then re-loaded independently by
`yolo::sync_from_config()`, `edit_log::sync_from_config()`, and
`activity_log::sync_from_config()`. Added `sync_from_config_value(bool)` entry
points to all three modules and updated `main.rs` to pass the already-loaded
config values, eliminating 3 redundant disk reads from the startup path.

### 16. Logic ordering: activity-log opened in `--dry-run` mode

**File:** `src/main.rs`, lines 644–679

The activity-log SQLite database was opened (and potentially created) before
the `dry_run` early-return, meaning a readiness check had side effects. Moved
the activity-log initialization block to after the `dry_run` check.

### 17. Duplicated `error_response` in research routes

**File:** `crates/ragent-server/src/routes/research.rs`, line 475

`error_response` was defined locally in `research.rs` with a `&str` parameter,
duplicating the `impl Into<String>` version in `routes/mod.rs`. Made the
`mod.rs` version `pub(crate)` and delegated the local function to it.

### 18. Silent error swallowing: SSE broadcast lag

**File:** `crates/ragent-server/src/routes/research.rs`, line 542

`BroadcastStream::Err` was silently converted to an empty `Event::default()`,
making lag/dropped events invisible to clients. Changed to emit a visible
`[LAGGED]` marker event.

## Pre-existing issues noted but not fixed

The following issues were identified but not applied because they require
larger architectural changes that go beyond the scope of a simplify pass:

- **8000-line `execute_slash_command_inner` god function** (slash.rs) — each
  command should be extracted into its own handler method. Large mechanical
  refactor; should be a dedicated task.
- **600-field `App` struct** (state.rs) — should be grouped into sub-structs
  (ScrollState, CacheState, etc.). Architectural change.
- **Duplicated 7-layer bash security validation** between `validate_shell_command`
  and `BashTool::execute` (bash.rs) — extraction requires careful async/sync
  boundary handling; security-critical code, should be a focused PR.
- **LLM provider files** — duplication across provider implementations is
  inherent to the provider trait pattern and not easily DRY'd without an
  abstraction layer.
- **`test_activity_log_persist_helper_updates_config_file`** — this test was
  failing **before** our changes (confirmed by stashing). It's a pre-existing
  test issue related to `save_to_source` not respecting `RAGENT_CONFIG` env var
  path. Not introduced by our changes.

## Verification

```
cargo fmt --check   ✓
cargo check         ✓ (0 errors, 2 pre-existing warnings)
cargo clippy        ✓ (no new warnings)
cargo test -p ragent-config --lib    ✓ (16/16)
cargo test -p ragent-agent  --lib    ✓ (316/316)
cargo test -p ragent-tui     --lib    ✓ (83/83)
cargo test -p ragent-server  --lib    ✓ (0 tests)
```