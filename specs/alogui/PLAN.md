# Implementation Plan — /alog Activity Log UI

Spec: `specs/alogui/SPEC.md`

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Register `alog` in `SLASH_COMMANDS` table | FR-001 | S | High | completed | — |
| T-002 | Add `/alog` dispatch arm in `execute_slash_command_inner` | FR-002 | S | High | completed | T-001 |
| T-003 | Implement `handle_alog_command` dispatcher skeleton | FR-002 | S | High | completed | T-002 |
| T-004 | Implement `/alog help` subcommand | FR-003 | S | High | completed | T-003 |
| T-005 | Resolve activity-log database path helper | FR-004, FR-005, FR-006 | S | High | completed | — |
| T-006 | Implement `/alog config` subcommand | FR-004 | M | Medium | completed | T-005 |
| T-007 | Implement `/alog list` subcommand (runs + per-kind counts) | FR-005 | L | High | completed | T-005 |
| T-008 | Implement `/alog status` subcommand (aggregate health) | FR-006 | M | High | completed | T-005 |
| T-009 | Run storage queries on a blocking thread | FR-008 | M | High | completed | T-006, T-007, T-008 |
| T-010 | Add `alog` subcommand autocomplete suggestions | FR-009 | S | Low | completed | T-003 |
| T-011 | Verify read-only guarantee for inspection subcommands | FR-010 | S | High | completed | T-006, T-007, T-008 |
| T-012 | Implement `/alog delete <run-id> --yes` subcommand | FR-011, FR-015 | M | High | completed | T-003, T-005 |
| T-013 | Add `--yes` flag guard and missing-flag warning for delete | FR-011, NFR-001, NFR-002 | S | Critical | completed | T-012 |
| T-014 | Validate `<run-id>` argument presence and syntax for delete | FR-012 | S | High | completed | T-012 |
| T-015 | Reject deletion of unknown run-id with warning | FR-013 | S | High | completed | T-012 |
| T-016 | Show post-delete confirmation (run-id + events removed) | FR-014 | S | Medium | completed | T-012 |
| T-017 | Run delete storage operation on a blocking thread | FR-015 | M | High | completed | T-012 |
| T-018 | Add `delete` row to `/alog help` table | FR-016 | S | Medium | completed | T-004, T-012 |
| T-019 | Add `delete` to `/alog` subcommand autocomplete | FR-017 | S | Low | completed | T-010, T-012 |
| T-020 | Verify `--yes` guard blocks all non-confirmed delete paths | NFR-001 | S | Critical | completed | T-012, T-013 |
| T-021 | Verify irreversibility warning in help and missing-flag msg | NFR-002 | S | High | completed | T-013, T-018 |
| T-022 | Implement `/alog export <run-id> --yes` subcommand | FR-018, FR-021, FR-023 | M | High | completed | T-003, T-005 |
| T-023 | Add `--yes` flag guard and missing-flag warning for export | FR-018, NFR-003 | S | Critical | completed | T-022 |
| T-024 | Validate `<run-id>` argument presence and syntax for export | FR-019 | S | High | completed | T-022 |
| T-025 | Reject export of unknown run-id with warning | FR-020 | S | High | completed | T-022 |
| T-026 | Create `log/exports/` dir and write `export-<run-id>.jsonl` | FR-021 | M | High | completed | T-022 |
| T-027 | Show post-export confirmation (run-id, events, file path) | FR-022 | S | Medium | completed | T-022, T-026 |
| T-028 | Run export storage operation on a blocking thread | FR-023 | M | High | completed | T-022 |
| T-029 | Add `export` row to `/alog help` table | FR-024 | S | Medium | completed | T-004, T-022 |
| T-030 | Add `export` to `/alog` subcommand autocomplete | FR-025 | S | Low | completed | T-010, T-022 |
| T-031 | Verify `--yes` guard blocks all non-confirmed export paths | NFR-003 | S | Critical | completed | T-022, T-023 |
| T-032 | Verify export does not mutate or delete log events | NFR-004 | S | High | completed | T-022, T-026 |
| T-033 | Verify output rendering and status bar for all `/alog` cmds | FR-007 | S | Medium | completed | T-004, T-006, T-007, T-008, T-012, T-022 |
| T-034 | Manual test pass against `TESTPLAN.md` | FR-001–FR-025, NFR-001–NFR-004 | S | Medium | completed | T-004, T-006, T-007, T-008, T-012, T-013, T-014, T-015, T-016, T-017, T-018, T-019, T-022, T-023, T-024, T-025, T-026, T-027, T-028, T-029, T-030 |
## Notes

- All work lives in `crates/ragent-tui/src/app/slash.rs` (dispatch + handler
  functions) and `crates/ragent-tui/src/app/state.rs` (`SLASH_COMMANDS` table).
- `ragent-tui` already depends on `ragent-storage`, so `ActivityLog::open`
  and the `ragent_types::activity` types are available without new crate deps.
- The activity-log database path follows the same convention as the main
  database: `dirs::data_dir().join("ragent").join(<activity-log file>)`. The
  exact filename used by the running system should be confirmed during
  T-005; if the main process does not yet open an `ActivityLog` at startup,
  T-005 documents the agreed path so `/alog` can open a read-only view of the
  same database the writer uses.
- Per-`EventKind` counting (T-007) iterates `read_run()` events and tallies by
  variant — no new storage API is required.
- T-009 uses `tokio::task::spawn_blocking` (or the existing blocking helper
  pattern used by other slash commands) so the async event loop is never
  blocked by SQLite reads.
- The `/alog delete` subcommand (T-012) breaks the read-only guarantee of
  FR-010. This is intentional: FR-010 applies to the inspection subcommands
  (`help`, `config`, `list`, `status`); `delete` is an explicit, opt-in
  destructive operation gated behind the mandatory `--yes` flag (FR-011,
  NFR-001). The implementation review for T-020 must confirm that no other
  `/alog` subcommand accidentally triggers a mutating storage call.
- T-012/T-017 use `ActivityLog::expire_run` (or an equivalent delete-all
  query) to remove all events for the given `RunId`. Unlike `archive_run`,
  `delete` does not export a JSONL copy first — deletion is irreversible
  (NFR-002).
- The `/alog export` subcommand (T-022) uses `ActivityLog::export_jsonl` (or
  `export_jsonl_to`) to serialise the run's events to JSON Lines. Unlike
  `delete`, export is non-destructive (NFR-004): it writes a read-only copy
  to `log/exports/export-<run-id>.jsonl` and leaves the database intact. The
  `--yes` flag is mandatory (FR-018, NFR-003) to guard against accidental
  file writes, consistent with the `delete` safety pattern.
- T-026 creates the `log/exports/` directory (under the current working
  directory) with `std::fs::create_dir_all` before writing the export file.