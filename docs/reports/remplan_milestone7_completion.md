# REMPLAN.md Milestone 7 — Remove dead code & compat shims — Completion Report

**Date:** 2025-01-17  
**Status:** ✅ COMPLETE (T7.1–T7.5 all landed)

## Summary

Milestone 7 removed dead/orphan code and collapsed the compatibility-shim
modules that hid the real dependency direction between `ragent-agent`,
`ragent-llm`, `ragent-tools-extended`, and `ragent-tools-vcs` on the one hand
and `ragent-config` / `ragent-types` on the other. The workspace compiles
clean (with and without the `compression` feature) and all targeted test
suites pass.

## Tasks

### T7.1 — Delete `predictive.rs` ✅
- Deleted `crates/ragent-agent/src/predictive.rs` (454 lines, 0 internal
  references beyond its own `pub mod predictive;` declaration).
- Removed `pub mod predictive;` from `crates/ragent-agent/src/lib.rs`.

### T7.2 — Delete `message/pool.rs` ✅
- Deleted `crates/ragent-agent/src/message/pool.rs` (168 lines). The module
  was never exported from `message/mod.rs` (no `mod pool;` declaration) and
  had 0 references.

### T7.3 — Remove `ragent-research` from `ragent-specs` dev-deps ✅
- Removed `ragent-research = { path = "../ragent-research" }` from
  `crates/ragent-specs/Cargo.toml` `[dev-dependencies]`.
- Added `ragent-specs = { path = "../ragent-specs" }` to
  `crates/ragent-research/Cargo.toml` `[dev-dependencies]` and moved
  `tests/test_research_e2e.rs` from `ragent-specs/tests/` to
  `ragent-research/tests/`. The cross-crate integration test now lives in
  the crate that owns the `ResearchManager` type it exercises, and the
  dev-dependency edge is one-directional (`ragent-research` → `ragent-specs`
  as a dev-dep) instead of the reverse.

### T7.4 — Collapse `ragent-llm` compat shims ✅
- Removed `pub mod config { pub use ragent_config::{Capabilities, Cost}; }`
  and `pub mod event { pub use ragent_types::event::FinishReason; }` from
  `crates/ragent-llm/src/lib.rs`.
- Rewrote the 10 provider files that wrote `use crate::event::FinishReason;`
  to `use ragent_types::event::FinishReason;` (anthropic, bedrock, copilot,
  gemini, huggingface, mock_llm_client, openai, ollama, ollama_cloud,
  azure_resource).
- Rewrote `providers/mod.rs` from `use crate::config::{Capabilities, Cost};`
  to `use ragent_config::{Capabilities, Cost};`.

### T7.5 — Collapse `ragent-agent` config shim ✅
- Removed `pub mod config { pub use ragent_config::config::StreamConfig;
  pub use ragent_config::*; }` from `crates/ragent-agent/src/lib.rs`.
- Added `McpServerConfig` to the agent's root `pub use ragent_config::{…}`
  re-export so `ragent_agent::McpServerConfig` resolves.
- Updated the single internal `crate::config::Config` reference
  (`crates/ragent-agent/src/team/manager.rs`) to `crate::Config`.
- Mechanically replaced `ragent_agent::config::X` → `ragent_agent::X`
  across `crates/`, `src/`, and `examples/` (46 call sites in ragent-tui
  src/tests/benches, ragent-server, ragent-bench, ragent-agent tests,
  ragent-team shim, and `src/main.rs`). The agent crate root already
  re-exports `Config`, `StreamConfig`, `ToolVisibilityConfig`,
  `tool_family_names`, and now `McpServerConfig`, so the shorter paths
  resolve without the shim module.

### Incidental — Collapse `ragent-tools-extended` / `ragent-tools-vcs` config shims ✅
- While satisfying the exit-criteria grep (`pub mod config {` → 0 hits),
  also removed the identical compat shims from
  `crates/ragent-tools-extended/src/lib.rs`
  (`pub mod config { pub use ragent_config::CrossProjectConfig; }`) and
  `crates/ragent-tools-vcs/src/lib.rs`
  (`pub mod config { pub use ragent_config::Config; }`).
- Updated the 4 internal `crate::config::` references
  (`memory/cross_project.rs`, `memory_search.rs`, `memory_write.rs`,
  `gitlab/auth.rs`) to `ragent_config::` directly.

## Verification

| Check | Result |
|-------|--------|
| `cargo check --workspace` | ✅ |
| `cargo build --workspace --tests` | ✅ |
| `cargo build --workspace --tests --features ragent-agent/compression` | ✅ |
| `cargo test -p ragent-agent --lib` | ✅ 254 passed |
| `cargo test -p ragent-agent --test session_processor` | ✅ 22 passed |
| `cargo test -p ragent-agent --test test_compression_pipeline --features compression` | ✅ 29 passed |
| `cargo test -p ragent-llm --lib` | ✅ 263 passed |
| `cargo test -p ragent-tui --lib` | ✅ 59 passed |
| `cargo test -p ragent-specs` | ✅ 4 doctests passed |
| `cargo test -p ragent-research --test test_research_e2e` | ✅ 1 passed |
| `cargo test -p ragent-types --test structure_types` | ✅ 4 passed |

## Exit-criteria checks (all green)

- `grep -rn "pub mod config {" crates --include='*.rs'` → **0 hits**.
- `grep -rn "predictive" crates --include='*.rs'` → **0 hits**.
- `crates/ragent-agent/src/message/pool.rs` → **gone**.
- `ragent-research` no longer in `ragent-specs/Cargo.toml` → **confirmed**.
- `cargo check --workspace` → **clean**.

## Files modified

| File | Change |
|------|--------|
| `crates/ragent-agent/src/predictive.rs` | deleted (T7.1) |
| `crates/ragent-agent/src/lib.rs` | removed `pub mod predictive;`, `pub mod config {…}` shim, added `McpServerConfig` re-export (T7.1, T7.5) |
| `crates/ragent-agent/src/message/pool.rs` | deleted (T7.2) |
| `crates/ragent-agent/src/team/manager.rs` | `crate::config::Config` → `crate::Config` (T7.5) |
| `crates/ragent-specs/Cargo.toml` | removed `ragent-research` dev-dep (T7.3) |
| `crates/ragent-specs/tests/test_research_e2e.rs` | moved to `ragent-research/tests/` (T7.3) |
| `crates/ragent-research/Cargo.toml` | added `ragent-specs` dev-dep (T7.3) |
| `crates/ragent-research/tests/test_research_e2e.rs` | moved here (T7.3) |
| `crates/ragent-llm/src/lib.rs` | removed `pub mod config` + `pub mod event` shims (T7.4) |
| `crates/ragent-llm/src/providers/*.rs` (10 files) | `crate::event::FinishReason` → `ragent_types::event::FinishReason` (T7.4) |
| `crates/ragent-llm/src/providers/mod.rs` | `crate::config::{…}` → `ragent_config::{…}` (T7.4) |
| `crates/ragent-tools-extended/src/lib.rs` | removed `pub mod config` shim (incidental) |
| `crates/ragent-tools-vcs/src/lib.rs` | removed `pub mod config` shim (incidental) |
| `crates/ragent-tools-extended/src/memory/{cross_project,search,write}.rs` | `crate::config::` → `ragent_config::` (incidental) |
| `crates/ragent-tools-vcs/src/gitlab/auth.rs` | `crate::config::Config` → `ragent_config::Config` (incidental) |
| `crates/ragent-team/src/lib.rs` | removed `config` from `pub use ragent_agent::{…}` (T7.5) |
| `crates/ragent-server/src/routes/mod.rs` | `config::Config` → `Config` in `use ragent_agent::{…}` (T7.5) |
| `src/main.rs` | `config::Config` → `Config` in `use ragent_agent::{…}` (T7.5) |
| ~40 files across `crates/*/src`, `crates/*/tests`, `crates/*/benches` | `ragent_agent::config::X` → `ragent_agent::X` (T7.5) |