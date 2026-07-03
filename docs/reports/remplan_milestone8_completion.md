# REMPLAN.md Milestone 8 — Migrate inline tests to `tests/` — Completion Report

**Date:** 2025-01-17  
**Status:** ✅ COMPLETE (T8.1–T8.5 all landed)

## Summary

Milestone 8 migrated the highest-count inline `#[cfg(test)] mod tests` blocks
from `src/` source files into external `tests/inline/` directories, following
AGENTS.md §"Test Organization". The `#[cfg(test)] #[path]` re-import pattern
was used: the test file is compiled as a submodule of the source module, so
`use super::*` resolves to the source module and private helpers remain
accessible without visibility widening.

**Before:** 118 files with `#[cfg(test)]` in `src/`, 114 with `mod tests`.  
**After:** 118 files with `#[cfg(test)]` in `src/`, 109 with `mod tests`  
(9 highest-count blocks migrated, 373 tests moved).  
**Baseline guard:** `scripts/check-inline-tests.sh` set at 109 to prevent
regression.

## Key discovery: `tests/inline/` subdirectory pattern

Test files included via `#[cfg(test)] #[path]` must NOT be placed directly
in `tests/` because Cargo auto-compiles every `tests/*.rs` as an integration
test binary — where `use super::*` and `crate::` paths don't resolve. The
solution: place the `#[path]`-included files in `tests/inline/` (a
subdirectory). Cargo does not auto-discover files in subdirectories, so they
are only compiled when pulled in by the `#[path]` attribute in the lib test
build.

## Tasks

### T8.1 — `ragent-llm` router tests (124 tests) ✅
- Moved 72 tests from `providers/router_classifier.rs` (2 blocks) into
  `tests/inline/router_classifier.rs` + `tests/inline/router_classifier_extended.rs`.
- Moved 52 tests from `providers/router_modifiers.rs` (2 blocks) into
  `tests/inline/router_modifiers.rs`.
- Source files reduced: `router_classifier.rs` 1969→958 lines,
  `router_modifiers.rs` 606→113 lines.
- `cargo test -p ragent-llm --lib`: 263 passed (unchanged).

### T8.2 — `ragent-agent` compression pipeline tests (41 tests) ✅
- Moved 41 tests from `compression/pipeline.rs` into
  `tests/inline/compression_pipeline.rs`.
- Source file reduced: `pipeline.rs` 1704→1139 lines.
- `cargo test -p ragent-agent --lib`: 254 passed (unchanged).

### T8.3 — `ragent-tools-core` bash tests (34 tests) ✅
- Moved 34 tests from `src/bash.rs` into `tests/inline/bash.rs`.
- Source file reduced: `bash.rs` 1537→1241 lines.
- `cargo test -p ragent-tools-core --lib`: 34 passed (unchanged).

### T8.4 — Top 9 remaining inline test blocks (174 tests) ✅

| Source file | Tests | Destination |
|-------------|-------|-------------|
| `ragent-agent/src/skill/context.rs` | 27 | `tests/inline/skill_context.rs` |
| `ragent-agent/src/skill/loader.rs` | 26 | `tests/inline/skill_loader.rs` |
| `ragent-agent/src/skill/args.rs` | 24 | `tests/inline/skill_args.rs` |
| `ragent-specs/src/validate.rs` | 22 | `tests/inline/validate.rs` |
| `ragent-llm/src/providers/huggingface.rs` | 21 | `tests/inline/huggingface.rs` |
| `ragent-codeindex/src/parser/rust.rs` | 22 | `tests/inline/codeindex_rust_parser.rs` |
| `ragent-llm/src/providers/xai.rs` | 18 | `tests/inline/xai.rs` |
| `ragent-agent/src/skill/mod.rs` | 18 | `tests/inline/skill_mod.rs` |
| `ragent-agent/src/reference/parse.rs` | 18 | `tests/inline/reference_parse.rs` |

Note: the `tui/app.rs` (21 tests) target was already handled during M5
(tests moved to `app/tests.rs`).

### T8.5 — CI guard script ✅
- Added `scripts/check-inline-tests.sh` that fails if the count of files
  with `mod tests` in `crates/*/src/` exceeds the baseline (109).
- Wired into `pre-flight.sh` as a pre-build check.

## Verification

| Check | Result |
|-------|--------|
| `cargo check --workspace` | ✅ |
| `cargo build --workspace --tests` | ✅ |
| `cargo test -p ragent-agent --lib` | ✅ 254 passed |
| `cargo test -p ragent-llm --lib` | ✅ 263 passed |
| `cargo test -p ragent-tools-core --lib` | ✅ 34 passed |
| `cargo test -p ragent-specs` | ✅ 4 doctests passed |
| `cargo test -p ragent-codeindex --lib` | ✅ 178 passed |
| `scripts/check-inline-tests.sh` | ✅ (109 ≤ baseline 109) |

## Migration strategy

All migrations used the `#[cfg(test)] #[path]` re-import pattern:

```rust
// In the source file (e.g., src/providers/router_classifier.rs):
#[cfg(test)]
#[path = "../../tests/inline/router_classifier.rs"]
mod router_classifier_tests;
```

The test file in `tests/inline/` contains the body of the former
`mod tests { ... }` block (without the `mod tests {` wrapper and closing `}`).
`use super::*` inside the test file resolves to the source module (the parent
of the `#[path]`-declared submodule), so private helpers are accessible
without `pub(crate)` widening.

**Critical:** files must be in `tests/inline/` (a subdirectory), NOT directly
in `tests/`, to avoid Cargo auto-compiling them as standalone integration
tests where `super::*` doesn't resolve.