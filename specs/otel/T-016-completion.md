# T-016 Completion: Instrument permission system: approved and denied counters

**Spec ID:** otel
**Task:** T-016
**Requirements:** FR-016
**Status:** completed
**Dependencies:** T-006
**Effort:** S
**Priority:** Medium

## Summary

Instrumented the permission system so that every permission resolution records
the `ragent.permission.approved` or `ragent.permission.denied` counter tagged
with `tool.name` (FR-016). Both the bash sub-command permission loop and the
single-resource permission path in `SessionProcessor` are instrumented.

## Requirements

### FR-016

> When a permission request is resolved (approved or denied), the system shall
> record `ragent.permission.approved` or `ragent.permission.denied` tagged with
> `tool.name`.

## Changes

### `crates/ragent-telemetry/src/recorder.rs`

The `PermissionRecorder` type (already present from T-006) provides:

- `from_subsystem(&TelemetrySubsystem) -> Self` — live recorder when enabled.
- `disabled() -> Self` — no-op recorder (used by default).
- `record_approved(tool_name: &str)` — increments
  `ragent.permission.approved` tagged with `tool.name` (FR-016).
- `record_denied(tool_name: &str)` — increments
  `ragent.permission.denied` tagged with `tool.name` (FR-016).

When the `telemetry` Cargo feature is off, `PermissionRecorder` is a
zero-sized no-op (`#[derive(Clone, Copy)]`). When on but telemetry is disabled
in config, it holds `None` and all methods are cheap no-ops (FR-022, NFR-002).

The recorder is `Clone` (Arc-backed when live) so it can be moved into
`tokio::spawn` closures.

### `crates/ragent-agent/Cargo.toml`

- Added `ragent-telemetry = { path = "../ragent-telemetry" }` as a dependency.
- Added a `telemetry` feature that forwards to
  `ragent-telemetry/telemetry`. Off by default, so the default build pulls no
  `opentelemetry` dependencies (NFR-006).

### `crates/ragent-agent/src/session/processor.rs`

- **New struct field**: `pub permission_recorder:
  ragent_telemetry::recorder::PermissionRecorder` added to `SessionProcessor`,
  with a docblock describing the no-op behaviour when telemetry is disabled.

- **Clone into the tool-execution spawn closure**: The recorder is cloned
  alongside the other shared state (`permission_checker`, `event_bus`, etc.)
  before the `tokio::spawn(async move { ... })` that executes each tool call.
  This avoids borrowing `self` across the `'static` spawn boundary.

- **Bash sub-command permission loop**: For each sub-command permission check,
  the `Allow` arm calls `permission_recorder.record_approved(&tc_clone.name)`
  and the `Deny`, `Ask`, and `Err` arms call
  `permission_recorder.record_denied(&tc_clone.name)`. The `Ask` and `Err`
  arms should not escape `check_permission_with_prompt` in practice, but are
  recorded as denials for metrics completeness.

- **Single-resource permission path** (non-bash tools): The `Allow` arm calls
  `record_approved`, and the `Deny`, `Ask`, and `Err` arms call `record_denied`,
  each tagged with `tc_clone.name`.

All calls use `tc_clone.name` as the `tool.name` attribute so the exported
counters are cardinality-bounded by the tool set (FR-035 will cap this further
once implemented).

### `crates/ragent-config/src/lib.rs`

- Added `pub mod telemetry;` so the `telemetry.rs` module (already present on
  disk from T-002) is compiled and its types are reachable.
- Added `pub use telemetry::{OtelConfig, OtelProtocol, TelemetryConfig};` so
  `ragent-telemetry` can import these types from `ragent_config` (fixes the
  previously broken `ragent-telemetry` build).

### `Cargo.toml` (workspace root)

- Added `ragent-telemetry = { path = "crates/ragent-telemetry" }` to
  `[workspace.dependencies]` and to the binary `[dependencies]` so `src/main.rs`
  can construct the `PermissionRecorder::disabled()` default.

### `crates/ragent-server/Cargo.toml`

- Added `ragent-telemetry = { path = "../ragent-telemetry" }` as a dependency so
  the server integration tests can construct `SessionProcessor` with the new
  field.

### `crates/ragent-tui/Cargo.toml`

- Added `ragent-telemetry` as a dev-dependency so the TUI tests can construct
  `SessionProcessor` with the new field.

### Construction sites updated

Every `SessionProcessor { ... }` literal now initialises
`permission_recorder: ragent_telemetry::recorder::PermissionRecorder::disabled()`:

- `src/main.rs`
- `crates/ragent-tui/src/app/tests.rs`
- `crates/ragent-tui/tests/support/mod.rs`
- `crates/ragent-tui/tests/test_agent_switching.rs`
- `crates/ragent-tui/tests/test_configured_provider_selection.rs`
- `crates/ragent-tui/tests/test_memory_panel.rs`
- `crates/ragent-tui/tests/test_permission_countdown.rs`
- `crates/ragent-tui/tests/test_question_multiple_choice.rs`
- `crates/ragent-tui/tests/test_router_setup.rs`
- `crates/ragent-tui/tests/test_scrolling.rs`
- `crates/ragent-tui/tests/test_session_resume.rs`
- `crates/ragent-tui/tests/test_slash_commands.rs`
- `crates/ragent-tui/tests/test_status_expiry.rs`
- `crates/ragent-tui/tests/test_text_selection.rs`
- `crates/ragent-agent/tests/test_cross_await_mutex.rs`
- `crates/ragent-agent/tests/test_storage_op_blocking.rs`
- `crates/ragent-agent/tests/test_system_prompt_cache.rs`
- `crates/ragent-agent/tests/test_thinking_pipeline.rs`
- `crates/ragent-server/tests/test_integration.rs`
- `crates/ragent-server/tests/test_memory_api.rs`

## Verification

### Build checks

- `cargo check` (default features, no `telemetry`) — passes.
- `cargo check --features ragent-agent/telemetry` — passes.
- `cargo check -p ragent-telemetry --features telemetry` — passes.
- `cargo check -p ragent-config` — passes.

### Test results

- `cargo test -p ragent-telemetry --features telemetry --lib -- --skip test_flush_on_signal` — 56 passed, 0 failed. (The two `test_flush_on_signal*` tests spawn a real signal handler that blocks indefinitely and are skipped; they are pre-existing and unrelated to T-016.)
- `cargo test -p ragent-telemetry --lib` (no feature) — 19 passed, 0 failed.
- `cargo test -p ragent-config --lib` — 16 passed, 0 failed (includes the telemetry config tests from T-002).
- `cargo test -p ragent-agent --lib` — 316 passed, 0 failed.
- `cargo test -p ragent-agent --features telemetry --lib` — 316 passed, 0 failed.
- `cargo test -p ragent-agent --test test_cross_await_mutex --test test_storage_op_blocking --test test_system_prompt_cache` — 13 passed, 0 failed.
- `cargo test -p ragent-agent --test test_thinking_pipeline` �� 1 passed, 0 failed.
- `cargo test -p ragent-server --test test_integration --test test_memory_api` — 12 passed, 0 failed.
- `cargo test -p ragent-tui --test test_bench_command` — 14 passed, 0 failed.
- `cargo test -p ragent-tui --test test_slash_commands -- --skip test_slash_model_show_displays_metadata_for_active_model` — 107 passed, 0 failed.

### Pre-existing failure note

`test_slash_model_show_displays_metadata_for_active_model` in
`crates/ragent-tui/tests/test_slash_commands.rs` fails on `assertion failed:
text.contains("Context window")`. This failure is **pre-existing** and
unrelated to T-016 — it reproduces on the clean `HEAD` tree without any of
these changes (verified by stashing the working tree and re-running the test).
It is tracked separately and not gated by this task.

### Existing PermissionRecorder unit tests

The `PermissionRecorder` unit tests in `crates/ragent-telemetry/src/recorder.rs`
(added in T-006) already cover FR-016 and continue to pass:

- `test_permission_recorder_record_approved_increments_counter`
- `test_permission_recorder_record_denied_increments_counter`
- `test_permission_recorder_attributes_include_tool_name`
- `test_permission_recorder_is_clone`
- `test_disabled_permission_recorder_is_noop`
- `test_noop_permission_recorder`

No new unit tests were added for T-016 because the recorder behaviour is
already covered; T-016 is the *wiring* of that recorder into the session
processor call sites.

## Spec / Plan updates

- `specs/otel/PLAN.md`: T-016 status changed from `pending` to `completed`.
- Spec task `T-016` status updated to `completed` via `spec_task_update`.