# Implementation Plan: `/spec jtbd` — JTBD Analysis Slash Command

## Overview

This plan implements the `jtbdresearch` specification. The work is divided into
parser changes in `ragent-specs`, dispatch and prompt-construction wiring in
`ragent-tui`, observability parity with `/spec create`, documentation updates,
and verification.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `Jtbd { spec_id, force, agent }` variant to `SpecCommand` and extend `parse()` | FR-001, FR-003, FR-004, FR-005 | M | Critical | completed | — |
| T-002 | Add builder helpers: `build_jtbd_prompt`, `build_jtbd_message`, `build_jtbd_status`, `build_jtbd_log` | FR-002, FR-006, FR-007, FR-011 | M | Critical | completed | T-001 |
| T-003 | Update help text (`build_help_message`) and argument-hint table in `session_ops.rs` | FR-010 | S | High | completed | T-001 |
| T-004 | Implement TUI dispatch arm for `Jtbd` in `crates/ragent-tui/src/app/slash.rs` | FR-002, FR-003, FR-004, FR-005, FR-008, FR-009, FR-011, FR-012 | M | Critical | completed | T-001, T-002 |
| T-005 | Add overwrite guard (FR-003) and `--force` bypass (FR-004) in the dispatch path | FR-003, FR-004 | S | High | completed | T-004 |
| T-006 | Add error handling for unknown/invalid spec name and unreadable `SPEC.md` (FR-008, FR-009) | FR-008, FR-009 | S | High | completed | T-004 |
| T-007 | Wire cancellation flag and incomplete-file cleanup (FR-014) | FR-013, FR-014 | M | High | completed | T-004 |
| T-008 | Unit tests for parser variants, force flag, agent flag, and rejection of bad names | FR-001, FR-003, FR-004, FR-005, FR-008, NFR-002 | M | Critical | completed | T-001 |
| T-009 | Integration test in `crates/ragent-tui/tests/` covering dispatch, guard, and error paths | FR-002, FR-003, FR-008, FR-011, NFR-002 | M | High | completed | T-004, T-005, T-006 |
| T-010 | Update `QUICKSTART.md` and spec how-to docs to mention `/spec jtbd` | NFR-003 | S | Medium | completed | T-003 |
| T-011 | Run `cargo test`, `cargo clippy`, and `cargo fmt`; verify no regressions to other `/spec` sub-commands | NFR-002, NFR-004 | S | Critical | completed | T-008, T-009 |
## Notes

- **T-001** keeps parser changes additive only; existing variants (`Create`,
  `Validate`, `List`, `Search`, `Status`, `Task`, `Help`) are untouched
  (NFR-004).
- **T-002** mirrors the `build_create_*` family so the LLM prompt is
  deterministic and testable; the prompt explicitly lists the JTBD sections
  required by FR-006 / FR-007.
- **T-004** intentionally reuses the `/spec create` dispatch skeleton
  (session ID, explore-agent lookup, cancellation flag, `tokio::spawn`) rather
  than introducing a new mechanism (FR-012).
- **T-005** performs the existence check synchronously before spawning; it
  does not rely on the agent to notice the file.
- **T-007** leverages the existing cancel flag; if the task aborts before
  writing, no `JTBD.md` is created. If a partial file is observed after
  cancellation, the dispatch arm appends the `<!-- incomplete -->` marker.
- **T-009** follows the pattern of `test_slash_spec_validate_all` in
  `crates/ragent-tui/tests/test_slash_commands.rs` (see line 2631).
- **T-011** is the gate before the spec can transition to `implemented`.

## Milestones

1. **Milestone 1 — Parser and prompt builders** (T-001, T-002, T-003)
   - `SpecCommand::parse` recognises `jtbd`, `--force`, `--agent`; help text
     updated.
2. **Milestone 2 — Dispatch and guards** (T-004, T-005, T-006, T-007)
   - `/spec jtbd <name>` dispatches the explore agent; overwrite guard and
     error handling work end-to-end.
3. **Milestone 3 — Tests and docs** (T-008, T-009, T-010, T-011)
   - Parser unit tests pass; integration test covers dispatch; docs updated;
     clippy/fmt clean.