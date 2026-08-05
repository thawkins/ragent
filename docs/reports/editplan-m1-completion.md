# EDITPLAN Milestones 1+2 — Completion Log

**Date:** 2026-01-12
**Scope:** Milestone 1 (T-1..T-5) and Milestone 2 (T-6..T-11) of EDITPLAN.md —
simplify `edit`/`multi_edit`/`apply_patch` to strict exact-byte matching.

---

## Milestone 1 — Core refactor (behaviour change)

| Task | File | Change |
|------|------|--------|
| T-1 | `crates/ragent-tools-core/src/edit.rs` | Switched from `find_replacement_range_diag` to `find_exact_replacement_range`; `new_string` written verbatim; module docblock rewritten (exact-byte matching + retained FR-003 stale detection); `description()` + `old_string` schema text no longer claim whitespace tolerance; removed the "deliberately exceeds Claude Code" note; `check_stale_file`, `record_edit_timestamp`, create/delete/no-op logic, snippet builder, and canonical/legacy-param handling left untouched. |
| T-2 | `crates/ragent-tools-core/src/replace.rs` | Reduced 813 → 119 LOC. Kept: `FindError`, `find_exact_replacement_range`, slim `FindDiag { kind }` with `not_found()` / `multiple(n)` constructors, `format_match_failure` (dropped the pass-name text, kept the actionable re-read + context hint). Deleted 15 tolerance helpers: `find_replacement_range`, `find_replacement_range_diag`, `find_batch_normalized_replacement_range`, `strip_cr`, `strip_trailing_ws`, `byte_offset_of_line`, `common_leading_ws`, `norm_to_orig_byte`, `line_of_nth_match`, `closest_collapsed_line`, `leading_ws`, `reindent_with`, `strip_one_blank_edge_lines`, `try_blank_line_normalised`, `try_final_newline_normalised`, `disambiguate_by_whitespace_proximity`. |
| T-3 | `crates/ragent-tools-core/src/multiedit.rs` | Deleted `resolve_batch_edit`. Phase-2 now calls `find_exact_replacement_range` directly and maps `FindError` → `FindDiag` → `format_match_failure`. Module docblock, `old_string` schema description, and `ResolvedEdit::effective_new` comment updated. Unused imports removed. Atomicity, overlap detection, per-file stats, and dry-run behaviour unchanged. |
| T-4 | `crates/ragent-tools-core/src/apply_patch.rs` | `apply_update_hunks` switched to `find_exact_replacement_range` (§2.4 default: exact, mirroring upstream Codex). Module docblock states the strict exact-byte hunk context requirement. |
| T-5 | `crates/ragent-tools-core/src/lib.rs` | `replace` module comment rewritten: no longer "whitespace-tolerant"; notes exact matching + error formatting. |

**M1 gates:** `cargo check -p ragent-tools-core` ✅, `cargo check --workspace` ✅ (0 errors), `cargo clippy -p ragent-tools-core` ✅ 0 warnings, `cargo fmt` ✅ applied.

---

## Milestone 2 — Tests

| Task | File | Change |
|------|------|--------|
| T-6 | `crates/ragent-tools-core/tests/test_replace.rs` | Deleted (288 lines of seven-pass tests). |
| T-7 | `crates/ragent-tools-core/tests/test_edit_integration.rs` | Removed `test_edit_tolerant_accepts_crlf_mismatch`, `test_edit_tolerant_accepts_trailing_space_mismatch`, `test_edit_tolerant_accepts_indentation_mismatch`. Added `test_edit_exact_rejects_crlf_mismatch`, `test_edit_exact_rejects_trailing_space_mismatch`, `test_edit_exact_rejects_indentation_mismatch` (assert edit fails and file unmodified). Removed the obsolete "pass:" hint assertion from `test_edit_not_found_includes_pass_hint`. Stale-read tests (`test_edit_stale_file_rejected` et al.) untouched and still passing. |
| T-8 | `crates/ragent-tools-core/tests/test_multiedit.rs` | Replaced `test_batch_normalization_accepts_crlf_and_trailing_space_mismatch` with `test_batch_exact_rejects_crlf_mismatch`. Updated `test_batch_indentation_mismatch_still_rejected` comment ("strict exact-byte matching rejects the mismatch"). Updated module docblock ("seven-pass matcher" → strict exact byte). Kept `test_batch_stale_file_rejected` untouched. |
| T-9 | `crates/ragent-tools-core/tests/test_multiedit_helpers.rs` | Rewrote to a minimal helper suite: `find_exact_replacement_range` (exact match, not-found, multiple-matches, CRLF/trailing-whitespace rejection), slimmed `FindDiag` constructors (kind-only), and `format_match_failure` wording checks (byte-for-byte exact phrasing + re-read hint). Removed all `resolve_batch_edit` / `FindDiag {pass, closest_line}` references and `#[path]` shims. |
| T-10 | `crates/ragent-tools-core/tests/test_edit.rs` | Shim updated (re-exports `FindDiag`, `find_exact_replacement_range`, `format_match_failure`); snippet-helper tests still pass. |
| T-11 | `crates/ragent-tools-core/tests/test_apply_patch.rs` | Added `test_apply_patch_rejects_crlf_context_mismatch`: applies a LF patch, converts file to CRLF, then asserts a second LF-context patch fails cleanly and the file is unmodified — locks the §2.4 decision in place. Updated module docblock. |

**M2 gates:** `cargo test -p ragent-tools-core` ✅ green across all 13 suites
(lib 43, test_multiedit_helpers 9, test_edit 5, test_apply_patch 9,
test_edit_integration 20, test_multiedit 19, test_edit_smoke 1, plus the
remaining suites 9/4/11/1/12/5). `cargo fmt --check` ✅,
`cargo clippy --all-targets -- -W clippy::all` ✅ 0 warnings in edited files
(3 pre-existing `test_open` `to_path_buf` warnings unrelated to this work, left
untouched per surgical-change rules).

---

## Current state

- `edit`, `multi_edit`, `apply_patch` match strictly on exact bytes; CRLF, trailing-whitespace, and indentation mismatches are rejected with an actionable "not found" message.
- FR-003 stale-read detection, FR-006 create/delete, FR-007 no-change rejection, dry run, snippets, file locks, canonical+legacy parameter handling all preserved.
- `replace.rs` is now 119 LOC.

## Known incidental issues (untouched per surgical-change rules)

- `edit.rs` line 482 `// ── Unit tests ──` separator contains two U+FFFD replacement characters — pre-existing in HEAD, flagged for a later cleanup.
- `tests/test_apply_patch.rs` was fully rewritten to fix an existing indentation glitch and to include the new CRLF-context test; the public surface covered is unchanged.

## Milestone 3 — Docs & quality gates

| Task | File | Change |
|------|------|--------|
| T-12 | `TOOLS.md` | `edit` description + `old_string` schema text rewritten: no CRLF/whitespace tolerance; matching is byte-for-byte. `multi_edit` description states strict exact-byte matching. `apply_patch` description states hunk context must match byte-for-byte. Root `AGENTS.md`, `README.md`, `QUICKSTART.md` had no edit-matching tolerance claims (verified by grep); `SPEC.md` references are confined to the historical Appendix-D changelog and were left untouched. |
| T-13 | workspace | Gates re-run: `cargo fmt --all --check` ✅, `cargo clippy -p ragent-tools-core --all-targets -- -W clippy::all` ✅ (only 3 pre-existing `test_open` warnings), `cargo test -p ragent-tools-core` ✅, `cargo check --workspace` ✅ with **zero warnings**. |
| T-14 | `crates/ragent-tools-core/tests/test_edit_smoke.rs` (new) | Behavioural smoke test `t14_smoke_exact_mismatch_and_stale_behaviour` drives `EditTool` directly (the same code path `ragent run` uses) since no LLM provider was available in this environment. Verifies: **(a)** a successful exact edit applies `new_string` verbatim; **(b)** a trailing-space-mismatched `old_string` is rejected with an actionable error mentioning exact matching plus the re-read hint, leaving the file unmodified; **(c)** FR-003 stale-file rejection still fires after an external mtime bump. A live `ragent run` smoke pass (per the original task text) is deferred — it requires a configured provider. |

**M3 gates:** all green (fmt / clippy / tests / workspace check).

**M3 deviation note:** T-12's grep target `AGENTS.md` "Editing Files" section does not exist
in the current root `AGENTS.md` (the tool-usage guidance lives in the system
prompt, not the repo file); the equivalent agent-facing text lives in `TOOLS.md`,
which was updated. Historical mentions of the old matcher in `SPEC.md`
(Appendix D changelog) and `CHANGELOG.md` document past releases and are
intentionally not rewritten.

## Not done (by policy)

- No commit / push (requires explicit instruction per AGENTS.md).
