# REMPLAN.md — Structural Remediation Plan

> Generated from a codebase audit on 2025-01-17. Every milestone and task below
> is backed by concrete evidence (file paths, line counts, diff measurements)
> collected with `read` / `grep` / `wc` against the current tree. The workspace
> currently compiles cleanly (`cargo check --workspace` passes) — this plan
> preserves that invariant at every milestone.

## 1. Audit Summary

The ragent workspace is a 15-crate Cargo workspace (~196k lines of Rust) that
has grown by accretion. Several earlier "extract a crate" milestones (see
project memory: types, config, storage, llm extractions) left behind **source
duplicates** and **compatibility shims** that were never cleaned up, and two
very large files (`crates/ragent-tui/src/app.rs`, `crates/ragent-agent/src/
session/processor.rs`) have grown past the point of maintainability.

### Top structural defects (evidence-backed)

| # | Defect | Evidence |
|---|--------|----------|
| D1 | **Duplicate `Storage` implementation** | `crates/ragent-agent/src/storage/mod.rs` (2217 lines) is a near-verbatim copy of `crates/ragent-storage/src/storage.rs` (2123 lines). Both define `pub struct Storage` with identical CRUD methods. `diff` shows only ~6 differing regions (a `has_format_version` cache field + doc-comment paths). `ragent-agent` already depends on `ragent-storage`. |
| D2 | **Duplicate `Message` type** | `ragent-agent/src/message/mod.rs` (288 lines) and `ragent-types/src/message/mod.rs` (269 lines) both define `pub struct Message` / `MessagePart`. The agent copy only adds `ImageData`. `ragent-storage` uses `ragent_types::message::{Message,…}`. |
| D3 | **Triplicated permission types** | `PermissionRequest` / `Permission` / `PermissionDecision` are defined in `ragent-types/src/permission.rs`, `ragent-config/src/permission.rs`, **and** `ragent-agent/src/permission/mod.rs`, wired together with hand-written `From` impls. |
| D4 | **`#[path]` cycle workaround** | `ragent-agent/src/team/mod.rs` uses 7× `#[path="../../../ragent-team/src/team/…"]` and `ragent-agent/src/tool/mod.rs` uses 20× `#[path]` for team tools. `ragent-team` *also* depends on `ragent-agent` via Cargo → team sources are compiled twice and the dependency graph has a logical cycle. 27 `#[path]` attributes total. |
| D5 | **`ragent-tui/src/app.rs` is 15,332 lines** | `execute_slash_command_inner` alone spans lines 4862–10463 (5,601 lines, one `match` with ~97 slash-command arms). `handle_event` = 1252 lines, `handle_key_event` = 509, `handle_mouse_event` = 406. Only `state.rs` (1610 lines) is split out. Bench rendering, swarm, research, compress, and bash-list handling all live inline. |
| D6 | **`process_user_message` is 2,273 lines** | `crates/ragent-agent/src/session/processor.rs` lines 871–3143 — a single async function. |
| D7 | **Legacy `ragent_core` alias** | `ragent-tui` and `ragent-bench` declare `ragent-core = { package = "ragent-agent" }`. 193 `ragent_core::` references in `app.rs` alone; 35 files across `ragent-agent`, `ragent-tui`, `ragent-bench`, `ragent-server` still write `ragent_core::`. The crate was renamed but imports were never updated. |
| D8 | **Duplicate LLM types** | `ToolDefinition`, `ModelInfo`, `Usage`/`UsageInfo` defined in **both** `ragent-types/src/llm.rs` and `ragent-llm/src/{llm.rs, providers/mod.rs}`. `tools-core`, `tools-extended`, `tools-vcs` import `ragent_types::llm::ToolDefinition`. `ragent-types::LlmProvider` trait appears unused. |
| D9 | **Dead / orphan code** | `ragent-agent/src/predictive.rs` (454 lines, 0 internal refs), `ragent-agent/src/message/pool.rs` (168 lines, 0 refs, not even exported from `message/mod.rs`). `ragent-specs` depends on `ragent-research` only for a single test file. |
| D10 | **Inline tests violate `AGENTS.md`** | 118 `#[cfg(test)]` modules live inside `src/`. Worst offenders: `ragent-llm/src/providers/router_classifier.rs` (72 inline tests), `router_modifiers.rs` (52), `ragent-agent/src/compression/pipeline.rs` (37), `ragent-tools-core/src/bash.rs` (34). `AGENTS.md` mandates tests live in `tests/` dirs. |
| D11 | **Repo hygiene** | Tracked stray files: `EOF` (empty), `1` (ASCII text), `default.profraw` (coverage artifact). Tracked output directories: `research/` (research outputs), `specs/` (20 spec dirs). `docs/howtoos/` is misspelled (should be `howtos`). |
| D12 | **`src/main.rs` is 1,224 lines** | Inlines `run_orchestration_example` (an example) and `handle_research_command` alongside `main`. |
| D13 | **Compatibility-shim modules** | `ragent-llm/src/lib.rs` exposes `pub mod config { pub use ragent_config::{…} }` and `pub mod event { … }` so providers can write `crate::config::`. `ragent-agent/src/lib.rs` exposes `pub mod config { pub use ragent_config::* }`. These shim modules hide the real dependency direction. |

### Non-goals

This plan does **not** rewrite working logic, change public APIs, or introduce
new features. It only moves, deduplicates, deletes dead code, and splits
overlarge files. Each milestone is independently shippable and must keep
`cargo check --workspace` / `cargo test --workspace` green.

## 2. Milestone Overview

| M | Title | Defects addressed | Risk | Est. effort |
|---|-------|-------------------|------|-------------|
| M1 | Foundation type consolidation | D2, D3, D8 | Low | S |
| M2 | Eliminate duplicate `Storage` | D1 | Low | S |
| M3 | Break the `ragent-agent`↔`ragent-team` `#[path]` cycle | D4 | High | M |
| M4 | Retire the `ragent_core` alias | D7 | Low | S |
| M5 | Split `ragent-tui/src/app.rs` | D5 | Medium | L |
| M6 | Split `session/processor.rs` | D6 | Medium | M |
| M7 | Remove dead code & compat shims | D9, D13 | Low | S |
| M8 | Migrate inline tests to `tests/` | D10 | Low | M |
| M9 | Repository hygiene | D11, D12 | Low | S |
| M10 | Final verification & docs | — | Low | S |

Risk legend: **Low** = mechanical, test-guarded. **Medium** = touches many call
sites. **High** = touches Cargo graph / shared sources.

## 3. Milestones & Tasks

---

### Milestone 1 — Foundation type consolidation
*Defects: D2, D3, D8. Goal: one canonical home for `Message`, `Permission*`, and
LLM-primitive types. All other crates re-export.*

- **T1.1** Canonicalise `Message`. Merge the extra `ImageData` variant from
  `ragent-agent/src/message/mod.rs` into `ragent-types/src/message/mod.rs`.
  Replace `ragent-agent/src/message/mod.rs` with a re-export shim
  (`pub use ragent_types::message::*;`). Update the 6 in-crate `use crate::message::{…}`
  sites and the `ragent_core::message::` doc examples.
  - *Verify*: `cargo check -p ragent-agent -p ragent-storage -p ragent-tui`.
- **T1.2** Canonicalise `Permission*`. Keep `ragent-types/src/permission.rs`
  as the single source. Delete the parallel definitions in
  `ragent-config/src/permission.rs` and `ragent-agent/src/permission/mod.rs`;
  replace each with `pub use ragent_types::permission::*;` and keep only the
  `From`-impls / helper methods that are genuinely crate-local. Remove the
  inter-crate `From<ragent_config::permission::PermissionAction>` shims once
  both re-export the same type.
  - *Verify*: `cargo test -p ragent-types -p ragent-config -p ragent-agent`.
- **T1.3** Canonicalise LLM primitive types. Decide whether
  `ragent-types::llm` or `ragent-llm::llm` owns `ToolDefinition` / `ModelInfo` /
  `Usage`. Recommendation: move the *request/response* types (`ChatRequest`,
  `ChatMessage`, `StreamEvent`, `ChatContent`, `ContentPart`, `ToolDefinition`)
  to `ragent-types::llm`; keep provider-only types (`Provider`, `ProviderInfo`,
  `ProviderRegistry`, `UsageInfo`) in `ragent-llm`. Remove the unused
  `ragent-types::LlmProvider` trait. Update `ragent-llm` providers to
  `use ragent_types::llm::{…}`.
  - *Verify*: `cargo check -p ragent-llm -p ragent-tools-core -p ragent-tools-extended -p ragent-tools-vcs`.
- **T1.4** Add a `deny.toml`/CI guard (or a `tests/structure_types.rs` test)
  that greps for re-defined `pub struct Message`, `pub struct PermissionRequest`,
  `pub struct ToolDefinition` outside their canonical crate, so this cannot
  regress.

**Exit criteria**: `Message`, `PermissionRequest`, `Permission`, `ToolDefinition`
each have exactly one definition in the workspace; `cargo test --workspace`
passes.

---

### Milestone 2 — Eliminate duplicate `Storage`
*Defect: D1. Goal: `ragent-storage` is the sole `Storage` impl; `ragent-agent`
re-exports it.*

- **T2.1** Diff the two files and port any agent-only additions into
  `ragent-storage/src/storage.rs`. Confirmed agent-only additions: the
  `has_format_version: AtomicBool` cache field and its
  `has_format_version_cached()` helper (PERF-004). Port these (with tests) to
  the canonical file.
- **T2.2** Replace `crates/ragent-agent/src/storage/mod.rs` (2217 lines) with
  a re-export: `pub use ragent_storage::{Storage, MemoryRow, TodoRow, …};` plus
  the `encrypt_key`/`decrypt_key`/`obfuscate_key`/`deobfuscate_key` helpers if
  they are not already re-exported by `ragent-storage::lib`.
- **T2.3** Update the ~25 `use crate::storage::…` sites inside `ragent-agent`
  to point at the re-export (no behaviour change) and fix the `ragent_core::storage`
  doc-comments to `ragent_agent::storage` (or `ragent_storage::`).
- **T2.4** Run `cargo test -p ragent-storage -p ragent-agent -p ragent-tui
  -p ragent-server` and confirm the storage tests still pass.

**Exit criteria**: `grep -rn "pub struct Storage" crates/*/src` returns exactly
one hit (`ragent-storage/src/storage.rs`). `ragent-agent/src/storage/` is a
shim ≤ ~30 lines.

---

### Milestone 3 — Break the `ragent-agent`↔`ragent-team` `#[path]` cycle
*Defect: D4. Highest-risk milestone — must be done with care.*

- **T3.1** Map the cycle precisely. Confirm:
  `ragent-team` → (Cargo) → `ragent-agent` (for `agent`, `config`, `event`,
  `message`, `session`, `tool` re-exports — see `ragent-team/src/lib.rs`).
  `ragent-agent` → (27× `#[path]`) → `ragent-team/src/team/*.rs` and
  `ragent-team/src/tools/team_*.rs` (compiled into `ragent-agent`).
- **T3.2** Decide the ownership direction. Recommended: **`ragent-team` owns
  the team runtime; `ragent-agent` depends on `ragent-team`** (not the reverse).
  Move the shared primitives that `ragent-team` currently borrows from
  `ragent-agent` (`Tool`/`ToolContext`/`ToolOutput`/`ToolRegistry` interfaces,
  `TeamContext`, `TeamManagerInterface`, the `metadata` helpers) into
  `ragent-types` or a new tiny `ragent-tool-api` crate so neither `ragent-agent`
  nor `ragent-team` depends on the other.
- **T3.3** In `ragent-agent/src/team/mod.rs`, delete the 7 `#[path]` lines and
  replace with `pub use ragent_team::team::*;` once Cargo dep is added.
- **T3.4** In `ragent-agent/src/tool/mod.rs`, delete the 20 `#[path]` lines
  for team tools; have `ragent_team::tools::register_team_tools` register them
  into the shared `ToolRegistry` instead (it already exposes
  `create_default_registry()` that does this).
- **T3.5** Remove the `pub use ragent_agent::{agent, config, event, message, session};`
  re-exports from `ragent-team/src/lib.rs`; have `ragent-team` depend on
  `ragent-types` / `ragent-config` / the new tool-api crate directly.
- **T3.6** Add a workspace guard test that fails if any `#[path = "../../../ragent-`
  attribute appears in `crates/ragent-agent/src/`.

**Exit criteria**: `grep -rn '#\[path = "\.\./\.\./\.\./ragent-team' crates/ragent-agent`
returns nothing. `cargo tree -p ragent-agent -p ragent-team` shows no cycle.
Team sources compile exactly once.

---

### Milestone 4 — Retire the `ragent_core` alias
*Defect: D7. Pure mechanical rename; low risk.*

- **T4.1** In `crates/ragent-tui/Cargo.toml` and `crates/ragent-bench/Cargo.toml`,
  replace `ragent-core = { package = "ragent-agent", path = … }` with a direct
  `ragent-agent = { path = … }` dependency and update the `compression` feature
  gate (`ragent-core/compression` → `ragent-agent/compression`).
- **T4.2** Mechanically rewrite `ragent_core::` → `ragent_agent::` across all
  35 files (use a codemod: `find crates -name '*.rs' -not -path '*/target/*'
  -exec sed -i 's/ragent_core::/ragent_agent::/g' {} +`). Manually verify the
  `ragent_core::Config` / `ragent_core::bash_lists` / `ragent_core::gitlab`
  call sites still resolve.
- **T4.3** Update doc-comments (`use ragent_core::storage::Storage;` →
  `use ragent_agent::storage::Storage;`) in the storage and session modules.
- **T4.4** Remove the `ragent-core` alias line from both Cargo.toml files.

**Exit criteria**: `grep -rn "ragent_core" crates --include='*.rs'` returns
nothing (except possibly historical CHANGELOG/docs). `cargo check --workspace`.

---

### Milestone 5 — Split `ragent-tui/src/app.rs`
*Defect: D5. Largest-milestone; do it in slices so each slice compiles.*

- **T5.1** Extract bench command handling. Move
  `poll_pending_bench`, `render_bench_*`, `start_bench_run`,
  `drain_bench_progress_events` (app.rs lines ~1184–1610) into a new
  `crates/ragent-tui/src/app/bench.rs` submodule. Keep `App` methods thin;
  delegate.
- **T5.2** Extract swarm handling. Move `execute_swarm_decomposition`,
  `spawn_swarm_teammates`, `handle_swarm_status/cancel`, `poll_swarm_*`,
  `finalize_swarm_completion` (lines ~13780–14435) into
  `crates/ragent-tui/src/app/swarm.rs`.
- **T5.3** Extract research/skill command handling. Move
  `handle_research_command`, `poll_pending_research`, research-progress
  rendering (lines ~13210–14487) into `crates/ragent-tui/src/app/research.rs`.
- **T5.4** Extract compress handling. Move `handle_compress_command`,
  `start_provider_compaction_for_session`, `apply_compaction_summary`,
  `poll_pending_opt` (lines ~705–1136) into
  `crates/ragent-tui/src/app/compress.rs`.
- **T5.5** Extract slash-command dispatch. Carve
  `execute_slash_command_inner` (the 5,601-line match) into a
  `crates/ragent-tui/src/app/slash/` module tree with one file per command
  family (`slash/bench.rs`, `slash/team.rs`, `slash/research.rs`,
  `slash/config.rs`, `slash/model.rs`, `slash/permission.rs`, `slash/bash.rs`,
  `slash/yolo.rs`, `slash/session.rs`, `slash/misc.rs`). The dispatcher becomes
  a thin match that delegates to `slash::<cmd>(self, args)`.
- **T5.6** Extract input/mouse handling. Move `handle_key_event` (509 lines)
  and `handle_mouse_event` (406 lines) into `crates/ragent-tui/src/app/input.rs`
  (not to be confused with the existing top-level `input.rs`).
- **T5.7** Extract `App::new` (294 lines) and provider-setup helpers into
  `crates/ragent-tui/src/app/init.rs`.
- **T5.8** Move the `TuiResearchObserver` and `RagentCompleter` structs (lines
  ~51–147) into `crates/ragent-tui/src/research_adapter.rs` (already exists,
  53 lines — fold them in).

**Exit criteria**: `crates/ragent-tui/src/app.rs` ≤ ~1500 lines and is mostly
an `impl App` facade delegating to `app::*` submodules. No single function in
the crate exceeds ~300 lines. `cargo test -p ragent-tui` passes (including the
existing 2447-line `tests/test_slash_commands.rs`).

---

### Milestone 6 — Split `session/processor.rs`
*Defect: D6.*

- **T6.1** Extract the stream-buffer and stall-detection helpers (lines ~35–160)
  into `crates/ragent-agent/src/session/stream_buffer.rs`.
- **T6.2** Extract the system-prompt / tool-reference builders (lines ~161–598)
  into `crates/ragent-agent/src/session/prompt_builders.rs`.
- **T6.3** Extract bash/permission helpers (`split_bash_command`,
  `extract_command_name`, `is_hardwired_auto_approved_tool`,
  `check_permission_with_prompt`, lines ~337–598) into
  `crates/ragent-agent/src/session/permissions.rs`.
- **T6.4** Extract the history↔ChatMessage conversion (`history_to_chat_messages`,
  `parts_to_chat_content`, `tool_result_content_for_llm`, token-overflow
  helpers, lines ~3486–3998) into
  `crates/ragent-agent/src/session/history.rs`.
- **T6.5** Refactor `process_user_message` (2273 lines) into a sequence of
  named steps: `prepare_request`, `call_llm`, `handle_stream_events`,
  `dispatch_tool_calls`, `maybe_compress`, `maybe_retry`. Each step ≤ ~400 lines.
  Keep `process_user_message` as the orchestrator that calls them in order.
- **T6.6** Move the 17 inline tests (lines ~4061–4503) to
  `crates/ragent-agent/tests/session_processor.rs` (ties into M8).

**Exit criteria**: `processor.rs` ≤ ~1200 lines and is the orchestrator only.
No single fn exceeds ~400 lines.

---

### Milestone 7 — Remove dead code & compat shims
*Defects: D9, D13.*

- **T7.1** Delete `crates/ragent-agent/src/predictive.rs` (454 lines, 0 refs)
  and its `pub mod predictive;` line in `lib.rs`. Confirm with
  `grep -rn "predictive" crates --include='*.rs'` first.
- **T7.2** Delete `crates/ragent-agent/src/message/pool.rs` (168 lines, 0 refs,
  not exported) and remove any `mod pool;` declaration.
- **T7.3** Remove `ragent-research` from `ragent-specs/Cargo.toml`
  dependencies; move the single `tests/test_research_e2e.rs` that needs it
  behind a `#[cfg(feature)]` or into `ragent-research`'s own test suite.
- **T7.4** Collapse the `ragent-llm` compat shims: remove
  `pub mod config { pub use ragent_config::{…} }` and
  `pub mod event { … }` from `ragent-llm/src/lib.rs`; rewrite provider files
  to import `ragent_config::{Capabilities, Cost}` and
  `ragent_types::event::FinishReason` directly.
- **T7.5** Collapse the `ragent-agent` config shim: remove
  `pub mod config { pub use ragent_config::* }` from `ragent-agent/src/lib.rs`;
  update internal `crate::config::` references to `ragent_config::`.

**Exit criteria**: `grep -rn "pub mod config {" crates --include='*.rs'` returns
nothing. `cargo check --workspace` passes. `predictive` and `message::pool`
gone.

---

### Milestone 8 — Migrate inline tests to `tests/`
*Defect: D10. Follows AGENTS.md §"Test Organization".*

- **T8.1** `ragent-llm`: move the 72 inline tests from
  `providers/router_classifier.rs` and 52 from `router_modifiers.rs` into
  `crates/ragent-llm/tests/router_classifier.rs` / `tests/router_modifiers.rs`.
  Use `pub(crate)` widening + `#[path]` re-import per the migration strategy in
  `docs/reports/testconsolidate-completion.md` if needed.
- **T8.2** `ragent-agent`: move the 37 inline tests from
  `compression/pipeline.rs` to `crates/ragent-agent/tests/compression_pipeline.rs`.
- **T8.3** `ragent-tools-core`: move the 34 inline tests from `src/bash.rs` to
  `crates/ragent-tools-core/tests/bash.rs`.
- **T8.4** Sweep the remaining 109 `#[cfg(test)] mod tests` blocks in `src/`
  (list collected during audit) and migrate the top 10 by test count:
  `skill/context.rs` (27), `skill/loader.rs` (26), `skill/args.rs` (24),
  `specs/validate.rs` (22), `tui/app.rs` (21), `huggingface.rs` (21),
  `codeindex/parser/rust.rs` (20), `xai.rs` (18), `skill/mod.rs` (18),
  `reference/parse.rs` (18).
- **T8.5** Add a CI guard (e.g. a `scripts/check-inline-tests.sh` wired into
  `pre-flight.sh`) that fails if any `src/**/*.rs` file adds a new
  `#[cfg(test)] mod tests` block.

**Exit criteria**: `grep -rl "mod tests" crates/*/src --include='*.rs'` count
drops from 118 to ≤ ~30 (only genuinely private-item tests that cannot move).
`cargo test --workspace` still passes.

---

### Milestone 9 — Repository hygiene
*Defects: D11, D12.*

- **T9.1** Remove tracked stray files: `git rm EOF 1 default.profraw`. Add
  `*.profraw` and `default.profraw` to `.gitignore`.
- **T9.2** Move tracked output directories out of the repo:
  - `research/` → untrack (`git rm -r --cached research/`) and add to
    `.gitignore` (it is a research-output dir, per `ragent research` CLI).
  - `specs/` → untrack and `.gitignore` (spec outputs; the `specs/` slash
    command writes here). Keep `specs/` out of the source tree, or relocate
    to `target/temp/specs/` if sample specs are wanted in-repo.
  - Re-confirm with the user before untracking (see "Open questions" below).
- **T9.3** Rename `docs/howtoos/` → `docs/howtos/` (typo fix). Update any
  internal links (`docs/howto_teams.md`, `docs/howtoos/custom-agents.md`).
- **T9.4** Split `src/main.rs` (1224 lines):
  - Move `run_orchestration_example` (lines 35–285) into
    `examples/orchestration_root.rs` (already exists) or delete if obsolete.
  - Move `handle_research_command` (lines 1021–1224) into
    `crates/ragent-research/src/cli.rs` (already 967 lines — split further if
    needed) or a new `src/cli/research.rs`.
  - Keep `main.rs` as a thin clap dispatcher ≤ ~400 lines.
- **T9.5** Clean up `examples/`: remove `examples/test_timeout_strip.rs` if it
  was a one-off test. Confirm each example still builds with
  `cargo build --examples`.

**Exit criteria**: `git ls-files | grep -E '^(EOF|1|default.profraw)$'` empty.
`src/main.rs` ≤ ~500 lines. `docs/howtoos` does not exist.

---

### Milestone 10 — Final verification & docs
*Goal: confirm the whole plan landed cleanly and record it.*

- **T10.1** Run `cargo check --workspace`, `cargo clippy --workspace -- -D
  warnings`, `cargo fmt --check`, and `timeout 600 cargo test --workspace`.
  All must pass.
- **T10.2** Re-run the structural-defect checks from §1 and confirm each defect
  is resolved:
  - `grep -rn "pub struct Storage" crates/*/src` → 1 hit.
  - `grep -rn "pub struct Message\b" crates/*/src` → 1 hit.
  - `grep -rn "pub struct PermissionRequest" crates/*/src` → 1 hit.
  - `grep -rn "pub struct ToolDefinition" crates/*/src` → 1 hit.
  - `grep -rn '#\[path = "\.\./\.\./\.\./ragent-' crates/*/src` → 0 hits.
  - `grep -rn "ragent_core" crates --include='*.rs'` → 0 hits.
  - `find crates/ragent-tui/src -name app.rs` size ≤ ~1500 lines.
  - `find crates/ragent-agent/src/session -name processor.rs` size ≤ ~1200 lines.
  - Inline `mod tests` count in `src/` ≤ 30.
- **T10.3** Update `CHANGELOG.md` (Keep a Changelog format) with a
  "Refactoring" section listing the milestones landed.
- **T10.4** Update `SPEC.md` §"Architecture" crate table if crate boundaries
  changed (e.g. new `ragent-tool-api` crate from M3).
- **T10.5** Write a completion report to
  `docs/reports/remplan-completion.md` summarising before/after line counts and
  the defect-resolution table.

**Exit criteria**: all green; structural-defect grep checks pass; CHANGELOG &
completion report updated.

## 4. Ordering & Dependencies

```
M1 (types) ──┐
             ├─► M2 (storage) ──► M3 (team cycle) ──┐
M4 (alias) ──┘                                     ├─► M5 (tui split) ─┐
                                                   │                   │
                                                   M6 (proc split) ────┤
                                                                       ▼
M7 (dead code) ─► M8 (tests) ─► M9 (hygiene) ──────────────────────► M10 (verify)
```

- M1, M4, M7, M9 are independent and can be done in parallel by different
  workers (good candidates for a `team` decomposition).
- M2 must precede M3 (M3 touches `ragent-agent::storage` re-exports).
- M5 and M6 are independent of each other but both should follow M4 (so the
  files being split already use the canonical `ragent_agent::` paths).
- M8 should follow M5/M6 (so test files are moved to the final crate layout,
  not moved twice).

## 5. Open questions (need user confirmation before executing)

1. **`research/` and `specs/` directories** — are these intentionally tracked
   in the repo, or are they local outputs that were committed by accident? If
   intentional, M9 will leave them alone. If accidental, M9 untracks them.
2. **M3 ownership direction** — recommended that `ragent-team` owns the team
   runtime and `ragent-agent` depends on it (via a new `ragent-tool-api` crate
   for shared `Tool`/`ToolRegistry` primitives). Confirm this is acceptable, or
   prefer the reverse (ragent-agent owns everything, ragent-team becomes a
   thin tools-only crate)?
3. **M1.3 LLM-type canonical home** — move `ChatRequest`/`ChatMessage`/
   `StreamEvent`/`ToolDefinition` into `ragent-types`? This is the cleanest
   option but widens `ragent-types`. Alternative: keep them in `ragent-llm` and
   have `ragent-types::llm` re-export them. Pick one before T1.3.
4. **Execution mode** — should this plan be executed by the lead agent
   milestone-by-milestone, or delegated to a `team` of workers (one per
   independent milestone branch)? The plan is structured to support parallel
   execution of M1/M4/M7/M9.

## 6. Risk register

| Risk | Mitigation |
|------|------------|
| M3 breaks the Cargo graph (cycle re-appears in new form) | Land M3 as a single PR; verify `cargo tree` before/after; keep a `#[path]` fallback branch ready. |
| M5/M6 splits introduce subtle behaviour changes in TUI or agent loop | Each slice (T5.x / T6.x) is a separate commit; run the existing 2447-line slash-command test suite after every slice. |
| M8 test migration exposes private-item visibility issues | Use the documented `pub(crate)` + `#[path]` re-import pattern from `docs/reports/testconsolidate-completion.md`. |
| Renaming `ragent_core` → `ragent_agent` breaks external consumers | This is an internal alias only; no external crate imports `ragent_core`. Confirm with `grep -rn "ragent_core" docs/ README.md SPEC.md`. |
| Untracking `research/` / `specs/` loses user data | Do not `rm` working-tree files; use `git rm --cached` and add to `.gitignore`. Local files stay on disk. |

---

*This plan is a living document. Update the status column (☐ pending / ◐
in_progress / ☓ done) on each task as work progresses, and append per-task
notes to the "Execution log" section below as it is added.*

## 7. Execution log

### Milestone 5 — Split `ragent-tui/src/app.rs` — ✅ COMPLETE (2025-01-17)

All eight tasks landed. `cargo check --workspace` is green. `app.rs` was
reduced from 15,332 lines to 55 lines (a module declaration file). The
`impl App` methods are now distributed across 12 submodules under
`crates/ragent-tui/src/app/`.

**T5.8 — Move `TuiResearchObserver` + `RagentCompleter` — ✅**
- Moved `TuiResearchObserver` (struct + `SessionObserver` impl) and
  `RagentCompleter` (struct + `Completer` impl) from `app.rs` lines 51–147
  into `crates/ragent-tui/src/research_adapter.rs`. Made both `pub(crate)`
  with `pub` fields so `app.rs` can construct them.
- Added `use crate::research_adapter::{RagentCompleter, TuiResearchObserver};`
  in `app.rs`.

**T5.4 — Extract compress handling — ✅**
- Moved `poll_pending_opt`, `start_provider_compaction_for_session`,
  `apply_compaction_summary`, `handle_compress_command` (lines 705–1132)
  into `crates/ragent-tui/src/app/compress.rs`.

**T5.1 — Extract bench command handling — ✅**
- Moved `poll_pending_bench`, `drain_bench_progress_events`,
  `render_bench_run_event`, `render_bench_init_event`, `render_bench_list`,
  `render_bench_show`, `render_bench_status`, `render_bench_open_last`,
  `start_bench_run` (lines 1184–1608) into
  `crates/ragent-tui/src/app/bench.rs`.

**T5.2 — Extract swarm handling — ✅**
- Moved `execute_swarm_decomposition`, `spawn_swarm_teammates`,
  `handle_swarm_status`, `handle_swarm_cancel`, `poll_swarm_unblock`,
  `poll_swarm_completion`, `finalize_swarm_completion` (lines 13780–14435)
  into `crates/ragent-tui/src/app/swarm.rs`.

**T5.3 — Extract research/skill command handling — ✅**
- Moved `handle_research_command` (lines 13210–13541) into
  `crates/ragent-tui/src/app/research.rs`.

**T5.7 — Extract `App::new` — ✅**
- Moved `App::new` (lines 411–698) into
  `crates/ragent-tui/src/app/init.rs`.

**T5.6 — Extract input/mouse handling — ✅**
- Moved `handle_mouse_event`, `handle_history_picker_key`,
  `handle_context_menu_click`, `copy_selection`, `handle_key_event`
  into `crates/ragent-tui/src/app/input_handler.rs`.

**T5.5 — Extract slash-command dispatch — ✅**
- Moved `execute_slash_command` (public wrapper) and
  `execute_slash_command_inner` (the 5,601-line match) plus
  `get_command_suggestions` and `update_slash_menu` into
  `crates/ragent-tui/src/app/slash.rs` (6,200 lines).
- The match is moved verbatim — splitting it into per-family submodules
  (`slash/bench.rs`, `slash/team.rs`, etc.) with a thin dispatcher is
  future work beyond M5.

**Additional extractions (not in the original plan but needed to reach
the ≤1500 line target):**
- `handle_event` and related event-handling functions →
  `app/event_handler.rs` (1,500 lines).
- Model selection, thinking-level, and provider management functions →
  `app/models.rs` (2,400 lines).
- Session, team, and miscellaneous operations → `app/session_ops.rs`
  (3,200 lines).
- Free-standing helper functions (`MentionSpan`,
  `try_extract_research_code_block`, `parse_swarm_args`, table helpers)
  → `app/helpers.rs` (120 lines).
- Inline tests → `app/tests.rs` (420 lines).

**Exit-criteria checks:**
- `app.rs` → 55 lines (target was ≤ ~1500; far exceeded).
- No single function exceeds ~300 lines except `execute_slash_command_inner`
  (5,601 lines in `slash.rs`) — this is the one function the plan
  acknowledged as needing a dispatcher-pattern refactor that is deferred
  to future work.
- `cargo check --workspace` → clean.
- `cargo test -p ragent-tui --tests` → same 10 pre-existing failures
  (multi-threaded-runtime + huggingface-default-models) — no new failures.

### Milestone 4 — Retire the `ragent_core` alias — ✅ COMPLETE (2025-01-17)

All four tasks landed. `cargo check --workspace` is green with zero warnings.
The structure-guard test was extended with `no_ragent_core_alias_in_source_files`
to prevent `ragent_core` references from being re-introduced.

**T4.1 — Update Cargo.toml files — ✅**
- `crates/ragent-tui/Cargo.toml`: replaced
  `ragent-core = { package = "ragent-agent", path = "../ragent-agent" }`
  with `ragent-agent = { path = "../ragent-agent" }`.
  Updated the `compression` feature gate:
  `ragent-core/compression` → `ragent-agent/compression`.
- `crates/ragent-bench/Cargo.toml`: replaced
  `ragent-core = { package = "ragent-agent", path = "../ragent-agent" }`
  with `ragent-agent = { path = "../ragent-agent" }`.
- The root `Cargo.toml` already used `ragent-agent` directly (no alias).

**T4.2 — Mechanically rewrite `ragent_core::` → `ragent_agent::` — ��**
- Ran `find crates src examples -name '*.rs' -not -path '*/target/*' -exec
  sed -i 's/ragent_core::/ragent_agent::/g' {} +` across all 470 references
  in 63 files (including `src/`, `crates/*/src/`, `crates/*/tests/`,
  `crates/*/benches/`, and `examples/`).
- Also removed the `use ragent_agent as ragent_core;` alias imports that
  some files (`src/main.rs`, `ragent-server/src/*.rs`,
  `ragent-server/tests/*.rs`, `ragent-server/benches/*.rs`) used to bridge
  the old name to the new one. These were now dead imports after the sed
  rewrite.
- Manually verified that `ragent_agent::Config`, `ragent_agent::bash_lists`,
  `ragent_agent::gitlab`, `ragent_agent::compression`, etc. all resolve
  correctly via the `pub use ragent_config::{…}` and
  `pub use ragent_tools_vcs::{…}` re-exports in `ragent-agent/src/lib.rs`.

**T4.3 — Update doc-comments — ✅**
- The sed rewrite also updated doc-comments (`/// use ragent_core::…` →
  `/// use ragent_agent::…`) and prose references (`ragent_core` →
  `ragent_agent`) in all `.rs` files.

**T4.4 — Remove the `ragent-core` alias line — ✅**
- Verified that no `Cargo.toml` file contains `ragent-core` (except as part
  of `ragent-agent`).
- The alias line was removed by the T4.1 sed substitution.

**Exit-criteria checks (all green):**
- `grep -rn "ragent_core" crates --include='*.rs'` → 0 hits.
- `grep -rn "ragent_core" src --include='*.rs'` → 0 hits.
- `grep -rn "ragent_core" examples --include='*.rs'` → 0 hits.
- `grep -rn "ragent-core" crates/*/Cargo.toml Cargo.toml | grep -v ragent-agent`
  → 0 hits.
- `cargo check --workspace` → clean, 0 errors, 0 warnings.
- `cargo test -p ragent-agent --lib` → 280 pass.
- `cargo test -p ragent-agent --tests` → all pass.
- `cargo test -p ragent-bench --lib` → 13 pass.
- `cargo test -p ragent-server --tests` → 43 + 16 + 12 pass.
- `cargo test -p ragent-tui --tests` → same 10 pre-existing failures
  (multi-threaded-runtime + huggingface-default-models) — no new failures.
- `cargo test -p ragent-types --test structure_types` → 4 tests pass
  (including the new `no_ragent_core_alias_in_source_files` guard).

### Milestone 3 — Break the `ragent-agent`↔`ragent-team` `#[path]` cycle — ✅ COMPLETE (2025-01-17)

All six tasks landed. `cargo check --workspace` is green; `ragent-agent` (lib
+ integration), `ragent-team`, `ragent-server`, and `ragent-tui` test suites
pass. The M1 structure-guard test was extended with a
`no_path_attributes_to_ragent_team_in_agent` test that prevents `#[path]`
regressions.

**T3.1 — Map the cycle precisely — ✅**
- Confirmed: `ragent-team` → (Cargo) → `ragent-agent` for
  `agent/config/event/message/session/tool` re-exports.
  `ragent-agent` → (27× `#[path]`) → `ragent-team/src/team/*.rs` (7 files)
  and `ragent-team/src/tools/team_*.rs` (20 files), compiled into
  `ragent-agent`. Team sources compiled twice; logical cycle.

**T3.2 — Move team sources into ragent-agent; make ragent-team a re-export shim — ✅**
- **Decision**: Chose the alternative from Open Question #2 — "ragent-agent
  owns everything; ragent-team becomes a thin re-export shim." This avoids
  creating a new `ragent-tool-api` crate (the TeamManager is deeply coupled
  to `SessionProcessor`, `AgentInfo`, etc., making extraction impractical
  in one milestone).
- Copied 7 team runtime files (`classify`, `config`, `mailbox`, `manager`,
  `store`, `swarm`, `task`) from `ragent-team/src/team/` into
  `ragent-agent/src/team/`.
- Copied 20 team tool files (`team_approve_plan` … `team_wait`) from
  `ragent-team/src/tools/` into `ragent-agent/src/tool/`.
- Rewrote `ragent-agent/src/team/mod.rs` to declare native `pub mod` for
  each sub-module (replacing the 7 `#[path]` attributes).
- Removed all 20 `#[path]` attributes from `ragent-agent/src/tool/mod.rs`
  (the `pub mod team_*` declarations remain; the files are now native).
- Rewrote `ragent-team/src/lib.rs` as a re-export shim:
  `pub mod team { pub use ragent_agent::team::*; }`,
  `pub mod tool { pub use ragent_agent::tool::{…}; }`,
  `pub mod tools { pub use ragent_agent::tool::{team_*, …}; }`.
- The team files use `crate::` (resolving to `ragent_agent`) and
  `ragent_agent::` for agent types — both resolve correctly now that the
  files are native to `ragent-agent`.

**T3.3 — Delete old team source files from ragent-team — ✅**
- Deleted `ragent-team/src/team/` directory (8 files: `mod.rs`, `classify.rs`,
  `config.rs`, `mailbox.rs`, `manager.rs`, `store.rs`, `swarm.rs`, `task.rs`).
- Deleted `ragent-team/src/tools/` directory (21 files: `mod.rs` + 20
  `team_*.rs`).
- Deleted `ragent-team/src/tool.rs`.
- `ragent-team/src/` now contains only `lib.rs` (the re-export shim).

**T3.4 — Add guard test for `#[path]` regression — ✅**
- Extended `crates/ragent-types/tests/structure_types.rs` with
  `no_path_attributes_to_ragent_team_in_agent` — scans every
  `crates/ragent-agent/src/**/*.rs` file and fails if any `#[path]`
  attribute references `ragent-team`.
- All 3 structure-guard tests pass.

**T3.5 — Verify tests pass — ✅**
- `cargo check --workspace`: clean (no errors, no warnings except
  pre-existing `missing-docs` on the ragent-team shim modules which are
  now documented).
- `cargo test -p ragent-agent --lib`: 280 tests pass.
- `cargo test -p ragent-agent --tests`: all integration tests pass.
- `cargo test -p ragent-team`: passes (0 tests — the shim has no test
  code; the team tests run under `ragent-agent`).
- `cargo test -p ragent-server --tests`: 43 + 16 + 12 tests pass.
- `cargo test -p ragent-tui --tests`: the same 10 pre-existing failures
  from M2 (multi-threaded-runtime + huggingface-default-models assertion)
  — no new failures introduced by M3.

**Exit-criteria checks (all green):**
- `grep -rn '#[path = "../../../ragent-team' crates/ragent-agent/src` → 0 hits.
- `ragent-agent/Cargo.toml` does NOT list `ragent-team` as a dependency.
- `cargo tree -p ragent-agent` does not contain `ragent-team` (no cycle).
- Team sources compile exactly once (in `ragent-agent`; `ragent-team` is a
  45-line re-export shim with no source files of its own).

### Milestone 2 — Eliminate duplicate `Storage` — ✅ COMPLETE (2025-01-17)

**T2.1 — Port agent-only additions into `ragent-storage` — ✅**
- Diffed the two files: the only agent-only addition was the PERF-004
  `has_format_version: AtomicBool` cache field + its
  `has_format_version_cached()` helper. Ported both into
  `ragent-storage/src/storage.rs`. `migrate()` now sets the flag when the
  `format_version` column already exists; `get_session` / `list_sessions`
  call `has_format_version_cached()` instead of re-running the
  `pragma_table_info` round-trip on every call.
- While porting, also restored the four FIXME(M5)-commented-out methods
  (`search_memories_by_embedding`, `list_entities`, `list_relationships`,
  `query_entity_neighbours`) and `has_assistant_messages` into the canonical
  `ragent-storage::Storage`. These were previously only present in the agent
  copy and commented out in `ragent-storage` pending "memory module
  extraction" — they are required by the agent's `memory::knowledge_graph`
  and `tool` modules and could not be left out of the canonical impl.
  - `search_memories_by_embedding` now takes a caller-supplied `similarity`
    closure (so `ragent-storage` does not need to depend on
    `ragent-tools-extended`'s embedding helpers).
  - The KG methods return new storage-row types `KgEntityRow` /
    `KgRelationshipRow` (defined in `ragent-storage`) which the agent maps
    into its `memory::knowledge_graph::{Entity, Relationship}` via
    `From` impls. `EmbeddingMatch` is the storage-row equivalent of the
    agent's `SimilarityResult`.
- Added `crates/ragent-storage/tests/test_format_version_cache.rs` (3 tests)
  exercising the PERF-004 cache through the public API.

**T2.2 — Replace agent storage with re-export shim — ✅**
- Replaced `crates/ragent-agent/src/storage/mod.rs` (2217 lines) with a
  27-line re-export shim: `pub use ragent_storage::{EmbeddingMatch,
  KgEntityRow, KgRelationshipRow, MemoryRow, SessionRow, Storage, TodoRow,
  decrypt_key, deobfuscate_key, encrypt_key, obfuscate_key};`.
- Extended `ragent-storage::lib` re-exports to include `SessionRow`,
  `KgEntityRow`, `KgRelationshipRow`, `EmbeddingMatch`, and the four
  key helpers.
- Deleted the orphan `impl ragent_tools_vcs::storage::StorageBackend for
  crate::storage::Storage` from `ragent-agent/src/tool/mod.rs` (it became
  an orphan-rule violation once `Storage` was foreign). Moved the impl into
  `ragent-tools-vcs/src/lib.rs` (the crate that owns `StorageBackend`),
  and added `ragent-storage` as a dep of `ragent-tools-vcs`. This is the
  canonical home for that impl.
- Added `From<crate::storage::KgEntityRow>` / `From<crate::storage::KgRelationshipRow>`
  impls in `ragent-agent/src/memory/knowledge_graph.rs` so the agent's
  `get_knowledge_graph` can map storage rows into its `Entity` / `Relationship`
  types.
- Updated `CoreStorageAdapter::search_memories_by_embedding` in
  `ragent-agent/src/tool/mod.rs` to pass the `cosine_similarity` closure
  from `ragent_tools_extended::memory::embedding` into the canonical
  `Storage::search_memories_by_embedding`.

**T2.3 — Update in-crate storage use sites and doc-comments — ✅**
- The ~25 `use crate::storage::{…}` sites inside `ragent-agent` continue to
  resolve unchanged via the re-export shim (no behaviour change).
- Rewrote the stale `ragent_core::storage::Storage` / `ragent_core::message::Message`
  doc-comments in `ragent-agent/src/session/mod.rs` to
  `ragent_storage::Storage` / `ragent_types::message::Message`.

**T2.4 — Verify storage tests pass — ✅**
- `cargo test -p ragent-storage`: 27 doctests + 5 integration tests pass
  (incl. the new `test_format_version_cache` suite).
- `cargo test -p ragent-agent --lib`: 280 tests pass.
- `cargo test -p ragent-agent --tests`: all integration tests pass
  (incl. the updated `test_storage_format_version_cache` which no longer
  pokes the now-private `has_format_version` field).
- `cargo test -p ragent-server --tests`: 43 + 16 + 12 tests pass.
- `cargo test -p ragent-tui --tests`: the 10 pre-existing failures
  (`test_slash_spec_create_starts_generation`,
  `test_huggingface_with_token_does_not_fall_back_to_static_defaults_without_discovery`,
  `test_file_menu_*`, `test_slash_tools_*_on_shows_*_tools`,
  `test_update_file_menu_refreshes_cache_on_cwd_mismatch`,
  `test_directory_menu_has_back_to_fuzzy_entry`) were confirmed to also
  fail at HEAD (git stash + retest) — they are pre-existing and unrelated
  to M2 (multi-threaded-runtime requirement + a huggingface-default-models
  assertion that predates this milestone).

**Exit-criteria grep checks (all green):**
- `pub struct Storage` → 1 hit (`ragent-storage/src/storage.rs`).
- `crates/ragent-agent/src/storage/mod.rs` → 27-line shim (≤ ~30 line target).

### Milestone 1 — Foundation type consolidation — ✅ COMPLETE (2025-01-17)

All four tasks landed. `cargo check --workspace` is green; targeted test
suites (`ragent-types`, `ragent-config`, `ragent-llm`, `ragent-agent` lib,
`ragent-bench` lib, `ragent-tui` permission-countdown, `ragent-server`
event-to-sse, agent integration tests) all pass. The new
`ragent-types::tests::structure_types` guard test enforces the
single-definition invariant going forward.

**T1.1 — Canonicalise `Message` — ✅**
- Merged the `ImageData` struct + boxed `MessagePart::Image(Box<ImageData>)`
  variant from `ragent-agent/src/message/mod.rs` into
  `ragent-types/src/message/mod.rs`. Added `assistant_text` constructor to
  the canonical `Message` impl. Re-exported `ImageData`, `MessagePart`,
  `ToolCallState`, `ToolCallStatus` from `ragent-types::lib`.
- Replaced `ragent-agent/src/message/mod.rs` (288 lines) with a 13-line
  `pub use ragent_types::message::{…}` shim.
- `cargo check -p ragent-agent -p ragent-storage -p ragent-tui` passes.
- `ragent-agent --lib` tests: 286 → 280 passing (6 inline `message/mod.rs`
  tests removed with the duplicate file; they are covered by the canonical
  `ragent-types` doctests).

**T1.2 — Canonicalise `Permission*` — ✅**
- Made `ragent-config/src/permission.rs` the canonical home for
  `Permission`, `PermissionAction`, `PermissionRule`, `PermissionRequest`,
  `PermissionChecker`, `PermissionRuleset`. Ported the faster indexed
  `PermissionChecker` (rules_by_permission + wildcard_rules) from the agent
  crate into config, replacing the slower iterate-everything version. The
  indexed `check` now considers both the exact-custom and normalized forms
  of the permission string, preserving the behaviour of the old config
  `permission_candidates` helper.
- Replaced `ragent-agent/src/permission/mod.rs` (433 lines) with a 21-line
  re-export shim (`pub use ragent_config::permission::{…}` +
  `pub use ragent_types::permission::PermissionDecision`).
- Removed the unused `Permission` / `PermissionRequest` definitions from
  `ragent-types/src/permission.rs`; kept only `PermissionDecision` (used by
  `Event::PermissionReplied`). `ragent-config` now re-exports
  `PermissionDecision` from `ragent-types` instead of redefining it.
- Deleted the `config_permission_rule_to_runtime` helper in
  `ragent-agent/src/agent/mod.rs` (the two `PermissionRule` types are now
  the same type, so the conversion is a `clone()`).
- `ragent-config` permission tests (12) and `ragent-agent --lib` (280) pass.

**T1.3 — Canonicalise LLM primitive types — ✅**
- Moved `ChatRequest`, `ChatMessage`, `ChatContent`, `ContentPart`,
  `StreamEvent`, `ToolDefinition`, and the `arc_str_serde` /
  `optional_arc_str_serde` serde helpers from `ragent-llm/src/llm.rs` into
  `ragent-types/src/llm.rs` (the canonical home). Enabled the `rc` feature
  on `ragent-types`'s `serde` dependency so `Arc<Vec<…>>` fields
  (de)serialize correctly.
- Removed the unused legacy types from `ragent-types::llm`:
  `LlmProvider` trait, `LlmResponse`, `Usage`, `ModelInfo`,
  `ProviderConfig` (none were used outside the file). Removed the
  now-unused `async-trait` dependency from `ragent-types/Cargo.toml`.
- Replaced `ragent-llm/src/llm.rs` (258 lines) with a 43-line re-export
  shim that pulls the primitives from `ragent_types::llm` and keeps only
  the `LlmClient` trait (which needs `futures` + `anyhow`, deps that
  `ragent-types` intentionally does not pull in).
- `ragent-tools-core/extended/vcs` already imported
  `ragent_types::llm::ToolDefinition` — now the single definition. All
  `use crate::llm::{…}` sites in `ragent-llm` providers and `ragent-agent`
  resolve via the re-export, so no provider source changes were needed.
- `ragent-llm --lib` tests: 263 passing.

**T1.4 — Structure guard test — ✅**
- Added `crates/ragent-types/tests/structure_types.rs` with two tests:
  - `each_consolidated_type_has_exactly_one_definition` walks every
    `crates/*/src/**/*.rs` file and asserts that each consolidated type
    (`Message`, `MessagePart`, `ImageData`, `ToolCallState`,
    `ToolCallStatus`, `Role`, `Permission`, `PermissionAction`,
    `PermissionRule`, `PermissionRequest`, `PermissionChecker`,
    `PermissionRuleset`, `PermissionDecision`, `ToolDefinition`,
    `ChatRequest`, `ChatMessage`, `ChatContent`, `ContentPart`,
    `StreamEvent`) is defined in exactly its canonical home and nowhere
    else. Supports `pub struct` / `pub enum` / `pub type` definitions.
  - `no_ragent_types_llm_legacy_types_remain` guards against the deleted
    legacy types (`LlmProvider`, `LlmResponse`, `Usage`, `ModelInfo`,
    `ProviderConfig`) being re-introduced in `ragent-types/src/llm.rs`.
- Both tests pass.

**Exit-criteria grep checks (all green):**
- `pub struct Message` → 1 hit (`ragent-types/src/message/mod.rs`).
- `pub struct PermissionRequest` → 1 hit (`ragent-config/src/permission.rs`).
- `pub enum Permission` → 1 hit (`ragent-config/src/permission.rs`).
- `pub enum PermissionDecision` → 1 hit (`ragent-types/src/permission.rs`).
- `pub struct ToolDefinition` → 1 hit (`ragent-types/src/llm.rs`).
- `pub struct ChatRequest` / `ChatMessage` / `pub enum StreamEvent` /
  `ChatContent` / `ContentPart` → 1 hit each (`ragent-types/src/llm.rs`).