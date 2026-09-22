# Implementation Plan: OpenCode-Derived Context Compaction

This plan migrates ragent from the `headroom-core`-based compression pipeline
to a Rust port of OpenCode's summarisation-based compaction. Each task maps to
one or more requirements in [`SPEC.md`](SPEC.md).

## Summary

| Item | Value |
|------|-------|
| Spec ID | `compact` |
| Default status after migration | `auto: true` |
| Core new crate module | `ragent-agent::session::compaction` |
| Config schema | replace `CompressionConfig` with `CompactionConfig` |
| Removed dependency | `headroom-core` |
| Removed concepts | CCR markers, `headroom_retrieve`, content-type compressors |

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Audit and document current Headroom usage | FR-012, FR-013 | S | High | completed | — |
| T-002 | Design `CompactionConfig` in `ragent-config` | FR-008, FR-011 | S | High | completed | T-001 |
| T-003 | Add `compaction` message type to storage and events | FR-005, FR-007 | M | High | completed | T-001 |
| T-004 | Implement conversation serialiser for compaction prompts | FR-014 | M | High | completed | T-001 |
| T-005 | Implement token estimator and compaction trigger | FR-002, FR-003 | M | High | completed | T-002 |
| T-006 | Port OpenCode summarisation prompt template | FR-003, FR-010 | M | High | completed | T-004 |
| T-007 | Implement compaction runner and message replacement | FR-005, FR-007 | L | Critical | completed | T-003, T-005, T-006 |
| T-008 | Integrate compaction into agent loop pre-send path | FR-003, FR-006 | L | Critical | completed | T-005, T-007 |
| T-009 | Integrate emergency overflow compaction and single retry | FR-004 | M | High | completed | T-007, T-008 |
| T-010 | Replace `/compress` slash command with `/compact` | FR-009 | M | Medium | completed | T-007 |
| T-011 | Remove Headroom dependency, CCR store, and pipeline | FR-012, FR-013 | L | High | completed | T-007, T-008, T-009 |
| T-012 | Write unit and integration tests for compaction flow | All FR | L | High | pending | T-004–T-009 |
| T-013 | Update user documentation and `ragent.json` schema examples | FR-011 | S | Medium | completed | T-002, T-010 |
## Task details

### T-001 — Audit and document current Headroom usage

- List every call site of `headroom_core`, `headroom-core`, `CcrStoreHandle`,
  `compress_history`, `compress_chat_messages`, `CompressionStarted/Finished`
  events, and `headroom_retrieve` (if any tool exposes it).
- Produce an internal markdown map of files, symbols, and removal order.
- Identify tests and benchmarks that will be deleted or rewritten.

### T-002 — Design `CompactionConfig` in `ragent-config`

Replace `CompressionConfig` (and its sub-types) with a minimal schema matching
OpenCode's `ConfigCompaction.Info`:

```json
{
  "compaction": {
    "auto": true,
    "buffer": 20000,
    "keep": { "tokens": 8000 }
  }
}
```

- Fields: `auto: Option<bool>`, `buffer: Option<usize>`, `keep.tokens: Option<usize>`.
- Defaults mirror OpenCode: `auto = true`, `buffer = 20_000`, `keep.tokens = 8_000`.
- Update `Config` root struct and JSON tests.
- Mark old `compression` fields as deprecated aliases if backward compatibility
  is required for one release, otherwise remove them.

### T-003 ��� Add `compaction` message type to storage and events

- Add `Compaction` variant to `ragent_types::event::Event` carrying
  `session_id`, `original_tokens`, `compressed_tokens`, `compression_ratio`,
  `did_compress`, `reason` (auto / overflow / manual).
- Add `Role::Compaction` or a `type` discriminator on `Message` so SQLite can
  store compaction summaries distinctly from user/assistant/system messages.
- Ensure `Storage::get_messages` orders compaction messages correctly with
  respect to `created_at`.

### T-004 — Implement conversation serialiser for compaction prompts

Port `SessionCompaction.serialize` from OpenCode:

- `user` → `[User]: <text>` plus file attachments.
- `assistant` → `[Assistant]: <text>`, `[Assistant reasoning]: <reasoning>`,
  `[Assistant tool call]: name(input)`, `[Tool result]: <truncated text>`.
- `system` → `[System update]: <text>`.
- Tool output truncation at a configured character cap (default 2_000).
- Keep every message on a single pass; do not drop tool parts silently.

### T-005 — Implement token estimator and compaction trigger

- Port `Token.estimate` using characters/4 heuristic for the fast path.
- Provide a function `should_compact(request, context_window, buffer, reported_input_tokens) -> bool`
  that uses provider-reported tokens when available, otherwise the heuristic.
- Expose `select_turns_to_keep(entries, keep_tokens)` matching OpenCode's
  `select()` so recent verbatim turns are chosen deterministically.

### T-006 — Port OpenCode summarisation prompt template

- Define a constant `COMPACTION_SUMMARY_TEMPLATE` in Rust with the exact
  Markdown sections from OpenCode:
  `Objective`, `Important Details`, `Work State` (Completed/Active/Blocked),
  `Next Move`, `Relevant Files`.
- Implement `build_prompt(previous_summary, context)` that injects a previous
  summary update instruction when one exists.
- Configure summary output tokens (default 1_500) and guard against the prompt
  itself exceeding `context_window - summary_output`.

### T-007 — Implement compaction runner and message replacement

- New module `ragent_agent::session::compaction` with a `CompactionService`
  or free functions:
  - `compact_if_needed(...)` for pre-send path.
  - `compact_after_overflow(...)` for emergency path.
- On success, persist a single `Message` of the new compaction type with
  fields: `summary`, `recent` (verbatim recent turns), `seq` (monotonic
  message id/sequence).
- Delete or archive all messages older than the compaction sequence for that
  session (or keep them for audit if storage cost is acceptable; the runner
  must only load from the compaction point onward).

### T-008 — Integrate compaction into agent loop pre-send path

- Modify `crates/ragent-agent/src/session/processor.rs` and/or
  `loop_steps.rs` to call `compact_if_needed` before the LLM request when
  `compaction.auto` is true.
- After compaction, rebuild `chat_messages` from the latest compaction plus
  subsequent messages, update `last_reported_input_tokens` to 0, and
  continue the current turn.
- Publish `CompactionStarted` and `CompactionFinished` events.

### T-009 — Integrate emergency overflow compaction and single retry

- Detect context-overflow failures using the existing
  `is_token_overflow_error_message` helper.
- If overflow occurs before assistant output starts, call
  `compact_after_overflow` and retry the turn exactly once.
- If the retry still overflows, surface the error to the user.
- Ensure only one overflow recovery per turn to avoid infinite loops.

### T-010 — Replace `/compress` slash command with `/compact`

- In `crates/ragent-tui/src/app/slash.rs`, rename the command entry and
  update help text.
- In `crates/ragent-tui/src/app/compress.rs`, implement
  `handle_compact_command` using the new compaction service instead of the
  Headroom pipeline.
- Update `compress.rs` filename or module name to `compact.rs`.
- Preserve telemetry counters (`context_compressions`, compression ratio)
  where applicable.

### T-011 — Remove Headroom dependency, CCR store, and pipeline

- Delete `crates/ragent-agent/src/compression/` module tree.
- Remove `headroom-core` from `crates/ragent-agent/Cargo.toml`.
- Delete `ragent-config/src/compression.rs` or replace it with the new
  `compaction.rs`.
- Remove CCR marker parsing, `headroom_retrieve` tool registration, and
  `CompressionMode`.
- Clean up any remaining `headroom` re-exports or benchmark references.
- Run `cargo check` and `cargo test` for the full workspace.

### T-012 — Write unit and integration tests

- Test token estimator against known char/token counts.
- Test `select_turns_to_keep` edge cases (empty history, keep all, split one
  long turn).
- Test prompt construction with and without previous summary.
- Integration test: simulate a session whose total tokens exceed a small
  context window, verify compaction message is produced and subsequent turn
  loads only from compaction.
- Test emergency overflow path with a mock provider that returns a context
  exceeded error.

### T-013 — Update user documentation and config examples

- Update `SPEC.md` root, `QUICKSTART.md`, and any `docs/` references to
  `compression` with new `compaction` settings.
- Add example `ragent.json` snippets for `compaction`.
- Document `/compact` slash command and removed `/compress` modes.

## Migration notes

- Keep the change behind a single feature branch; do not attempt a partial
  migration where both Headroom and OpenCode compaction coexist.
- If users currently have `compression.enabled = true`, silently treat that as
  `compaction.auto = true` for one release if backward compatibility is
  desired, then remove the alias.
- The CCR store removal means previously compressed sessions lose the ability
  to retrieve original stashed content. Summarised sessions are not
  byte-reversible by design, so this is acceptable.
- Telemetry event names may remain `CompressionStarted`/`CompressionFinished`
  for compatibility with existing TUI handlers, even though the underlying
  mechanism is now summarisation.

## Definition of done

- `cargo build`, `cargo test`, `cargo clippy`, and `cargo fmt --check` all pass.
- No references to `headroom-core`, `headroom_core`, `<<ccr:`, or
  `headroom_retrieve` remain in the source tree.
- New compaction path is covered by at least one integration test.
- `ragent.json` schema supports the new `compaction` section.
- `/compact` slash command works in the TUI and reports token savings.