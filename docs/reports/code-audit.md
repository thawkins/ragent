# Code Audit Report

**Project**: Rust Cargo workspace (edition 2024, 17 crates, ratatui TUI + axum server; per-crate `tests/` dirs; AGENTS.md standards: cargo fmt/clippy enforced, no `unwrap()` in production, tests externalised)
**Scope**: `crates/ragent-prompt_opt` (src/lib.rs 552 lines, 2 test files, README.md, DOCS.md, Cargo.toml)
**Findings**: 10 total (0 high, 4 medium, 6 low)

---

## Medium Priority

### Standards
- Stale/contradictory README: describes a non-existent `TemplateOptimizer` implementing an `Optimizer` trait and claims the crate "do[es] not call external APIs", contradicting the actual `Completer`-based LLM-call design — `crates/ragent-prompt_opt/README.md:5`
- `FromStr` impl uses unit-type error `Err = ()` instead of a meaningful error type (project standard: `thiserror`/`anyhow` with descriptive errors); callers already use `.ok()` so the fix is non-breaking — `crates/ragent-prompt_opt/src/lib.rs:57`

### Testing
- Duplicate test: `test_system_prompt_non_empty` in both test files with identical logic — `crates/ragent-prompt_opt/tests/basic.rs:31` and `crates/ragent-prompt_opt/tests/test_prompt_opt_api.rs:34`
- Missing error-path coverage: no test that a failing `Completer` propagates its error through `optimize()` (the crate's only async entry point) — `crates/ragent-prompt_opt/src/lib.rs:172-179`

## Low Priority

### Duplication
- Duplicated test setup: `MockCompleter` defined separately in both test files with near-identical bodies — `crates/ragent-prompt_opt/tests/basic.rs:6` and `crates/ragent-prompt_opt/tests/test_prompt_opt_api.rs:14`

### Standards
- Version drift: crate hardcodes `version = "0.1.0"` while 6 sibling crates inherit `version.workspace = true` (workspace at 1.0.92); 9 crates hardcode 0.1.0, so alignment is a workspace-wide decision — `crates/ragent-prompt_opt/Cargo.toml:3` and `Cargo.toml:6`
- Doc-name mismatch: enum doc says RISE = "Recursive Introspection for iterative self-improvement" while the meta-prompt defines RISE = "Recursive Introspection for Self-improvement and Evaluation" — `crates/ragent-prompt_opt/src/lib.rs:39` and `crates/ragent-prompt_opt/src/lib.rs:293`

### Testing
- No test for input trimming behaviour: `optimize()` calls `input.trim()` (whitespace-only input forwards an empty string) — `crates/ragent-prompt_opt/src/lib.rs:178`

### Dependencies
- `cargo audit`: 0 vulnerabilities for prompt_opt deps (anyhow, async-trait, tokio); 6 unmaintained transitive crates (async-std, instant, number_prefix, paste, proc-macro-error, ttf-parser) + unsound `lru` advisories exist in the workspace lockfile but are not prompt_opt dependencies — `Cargo.lock`
- `cargo outdated` cannot resolve: `ratatui 0.29.0` pins `unicode-width =0.2.0` which conflicts with `ragent-tui`'s `unicode-width ^0.2.2`; freshness check blocked by this workspace version conflict (tooling note, not a crate defect) — `Cargo.lock` (ratatui/unicode-width)

### Logging
- No issues found. (No `println!`/`eprintln!`/`dbg!`/tracing calls in src; `.unwrap()` appears only in tests, which is permitted.)

### Security
- No issues found. (No hardcoded secrets, no ad-hoc env access, no credential files under the crate path; Cargo.lock is committed.)
# Implementation Plan

## Quick Wins (< 30 min each)
| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | Stale README describing non-existent `TemplateOptimizer`/`Optimizer` trait | `crates/ragent-prompt_opt/README.md` | Rewrite README to describe `Completer` trait, `OptMethod`, `system_prompt()`, `optimize()`; remove "does not call external APIs" claim |
| 2 | Duplicate test `test_system_prompt_non_empty` | `crates/ragent-prompt_opt/tests/basic.rs:31`, `tests/test_prompt_opt_api.rs:34` | Keep the copy in `test_prompt_opt_api.rs` (api suite), delete from `basic.rs` |
| 3 | Duplicated `MockCompleter` test helper | `crates/ragent-prompt_opt/tests/basic.rs:6`, `tests/test_prompt_opt_api.rs:14` | Extract to `tests/common/mod.rs` (`pub mod common;` in each test file) |
| 4 | RISE doc-name mismatch | `crates/ragent-prompt_opt/src/lib.rs:39` | Align doc comment with meta-prompt wording: "Recursive Introspection for Self-improvement and Evaluation" |
| 5 | Unit-type `FromStr::Err = ()` | `crates/ragent-prompt_opt/src/lib.rs:57` | Replace with `type Err = String` returning `format!("unknown optimization method: {s}")`; callers use `.ok()` so no breakage |

## Medium Effort (30 min - 2 hours each)
| # | Finding | File(s) | Fix |
|---|---------|---------|-----|
| 1 | No error-path test for failing `Completer` | `crates/ragent-prompt_opt/tests/` (new test in `test_prompt_opt_api.rs`) | Add `FailingCompleter` returning `Err(anyhow!("boom"))`; assert `optimize()` propagates the error via `is_err()` + message match |
| 2 | Version drift (0.1.0 vs workspace 1.0.92) | `crates/ragent-prompt_opt/Cargo.toml:3` (+ 8 sibling crates) | Switch to `version.workspace = true` — workspace-wide consistency decision; touches 9 crates and all `Cargo.toml` versions at once |

## Complex (> 2 hours)
(none — no architectural or cross-cutting findings)

## Suggested Fix Order
1. **Documentation (Quick Win 1, 4)**: README rewrite + doc-comment alignment — zero risk, removes contradictions before code changes
2. **Test dedup (Quick Win 2, 3)**: extract shared `MockCompleter`, drop duplicate test — do before adding new tests so the shared helper is in place
3. **Error-path tests (Medium 1)**: add failing-completer coverage using the now-shared helper
4. **API polish (Quick Win 5)**: `FromStr` error type change, last, since it touches the public API surface
5. **Version alignment (Medium 2)**: separate concern; needs a user decision because it spans 9 crates outside the audit scope

## Verification
- [ ] `cargo check -p ragent-prompt_opt` reports 0 errors
- [ ] `cargo test -p ragent-prompt_opt` passes (all suites)
- [ ] `cargo clippy -p ragent-prompt_opt` clean
- [ ] `cargo fmt -p ragent-prompt_opt --check` clean
- [ ] New failing-completer test covers the untested error path
- [ ] README and lib.rs doc comments describe the same API