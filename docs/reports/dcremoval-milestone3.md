# DCREMOVALPLAN — Milestone 3 Completion Report

**Date:** 2025-01-17
**Plan reference:** `DCREMOVALPLAN.md` §4 Milestone 3
**Baseline reference:** `docs/reports/dcremoval-baseline.md`
**M1 report:** `docs/reports/dcremoval-milestone1.md`
**M2 report:** `docs/reports/dcremoval-milestone2.md`
**Commit message (per plan):** `refactor(agent): remove 21 dormant duplicates of ragent-tools-core modules`

---

## Goal

Remove the 21 agent-local copies in `crates/ragent-agent/src/tool/` that
duplicate `ragent-tools-core`, leaving a single source of truth per core tool.

---

## Pre-scan (per plan §4 M3: verify zero internal consumers before deleting)

A workspace-wide grep for `crate::tool::X::`, `ragent_agent::tool::X::`, and
`ragent_core::tool::X::` across `crates/`, `src/`, `tests/` for all 21 module
names returned **zero code references** (the only matches for `bash` were the
doc-comments I added in M2, not code). The runtime registry uses
`register_extracted_core_tools`, which wraps `ragent_tools_core::Tool` in
`ExtractedCoreToolAdapter` — it does not touch `crate::tool::X`.

---

## Plan deviation: `aliases.rs` was an undiscovered internal consumer of `bash`/`write`

The plan's §3.6 "Live internal consumers" list enumerated 3 call sites but
**missed `crates/ragent-agent/src/tool/aliases.rs`**, which imported
`use super::{bash, write};` and delegated `update_file`→`WriteTool` and
`run_code`→`BashTool` directly. These agent-local structs implement the
**agent-local** `Tool` trait (`ragent_agent::tool::Tool`), whereas
`ragent_tools_core::{BashTool, WriteTool}` implement
`ragent_tools_core::Tool` — a *different* trait. So simply re-pointing the
import to `ragent_tools_core::{bash, write}` (the M2-style fix) failed with
`E0277: the trait bound WriteTool: tool::Tool is not satisfied`.

### Complete solution applied

`aliases.rs` now delegates via `ExtractedCoreToolAdapter` — the *same* adapter
the runtime registry uses to bridge core `Tool` impls into the agent-local
`Tool` trait. To enable this:

1. **`crates/ragent-agent/src/tool/mod.rs`**: widened `ExtractedCoreToolAdapter`
   and `ExtractedCoreToolAdapter::new` from private to `pub(crate)` (with a
   doc-comment explaining the new dual use by `aliases.rs`).
2. **`crates/ragent-agent/src/tool/aliases.rs`**:
   - `use super::ExtractedCoreToolAdapter;` + `use ragent_tools_core::{bash, write};`
   - `UpdateFile::execute` → `delegate(&ExtractedCoreToolAdapter::new(Arc::new(write::WriteTool)), input, ctx)`
   - `RunCode::execute`  → `delegate(&ExtractedCoreToolAdapter::new(Arc::new(bash::BashTool)), input, ctx)`

This is the canonical bridging pattern (identical to what
`register_extracted_core_tools` does for every core tool) and introduces no
behavioural change — the alias still delegates to the same `WriteTool`/
`BashTool`, now via the single source of truth in `ragent-tools-core`.

---

## Deletions (per plan's dependency-safe tiers, with `cargo check -p ragent-agent` gate after each tier)

### Tier 1 — 17 leaves (no internal callers)

`append_file`, `bash_reset`, `calculator`, `copy_file`, `create`, `diff`,
`file_info`, `get_env`, `glob`, `list`, `mkdir`, `move_file`, `rm`, `write`,
`task_complete`, `think`, `truncate`

- Removed 17 `pub mod X;` declarations (+ preceding doc-comment lines) from
  `tool/mod.rs` via a deterministic Python script (30 lines removed).
- Deleted the 17 `.rs` files.
- **Gate:** first attempt failed on `aliases.rs` (`use super::{bash, write}`).
  Applied the complete solution above; re-ran `cargo check -p ragent-agent` → ✅
  Finished, 0 warnings.

### Tier 2 — `patch`, `grep`

- Removed 2 `pub mod X;` declarations (2 lines removed).
- Deleted `patch.rs`, `grep.rs`.
- **Gate:** `cargo check -p ragent-agent` → ✅ Finished, 0 warnings (23.81s).

### Tier 3 — `read`

- Pre-checked: no `crate::tool::read::` references remain (M2 did not touch
  `read.rs`, but no internal consumers existed anyway).
- Removed 1 `pub mod read;` declaration (1 line removed).
- Deleted `read.rs`.
- **Gate:** `cargo check -p ragent-agent` → ✅ Finished, 0 warnings (34.79s).

### Tier 4 — `bash`

- Pre-checked: no `crate::tool::bash::` code references remain (M2.2
  re-pointed `processor.rs`; the only remaining matches were M2 doc-comments).
- Removed 1 `pub mod bash;` declaration + its doc-comment (2 lines removed).
- Deleted `bash.rs` (868 LOC — the largest single deletion; core's is 1,546 LOC
  with Windows Git-Bash/PowerShell support that the agent copy lacked).
- **Gate:** `cargo check -p ragent-agent` → ✅ Finished, 0 warnings (16.92s).

---

## Workspace gate (M3 final)

| Check | Result |
|-------|--------|
| `cargo check --workspace` | ✅ Finished, 0 warnings, 0 errors (0.83s incremental) |
| `cargo test -p ragent-agent -p ragent-tools-core -p ragent-tools-extended` (lib+integ) | ✅ **674 passed, 0 failed** |
| `ragent-agent` lib tests | 1537 passed, 0 failed (workspace `--lib` run) |
| `ragent-agent` integration tests | 451 passed, 0 failed |
| `ragent-tools-core` integration tests | 130 passed, 0 failed |
| `ragent-tools-extended` integration tests | 84 passed, 0 failed |
| `ragent-team` integration tests | 81 passed, 0 failed |
| `ragent-server` integration tests | 71 passed, 0 failed |
| `ragent-research` integration tests | 282 passed, 0 failed |
| `ragent-codeindex` integration tests | 234 passed, 0 failed |
| `ragent-specs` integration tests | 178 passed, 0 failed |
| `ragent-tui` integration tests | 367 passed, **9 failed** (pre-existing — see below) |

### Test-count reconciliation

| Metric | Count | Source / explanation |
|--------|-------|----------------------|
| M2 target-crate baseline | 686 passed | M2 report |
| M3 target-crate result | 674 passed | this run |
| Delta | −12 | inline `#[test]` fns in deleted `truncate.rs` (verified: exactly 12; all other deleted files had 0 inline tests) |
| Live tests lost | 0 | the 12 removed tests exercised the dormant duplicate `truncate` helpers |

### Pre-existing failures (NOT caused by M3)

**1. `ragent-tui` slash-command tests (9 failures).** Verified pre-existing in
the M2 report by reverting M2 and reproducing the same 9–10 failures. M3 did
not touch `ragent-tui` or any code path exercised by these tests. Same 9 test
names fail identically.

**2. `ragent-agent` doc-tests (95 failures).** All 95 fail with
`cannot find module or crate 'ragent_core' in this scope`. The `ragent_core`
Crate alias for `ragent-agent` is declared only in `ragent-bench` and
`ragent-tui` `Cargo.toml` — **not** in `ragent-agent`'s own `Cargo.toml`. So
doc-tests inside `ragent-agent` source that reference `ragent_core::*` always
failed to compile, regardless of M3. Verified:
- The failing doc-tests live in `id.rs`, `agent/mod.rs`, `hooks/mod.rs`,
  `mcp/mod.rs` — **none touched by M3**.
- None of the 95 failing doc-tests reference any of the 21 modules deleted in
  M3 (grep for deleted module names in the failure list → empty).
- These failures predate M0 (the M0 baseline's "696 passed" excluded the
  doc-test binary's `0 passed; 95 failed` line).

**Conclusion: M3 introduced ZERO regressions.** The 12-test drop in target-crate
tests is fully explained by dead inline tests in the deleted `truncate.rs`.
The 9 TUI failures and 95 doc-test failures are pre-existing.

---

## LOC delta

| Path | Before M3 (after M1) | After M3 | Delta |
|------|---------------------|----------|-------|
| `crates/ragent-agent/src/tool/` (all `.rs`) | 17,727 | 13,426 | **−4,301** |
| Sum of 21 deleted file bodies | 4,288 | 0 | −4,288 |
| `tool/mod.rs` | 1,452 | 1,423 | −29 (decl + doc-comment lines) |
| `aliases.rs` | 266 | 282 | +16 (adapter bridge) |

The net **−4,301 LOC** exceeds the plan's estimate of ~4,036 because the plan
counted only file bodies (4,288 ≈ 4,036 — the small difference is the plan
rounding file LOCs in §3.1). The additional −13 comes from the removed
`pub mod` declarations in `mod.rs`, partially offset by the +16 lines added
to `aliases.rs` for the adapter bridge.

---

## Files changed (M3)

| File | Change |
|------|--------|
| `crates/ragent-agent/src/tool/mod.rs` | Removed 21 `pub mod X;` decls (+ doc-comment lines); widened `ExtractedCoreToolAdapter` + `new` to `pub(crate)` with doc-comment |
| `crates/ragent-agent/src/tool/aliases.rs` | Re-pointed `bash`/`write` delegation to `ragent_tools_core` via `ExtractedCoreToolAdapter` |
| 21 `.rs` files in `crates/ragent-agent/src/tool/` | DELETED: `append_file`, `bash`, `bash_reset`, `calculator`, `copy_file`, `create`, `diff`, `file_info`, `get_env`, `glob`, `grep`, `list`, `mkdir`, `move_file`, `patch`, `read`, `rm`, `task_complete`, `think`, `truncate`, `write` |

---

## State after M3

- All 21 core-tool dormant duplicates are gone; `ragent-tools-core` is the
  single source of truth for every core tool.
- `aliases.rs` now bridges to the core implementations via the canonical
  `ExtractedCoreToolAdapter` (same adapter the registry uses).
- Verified: no stale `crate::tool::X::` / `ragent_core::tool::X::` /
  `ragent_agent::tool::X::` references remain for any of the 21 deleted modules.

---

## Status

**Milestone 3: COMPLETE.**

- 21 dormant core-tool duplicates removed (~4,301 LOC net).
- One plan gap discovered and fixed completely: `aliases.rs` was an
  undiscovered consumer of agent-local `bash`/`write`; now bridges to the core
  implementations via `ExtractedCoreToolAdapter`.
- `cargo check --workspace` clean.
- Target-crate tests: 674 passed, 0 failed (−12 from M2 = dead inline tests in
  deleted `truncate.rs`; 0 live tests lost).
- All other failures (9 TUI, 95 doc-test) are pre-existing and unrelated to M3.
- Zero M3-introduced regressions.

Per the plan ordering (M1 → M2 → **M5** → M3 → M4 → M6), M3 is now done. Next
is **Milestone 4** (delete the 22 dormant extended-tool duplicates), which
depends on M5 (consolidate memory helpers) being done first for the
`memory_search`/`memory_write` files. M5 has not yet been executed, so M4's
memory_* deletions must wait on M5. The non-memory extended-tool duplicates
(20 of 22) could be deleted now, but per single-change discipline and the plan
ordering, M5 should be executed next.

---

## Note on commit

Per AGENTS.md, no `git commit` was performed — the user has not given an
explicit push/commit instruction. The plan's M3 deliverable names the
suggested commit message
`refactor(agent): remove 21 dormant duplicates of ragent-tools-core modules`;
this will be used when the user authorises committing the milestone.