# DUPPLAN.md Milestones A & B — Completion Report

**Date:** 2025-07-05
**Plan:** [DUPPLAN.md](../../DUPPLAN.md)

## Summary

Milestones A and B of the code-duplication-removal plan are complete. Combined,
they remove **~2,597 lines** of duplicated code and eliminate **31 exact-duplicate
groups** from the codebase, with zero test failures and zero new warnings.

## Milestone A — Dead-Code VCS Tool Copies (Tier 1, Low Risk) ✅

### Task A-1: Delete the five dead VCS tool copies ✅

**Action:** Removed five full verbatim copies of GitHub/GitLab tool
implementations from `crates/ragent-agent/src/tool/` that were never registered
or referenced (registration goes through `ExtractedVcsToolAdapter` →
`ragent_tools_vcs::registry::create_vcs_registry()`).

| File deleted | Canonical source (kept) | Lines |
|--------------|---------------------------|-------|
| `github_issues.rs` | `ragent-tools-vcs/src/github/github_issues.rs` | 457 |
| `github_prs.rs` | `ragent-tools-vcs/src/github/github_prs.rs` | 421 |
| `gitlab_issues.rs` | `ragent-tools-vcs/src/gitlab/gitlab_issues.rs` | 445 |
| `gitlab_mrs.rs` | `ragent-tools-vcs/src/gitlab/gitlab_mrs.rs` | 425 |
| `gitlab_pipelines.rs` | `ragent-tools-vcs/src/gitlab/gitlab_pipelines.rs` | 713 |
| **Total** | | **2,461** |

Also removed the five `pub mod` declarations from `tool/mod.rs`.

**Verification:**
- `cargo check -p ragent-agent` — passes.
- `cargo test -p ragent-agent` — 320+ tests pass, 0 failed.
- `grep` confirms no `github_issues::`/`gitlab_issues::` references remain in
  `ragent-agent/src/` (only `ragent_tools_vcs` references in `tool/mod.rs`).

**Acceptance:** `cargo dupes` exact-dup groups 50, 51, 53 and the cross-crate
near-dup group 102 are eliminated.

### Task A-2: Add CI guard for VCS tool duplication ✅

**Action:** Created `scripts/check-vcs-duplication.sh` — a CI guard analogous
to `scripts/check-team-duplication.sh` that fails if any `github_*.rs` or
`gitlab_*.rs` file reappears under `crates/ragent-agent/src/tool/`. Wired it
into `pre-flight.sh` (after the inline-test guard).

**Verification:**
- `bash scripts/check-vcs-duplication.sh` prints
  `OK: ragent-agent has no local GitHub/GitLab VCS tool copies (uses ragent-tools-vcs).`
  and exits 0.
- `pre-flight.sh` now invokes the guard.

## Milestone B — `resolve_path` Extraction (Tier 1, Low Risk) ✅

### Task B-1: Add `resolve_path` to `ragent-tools-core` shared module ✅

**Action:** Created `crates/ragent-tools-core/src/path_util.rs` with a
documented `pub fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf`.
Added `pub mod path_util;` to `lib.rs`.

**Verification:** `cargo check -p ragent-tools-core` passes.

### Task B-2: Replace the 16 core copies with the shared helper ✅

**Action:** For each of the 16 files in `ragent-tools-core/src/`
(`append_file.rs`, `copy_file.rs`, `create.rs`, `diff.rs`, `edit.rs`,
`file_info.rs`, `glob.rs`, `grep.rs`, `list.rs`, `mkdir.rs`, `move_file.rs`,
`multiedit.rs`, `patch.rs`, `read.rs`, `rm.rs`, `write.rs`):
1. Deleted the local `fn resolve_path` definition.
2. Added `use super::path_util::resolve_path;` (using `super::` rather than
   `crate::` so the `#[path]`-based test shims that re-include source modules
   resolve correctly).
3. `cargo fix` cleaned up now-unused `Path`/`PathBuf` imports in 9 files.

Also fixed the two `#[path]`-based test files (`test_edit.rs`,
`test_multiedit_helpers.rs`) to add a `path_util` shim module and import
`resolve_path` from `ragent_tools_core::path_util`.

**Verification:** `cargo test -p ragent-tools-core` — all 120+ tests pass,
0 failed.

**Acceptance:** `cargo dupes` exact-dup group 1 (the largest group, 18 copies)
is eliminated.

### Task B-3: Consolidate the 2 extended copies ✅

**Action:** Replaced `pub fn resolve_path` in `libreoffice_common.rs` and
`office_common.rs` with `pub use ragent_tools_core::path_util::resolve_path;`
re-exports so existing office/pdf tool call sites still resolve. `cargo fix`
cleaned up the now-unused `PathBuf` imports.

**Verification:** `cargo test -p ragent-tools-extended` — all 84 tests pass,
0 failed.

**Acceptance:** No `fn resolve_path` definition remains outside `path_util.rs`
(verified by grep across `crates/ragent-tools-core/src/` and
`crates/ragent-tools-extended/src/`).

## Combined Metrics

| Metric | Before | After A | After B |
|--------|--------|---------|---------|
| Total lines analysed | 142,348 | 141,121 | 140,985 |
| Exact-dup groups | 385 | 355 | 354 |
| Exact-dup lines | 15,573 | 13,347 | 13,203 |
| Near-dup groups | 142 | 142 | 142 |
| Near-dup lines | 5,963 | 5,938 | 5,938 |
| Exact duplication % | 10.9% | 9.5% | 9.4% |

**Net effect of Milestones A + B:**
- 1,363 total lines removed.
- 31 exact-duplicate groups eliminated (385 → 354).
- 2,370 exact-duplicated lines eliminated (15,573 → 13,203).
- Exact duplication reduced from 10.9% → 9.4%.

## Verification

- `cargo check --workspace` — passes.
- `cargo test -p ragent-agent` — passes (0 failed).
- `cargo test -p ragent-tools-core` — passes (0 failed).
- `cargo test -p ragent-tools-extended` — passes (0 failed).
- `cargo clippy -p ragent-tools-core -p ragent-tools-extended` — clean
  (`-D warnings`).
- `cargo fmt` applied to changed crates; `cargo fmt --check` clean.
- `cargo dupes` confirms group 1, 50, 51, 53, and cross-crate group 102 gone.

## Files Changed

### Created
- `crates/ragent-tools-core/src/path_util.rs` (canonical `resolve_path`).
- `scripts/check-vcs-duplication.sh` (CI guard).
- `docs/reports/dupplan-milestone-ab-completion.md` (this report).

### Deleted (Milestone A)
- `crates/ragent-agent/src/tool/github_issues.rs`
- `crates/ragent-agent/src/tool/github_prs.rs`
- `crates/ragent-agent/src/tool/gitlab_issues.rs`
- `crates/ragent-agent/src/tool/gitlab_mrs.rs`
- `crates/ragent-agent/src/tool/gitlab_pipelines.rs`

### Modified
- `crates/ragent-agent/src/tool/mod.rs` (removed 5 `pub mod` declarations).
- `crates/ragent-tools-core/src/lib.rs` (added `pub mod path_util;`).
- 16 files in `crates/ragent-tools-core/src/` (removed local `resolve_path`,
  added `use super::path_util::resolve_path;`, cleaned unused imports).
- `crates/ragent-tools-core/tests/test_edit.rs` (path_util shim + import fix).
- `crates/ragent-tools-core/tests/test_multiedit_helpers.rs` (path_util shim +
  import fix).
- `crates/ragent-tools-extended/src/libreoffice_common.rs` (re-export).
- `crates/ragent-tools-extended/src/office_common.rs` (re-export).
- `pre-flight.sh` (wired in VCS duplication guard).
- `CHANGELOG.md` (Milestones A & B entries).
- `DUPPLAN.md` (Milestone A & B tasks marked ✅ DONE).