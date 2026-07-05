# DUPPLAN.md — Code Duplication Removal Plan

> Generated from `cargo dupes report --min-lines 6 --threshold 0.85 --exclude "target/*"`

## Executive Summary

`cargo-dupes` analysed **5,141 code units** (142,348 source lines) and found:

| Metric | Count |
|--------|-------|
| Exact-duplicate groups | 385 groups / 1,108 code units |
| Near-duplicate groups | 142 groups / 332 code units |
| Duplicated lines (exact) | 15,573 (10.9%) |
| Duplicated lines (near) | 5,963 (4.2%) |
| **Total duplication** | **~15.1%** |

Breakdown by location:

| Location | Exact hits | Near hits |
|----------|-----------|-----------|
| `/src/` (production) | 607 | 198 |
| `/tests/` (test code) | 489 | 135 |
| `/benches/` (benchmarks) | 55 | 6 |

This plan targets the **highest-impact, lowest-risk** duplications first.
Test-only duplication is addressed in later milestones because it carries no
runtime cost and is higher-churn.

---

## Triage Methodology

Each duplicate group was classified into one of three tiers:

| Tier | Criteria | Action |
|------|----------|--------|
| **Tier 1** | Production source (`/src/`), ≥4 copies, or dead-code copies | Remove now |
| **Tier 2** | Test/bench helpers (`/tests/`, `/benches/`), ≥5 copies | Extract to shared test-support module |
| **Tier 3** | Idiomatic Rust patterns (`as_str`, `Display`, `Default`), or 2–3 near-dup copies with legitimate variation | Leave (document as accepted) |

### Risk levels

- **Low** — pure deletion of dead code, or mechanical extraction of a leaf function with no behavioural change.
- **Medium** — extraction that touches cross-crate module visibility (e.g. adding a `pub` re-export) but has no logic change.
- **High** — structural refactor (macros, trait reshaping) that could affect compilation of unrelated code.

Each task below is sized to be independently testable: `cargo check --workspace`
and `cargo test -p <affected-crate>` must pass before the task is marked complete.

---

## Milestone A — Dead-Code VCS Tool Copies (Tier 1, Low Risk) ✅ DONE

**Problem:** `crates/ragent-agent/src/tool/` contains five full verbatim copies
of GitHub/GitLab tool implementations that already live canonically in
`crates/ragent-tools-vcs/`. The agent crate never registers or references the
local copies — VCS tool registration goes through the
`ExtractedVcsToolAdapter` which wraps `ragent_tools_vcs::registry::create_vcs_registry()`
(see `crates/ragent-agent/src/tool/mod.rs:1089`). These five files are
**2,461 lines of dead, duplicated production code** that compile but are never used.

| File (agent copy — DELETE) | Canonical source (keep) | Lines |
|-----------------------------|-------------------------|-------|
| `ragent-agent/src/tool/github_issues.rs` | `ragent-tools-vcs/src/github/github_issues.rs` | 457 |
| `ragent-agent/src/tool/github_prs.rs` | `ragent-tools-vcs/src/github/github_prs.rs` | 421 |
| `ragent-agent/src/tool/gitlab_issues.rs` | `ragent-tools-vcs/src/gitlab/gitlab_issues.rs` | 445 |
| `ragent-agent/src/tool/gitlab_mrs.rs` | `ragent-tools-vcs/src/gitlab/gitlab_mrs.rs` | 425 |
| `ragent-agent/src/tool/gitlab_pipelines.rs` | `ragent-tools-vcs/src/gitlab/gitlab_pipelines.rs` | 713 |

Confirmed by `cargo dupes` exact-dup groups 50, 51, 53 (and near-dup group 102).
A `diff` of each pair shows byte-for-byte identity (only `let ... &&` vs
nested-`if` style differs in one file).

### Task A-1: Delete the five dead VCS tool copies ✅ DONE

- **Scope:** Remove the five files listed above.
- **Edits:** Delete `pub mod github_issues;`, `pub mod github_prs;`,
  `pub mod gitlab_issues;`, `pub mod gitlab_mrs;`, `pub mod gitlab_pipelines;`
  from `crates/ragent-agent/src/tool/mod.rs` (lines 18–25).
- **Verify:**
  - `cargo check -p ragent-agent` passes.
  - `cargo test -p ragent-agent` passes (no test imports these modules — confirmed).
  - `grep -rn "github_issues::\|gitlab_issues::\|GithubListIssuesTool\|GitlabListIssuesTool" crates/ragent-agent/src/` returns only `ragent_tools_vcs` references in `tool/mod.rs`.
- **Risk:** Low. The modules are unreferenced outside their own files.
  `register_extracted_vcs_tools()` at line 1089 uses `ragent_tools_vcs`, not
  the local copies.
- **Acceptance:** `cargo dupes report` no longer lists groups 50, 51, 53, 102.

### Task A-2: Add CI guard for VCS tool duplication ✅ DONE

- **Scope:** Add a shell guard analogous to
  `scripts/check-team-duplication.sh` that fails if any
  `github_*.rs` or `gitlab_*.rs` file reappears under
  `crates/ragent-agent/src/tool/`.
- **File:** `scripts/check-vcs-duplication.sh`
- **Verify:** `bash scripts/check-vcs-duplication.sh` prints `OK:` and exits 0.
- **Acceptance:** The script is listed in `pre-flight.sh` alongside the team
  duplication check.

**Milestone A exit criteria:** 2,461 lines removed, zero `cargo dupes` VCS
cross-crate groups, CI guard in place.

---

## Milestone B — `resolve_path` Extraction (Tier 1, Low Risk) ✅ DONE

**Problem:** The identical 8-line `resolve_path` helper is copy-pasted across
**18 files** (exact-dup group 1, the largest single group in the report):

- 16 copies in `crates/ragent-tools-core/src/` (`append_file.rs`, `copy_file.rs`,
  `create.rs`, `diff.rs`, `edit.rs`, `file_info.rs`, `glob.rs`, `grep.rs`,
  `list.rs`, `mkdir.rs`, `move_file.rs`, `multiedit.rs`, `patch.rs`, `read.rs`,
  `rm.rs`, `write.rs`).
- 2 copies in `crates/ragent-tools-extended/src/` (`libreoffice_common.rs`,
  `office_common.rs`) — these are already `pub` and used by the office/pdf
  tools in the same crate.

The function body is trivial:

```rust
fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() { p } else { working_dir.join(p) }
}
```

### Task B-1: Add `resolve_path` to `ragent-tools-core` shared module ✅ DONE

- **Scope:** Create `crates/ragent-tools-core/src/path_util.rs` exposing
  `pub fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf`.
- **Edits:** Add `pub mod path_util;` to `lib.rs`.
- **Doc:** Add a `///` doc-block per AGENTS.md documentation standards.
- **Verify:** `cargo check -p ragent-tools-core` passes.

### Task B-2: Replace the 16 core copies with the shared helper ✅ DONE

- **Scope:** For each of the 16 `ragent-tools-core/src/*.rs` files:
  1. Delete the local `fn resolve_path` definition.
  2. Replace call sites `resolve_path(...)` with `crate::path_util::resolve_path(...)`
     (or add `use crate::path_util::resolve_path;` if preferred for brevity).
- **Verify:** `cargo test -p ragent-tools-core` — all existing file-tool tests
  must pass unchanged (behaviour is identical).
- **Risk:** Low. Pure mechanical rename; the function is a leaf helper with no
  state.
- **Acceptance:** `cargo dupes report` group 1 disappears.

### Task B-3: Consolidate the 2 extended copies ✅ DONE

- **Scope:** `ragent-tools-extended` depends on `ragent-tools-core` (check
  `Cargo.toml`). Replace the `pub fn resolve_path` in
  `libreoffice_common.rs` and `office_common.rs` with a re-export
  `pub use ragent_tools_core::path_util::resolve_path;` so existing office/pdf
  tool call sites still resolve.
- **Verify:** `cargo test -p ragent-tools-extended` passes.
- **Acceptance:** No `fn resolve_path` definition remains outside
  `path_util.rs`.

**Milestone B exit criteria:** 18 → 1 copy; ~136 duplicated lines removed.

---

## Milestone C — Code-Index Parser Boilerplate (Tier 1, Medium Risk) ✅ DONE

**Problem:** The tree-sitter parser subsystem in `crates/ragent-codeindex/src/parser/`
repeats three boilerplate patterns across 7–10 language files:

| Group | Pattern | Copies | Files |
|-------|---------|--------|-------|
| 10 | `fn create_parser() -> Result<Parser>` (8 lines) | 9 | gradle, cmake, go, gradle_kts, hcl, openscad, python, maven, rust |
| 16 | `fn parse_tree(source: &[u8]) -> Result<Tree>` (6 lines) | 7 | gradle, cmake, gradle_kts, hcl, openscad, maven, rust |
| 8 | `fn build_qname(scope, name) -> String` (7 lines) | 10 | gradle, c_cpp, go, gradle_kts, hcl, java, openscad, python (as `build_qualified`), typescript, rust (as `build_qualified_name`) |

The `create_parser`/`parse_tree` pair differs only in the `tree_sitter_<lang>::LANGUAGE`
constant and the grammar name in the error context string. The `build_qname`
helpers differ only in the separator (`::` vs `.`).

### Task C-1: Extract `build_qname` into a shared helper ✅ DONE

- **Scope:** Add `pub fn build_qname(scope: &[String], name: &str, sep: &str) -> String`
  to `crates/ragent-codeindex/src/parser/mod.rs` (or a new
  `parser/util.rs`). Replace the 10 local copies, passing `"::"` or `"."` per
  language convention.
- **Verify:** `cargo test -p ragent-codeindex` — the inline parser unit tests
  (`test_c_function`, `test_type_alias`, etc.) must pass.
- **Risk:** Medium — touches 10 files across the parser subsystem, but each
  change is a one-line call replacement.
- **Acceptance:** `cargo dupes report` group 8 disappears.

### Task C-2: Macro-ify `create_parser` + `parse_tree` ✅ DONE

- **Scope:** Define a `tree_sitter_parser!(StructName, tree_sitter_lang::LANGUAGE, "Lang")`
  declarative macro in `crates/ragent-codeindex/src/parser/mod.rs` that expands
  to the `create_parser` + `parse_tree` pair. Apply it to the 7–9 language
  structs that use the uniform pattern (exclude `typescript` and `c_cpp` if
  they have variant logic — verify first).
- **Verify:** `cargo test -p ragent-codeindex` passes; `cargo bench -p ragent-codeindex`
  shows no regression in parse throughput.
- **Risk:** Medium — macros can obscure errors; keep the macro simple and
  document it.
- **Acceptance:** `cargo dupes report` groups 10 and 16 disappear.

**Milestone C exit criteria:** ~150 lines of parser boilerplate removed;
3 duplicate groups eliminated.

---

## Milestone D — `not_available` Code-Index Fallback (Tier 1, Low Risk) ✅ DONE

**Problem:** Six codeindex tool files in `crates/ragent-tools-extended/src/`
each define an identical 10-line `fn not_available() -> ToolOutput` that
returns the "code index disabled" fallback message (exact-dup group 22):

`codeindex_reindex.rs`, `codeindex_dependencies.rs`, `codeindex_references.rs`,
`codeindex_search.rs`, `codeindex_status.rs`, `codeindex_symbols.rs`.

### Task D-1: Extract `not_available` to a shared helper ✅ DONE

- **Scope:** Add `pub(crate) fn codeindex_not_available() -> ToolOutput` to
  `crates/ragent-tools-extended/src/codeindex_common.rs` (new file) or to the
  existing `lib.rs` if a module already exists. Replace the 6 local definitions.
- **Verify:** `cargo test -p ragent-tools-extended` passes; manually invoke
  each tool with `code_index = None` and confirm the message is unchanged.
- **Risk:** Low — leaf function, no state.
- **Acceptance:** `cargo dupes report` group 22 disappears.

**Milestone D exit criteria:** 6 → 1 copy; ~50 lines removed.

---

## Milestone E — `resource.rs` Triple-Copy (Tier 1, Medium Risk) ✅ DONE

**Problem:** The process/tool concurrency semaphore module is duplicated three
times (exact-dup group 32):

1. `crates/ragent-types/src/resource.rs` — **canonical** (has tests).
2. `crates/ragent-agent/src/resource.rs` — near-identical (only adds a
   `#[cfg(test)] mod tests` block; the production code is byte-identical).
3. `crates/ragent-tools-core/src/lib.rs` lines 66–88 — an inline `pub mod resource`
   containing only `acquire_process_permit` (a subset of the above).

Call sites:
- `ragent-agent/src/session/processor.rs:1014` → `crate::resource::acquire_tool_permit`
- `ragent-agent/src/skill/context.rs:235` → `crate::resource::acquire_process_permit`
- `ragent-tools-core/src/bash.rs:1051` → `crate::resource::acquire_process_permit`

Both `ragent-agent` and `ragent-tools-core` already depend on `ragent-types`
(confirmed in `Cargo.toml`).

### Task E-1: Delete `ragent-agent/src/resource.rs`, re-export from `ragent-types` ✅ DONE

- **Scope:**
  1. Delete `crates/ragent-agent/src/resource.rs`.
  2. In `crates/ragent-agent/src/lib.rs`, replace `pub mod resource;` with
     `pub use ragent_types::resource;` (or `pub mod resource { pub use ragent_types::resource::*; }`
     if path-qualified imports are needed).
  3. Move the `#[cfg(test)] mod tests` block from the deleted file into
     `crates/ragent-types/tests/test_resource.rs` if not already present
     (it is — confirmed at `ragent-types/tests/test_resource.rs`).
- **Verify:** `cargo test -p ragent-agent` and `cargo test -p ragent-types`
  pass.
- **Risk:** Medium — the `pub use` must preserve `acquire_tool_permit` and
  `acquire_process_permit` visibility for downstream call sites.
- **Acceptance:** `ragent-agent/src/resource.rs` no longer exists.

### Task E-2: Replace `ragent-tools-core` inline `resource` module ✅ DONE

- **Scope:** In `crates/ragent-tools-core/src/lib.rs`, delete the inline
  `pub mod resource { ... }` block (lines 66–88). Add
  `pub use ragent_types::resource::acquire_process_permit;` at the crate root,
  or update `bash.rs:1051` to call `ragent_types::resource::acquire_process_permit`.
- **Verify:** `cargo test -p ragent-tools-core` (the `bash` tool tests must pass).
- **Risk:** Medium — `bash.rs` is the only consumer; the change is a path rename.
- **Acceptance:** No `fn acquire_process_permit` definition outside
  `ragent-types/src/resource.rs`.

**Milestone E exit criteria:** 3 → 1 copy; ~60 lines removed; single source of
truth for concurrency permits.

---

## Milestone F — `strip_tags` and Other Small Cross-Crate Helpers (Tier 1, Low Risk) ✅ DONE

**Problem:** A handful of small utility functions are duplicated across crates
that already share a dependency:

| Near-dup group | Function | Copies | Notes |
|----------------|----------|--------|-------|
| 121 | `strip_tags(html: &str) -> String` | 2 | `ragent-tools-extended/src/webfetch.rs:185` and `ragent-research/src/web_date.rs:178`. Slight variation (one pushes a space on `<`). |
| 80 | `detect_python_sections` / `detect_go_sections` | 2 | `ragent-tools-core/src/read.rs:451,590` — same shape, different language. Accept as-is. |

### Task F-1: Unify `strip_tags` ✅ DONE

- **Scope:** Decide which variant is correct (the space-pushing variant in
  `web_date.rs` prevents words from merging across tag boundaries — that is the
  better behaviour). Add `pub fn strip_tags(html: &str) -> String` to
  `ragent-tools-extended/src/webfetch.rs` (already `pub`-ish) and have
  `ragent-research` import it, **or** move it to `ragent-types` if both crates
  should share it without a direct dependency.
  - Check: does `ragent-research` depend on `ragent-tools-extended`? If not,
    place the helper in `ragent-types` (both crates depend on it).
- **Verify:** `cargo test -p ragent-research` (web_date tests) and
  `cargo test -p ragent-tools-extended` (webfetch tests) pass.
- **Risk:** Low — leaf function. Confirm the space-on-`<` behaviour is
  acceptable for webfetch callers.
- **Acceptance:** `cargo dupes report` near-dup group 121 disappears.

**Milestone F exit criteria:** 2 → 1 copy; behaviour unified.

---

## Milestone G — TUI `make_app` Test Helper (Tier 2, Medium Risk) ✅ DONE

**Problem:** The `make_app()` function — a ~45-line `App` constructor that
wires up `EventBus`, `Storage::open_in_memory()`, `SessionProcessor`, and
`App::new(...)` — is copy-pasted across **27 files** (exact-dup groups 2 and
34, the second-largest group overall):

- 15 test files in `crates/ragent-tui/tests/`
- 3 bench files in `crates/ragent-tui/benches/`
- 9 additional test files (groups 34 covers another 5)

All 27 copies are byte-for-byte identical except for occasional trailing
parameter variation (`false, std::path::PathBuf::new()`).

### Task G-1: Create `crates/ragent-tui/tests/support/mod.rs` ✅ DONE

- **Scope:** Create a shared test-support module:
  1. Add `crates/ragent-tui/tests/support/mod.rs` with `pub fn make_app() -> App`.
  2. Make it available to all integration tests by adding a
     `#[path = "support/mod.rs"] mod support;` line at the top of each test
     file (or use a `support` module declared in a common `tests/support.rs`
     harness if the test framework permits).
  3. Each test file replaces its local `make_app` with `support::make_app()`.
- **Verify:** `cargo test -p ragent-tui` — all TUI tests pass.
  `cargo bench -p ragent-tui` — benches still compile.
- **Risk:** Medium — the `App` constructor imports many types (`provider`,
  `tool`, `agent`, `SessionProcessor`, `Storage`, `EventBus`). The support
  module must re-export or `use` all of them. Watch for test files that
  customise `make_app` (e.g. `test_history.rs` has a slightly different body —
  verify before replacing).
- **Acceptance:** `cargo dupes report` groups 2 and 34 disappear; only one
  `make_app` definition remains.

**Milestone G exit criteria:** 27 → 1 copy; ~1,200 lines of test boilerplate
removed.

---

## Milestone H — `MockStorage` / `DemoStorage` Test Helpers (Tier 2, Low Risk) ✅ DONE

**Problem:** An in-memory `StorageBackend` mock is duplicated verbatim across
4 files in `crates/ragent-tools-extended/` (exact-dup groups 49, 60, 65, 67 —
each group is one trait method: `get_todos`, `create_todo`, `update_todo`,
`clear_todos`):

- `examples/todo_cycle.rs` (defines `DemoStorage`)
- `tests/test_todo_demo.rs` (defines `MockStorage`)
- `tests/test_todo_lifecycle.rs` (defines `MockStorage`)
- `tests/test_todo_status_change.rs` (defines `MockStorage`)

The four `MockStorage` copies are identical; `DemoStorage` differs only in
struct name.

### Task H-1: Extract `MockStorage` to a shared test module ✅ DONE

- **Scope:** Create `crates/ragent-tools-extended/tests/support/mock_storage.rs`
  containing the `MockStorage` struct and its `StorageBackend` impl. Each of the
  3 test files does `#[path = "support/mock_storage.rs"] mod mock_storage;` and
  `use mock_storage::MockStorage;`. Update the example to either use the same
  shared module or keep `DemoStorage` as a documented example variant.
- **Verify:** `cargo test -p ragent-tools-extended` and
  `cargo run --example todo_cycle` both pass.
- **Risk:** Low — test-only code.
- **Acceptance:** `cargo dupes report` groups 49, 60, 65, 67 disappear.

**Milestone H exit criteria:** 4 → 1 copy; ~120 lines removed.

---

## Milestone I — `setup` / `setup_workspace` Test Helpers (Tier 2, Low Risk) ✅ DONE

**Problem:** Two temp-directory setup helpers are duplicated across team and
memory test suites:

| Group | Function | Copies | Location |
|-------|----------|--------|----------|
| 33 | `fn setup() -> TempDir` (5 lines) | 5 | `ragent-agent/src/memory/{defaults,import_export}.rs`, `ragent-tools-extended/src/memory/{migrate,storage,cross_project}.rs` |
| 37 | `fn setup_workspace() -> (TempDir, PathBuf)` (5 lines) | 5 | `ragent-team/tests/{test_perf022_jsonl_mailbox,test_m3_lifecycle,test_m4_delivery,test_m6_resilience,test_perf023_task_cache}.rs` |

### Task I-1: Share `setup_workspace` across team tests ✅ DONE

- **Scope:** Create `crates/ragent-team/tests/support/mod.rs` with
  `pub fn setup_workspace() -> (TempDir, PathBuf)`. Each of the 5 test files
  includes it via `#[path]`.
- **Verify:** `cargo test -p ragent-team` passes.
- **Acceptance:** Group 37 disappears.

### Task I-2: Share `setup` across memory tests ✅ DONE

- **Scope:** The 5 `setup` functions are in `src/` (inline `#[cfg(test)]`
  modules) and `tests/`. For the `src/` inline ones, extract to a
  `#[cfg(test)] pub(crate) fn setup_temp_dir()` in
  `crates/ragent-agent/src/memory/mod.rs` and have each submodule's test module
  call it. For `ragent-tools-extended`, do the same in
  `src/memory/mod.rs`.
- **Verify:** `cargo test -p ragent-agent` and `cargo test -p ragent-tools-extended` pass.
- **Note:** AGENTS.md says "Do not add new inline `#[cfg(test)]` modules to
  library source files." These already exist; prefer moving them to `tests/`
  if practical, but the `src/` inline tests are the existing convention here.
  At minimum, consolidate to one definition per crate.
- **Acceptance:** Group 33 disappears (or reduces to ≤2 legitimate variants).

**Milestone I exit criteria:** 10 → 2 copies; ~40 lines removed.

---

## Milestone J — Accepted / Documented Duplications (Tier 3, No Action) ✅ DONE

The following duplicate groups are **idiomatic Rust patterns** or have
legitimate per-variant logic. They are documented here as explicitly accepted
and should **not** be refactored.

| Group(s) | Pattern | Reason to keep |
|----------|---------|----------------|
| 30, 36, 43 | `fn as_str(&self) -> &'static str` match blocks on enums | Idiomatic enum Display; each enum has different variants. A macro would hurt readability. |
| 63 (near) | `<T as Display>::fmt` impls | Same — per-type formatting. |
| 6 (near), 86 | `Default` impls for config structs | Boilerplate but per-struct fields differ. |
| 13 (near) | `From` impl blocks | Per-type conversions; cannot be generic. |
| 28 | closures in `anthropic.rs` | Two distinct streaming handlers with different capture sets. |
| 80 | `detect_python_sections` vs `detect_go_sections` | Same shape, different language grammar. Generalising would obscure intent. |
| 40 | `IndexStore::file_count`/`total_bytes`/`symbol_count`/`reference_count` | Four distinct DB accessors; identical shape is fine. |
| 44 | `LocalTool::grep` no-op impls | Trait impls for different mock types; cannot be deduplicated. |
| 47 | `BenchSuiteAdapter::build_prompt` | Per-suite prompt builders; identical shape is coincidental. |
| 58 | `LanguageParser::parse` for gradle/gradle_kts/hcl/openscad | Same walk pattern but different `Ctx` types per language. |
| 3, 4, 9, 11, 12, 14, 15 | Test functions with similar assertion shape | Test-only; each exercises a different input. Extracting a helper would obscure what each test asserts. |

### Task J-1: Document accepted duplications in code comments ✅ DONE

- **Scope:** Where practical, add a brief `// NOTE: intentional duplication — see DUPPLAN.md Milestone J` comment above the accepted groups so future readers don't attempt to "fix" them.
- **Risk:** None (comment-only).
- **Acceptance:** No behaviour change; `cargo dupes` numbers unchanged for these groups.

**Milestone J exit criteria:** Accepted duplications are documented; no
regressions introduced.

---

## Milestone K — Verification & Regression Baseline ✅ DONE

### Task K-1: Capture pre-refactor baseline ✅ DONE

- **Before any changes:** run
  `cargo dupes report --min-lines 6 --threshold 0.85 --exclude "target/*" > docs/reports/dupes-baseline.txt`
  and save the stats summary. This is the "before" snapshot for the
  changelog.

### Task K-2: Post-refactor measurement ✅ DONE

- After each milestone, re-run `cargo dupes report` and record the new
  stats in `CHANGELOG.md`. Target:
  - Exact-duplicate groups: 385 → ≤ 330 (remove groups 1, 2, 8, 10, 16, 22, 32, 34, 49, 50, 51, 53, 60, 65, 67, 102, 121).
  - Duplicated lines (exact): 15,573 → ≤ 11,500.
  - No new near-duplicate groups introduced.

### Task K-3: Full test sweep ✅ DONE

- After all milestones: `timeout 600 cargo test --workspace -- --test-threads=1`
  must pass with zero failures.

### Task K-4: Update CHANGELOG.md ✅ DONE

- Add a "Removed code duplication" section summarising total lines removed and
  groups eliminated, referencing this plan.

**Milestone K exit criteria:** Baseline captured, regression-free, changelog
updated.

---

## Summary Table

| Milestone | Tier | Risk | Tasks | Est. lines removed | Key groups eliminated |
|-----------|------|------|-------|--------------------|-----------------------|
| A | 1 | Low | 2 | 2,461 | 50, 51, 53, 102 |
| B | 1 | Low | 3 | ~136 | 1 |
| C | 1 | Medium | 2 | ~150 | 8, 10, 16 |
| D | 1 | Low | 1 | ~50 | 22 |
| E | 1 | Medium | 2 | ~60 | 32 |
| F | 1 | Low | 1 | ~15 | 121 (near) |
| G | 2 | Medium | 1 | ~1,200 | 2, 34 |
| H | 2 | Low | 1 | ~120 | 49, 60, 65, 67 |
| I | 2 | Low | 2 | ~40 | 33, 37 |
| J | 3 | None | 1 | 0 | (documented) |
| K | — | Low | 4 | 0 | (verification) |
| **Total** | | | **20** | **~4,230** | **~20 groups** |

## Recommended Execution Order

1. **Milestone A** first — largest single win (2,461 lines), lowest risk
   (dead-code deletion), immediately shrinks the `ragent-agent` compile surface.
2. **Milestone B** — mechanical, high visibility (group 1 is the largest group).
3. **Milestone D** — quick win, same crate as B.
4. **Milestone E** — consolidates a cross-crate triple-copy; do before C so
   the codeindex milestone is isolated.
5. **Milestone C** — medium-risk macro work; do once B/D/E are green.
6. **Milestone F** — small tidy-up.
7. **Milestones G, H, I** — test helpers; batch together at the end.
8. **Milestone J** — comment-only; can be done anytime.
9. **Milestone K** — final verification and changelog.

Each milestone is independently shippable. If time is constrained, Milestones
A + B alone remove ~2,600 lines and eliminate the top two duplicate groups.