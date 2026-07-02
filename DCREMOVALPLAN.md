# DCREMOVALPLAN.md — Dead-Code & Duplication Remediation Plan

**Scope:** `ragent-agent`, `ragent-tools-core`, `ragent-tools-extended`
**Author:** Rust Agent (review pass)
**Status:** Proposed — awaiting approval before execution
**Date:** 2025-01-17

---

## 1. Executive Summary

A review of the three tool crates reveals a **large-scale source duplication**
between `ragent-agent/src/tool/` and the extracted tool crates
(`ragent-tools-core`, `ragent-tools-extended`), plus a second, smaller
duplication layer in the `memory/` modules. The duplicated copies in
`ragent-agent` are **not registered at runtime** — the live registry is built
from the extracted crates via adapter wrappers in
`ragent-agent/src/tool/mod.rs` (`register_extracted_core_tools` /
`register_extracted_extended_tools` / `register_extracted_vcs_tools`).

The agent-local copies are therefore **dormant duplicates** that:

1. Compile (because `pub mod X;` is declared in `tool/mod.rs`) but are never
   instantiated by the registry.
2. Have **drifted** behind the extracted-crate versions (missing Windows shell
   support in `bash.rs`, missing `num_lines` in `read.rs`, missing busy-lock
   retry in `codeindex_*`, missing status-change tracking in `todo.rs`, older
   `let`-chain style in `memory_*`, etc.).
3. Total **~11,015 lines** of duplicated source in `ragent-agent/src/tool/`
   (≈60% of that directory's 18,215 lines).
4. Contain at least one genuinely **dead** module (`file_ops_tool`) and one
   **dead helper module** (`format`) with no callers anywhere.

The plan below removes the dormant duplicates, re-points the two genuine
internal call sites at the extracted-crate APIs, consolidates the duplicated
`memory/` helpers, and deletes the dead modules. Net effect: **≈11,000+ lines
removed**, a single source of truth per tool, and elimination of the drift
risk that has already caused behavioural divergence.

---

## 2. Crate Dependency Context (verified)

- `ragent-agent/Cargo.toml` depends on `ragent-tools-core`,
  `ragent-tools-extended`, and `ragent-tools-vcs`.
- `ragent-tui` and `ragent-server` do **NOT** depend on the tool crates
  directly; they access tools through `ragent-core` (a Cargo alias for
  `ragent-agent`: `ragent-core = { package = "ragent-agent", path = ... }`).
- `ragent-team` re-exports `ragent_agent::tool::{Tool, ToolContext, ...}` and
  `ragent_agent::tool::metadata::*`.
- The runtime registry (`create_default_registry`) registers tools via:
  - `register_extracted_core_tools(&registry)` → wraps each
    `ragent_tools_core::Tool` in `ExtractedCoreToolAdapter`.
  - `register_extracted_extended_tools(&registry)` → wraps each
    `ragent_tools_extended::Tool` in `ExtractedExtendedToolAdapter`.
  - `register_extracted_vcs_tools(&registry)` → wraps each
    `ragent_tools_vcs::Tool` in `ExtractedVcsToolAdapter`.
- Only the agent-specific tools (`plan`, `new_task`, `cancel_task`,
  `list_tasks`, `wait_tasks`, `structured_memory`, `team_*`, `spec_*`,
  `aliases`, `mcp_tool`) are registered from the agent-local modules.

**Conclusion:** the 44 duplicated tool files under `ragent-agent/src/tool/`
are not in the runtime tool path. They are compile-only dead weight.

---

## 3. Findings

### 3.1 Duplication: `ragent-agent/src/tool/` ↔ `ragent-tools-core`

21 files exist in BOTH locations. Lines are the agent copy's size.

| File | Agent LOC | Core LOC | Status |
|------|-----------|----------|--------|
| `append_file.rs` | 99 | 99 | identical |
| `bash.rs` | 868 | 1546 | **DRIFTED** — agent copy lacks Windows Git-Bash/PowerShell support, smaller SAFE_COMMANDS list |
| `bash_reset.rs` | 51 | 51 | identical |
| `calculator.rs` | 85 | 85 | identical |
| `copy_file.rs` | 87 | 87 | identical |
| `create.rs` | 114 | 114 | identical |
| `diff.rs` | 143 | 143 | identical |
| `file_info.rs` | 179 | 179 | identical |
| `get_env.rs` | 101 | 101 | identical |
| `glob.rs` | 208 | 208 | identical |
| `grep.rs` | 313 | 312 | **DRIFTED** — agent copy adds a one-line description tweak |
| `list.rs` | 174 | 174 | identical |
| `mkdir.rs` | 67 | 67 | identical |
| `move_file.rs` | 84 | 84 | identical |
| `patch.rs` | 435 | 435 | **DRIFTED** — trivial `sort_by_key`/fuzz cosmetics |
| `read.rs` | 583 | 641 | **DRIFTED** — agent copy lacks `num_lines` support and read-timestamp recording |
| `rm.rs` | 98 | 98 | identical |
| `task_complete.rs` | 110 | 110 | **DRIFTED** — agent copy has a stale "NOTE" header; core is the live one |
| `think.rs` | 57 | 91 | **DRIFTED** — agent copy dropped the inline tests |
| `truncate.rs` | 325 | 325 | **DRIFTED** — doc-comment `ragent_core` vs `ragent_tools_core` path strings only |
| `write.rs` | 107 | 107 | identical |

**Total duplicated core-tool LOC in agent:** ~4,036

### 3.2 Duplication: `ragent-agent/src/tool/` ↔ `ragent-tools-extended`

22 files exist in BOTH locations.

| File | Agent LOC | Ext LOC | Status |
|------|-----------|----------|--------|
| `http_request.rs` | 163 | 163 | identical |
| `libreoffice_common.rs` | 230 | 230 | identical |
| `libreoffice_info.rs` | 232 | 232 | identical |
| `libreoffice_read.rs` | 307 | 307 | identical |
| `libreoffice_write.rs` | 653 | 654 | **DRIFTED** — let-chains modernization in ext |
| `memory_migrate.rs` | 106 | 106 | identical |
| `memory_replace.rs` | 7 | 7 | identical (re-export shim) |
| `memory_search.rs` | 298 | 297 | **DRIFTED** — let-chains + `CrossProjectConfig` import path |
| `memory_write.rs` | 690 | 686 | **DRIFTED** — let-chains + `CrossProjectConfig` import path |
| `office_common.rs` | 122 | 122 | identical |
| `office_info.rs` | 307 | 307 | identical |
| `office_read.rs` | 738 | 738 | identical |
| `office_write.rs` | 928 | 929 | **DRIFTED** — let-chains |
| `pdf_read.rs` | 312 | 312 | identical |
| `pdf_write.rs` | 605 | 605 | **DRIFTED** — let-chains |
| `todo.rs` | 373 | 427 | **DRIFTED** — ext has old→new status tracking |
| `codeindex_dependencies.rs` | 102 | 106 | **DRIFTED** — ext has `with_retry`/`busy_output` busy-lock handling |
| `codeindex_references.rs` | 106 | 110 | **DRIFTED** — ext has `with_retry`/`busy_output` |
| `codeindex_reindex.rs` | 75 | 75 | identical |
| `codeindex_search.rs` | 137 | 141 | **DRIFTED** — ext has `with_retry`/`busy_output` |
| `codeindex_status.rs` | 94 | 98 | **DRIFTED** — ext has `with_retry`/`busy_output` |
| `codeindex_symbols.rs` | 142 | 146 | **DRIFTED** — ext has `with_retry`/`busy_output` |

**Total duplicated extended-tool LOC in agent:** ~6,979

### 3.3 Duplication: `ragent-agent/src/memory/` ↔ `ragent-tools-extended/src/memory/`

`ragent-agent/src/memory/block.rs` and `storage.rs` are already thin
`pub use ragent_tools_extended::memory::{block,storage}::*;` re-exports
(the correct pattern). However:

| File | Agent LOC | Ext LOC | Status |
|------|-----------|----------|--------|
| `cross_project.rs` | 439 | 442 | **DUPLICATED** — differs only in `CrossProjectConfig` import path + let-chains + clippy allows |
| `embedding.rs` | 317 | 316 | **DUPLICATED** — differs only in doc-comment crate path + a dropped clippy allow |
| `migrate.rs` | 345 | 346 | **DUPLICATED** — differs only in a clippy allow |
| `embedding/local.rs` | 356 | 334 | **DUPLICATED** — differs in comments/docstrings only |

The agent-local copies are consumed by:
- `ragent-agent/src/storage/mod.rs` (`crate::memory::embedding::*`)
- `ragent-agent/src/memory/import_export.rs` (`crate::memory::migrate::*`)
- `ragent-agent/src/tool/memory_search.rs`, `memory_write.rs`,
  `memory_migrate.rs` (`crate::memory::{embedding, cross_project, migrate}`)

The extended crate's copies are consumed by the extended tool modules
(`memory_search.rs`, `memory_write.rs`, `memory_migrate.rs` in
`ragent-tools-extended/src/`). So both copies are "live" within their own
crate but only **one** is needed; the agent crate can re-export the extended
crate's versions exactly as it already does for `block` and `storage`.

**Total duplicated memory-helper LOC in agent:** ~1,457

### 3.4 Dead code in `ragent-agent/src/tool/`

| Module | Evidence of death |
|--------|-------------------|
| `file_ops_tool.rs` (103 LOC) | `FileOpsTool` is **never registered** in `create_default_registry`. No external reference outside `mod.rs`'s `pub mod file_ops_tool;` declaration. The `TEST_COVERAGE.md` references a historical `tests/test_file_ops_tool.rs` that no longer exists. |
| `format.rs` (381 LOC) | `pub mod format;` declared, 10 public items exported, but **zero callers** anywhere in the workspace (`grep` for `format::`, `format_summary_content`, `format_status_output`, `FormatBuilder` returns nothing outside the file itself). Pure dead helper module. |

### 3.5 Dead code in `ragent-tools-core` / `ragent-tools-extended`

- No unused-public-tool warnings are emitted (the registries consume every
  tool struct), and `cargo check`/`cargo build` are warning-clean for all
  three crates. The extracted crates themselves are clean.
- The only "dead" surface in the extracted crates is the
  `#[allow(dead_code)] fn program(&self) -> &OsStr` in
  `ragent-tools-core/src/bash.rs::ShellType`, which is intentional
  (kept for future PowerShell invocation paths). **Leave as-is.**

### 3.6 Live internal consumers of the agent-local duplicated modules

Only two call sites genuinely depend on the agent-local copies (not the
registry path):

1. `crates/ragent-agent/src/reference/resolve.rs:12-13,231-246` uses
   `crate::tool::office_read::{read_docx, read_xlsx, read_pptx}` and
   `crate::tool::pdf_read::read_pdf`.
2. `crates/ragent-agent/src/session/processor.rs:2598` uses
   `crate::tool::bash::is_safe_command`.
3. `crates/ragent-tui/src/app.rs:7494,7500` uses
   `ragent_core::tool::bash::{get_safe_commands, get_builtin_lists}` (via the
   `ragent-core` = `ragent-agent` alias).

All three consume free functions that exist in both copies. After removing
the agent-local copies, these call sites must be re-pointed to the
extracted-crate APIs (`ragent_tools_extended::office_read::*`,
`ragent_tools_extended::pdf_read::read_pdf`,
`ragent_tools_core::bash::{is_safe_command, get_safe_commands,
get_builtin_lists}`).

---

## 4. Remediation Plan

The plan is split into 7 milestones. Each milestone is independently
compilable and testable, and each ends with `cargo check` + `cargo test`
gating. **Change only one thing at a time** (per AGENTS.md rule 3).

### Milestone 0 — Baseline & safety net

**Goal:** Establish a clean baseline before deleting anything.

- [ ] 0.1 Run `cargo check --workspace` and record the warning count (expected: clean).
- [ ] 0.2 Run `cargo test -p ragent-agent -p ragent-tools-core -p ragent-tools-extended` and record the pass count.
- [ ] 0.3 Commit the current state (or note the existing dirty status) so each later milestone is a clean diff.
- [ ] 0.4 Capture the list of "live internal consumer" call sites from §3.6 into a checklist for re-pointing.

**Deliverable:** Baseline metrics recorded in `docs/reports/dcremoval-baseline.md`.

---

### Milestone 1 — Delete the genuinely dead modules

**Goal:** Remove the two confirmed-dead modules from `ragent-agent/src/tool/`.

- [ ] 1.1 Remove `pub mod file_ops_tool;` from `tool/mod.rs` and delete `crates/ragent-agent/src/tool/file_ops_tool.rs` (103 LOC).
  - Verify no test file references `FileOpsTool` (already confirmed: none).
  - Verify `cargo check -p ragent-agent` still passes.
- [ ] 1.2 Remove `pub mod format;` from `tool/mod.rs` and delete `crates/ragent-agent/src/tool/format.rs` (381 LOC).
  - Verify `grep -rn "tool::format\|format_summary_content\|format_status_output"` is empty.
  - Verify `cargo check -p ragent-agent -p ragent-team` still passes (ragent-team does not import `format`).

**Deliverable:** ~484 LOC removed. Commit: `refactor(agent): remove dead file_ops_tool and format helper modules`.

---

### Milestone 2 — Re-point the live internal call sites to extracted-crate APIs

**Goal:** Break the dependency on the agent-local duplicated copies so they
can be deleted in Milestone 3.

- [ ] 2.1 `crates/ragent-agent/src/reference/resolve.rs`:
  - Replace `use crate::tool::office_read;` and `use crate::tool::pdf_read;`
    with `use ragent_tools_extended::office_read;` and
    `use ragent_tools_extended::pdf_read;`.
  - Confirm the function signatures match (`read_docx(&Path, &str)`,
    `read_xlsx(&Path, Option<_>, Option<_>, &str)`,
    `read_pptx(&Path, Option<_>, &str)`, `read_pdf(&Path, Option<_>,
    Option<_>, &str)`). They are identical (verified: the agent and extended
    `office_read.rs`/`pdf_read.rs` files are byte-identical).
- [ ] 2.2 `crates/ragent-agent/src/session/processor.rs:2598`:
  - Replace `use crate::tool::bash::is_safe_command;` with
    `use ragent_tools_core::bash::is_safe_command;`.
- [ ] 2.3 `crates/ragent-tui/src/app.rs:7494,7500`:
  - Replace `ragent_core::tool::bash::get_safe_commands()` and
    `ragent_core::tool::bash::get_builtin_lists()` with
    `ragent_tools_core::bash::get_safe_commands()` and
    `ragent_tools_core::bash::get_builtin_lists()`.
  - Note: `ragent-tui` does not currently depend on `ragent-tools-core`
    directly. Add `ragent-tools-core = { path = "../ragent-tools-core" }` to
    `crates/ragent-tui/Cargo.toml` (small, focused dependency addition — the
    crate is already transitively compiled via `ragent-agent`).
- [ ] 2.4 `cargo check --workspace && cargo test --workspace` must pass.

**Deliverable:** Three call sites re-pointed; agent-local `bash.rs`,
`office_read.rs`, `pdf_read.rs` now have zero internal consumers. Commit:
`refactor(agent,tui): re-point bash/office_read/pdf_read call sites to extracted crates`.

---

### Milestone 3 — Delete the dormant core-tool duplicates

**Goal:** Remove the 21 agent-local copies that duplicate
`ragent-tools-core`.

For each file in §3.1, remove the `pub mod X;` line from `tool/mod.rs` and
delete `crates/ragent-agent/src/tool/X.rs`. Do this **one file at a time**
with a `cargo check -p ragent-agent` between each deletion to catch any
surprise reference.

Files (in dependency-safe order — pure leaves first):

1. `append_file.rs`, `bash_reset.rs`, `calculator.rs`, `copy_file.rs`,
   `create.rs`, `diff.rs`, `file_info.rs`, `get_env.rs`, `glob.rs`,
   `list.rs`, `mkdir.rs`, `move_file.rs`, `rm.rs`, `write.rs`,
   `task_complete.rs`, `think.rs`, `truncate.rs` (no internal callers).
2. `patch.rs`, `grep.rs` (no internal callers beyond the registry).
3. `read.rs` (verify no `crate::tool::read::` references remain after
   Milestone 2).
4. `bash.rs` (verify after Milestone 2.2 — processor no longer imports it).

After each deletion:
- `cargo check -p ragent-agent` must pass.
- `grep -rn "crate::tool::X::\|ragent_agent::tool::X::\|ragent_core::tool::X::"` must return empty for the deleted `X`.

**Deliverable:** ~4,036 LOC removed. Commit:
`refactor(agent): remove 21 dormant duplicates of ragent-tools-core modules`.

---

### Milestone 4 — Delete the dormant extended-tool duplicates

**Goal:** Remove the 22 agent-local copies that duplicate
`ragent-tools-extended`.

Same per-file delete-and-check procedure as Milestone 3. Files (leaves first):

1. `http_request.rs`, `libreoffice_common.rs`, `libreoffice_info.rs`,
   `libreoffice_read.rs`, `libreoffice_write.rs`, `memory_replace.rs`,
   `memory_migrate.rs`, `office_common.rs`, `office_info.rs`,
   `office_write.rs`, `pdf_write.rs`, `todo.rs`, `codeindex_reindex.rs`,
   `codeindex_search.rs`, `codeindex_status.rs`, `codeindex_symbols.rs`,
   `codeindex_dependencies.rs`, `codeindex_references.rs` (no internal
   callers).
2. `memory_search.rs`, `memory_write.rs` (verify after Milestone 5 — these
   currently use `crate::memory::{embedding, cross_project}` which will be
   re-pointed).
3. `office_read.rs`, `pdf_read.rs` (verify after Milestone 2.1 — resolve.rs
   no longer imports them).

**Deliverable:** ~6,979 LOC removed. Commit:
`refactor(agent): remove 22 dormant duplicates of ragent-tools-extended modules`.

---

### Milestone 5 — Consolidate the duplicated `memory/` helpers

**Goal:** Make `ragent-agent/src/memory/{cross_project, embedding, migrate,
embedding/local}` thin re-exports from `ragent-tools-extended`, matching the
existing pattern already used for `block.rs` and `storage.rs`.

- [ ] 5.1 Verify the extended crate's `memory::cross_project`,
      `memory::embedding`, `memory::migrate`, and `memory::embedding::local`
      expose every symbol the agent crate consumes:
      - `cross_project::{resolve_block, search_blocks_cross_project, ResolvedBlock}`
      - `embedding::{EmbeddingProvider, NoOpEmbedding, serialise_embedding, deserialise_embedding, cosine_similarity, SimilarityResult}`
      - `migrate::{migrate_memory_md, analyse_memory_md}`
      - `embedding::local::LocalEmbeddingProvider`
      (Initial grep confirms all are `pub` in the extended crate; verify
      exactly during execution.)
- [ ] 5.2 Replace the four agent-local files with thin re-exports:
  ```rust
  // crates/ragent-agent/src/memory/cross_project.rs
  //! Compatibility wrapper; source-of-truth in ragent-tools-extended.
  pub use ragent_tools_extended::memory::cross_project::*;
  ```
  and likewise for `embedding.rs`, `migrate.rs`, and
  `embedding/local.rs` (keep the `embedding/` directory + `mod.rs`).
- [ ] 5.3 Delete the now-unused `ragent_config::CrossProjectConfig` direct
      import in `memory_write.rs`/`memory_search.rs` agent copies — but note
      these files are themselves deleted in Milestone 4, so order Milestone 5
      **before** Milestone 4's deletion of `memory_search.rs`/`memory_write.rs`.
      (Reorder: execute 5.2 first, then the memory_* tool deletions in 4.)
- [ ] 5.4 `cargo check --workspace && cargo test -p ragent-agent -p ragent-tools-extended` must pass.

**Deliverable:** ~1,457 LOC of duplicated logic replaced by ~20 LOC of
re-exports. Commit:
`refactor(agent): re-export memory cross_project/embedding/migrate from ragent-tools-extended`.

---

### Milestone 6 — Cleanup pass & verification

**Goal:** Tidy up after the bulk deletions and prove nothing regressed.

- [ ] 6.1 Re-scan for any remaining `ragent_core::tool::X` / `crate::tool::X`
      references to deleted modules:
      `grep -rn "ragent_core::tool::(append_file|bash_reset|calculator|copy_file|create|diff|file_info|glob|grep|list|mkdir|move_file|rm|think|truncate|write|get_env|patch|read|bash|task_complete|http_request|libreoffice|office_|pdf_|todo|codeindex|memory_migrate|memory_replace|memory_search|memory_write|file_ops_tool|format)\b" crates/ src/`
      Expected: empty.
- [ ] 6.2 Remove the now-stale `NOTE: This module is currently registered
      via the ragent-tools-core ...` header comments that referenced the
      duplication (they live in the deleted files, so this is automatic).
- [ ] 6.3 Run `cargo fmt --check` and `cargo clippy --workspace -- -D warnings` (or at least `cargo clippy --workspace` and triage new lints).
- [ ] 6.4 Run `cargo test --workspace` with a 600s timeout. Record pass/fail counts and compare to the Milestone 0 baseline.
- [ ] 6.5 Update `docs/reports/dcremoval-baseline.md` with the final metrics: LOC removed, crate sizes before/after, test counts before/after.
- [ ] 6.6 Update `CHANGELOG.md` with an entry under the next alpha version: "Removed ~11k lines of duplicated tool source from ragent-agent/src/tool/; consolidated memory helpers via re-exports."

**Deliverable:** Verification report + changelog entry. Commit:
`docs: record dead-code/duplication remediation completion`.

---

## 5. Ordering & Dependencies

```
M0 (baseline)
  └─ M1 (delete dead modules: file_ops_tool, format)
       └─ M2 (re-point 3 live call sites)
            ├─ M3 (delete 21 core-tool duplicates)
            └─ M5 (consolidate memory helpers) ──┐
                 └─ M4 (delete 22 extended-tool   │
                        duplicates, incl. the     │
                        memory_* tools that used  │
                        the now-re-exported       │
                        memory helpers)           │
                      └───────────────────────────┘
            └─ M6 (cleanup & verify)
```

M3 and M4 can run in parallel after M2 + M5, but for safety (single-change
discipline) execute them sequentially: M1 → M2 → M5 → M3 → M4 → M6.

---

## 6. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| A deleted module has an undiscovered external caller | Low | Build break | Per-file `cargo check` between deletions; workspace grep for `tool::X` references before each delete. |
| `ragent-tui` gaining a direct `ragent-tools-core` dep is undesirable | Low | Minor dep-graph widening | Acceptable: the crate is already transitively compiled. Alternatively, expose `get_safe_commands`/`get_builtin_lists`/`is_safe_command` via a small `pub use` in `ragent-agent::tool::bash` shim that forwards to `ragent_tools_core::bash`. The plan prefers the direct dep for clarity. |
| Memory-helper re-exports drop a symbol the agent crate needs | Medium | Build break | M5.1 explicitly enumerates the required symbols and verifies they are `pub` in the extended crate before swapping. |
| Doc-test references to `ragent_core::tool::X` for deleted `X` | Low | Test break | The affected doc-tests (e.g. in `truncate.rs`, `think.rs`) are inside the files being deleted, so they vanish with them. Re-run `cargo test --doc` in M6. |
| Behavioural drift: agent-local `codeindex_*` copies lack busy-lock retry; deleting them is an *improvement*, not a regression | None (positive) | Positive | Documented in §3.2 — the extracted-crate versions are strictly newer and are already what the runtime uses. |
| `bash.rs` agent copy is 868 LOC vs 1546 in core — confirms core is the newer/canonical version | None | Positive | Deletion removes stale code; runtime already uses the core version. |

**Overall risk: LOW.** The runtime registry already uses the extracted-crate
versions exclusively, so deleting the dormant copies cannot change runtime
behaviour. The only behavioural change is in §3.6 call sites, which are
re-pointed to the same functions in the extracted crates (verified
byte-identical signatures).

---

## 7. Expected Outcome

| Metric | Before | After (est.) | Delta |
|--------|--------|--------------|-------|
| `ragent-agent/src/tool/` LOC | 18,215 | ~6,700 | **−11,515** |
| `ragent-agent/src/memory/` duplicated LOC | ~1,457 | ~20 (re-exports) | **−1,437** |
| Dead modules (`file_ops_tool`, `format`) | 484 LOC | 0 | **−484** |
| Total duplicated/dead LOC removed | — | — | **≈13,000** |
| Source-of-truth copies per tool | 2 | 1 | drift risk eliminated |
| `cargo check --workspace` | clean | clean | — |
| `cargo test --workspace` | baseline | ≥ baseline | to be confirmed in M6 |

---

## 8. Out of Scope (explicitly NOT touched)

- The agent-specific tool modules that have **no** counterpart in the
  extracted crates: `plan.rs`, `new_task.rs`, `cancel_task.rs`,
  `list_tasks.rs`, `wait_tasks.rs`, `structured_memory.rs`, `team_*` (via
  `#[path]`), `spec_*.rs`, `aliases.rs`, `mcp_tool.rs`, `metadata.rs`.
- The `ragent-tools-vcs` crate and its `ExtractedVcsToolAdapter` (already
  single-source via `#[path]` includes for team tools; VCS tools are not
  duplicated).
- Behavioural changes to any tool — this is a pure deletion/re-pointing
  refactor. No tool's logic changes.
- The `ragent_core` → `ragent-agent` Cargo alias in `ragent-tui`/`ragent-server`
  (kept for compatibility).

---

## 9. Approval

This plan is **proposed** and not yet started. On approval, execution will
proceed milestone-by-milestone with a `cargo check` + `cargo test` gate
after each, and a single commit per milestone as named in §4.