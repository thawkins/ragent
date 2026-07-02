---
status: draft
audit:
  - { time: 1782966953, from: "none", to: "draft", actor: "system" }
---
# Specification: Test Consolidation — Relocate Inline Tests to Per-Crate `tests/` Directories

## Executive Summary

The ragent workspace currently contains **1,506 inline test functions** spread across
**135 source files** in 15 crates. These tests live inside `#[cfg(test)]` modules at the
bottom of library source files, directly contradicting the project guideline in
`AGENTS.md` which mandates: *"All tests MUST be located in the `tests/` directory inside
each crate."* This specification defines a structured, mechanical, and verifiable migration
that relocates every inline test into a dedicated external test file under the
appropriate crate's `tests/` folder, without altering the behaviour or coverage those
tests provide.

## Background

The ragent workspace is a Cargo workspace composed of 15 focused crates
(`ragent-agent`, `ragent-bench`, `ragent-codeindex`, `ragent-config`, `ragent-llm`,
`ragent-prompt_opt`, `ragent-research`, `ragent-server`, `ragent-specs`,
`ragent-storage`, `ragent-team`, `ragent-tools-core`, `ragent-tools-extended`,
`ragent-tools-vcs`, `ragent-tui`, `ragent-types`).

A survey of the codebase found:

| Crate               | Files with inline tests | Tests using `super::` | Existing external test files |
|---------------------|-------------------------|-----------------------|------------------------------|
| ragent-agent         | 34                      | 56                    | 22                           |
| ragent-bench         | 4                       | 5                     | 1                            |
| ragent-codeindex     | 19                      | 19                    | 5                            |
| ragent-config        | 2                       | 2                     | 10                           |
| ragent-llm           | 16                      | 19                    | 8                            |
| ragent-prompt_opt     | 1                      | 1                     | 1                            |
| ragent-research      | 20                      | 20                    | 1                            |
| ragent-server        | 1                       | 1                     | (present)                    |
| ragent-specs         | 9                       | 9                     | 5                            |
| ragent-storage       | 1                       | 1                     | **0 (no `tests/` dir)**       |
| ragent-team          | 3                       | 23                    | 10                           |
| ragent-tools-core    | 6                       | 24                    | 3                            |
| ragent-tools-extended | 7                      | 28                    | 4                            |
| ragent-tools-vcs     | (present)               | 5                     | (present)                    |
| ragent-tui           | 10                      | 10                    | 29                           |
| ragent-types         | 3                       | 3                     | 1                            |

**Key observations:**

1. **`ragent-storage` has no `tests/` directory at all** — it must be created.
2. **193 source files use `use super::`** inside their inline test modules to exercise
   private items. Relocating these tests requires either widening visibility to
   `pub(crate)` on the accessed items, or using the `#[path = "../src/<module>.rs"]
   mod <module>;` re-import pattern so external tests retain access to private items.
3. The established migration pattern (seen in `ragent-tools-core/tests/`) uses public
   crate APIs (`use ragent_tools_core::edit::EditTool;`) with helper functions (`ctx()`,
   `write_file()`) and follows the naming convention `test_<component>_<scenario>.rs`.

## Scope & Objectives

### Scope

The Test Consolidation effort covers:

- **Discovery**: enumerate every `#[cfg(test)]` module in every crate and at the
  workspace root.
- **Classification**: categorise each inline test module by (a) which crate it belongs
  to, (b) whether it accesses private items via `super::`, and (c) whether a target
  external test file already exists.
- **Relocation**: move each inline test module into a dedicated file under the most
  suitable crate's `tests/` directory, preserving test names, assertions, and intent.
- **Visibility resolution**: for tests that access private items, either widen the
  item to `pub(crate)` (preferred where the item is an internal implementation detail
  of the crate) or re-import the source module via `#[path]` so the external test file
  retains access to private items.
- **Workspace-root inline tests**: any inline tests at the workspace root (e.g. in
  `src/main.rs`) are relocated to the root `tests/` folder.
- **Verification**: after migration, `cargo test --workspace` must pass with no
  reduction in test count or coverage, and `cargo clippy --workspace` must report
  no new warnings.
- **Documentation**: update `AGENTS.md` test-organisation guidance to reflect the
  completed state and codify the migration rules for future additions.

### Out of Scope

- Writing **new** tests or improving test coverage beyond the mechanical relocation.
- Refactoring the production source code for its own sake (only visibility adjustments
  needed to unblock relocation are permitted).
- Migrating benchmark suites (`benches/`) — those are out of scope; only `#[test]`
  and `#[tokio::test]` functions are moved.
- Doctests (`///` examples) — these remain inline by Rust convention.
- Performance optimisation of the test harness.

### Objectives

1. Eliminate every `#[cfg(test)]` module from library source files across the workspace.
2. Maintain 100% of existing test behaviour — no test is dropped, renamed in meaning,
   or weakened during migration.
3. Establish a uniform `tests/` directory in every crate that currently lacks one
   (notably `ragent-storage`).
4. Preserve a clear audit trail via one migration commit per crate (or per logical
   batch), so the relocation can be reviewed and, if needed, reverted per-crate.
5. Enforce the convention going forward so any new inline test added to a source file
   is caught by a lint or CI check.

---

## Requirements

### FR-001 — Inline Test Discovery (Ubiquitous)

`The <migration tooling> shall <enumerate every source file in the workspace that contains a top-level #[cfg(test)] module, producing a manifest of file path, crate name, line range, and count of test functions>.`

*Ubiquitous requirement — applies at all times during discovery.*

### FR-002 — Private-Access Classification (Event-Driven)

`When <an inline test module is discovered>, the <migration tooling> shall <classify the module as either "public-API-only" (uses only `pub` items from the crate) or "private-access" (uses `use super::` or references non-`pub` items), recording the specific private items referenced>.`

*Event-driven requirement — triggered by the discovery event.*

### FR-003 — Target File Selection (State-Driven)

`While <an inline test module is being relocated>, the <migration tooling> shall <select a target file under the crate's tests/ directory using the naming convention test_<component>_<scenario>.rs, creating the tests/ directory if it does not already exist>.`

*State-driven requirement — active throughout the relocation phase.*

### FR-004 — Create Missing `tests/` Directories (Event-Driven)

`When <a crate containing inline tests is found to have no tests/ directory>, the <migration> shall <create the tests/ directory and add a corresponding [[test]] or default test target stanza to the crate's Cargo.toml so that cargo test discovers the new files>.`

*Event-driven requirement — triggered when the directory is absent.*

### FR-005 — Behaviour-Preserving Relocation (Ubiquitous)

`The <migration> shall <preserve every test function's name, assertions, helper functions, and async/sync attribute (#[test] vs #[tokio::test]) without alteration when moving it from the inline module to the external test file>.`

*Ubiquitous requirement — applies to every relocation.*

### FR-006 — Import Path Rewriting (State-Driven)

`While <an inline test module is being moved to an external file>, the <migration> shall <rewrite `use super::...` statements to reference the crate's public API (e.g. `use ragent_agent::...`) or, where the item is private, apply one of the two approved resolution strategies before the test file is committed>.`

*State-driven requirement — active during each relocation.*

### FR-007 — Private-Item Visibility Widening (Optional)

`Where <an inline test accesses a private item that is an internal implementation detail of the crate>, the <migration> <may widen the item's visibility to pub(crate) so that an external test file in the same crate can import it via `use crate::...`>.`

*Optional requirement — applied when the item is genuinely internal.*

### FR-008 — Private-Item Re-Import via `#[path]` (Optional)

`Where <widening visibility is undesirable because the item should remain private to its module>, the <migration> <may re-import the source module into the external test file using `#[path = "../src/<module>.rs"] mod <module>;` so that the test retains access to private items>.`

*Optional requirement — alternative strategy to FR-007.*

### FR-009 — No Coverage Regression (Unwanted)

`If <the migration of any crate would reduce the number of passing tests or the measured coverage compared to the pre-migration baseline>, the <migration> shall <abort that crate's migration, report the discrepancy, and leave the source file unchanged>.`

*Unwanted-behaviour requirement — guards against silent coverage loss.*

### FR-010 — Workspace-Wide Build & Clippy Gate (State-Driven)

`While <any crate migration is in progress>, the <migration process> shall <run `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` after each crate is migrated, and shall not proceed to the next crate until both commands pass cleanly>.`

*State-driven requirement — enforced as a per-crate gate.*

### FR-011 — Removal of Inline `#[cfg(test)]` Module (Ubiquitous)

`The <migration> shall <delete the entire #[cfg(test)] module block from the source file once its tests have been successfully relocated and verified, leaving no orphaned `mod tests` declaration behind>.`

*Ubiquitous requirement — applies to every successfully migrated file.*

### FR-012 — Root-Level Test Relocation (Event-Driven)

`When <inline tests are discovered in the workspace root (src/main.rs or root-level modules)>, the <migration> shall <relocate them into the root tests/ directory, creating it if absent, rather than into any crate's tests/ directory>.`

*Event-driven requirement — triggered by root-level test discovery.*

### FR-013 — Audit Trail & Per-Crate Commits (State-Driven)

`While <the migration is being committed to version control>, the <migration> shall <produce one commit per crate (or one per logical batch of closely-related crates), with a commit message naming the crate and the count of tests relocated>.`

*State-driven requirement — governs the commit cadence.*

### FR-014 — Post-Migration Lint Enforcement (Optional)

`Where <the migration is complete>, the <project> <may add a CI lint or pre-commit check that rejects any new #[cfg(test)] module added to a library source file, ensuring the consolidation is not silently undone>.`

*Optional requirement — hardens the convention after the fact.*

### FR-015 — Migration Report (Event-Driven)

`When <all crates have been migrated>, the <migration> shall <generate a final report at docs/reports/testconsolidate-completion.md listing per-crate counts of relocated tests, visibility changes applied, files created, and the pre/post `cargo test --workspace` pass counts>.`

*Event-driven requirement — fires once at completion.*

---

## Acceptance Criteria

1. `grep -rl "^#\[cfg(test)\]" --include="*.rs" crates/ src/` returns **zero** results.
2. `cargo test --workspace` passes with the **same or greater** number of test cases as
   the recorded pre-migration baseline.
3. `cargo clippy --workspace -- -D warnings` passes with no new warnings.
4. Every crate that previously had inline tests now has a `tests/` directory containing
   at least one `test_*.rs` file.
5. `ragent-storage` has a `tests/` directory (previously absent).
6. `docs/reports/testconsolidate-completion.md` exists and contains the per-crate
   relocation summary.
7. No production source file retains a `mod tests` or `#[cfg(test)]` block.

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Tests accessing private items break after relocation | Apply FR-007 or FR-008 before deletion; verify with `cargo test` per FR-010 |
| A crate's `Cargo.toml` lacks a `[[test]]` stanza for the new file | Add a default test target or rely on auto-discovery (Rust auto-discovers `tests/*.rs`) |
| Async tests lose their runtime context | Preserve `#[tokio::test]` attribute exactly (FR-005); verify with `cargo test` |
| Large crates (`ragent-agent`: 34 files, 74-test files) produce unreviewable commits | Split into sub-batches per module family (e.g. `skill/`, `compression/`, `memory/`), each its own commit |
| Doc-tests (`///`) are accidentally moved | Out of scope; migration targets only `#[test]`/`#[tokio::test]` functions inside `#[cfg(test)]` modules |

---

## Dependencies

- Rust toolchain (edition 2024, Rust 1.85+).
- `cargo`, `cargo clippy`, `cargo test` available in the build environment.
- No external crates required — migration is mechanical.