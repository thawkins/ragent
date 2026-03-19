# Code Improvements (Rust Best Practices)

This document captures a set of actionable improvements to bring the workspace closer to Rust best practices, improve correctness, and reduce technical debt.

---

## ✅ Goal

Reduce existing Clippy/formatting issues, improve error handling, and make the codebase easier to maintain and evolve.

---

## Milestone 1: Enforce linting & formatting (CI guardrails)

1. **Add a CI job that runs** `cargo fmt --all` and `cargo clippy --all-targets --all-features -- -D warnings`.
2. Ensure the repository has a stable Rust toolchain pinned (e.g., via `rust-toolchain.toml`) and update documentation accordingly.

---

## Milestone 2: Fix the current Clippy warnings

The workspace currently produces Clippy warnings treated as errors. Fixing these will restore clean CI and improve code quality.

### 2.1 `unnested_or_patterns`
- Convert pattern matches like `Some("docx") | Some("xlsx")` into nested or-patterns: `Some("docx" | "xlsx")`.
- Files affected (non-exhaustive):
  - `crates/ragent-core/src/reference/resolve.rs`
  - `crates/ragent-core/src/tool/office_common.rs`
  - `crates/ragent-core/src/tool/libreoffice_common.rs`
  - `crates/ragent-core/src/tool/libreoffice_info.rs`

### 2.2 `similar_names`
- Rename conflicting bindings to avoid `clippy::similar_names` warnings.
- Files affected (non-exhaustive):
  - `crates/ragent-core/src/skill/loader.rs`
  - `crates/ragent-core/src/tool/glob.rs`
  - `crates/ragent-core/src/tool/patch.rs`
  - `crates/ragent-core/src/tool/pdf_write.rs`
  - `crates/ragent-core/src/tool/plan.rs`

### 2.3 `redundant_else`
- Remove redundant `else` blocks and simplify control flow. (Example in `crates/ragent-core/src/orchestrator/mod.rs`.)

### 2.4 `unused_imports`
- Remove unused imports that cause Clippy failures.
- Files affected (non-exhaustive):
  - `crates/ragent-core/src/file_ops/mod.rs`
  - `crates/ragent-core/src/orchestrator/mod.rs`

---

## Milestone 3: Improve error handling (reduce `unwrap` / `expect` usage)

### 3.1 Replace `unwrap()`/`expect()` in library/business code
- Replace `unwrap()`/`expect()` usage in non-test code with explicit error propagation (e.g., `?` and custom error types).
- Files with notable `unwrap` usage (non-test):
  - `crates/ragent-core/src/file_ops/mod.rs`
  - `crates/ragent-core/src/orchestrator/mod.rs`
  - `crates/ragent-core/src/task/mod.rs`
  - `crates/ragent-core/src/tool/edit.rs`
  - `crates/ragent-core/src/reference/resolve.rs`
  - `crates/ragent-core/src/provider/copilot.rs`

### 3.2 Validate `Option` handling in UI code
- Avoid repeated `.as_ref().unwrap()` and `.clone().unwrap()` in the TUI (`crates/ragent-tui/src`). Replace with safe pattern matching or `if let Some(...)` guards.

---

## Milestone 4: Structural and readability improvements (✅ Implemented)

### 4.1 Refactor large modules
- ✅ Split `crates/ragent-core/src/orchestrator/mod.rs` into smaller submodules:
  - `orchestrator/registry.rs`
  - `orchestrator/router.rs`
  - `orchestrator/coordinator.rs`
  - `orchestrator/mod.rs` now re-exports the public API for backwards compatibility.
- (Still pending: further refactor of `crates/ragent-tui/src/app.rs` into smaller pieces, if desired.)

### 4.2 Document public APIs and tighten visibility
- Added doc comments on newly extracted orchestrator types and re-exports.
- Public API shape remains stable and uses explicit re-exports to avoid breaking consumers.

---

## Milestone 5: Add/expand automated tests

### 5.1 Coverage gaps
- Identify uncovered components (e.g., `tool` submodules, LSP integration logic, provider implementations) and add unit/integration tests.
- ✅ Added regression test for `try_read_binary` via `resolve_all_refs` + `.docx` fixture.
- ✅ Added regression test for `ConflictResolver::Consensus` prefix handling (first 64 characters).
- ✅ Added unit tests for `tool::office_common` helpers (`detect_format`, `resolve_path`, `truncate_output`).

### 5.2 Regression tests for edge cases (✅ Completed)
- ✅ `try_read_binary` fallback logic (binary vs text file handling) — implemented for `.docx`.
- ✅ Policy resolution edge cases (e.g., no matches, partial failures) — existing tests cover many cases; new prefix-based consensus test added.
- ✅ Error propagation paths throughout orchestrator and provider layers — added tests for router failures, coordinator responses when all agents fail, and session processor/provider errors.

---

## Communication & Execution

- Break work into small PRs grouped by milestone.
- For each PR, include a short “what changed / why” summary and reference any corresponding Clippy lint or coverage gap.

---

> ⚠️ Note: This file is a living checklist; as improvements are made, mark tasks complete or expand the list with new findings.
