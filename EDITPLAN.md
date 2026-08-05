# EDITPLAN — Simplify edit/multi_edit to Exact-Byte Matching

**Status:** Implemented (M1–M3; T-14 automated smoke in lieu of live `ragent run` — see completion log)
**Created:** 2026-01-12
**Scope:** `crates/ragent-tools-core` (edit, multiedit, replace, apply_patch tools + tests) plus the root `AGENTS.md` tool documentation.
**Out of scope:** the unified-diff `patch` tool, snippet construction, dry-run plumbing, create/delete semantics, parameter-name handling (canonical + legacy aliases — unchanged), permissions, file locking.

## 1. Problem Statement

The `edit` and `multi_edit` tools carry a complex, whitespace-tolerant matching
layer that is hard to reason about and expensive to maintain:

- `edit` resolves `old_string` through a **seven-pass cascade**
  (`replace.rs::find_replacement_range_diag`, ~250 LOC):
  exact → CRLF-normalised → trailing-whitespace-stripped →
  leading-whitespace-stripped (with indentation re-application to `new_string`)
  → collapsed-whitespace (with proximity disambiguation) →
  blank-line-normalised → final-newline-normalised.
- `multi_edit` resolves through **two passes**
  (`multiedit.rs::resolve_batch_edit`): strict exact, then a
  "batch-normalized" (CRLF + trailing-whitespace) fallback
  (`replace.rs::find_batch_normalized_replacement_range`).
- `apply_patch` re-uses the seven-pass cascade via `find_replacement_range`.
- The supporting machinery in `replace.rs` (813 LOC total) includes
  `strip_cr`, `strip_trailing_ws`, `norm_to_orig_byte`, `byte_offset_of_line`,
  `leading_ws`, `common_leading_ws`, `reindent_with`,
  `strip_one_blank_edge_lines`, `try_blank_line_normalised`,
  `try_final_newline_normalised`, `closest_collapsed_line`,
  `line_of_nth_match`, and `disambiguate_by_whitespace_proximity`.
- `FindDiag` carries two fields (`pass`, `closest_line`) that only exist to
  describe which tolerance pass failed — meaningless under exact matching.
- Tests codify tolerant behaviour
  (`test_edit_tolerant_accepts_{crlf,trailing_space,indentation}_mismatch`,
  `test_batch_normalization_accepts_crlf_and_trailing_space_mismatch`,
  `resolve_batch_edit_normalizes_crlf_and_trailing_space`, plus the entire
  288-line `test_replace.rs` seven-pass suite).

Goal: **replace all of this with a single strict exact-byte match** — the same
semantics as Claude Code's `Edit` tool — while **retaining the stale-read
detection** (FR-003) in both `edit` and `multi_edit` unchanged.

## 2. Design

### 2.1 Matching semantics (target state)

For every edit (`edit`, each element of `multi_edit.edits[]`, every
`apply_patch` update hunk):

1. Count occurrences of `old_string` in the file content with
   `content.matches(old_string).count()`.
2. `0` → error "old_string not found in {path}".
3. `>1` → error "found {n} times; must match exactly once".
4. `1` → apply `str::replace`-equivalent byte-range splice; `new_string` is
   inserted **verbatim** (never re-indented, never line-ending-normalised).

No CRLF tolerance, no trailing/leading whitespace tolerance, no indentation
re-application, no blank-line or final-newline normalisation, no disambiguation
heuristics. What you read (bytes) is what you match.

The existing `replace.rs::find_exact_replacement_range` already implements
exactly this and is reused as-is; everything else in `replace.rs` not required
by the remaining callers is deleted.

### 2.2 Retained behaviour

- **Stale-read detection (FR-003)**: `check_stale_file` in `edit.rs` and
  `multiedit.rs` (compares recorded read mtime vs current mtime, rejects with
  an actionable "re-read the file" error) is **unchanged**, as is
  `record_edit_timestamp`.
- Create (`old_string` empty) / delete (`new_string` empty) / no-op rejection
  (FR-006/FR-007) — unchanged.
- `multi_edit` atomicity, overlap detection, highest-end-first application,
  per-file stats — unchanged.
- Dry-run preview and `cat -n` result snippet (`build_snippet`,
  `byte_offset_to_line`) — unchanged.
- Canonical (`file_path`/`old_string`/`new_string`) and legacy
  (`path`/`old_str`/`new_str`) parameter names + deprecation warning —
  unchanged.
- Error type surface: `FindError::{NotFound, MultipleMatches(n)}` is kept;
  `FindDiag` is retained only if still needed to format errors (see T-2).

### 2.3 `replace.rs` target shape (~120 LOC)

Keep (renumbered line ranges):

| Item | Consumers after refactor |
|---|---|
| `FindError` | `replace.rs` itself, `multiedit.rs`, tests |
| `find_exact_replacement_range` | `edit.rs`, `multiedit.rs`, `apply_patch.rs` |
| `FindDiag` / `FindDiagKind` / `format_match_failure` | `edit.rs` + `multiedit.rs` error formatting — **simplified**: fields `pass`/`closest_line` dropped; `format_match_failure` keeps the actionable re-read hint minus the pass name |

Delete: `find_replacement_range`, `find_replacement_range_diag`,
`find_batch_normalized_replacement_range`, `strip_cr`, `strip_trailing_ws`,
`byte_offset_of_line`, `common_leading_ws`, and all private helpers
(`norm_to_orig_byte`, `line_of_nth_match`, `closest_collapsed_line`,
`leading_ws`, `reindent_with`, `strip_one_blank_edge_lines`,
`try_blank_line_normalised`, `try_final_newline_normalised`,
`disambiguate_by_whitespace_proximity`).

> Decision point: alternatively delete `FindDiag` entirely and have callers
> format errors from `FindError` + path directly. Preferred: keep the slimmed
> `FindDiag` so `edit.rs`/`multiedit.rs` error messages stay centralised in
> one formatter. Confirm during T-2.

### 2.4 `apply_patch` decision (flagged for review)

`apply_patch.rs::apply_update_hunks` currently uses the seven-pass matcher.
Under this plan it switches to `find_exact_replacement_range`.

**Consequence:** patches generated against files whose bytes differ only in
line endings/trailing spaces will now fail where they previously succeeded.
This matches upstream Codex `apply_patch` behaviour (exact context matching)
and is judged acceptable — patch hunks are machine-generated, not
model-retyped, so tolerance provides little value. **If review disagrees, the
alternative is to leave a single CRLF-only normalisation in `apply_patch`;**
that is the one place a fallback might still earn its keep. Default: exact.

## 3. Tasks & Milestones

### Milestone 1 — Core refactor (behaviour change)

**M1 success criteria:** `edit`, `multi_edit`, `apply_patch` all match
strictly on exact bytes; the crate compiles; stale-read detection intact.

| Task | Description | Verify |
|------|-------------|--------|
| T-1 | **`edit.rs`**: replace `find_replacement_range_diag` call with `find_exact_replacement_range`; drop `effective_new_str` handling (use `new_string` verbatim); update module docblock (remove seven-pass description, state exact-byte matching + stale detection); update `description()` and `old_string` schema text to remove "tolerated" wording. Keep `check_stale_file`, `record_edit_timestamp`, snippet logic, legacy-param handling untouched. | `cargo check -p ragent-tools-core` |
| T-2 | **`replace.rs`**: reduce to the §2.3 target shape. Slim `FindDiag` to `{kind}` only (constructors `not_found()`/`multiple(n)`); update `format_match_failure` to drop the pass-name/closest-line text while keeping the re-read + context hint. | `cargo check -p ragent-tools-core` |
| T-3 | **`multiedit.rs`**: delete `resolve_batch_edit`; call `find_exact_replacement_range` directly at the Phase-2 site and map errors through the slimmed `format_match_failure`; update module docblock (matching section) and the `old_string` schema description. Remove now-unused imports (`FindError`, `find_batch_normalized_replacement_range`) and the `effective_new` doc-comment mention of indentation re-application. | `cargo check -p ragent-tools-core` |
| T-4 | **`apply_patch.rs`**: switch `apply_update_hunks` to `find_exact_replacement_range`; update module docblock if it references tolerant matching. | `cargo check -p ragent-tools-core` |
| T-5 | **`lib.rs`**: update the `replace` module comment (no longer "whitespace-tolerant"; note retained for exact matching + error formatting). | `cargo check -p ragent-tools-core` |

### Milestone 2 — Tests

**M2 success criteria:** all tolerance-encoding tests removed; explicit
rejection tests added; `cargo test -p ragent-tools-core` green.

| Task | Description | Verify |
|------|-------------|--------|
| T-6 | **Delete `tests/test_replace.rs`** (288 lines of seven-pass tests). The `replace` module shrinks to near-trivial; `find_exact_replacement_range` is already covered through `test_multiedit_helpers.rs` and `test_edit_integration.rs`; no re-import shim for `replace.rs` remains needed. | `cargo test -p ragent-tools-core` compiles |
| T-7 | **`tests/test_edit_integration.rs`**: remove `test_edit_tolerant_accepts_crlf_mismatch`, `test_edit_tolerant_accepts_trailing_space_mismatch`, `test_edit_tolerant_accepts_indentation_mismatch`. Add `test_edit_exact_rejects_crlf_mismatch`, `test_edit_exact_rejects_trailing_space_mismatch`, `test_edit_exact_rejects_indentation_mismatch` (assert the edit **fails** and the file is unmodified). Update the module docblock. Keep stale-read tests (`test_edit_stale_file_rejected` et al.) untouched and passing. | `cargo test -p ragent-tools-core --test test_edit_integration` |
| T-8 | **`tests/test_multiedit.rs`**: replace `test_batch_normalization_accepts_crlf_and_trailing_space_mismatch` with `test_batch_exact_rejects_crlf_mismatch`; keep `test_batch_indentation_mismatch_still_rejected` (now trivially aligned with strict matching — update its comment); keep stale-file batch test `test_batch_stale_file_rejected` untouched; update module docblock ("seven-pass matcher" → strict exact). | `cargo test -p ragent-tools-core --test test_multiedit` |
| T-9 | **`tests/test_multiedit_helpers.rs`**: remove `resolve_batch_edit` tests (function deleted in T-3); delete the file **or** reduce it to a minimal `format_match_failure` assertion if such coverage isn't already in the integration tests. Keep the exact-match + not-found + multiple-matches cases, reworked against the slimmed `FindDiag`/error text. | `cargo test -p ragent-tools-core --test test_multiedit_helpers` (or file removal verified) |
| T-10 | **`tests/test_edit.rs`**: update the local `mod replace` shim to re-export only the surviving items (`find_exact_replacement_range`, `format_match_failure`); the file otherwise tests only snippet helpers (unaffected). | `cargo test -p ragent-tools-core --test test_edit` |
| T-11 | **`tests/test_apply_patch.rs`**: run suite; add one negative test asserting a hunk whose context differs only by CRLF/trailing whitespace now fails cleanly (documents the T-4 decision). | `cargo test -p ragent-tools-core --test test_apply_patch` |

### Milestone 3 — Docs & quality gates

**M3 success criteria:** no doc or agent-facing text claims whitespace
tolerance; workspace clean under fmt/clippy/test.

| Task | Description | Verify |
|------|-------------|--------|
| T-12 | **`AGENTS.md`**: update the `edit` tool bullets in "Editing Files" ("common whitespace and line-ending differences ... are tolerated" → "must match byte-for-byte") and the File Tool Quick Reference row. Search for any other "tolerat" mentions tied to edit/multiedit in root docs (`README.md`, `QUICKSTART.md`, `TOOLS.md`, `SPEC.md`) and sync. | `grep -ri tolerat AGENTS.md README.md QUICKSTART.md TOOLS.md SPEC.md` shows no edit-matching claims |
| T-13 | **Gates**: `cargo fmt`, `cargo clippy -p ragent-tools-core -- -W clippy::all`, `cargo test -p ragent-tools-core` (10-min timeout), then `cargo check --workspace` to catch any cross-crate fallout (agent/TUI metadata builders reference line counts only — expected unaffected, verify). | all green, zero new warnings |
| T-14 | **Behavioural smoke test**: via `ragent run`, perform one successful exact edit and one deliberately whitespace-mismatched edit against a scratch file in `target/temp/`; confirm (a) success applies verbatim, (b) mismatch error message is actionable (mentions exact match + re-read hint), (c) stale-file rejection still fires after an external `touch`. | manual run log attached to PR/commit message |

## 4. Risks & Rollback

- **R-1 — Behavioural regression for sloppy model output.** Edits that the
  tolerant matcher used to rescue (CRLF, trailing spaces, stripped indentation)
  will now fail. Mitigation: the slimmed `format_match_failure` keeps an
  explicit "byte-for-byte match required; re-read the file" hint so models
  self-correct on the retry; the stale-read guard is orthogonal and retained.
- **R-2 — `apply_patch` compatibility** (see §2.4). Mitigation: decision
  explicitly flagged; T-11 locks the chosen behaviour with a test.
- **R-3 — Hidden consumers of deleted helpers.** `strip_cr` et al. are `pub`.
  Mitigation: T-2 followed immediately by `cargo check --workspace`; the only
  known external re-exports are the test shims handled in T-9/T-10.
- **Rollback:** single-commit revert of the Milestone-1/2 commits restores the
  seven-pass matcher; no file-format or config changes are involved.

## 5. References

- Current matcher: `crates/ragent-tools-core/src/replace.rs` (813 LOC → ~120)
- Edit tool: `crates/ragent-tools-core/src/edit.rs` (`execute` L165–L299,
  stale check L367–L414)
- Multi-edit tool: `crates/ragent-tools-core/src/multiedit.rs`
  (`resolve_batch_edit` L412–L430, stale check L441–L486)
- Patch tool: `crates/ragent-tools-core/src/apply_patch.rs` (L446–L469)
- Stale-read detection requirement: FR-003 (editrenewal spec, "SPEC.md" history)
- Benchmark precedent: Claude Code `Edit` uses strict byte-for-byte matching
