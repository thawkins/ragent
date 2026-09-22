# Implementation Plan — /toolchain Runtime Toolchain Report

Spec: `specs/toolchain/SPEC.md`

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Create `toolchain` module with language-classification + runtime mapping consts | FR-005, FR-006 | M | High | completed | — |
| T-002 | Add mapping-exhaustiveness guarantee against `SUPPORTED_LANGUAGES` | FR-005 | S | High | completed | T-001 |
| T-003 | Implement PATH presence probe helper | FR-007 | S | High | completed | — |
| T-004 | Implement version-probe helper with per-tool flags and timeout | FR-008, FR-012 | M | High | completed | T-003 |
| T-005 | Implement report renderer (markdown table + summary line) | FR-009, FR-005 | M | High | completed | T-001 |
| T-006 | Implement JSON report renderer | FR-015 | S | Medium | completed | T-005 |
| T-007 | Register `toolchain` in `SLASH_COMMANDS` and help index | FR-001 | S | High | completed | — |
| T-008 | Add `toolchain` dispatch arm in `execute_slash_command_inner` | FR-002 | S | High | completed | T-007 |
| T-009 | Implement `handle_toolchain_command` dispatcher (help/usage/unknown) | FR-002, FR-003, FR-014 | S | High | completed | T-008 |
| T-010 | Implement `/toolchain help` page | FR-003 | S | High | completed | T-009 |
| T-011 | Implement `/toolchain list` full walk + probe + render | FR-004, FR-006, FR-009, FR-010 | M | Critical | completed | T-003, T-004, T-005 |
| T-012 | Implement language-filter argument (`list <lang>`) with unknown-id warning | FR-011 | S | High | completed | T-011 |
| T-013 | Implement `--json` flag handling | FR-015 | S | Medium | completed | T-006, T-011 |
| T-014 | Run probes on blocking thread with `[wait]` status indicator | FR-016, NFR-001 | M | High | completed | T-011 |
| T-015 | Verify absent-runtime continuation (no truncation) | FR-010, FR-012 | S | High | completed | T-011 |
| T-016 | Verify read-only guarantee (no writes, no config mutation) | FR-013 | S | High | completed | T-011 |
| T-017 | Add `toolchain` autocomplete suggestions | FR-001 | S | Low | completed | T-009 |
| T-018 | Add `/toolchain` row to `/help` command index | FR-001 | S | Medium | completed | T-007 |
| T-019 | Manual test pass against `TESTPLAN.md` | FR-001–FR-016 | S | Medium | completed | T-010, T-011, T-012, T-013 |
## Notes

- All dispatch/UI work lives in `crates/ragent-tui/src/app/slash.rs`
  (dispatch arm + handler methods + suggestions) and
  `crates/ragent-tui/src/app/state.rs` (`SLASH_COMMANDS` table). The probe
  engine and renderers live in a new focused module
  (`crates/ragent-tui/src/app/toolchain.rs`) so the mapping table and probe
  helpers are testable independently of the TUI.
- `ragent-tui` already depends on `ragent-codeindex` (the `/codeindex` arm
  constructs `CodeIndex` directly), so `SUPPORTED_LANGUAGES` is available
  without a new crate edge.
- Probe commands are compile-time constants (NFR-003); the language filter is
  matched case-insensitively against the scanner ids and never reaches a
  command line.
- T-002 implements the FR-005 exhaustiveness requirement as a first-principles
  loop over `SUPPORTED_LANGUAGES` (every id must classify) plus a registry
  check that the classification set contains no ids outside the const.
- T-014 follows the existing async-slash-command pattern: set status to
  `[wait] toolchain`, `tokio::task::spawn_blocking` the probe loop, emit the
  rendered report on completion; `execute_slash_command` already defers the
  `Finished` log line while status starts with `[wait]`.
- Per-tool version flags with exceptions (`java -version`, `erl -version`,
  `R --version` multi-line) are captured in the FR-006 mapping table, not
  sprinkled in probe code.
- Data-format rows are rendered with the `(data format — runtime n/a)`
  marker so the full 50-entry language surface remains visible without
  probing (FR-005).
- No HTTP/CLI surface in this spec; the module boundary (pure
  classify/probe/render functions) keeps a later `ragent toolchain` CLI or
  `/toolchain` REST endpoint cheap to add.