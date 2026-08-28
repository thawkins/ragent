# GUIDEANCEFIX — Codebase Conformance Plan

Audit of the ragent codebase against the guidelines in `AGENTS.md` and
`AGENTS-RUST.md`, with a prioritized task list to resolve conflicts.

**Audit date:** 2026-08-28
**Scope:** all 17 crates under `crates/`, root `src/`, root-level docs, `Cargo.toml`.
**Method:** `grep`/`codeindex` scans, `cargo fmt --check`, `cargo check`,
`cargo clippy --workspace --all-targets`, `cargo build`.

---

## Conformance summary

| Area | Status |
| --- | --- |
| `cargo fmt --check` | PASS — clean |
| `cargo check --workspace` | PASS |
| `cargo build` | PASS |
| `cargo clippy` (src-only, 6 lint-inheriting crates) | Clean except 1 pre-existing deprecation |
| `cargo clippy --all-targets` | **FAIL** — broken bench `gathering_bench.rs` blocks compilation |
| Inline `#[cfg(test)] mod` in `src/` | **111 violations** |
| `unsafe` in production | 1 real block (bash.rs killpg) |
| Workspace lints inheritance | 6 of 17 crates inherit |
| Root `Cargo.toml` | Has `[package]` + deps (violates "workspace manifest only") |
| Root-level docs outside approved exceptions | 8 files |
| Crate metadata (description/repo/keywords/categories) | 5 crates missing all |
| Editorial `((INCONSISTENCY n:))` markers in AGENTS-RUST.md | 4 unresolved |

Priorities use the AGENTS.md scale: **0** Critical, **1** High, **2** Medium,
**3** Low, **4** Backlog.

---

## P-0 — Critical (broken builds, data loss, security)

### T-001 — Fix broken benchmark `crates/ragent-research/benches/gathering_bench.rs`
- **Guideline:** AGENTS-RUST "Build Commands" / "Zero Warnings Policy"; any
  compile failure breaks `cargo clippy --all-targets` and
  `cargo test --workspace --all-targets`.
- **Finding:** The bench references `SessionConfig.topic`,
  `SessionConfig.max_web_results`, `SessionConfig.max_local_sources`, none of
  which exist. Confirmed pre-existing on `HEAD` (the same 3 errors appear with
  all working-tree changes stashed). Blocks `cargo clippy --all-targets` and
  any `--all-targets` CI gate.
- **Fix:** Update `gathering_bench.rs` to the current `SessionConfig` API
  (add the new fields / remove the stale ones), or delete the bench if the
  scenario it benchmarks no longer exists. Do not ship a workspace where
  `cargo clippy --all-targets` fails.
- **Verify:** `cargo clippy --workspace --all-targets` reaches "Finished"
  without a compile error.

### T-002 — Resolve the single `unsafe` block in production code
- **Guideline:** AGENTS.md General Preference #6 "No unsafe code";
  AGENTS-RUST Core Code Rules "No `unsafe` Blocks: Never use `unsafe` unless
  explicitly approved"; AGENTS-RUST Quality Checklist "No unnecessary `unsafe`".
- **Finding:** `crates/ragent-tools-core/src/bash.rs:1109` —
  `unsafe { libc::killpg(pgid, libc::SIGKILL) }` in `kill_process_group`, the
  production bash-timeout kill path. It is the only real `unsafe` block in the
  workspace (research crate's `#![deny(unsafe_code)]` is a deny directive, not
  a block). Critically, **`ragent-tools-core` does not inherit the workspace
  `unsafe_code = "deny"` lint**, so this is currently unenforced.
- **Decision required:** Per the "unless explicitly approved" clause, either
  (a) obtain explicit approval and document the safety invariant (the comment
  already documents it) plus add a `#[allow(unsafe_code)]`/`// SAFETY:` note,
  or (b) replace with a safe wrapper. `killpg` has no safe std alternative, so
  (a) is the pragmatic path. Record the approval in AGENTS-RUST.md.
- **Verify:** Documented decision; CI passes.

---

## P-1 — High (major guideline conflicts)

### T-003 — Migrate 111 inline `#[cfg(test)] mod tests` blocks out of `src/`
- **Guideline:** AGENTS-RUST "Test Organization" — "All tests **MUST** be
  located in the `tests/` directory inside each crate... Do **not** add new
  inline `#[cfg(test)]` modules to library source files."
- **Finding:** 111 `src/` files still contain inline test modules. The 3
  `#[path = "../tests/inline/..."]` files (`validate.rs`, `askpass.rs`,
  `bash.rs`) are the documented exception and are **excluded**. Breakdown by
  crate:
  - `ragent-research` 34, `ragent-agent` 27, `ragent-tools-extended` 14,
    `ragent-llm` 13, `ragent-tui` 7, `ragent-specs` 4, `ragent-types` 3,
    `ragent-bench` 3, `ragent-tools-vcs` 2, `ragent-tools-core` 2,
    `ragent-telemetry` 1, `ragent-config` 1.
- **Fix:** Use the documented migration strategies in AGENTS-RUST
  (public-API tests; private-item tests via `pub(crate)` + `#[path = "../src/..."]`;
  complex cases via `#[cfg(test)] #[path = "../../tests/..."] mod ...`).
  Track this as a phased migration per crate (research and agent first — they
  hold 61 of the 111). This is a large mechanical task; consider batching by
  crate and landing per-crate PRs.
- **Verify:** `grep -rl '#[cfg(test)]' crates/*/src/` returns only the 3
  `#[path = tests/inline]` files; `cargo test --workspace` green.

### T-004 — Make all 17 crates inherit the workspace lints (or document the divergence)
- **Guideline:** AGENTS-RUST Core Code Rules (no `unsafe`, no `unwrap` on
  user paths), "Zero Warnings Policy", Clippy Compliance. The root
  `Cargo.toml` defines `[workspace.lints]` with `unsafe_code = "deny"`,
  `missing_docs = "warn"` and an extensive clippy allow-list.
- **Finding:** Only 6 of 17 crates declare `[lints]` (agent, codeindex, server,
  team, telemetry, tui). The other 11 (`bench`, `config`, `llm`, `prompt_opt`,
  `research`, `specs`, `storage`, `tools-core`, `tools-extended`, `tools-vcs`,
  `types`) get **none** of the workspace lints, which is why `unsafe` in
  `bash.rs` and many clippy warnings in `ragent-research/src` go unchecked.
  The `missing_docs = "warn"` check reporting 0 warnings is misleading —
  it only covers the 6 inheriting crates.
- **Fix:** Add `[lints] workspace = true` to the 11 crates, then address the
  newly-surfaced warnings (many are already `allow`ed at workspace level). Do
  this crate-by-crate so each lands green. If any crate genuinely cannot meet
  a lint, add a per-crate override with a comment — do not silently skip.
- **Verify:** Every crate reports the same lint baseline; `cargo clippy
  --workspace` shows no new warnings.

### T-005 — Fix clippy warnings in `ragent-research/src`
- **Guideline:** AGENTS-RUST "Zero Warnings Policy", "Clippy Compliance".
- **Finding:** `cargo clippy` (with `--all-targets` once T-001 is fixed) reports
  in `ragent-research/src`: 29 `useless_conversion` (`body256("...").into()`
  in `session.rs` / `web_gatherer.rs`), 2 `manual_contains`, 2 `useless_format`,
  1 `useless_vec`. All are in test blocks inside `src/` (they will largely
  disappear as part of T-003 migration). 1 pre-existing `f64::NAN` deprecation
  in the workspace.
- **Fix:** As part of T-003, fix or remove the `.into()` conversions; fix the
  `manual_contains`/`format!`/`vec!` nits. The `f64::NAN` deprecation should
  become `f64::NAN` → `f64::NAN` associated const (single site).
- **Verify:** `cargo clippy --workspace --all-targets` produces 0 warnings
  (except `pdf-extract`/`attribute-derive-macro` third-party notes).

### T-006 — Remove production `unwrap()`/`expect()` on user-facing paths
- **Guideline:** AGENTS-RUST "No Silent Error Swallowing: Never use `.unwrap()`
  or `.expect()` in production code paths"; "No `.unwrap()` on user-facing
  paths" (AGENTS.md #7). Note: the workspace clippy config sets
  `unwrap_used = "allow"`, which conflicts with this rule — see T-009.
- **Finding:** Real production (non-test) `unwrap()`/`expect()` sites:
  - `crates/ragent-agent/src/file_ops/mod.rs:70,203` — `sem.acquire_owned().await.unwrap()`
  - `crates/ragent-agent/src/orchestrator/policy.rs:136,141` — `responses.last().unwrap()`
  - `crates/ragent-agent/src/session/stream_buffer.rs:40` — `Regex::new(...).expect(...)`
  - `crates/ragent-agent/src/task/mod.rs:86` — `...expect("UUID always has a first segment")`
  - `crates/ragent-agent/src/session/archive.rs:278,535` — `path.file_name().unwrap()`
  - `crates/ragent-tools-core/src/read.rs:45` — `NonZeroUsize::new(...).expect(...)`
- **Fix:** Replace with proper `Result`/`Option` handling: `let Some(x) = ... else { return Err(...) }`,
  `ok_or_else`, or `with_context`. For the `Regex::new` sites the pattern is
  static, so a lazily-initialized `OnceLock`/`LazyLock` returning a compiled
  regex (no Result at call site) is idiomatic. For `responses.last()` use
  `ok_or_else(|| anyhow!("no responses"))`.
- **Verify:** `grep -rnE '\.unwrap\(\)|\.expect\(' crates/*/src/ src/` outside
  test blocks returns only sites in test/bench code; `cargo test` green.

### T-007 — Add missing crate metadata to `Cargo.toml`s
- **Guideline:** AGENTS-RUST Project Organization — "Include comprehensive
  metadata: `description`, `license`, `repository`, `keywords`, `categories`."
- **Finding:** These crates have **none** of `description`/`repository`/
  `keywords`/`categories`: `ragent-agent`, `ragent-prompt_opt`, `ragent-server`,
  `ragent-team`, `ragent-tui`. The remaining crates lack `repository`/
  `keywords`/`categories` (most have `description` + `license`). Root
  `Cargo.toml` already defines the canonical `repository`/`license`/`authors` —
  consider inheriting them via `{ workspace = true }` for consistency.
- **Fix:** Add a one-line `description` to each crate, and set `repository`/
  `keywords`/`categories` either directly or via workspace inheritance
  (`repository.workspace = true`). Prefer workspace inheritance for
  `repository`/`license` to avoid drift.
- **Verify:** `for c in crates/*/Cargo.toml; do grep -c '^description' $c; done`
  all ≥ 1; `cargo metadata` shows the fields.

---

## P-2 — Medium (default, nice-to-have)

### T-008 — Resolve the `((INCONSISTENCY n:))` editorial markers in AGENTS-RUST.md
- **Guideline:** AGENTS.md is the working contract; unresolved review notes
  leave the rules ambiguous.
- **Finding:** Four editorial markers remain in `AGENTS-RUST.md`: INCONSISTENCY 4
  (wildcard imports), 6 (async-std claim), 7 (test-module wording), 9
  (duplicate sections). Each flags a self-conflict or stale claim:
  - **4 (wildcard imports):** The "No wildcard imports" rule conflicts with the
    idiomatic `use super::*;` inside test modules and `pub use foo::*` re-exports.
  - **6 (async-std):** Verified **false** — every `async-std` mention in the
    codebase is in comments/string literals/tests; there is no `async-std`
    dependency and no runtime usage. The note can be removed and the "tokio is
    the async runtime" claim left as-is.
  - **7 (test modules):** Already resolved — the "prefer tests/ directory"
    wording matches the Test Organization rule. Note can be removed.
  - **9 (duplicate sections):** "Idiomatic Rust Practices" duplicates "Code
    Style Guidelines". Add a pointer line ("this section supplements Code Style
    Guidelines; on conflict the earlier, more specific section wins") or
    consolidate.
- **Fix:** Edit `AGENTS-RUST.md` to remove the resolved markers and add the
  clarifying pointer for the wildcard-import rule (carve out `use super::*` in
  tests and `pub use` re-exports) and the duplicate-section precedence note.
- **Verify:** `grep -n INCONSISTENCY AGENTS-RUST.md` returns 0; rules are
  unambiguous.

### T-009 — Reconcile `unwrap_used = "allow"` with the "no unwrap" rule
- **Guideline:** AGENTS-RUST "No Silent Error Swallowing" vs the clippy config.
- **Finding:** The root `[workspace.lints.clippy]` sets `unwrap_used = "allow"`,
  which directly contradicts AGENTS-RUST's "Never use `.unwrap()` or
  `.expect()` in production code paths." The allow list was presumably added
  to silence noise, but it weakens enforcement of a stated core rule.
- **Decision required:** Either (a) change `unwrap_used` to `warn` and fix the
  T-006 sites, or (b) keep it `allow` and amend AGENTS-RUST.md to scope the
  "no unwrap" rule to user-facing paths only (matching AGENTS.md #7). Recommend
  (a) with `allow` reserved for test modules, to make the core rule enforceable.
- **Verify:** `cargo clippy` reflects the chosen policy; AGENTS-RUST.md and
  AGENTS.md agree on the scope.

### T-010 — Move root-level markdown into `docs/` (or grant explicit exemptions)
- **Guideline:** AGENTS.md Documentation Standards — docs go in `docs/` except
  `QUICKSTART.md`, `STATS.md`, `SPEC.md`, `AGENTS.md`, `README.md`, `PLAN.md`,
  `CHANGELOG.md`. "Existing root-level project documents that predate this
  convention may remain until they are explicitly reorganized."
- **Finding:** Root files not in the approved-exceptions list:
  `RELEASE.md`, `PIPELINEPLAN.md`, `RESEARCHPLAN.md`, `RESOURCEPLAN.md`,
  `SECPLAN.md`, `SIMPPLAN.md`, `TOOLS.md`, `TUI-QUICKSTART.md`,
  `AGENTS-RUST.md`. `README.md` line 12 references `TUI-QUICKSTART`.
- **Decision required:** These predate the convention (allowed to remain), but
  the guideline prefers migrating on touch. Options: (a) move each to `docs/`
  and update cross-references (e.g. README's `TUI-QUICKSTART` → `docs/TUI-QUICKSTART.md`,
  and AGENTS.md's `[AGENTS-RUST.md](AGENTS-RUST.md)` link), or (b) add an
  explicit exemption note to AGENTS.md listing the root files that stay.
  Recommend (a) for `TUI-QUICKSTART`/`TOOLS`/`RELEASE` and (b) for the
  `*PLAN`/`*PLAN` tracking docs if the team prefers them at root.
- **Verify:** `ls *.md` contains only approved exceptions (plus any
  documented exemptions); in-repo links resolve.

---

## P-3 — Low (polish)

### T-011 — Remove box-drawing / fancy unicode where it is not required for UI output
- **Guideline:** AGENTS-RUST "No Unicode/Emojis: Exclude emojis or fancy
  unicode symbols from comments and output code."
- **Finding:** 123 `src/` files use box-drawing characters (─, │, ├, └), and
  360 files use em-dashes (—). Some are in comments (section separators), some
  in **runtime string output** (e.g. `agent/mod.rs` agent-tree drawing,
  `list.rs`, `config.rs`, `sse.rs`, codeindex tools, TUI). The rule as written
  bans these everywhere, which conflicts with legitimate TUI/tree rendering.
- **Decision required:** Narrow the rule (recommended) to "no emojis and no
  non-ASCII in comments/identifiers; ASCII tree-drawing in user-facing terminal
  output is allowed where it improves readability", OR enforce ASCII-only
  everywhere (larger, lower-value churn). Recommend scoping the rule to
  comments and to be lenient on intentional TUI tree glyphs.
- **Verify:** AGENTS-RUST.md rule updated; no action on the rendering sites if
  scoped out; comment-only occurrences cleaned up.

### T-012 — Rename the 4 non-conforming test files
- **Guideline:** AGENTS-RUST Test Organization — "Follow the naming convention:
  `test_<component>_<scenario>`."
- **Finding:** In `tests/` roots: `crates/ragent-agent/tests/session_processor.rs`,
  `crates/ragent-prompt_opt/tests/basic.rs`,
  `crates/ragent-types/tests/structure_types.rs`,
  `crates/ragent-research/tests/source_vault.rs` lack the `test_` prefix.
- **Fix:** Rename to `test_session_processor.rs`, `test_prompt_opt_basic.rs`,
  `test_structure_types.rs`, `test_source_vault.rs` and update any
  `#[path]`/module references. (Only 4 files — 381/385 already conform.)
- **Verify:** `find crates -path '*/tests/*.rs' ! -name 'test_*.rs' ! -path
  '*/tests/inline/*' ! -path '*/tests/support/*'` returns empty.

### T-013 — Replace remaining production `println!`/`eprintln!` with `tracing`
- **Guideline:** AGENTS-RUST Code Style — "Use the `tracing` crate with
  structured logging; avoid `println!` or `eprintln!` in application code."
- **Finding:** Root `src/cli.rs` (15), `src/main.rs` (7), `src/panic_hook.rs`
  (3) use `println!`/`eprintln!`. These are user-facing CLI display paths (not
  internal logging), which is a legitimate exception, but they are not
  `tracing`.
- **Decision required:** These are direct command output the user expects on
  stdout/stderr (e.g. `ragent research` results, config dump, panic hook), so
  `tracing` would be wrong for most. Recommend amending the rule to exempt
  "direct CLI stdout/stderr presentation" (use `println!` there, `tracing`
  everywhere else) and convert only genuine diagnostic output to `tracing`.
- **Verify:** AGENTS-RUST.md rule scoped; no new `println!` in library code
  paths.

---

## P-4 — Backlog / informational

### T-014 — Migrate `mod.rs` module layout (optional)
- **Guideline:** AGENTS-RUST Project Organization — "reserve `mod.rs` for
  legacy or deeply nested modules"; prefer named files (`src/agent/custom.rs`
  via `mod custom;`).
- **Finding:** 45 non-test `mod.rs` files exist. This is a stylistic preference
  and a large mechanical rename with no functional benefit.
- **Action:** Backlog. Track separately; do not churn unless the team wants
  uniform layout.

### T-015 — Root `Cargo.toml` "workspace manifest only" — document the exception
- **Guideline:** AGENTS-RUST Workspace Layout — "Keep the root `Cargo.toml`
  configured as a workspace manifest only. Do not declare package metadata or
  dependencies in the root."
- **Finding:** The root `Cargo.toml` legitimately hosts the `ragent` binary
  package (`[package] name = "ragent"`, `[dependencies]`, `[features]`,
  `[profile.release]`, `[[bin]]`). This is a reasonable, common monorepo
  pattern (single binary + workspace), but it contradicts the guideline's
  literal wording.
- **Decision required:** Amend AGENTS-RUST.md to state that the root manifest
  may host the primary binary package alongside the `[workspace]` block
  (recommended), OR split the binary into a dedicated `crates/ragent-bin`
  crate (large move, no functional gain). Recommend documenting the exception.
- **Verify:** AGENTS-RUST.md updated; no code change.

---

## Suggested execution order

1. **T-001** (broken bench) — unblocks all `--all-targets` gates. → verify: `cargo clippy --workspace --all-targets` compiles.
2. **T-002** (unsafe) — security/compliance. → verify: documented + linted.
3. **T-008 + T-009 + T-015** (guideline edits) — do these first so the rules reflect the reality the code already meets; then the code tasks target the correct rule.
4. **T-004** (lint inheritance) — add `[lints]` crate-by-crate; then **T-005** (research clippy) and **T-006** (unwrap) surface and can be fixed together.
5. **T-003** (test migration) — largest task; phase by crate (research, then agent).
6. **T-007** (crate metadata), **T-010** (docs), **T-011/T-012/T-013** (polish).
7. **T-014** — backlog.

Each task is complete only when its **Verify** step passes. Do not mark done
on assertion alone — run the stated check (AGENTS.md General Preference #5).
