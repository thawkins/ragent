# Release

## Current Version: 0.1.0-alpha.129

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.129`.
- **Compression made permanent** — Removed the `compression` and
  `compression-ml` Cargo feature flags across the workspace.
  `headroom-core` is now an unconditional dependency of `ragent-agent`,
  and the context-compression pipeline is always compiled in. Specific
  changes:
    - `Cargo.toml` (workspace root): removed `compression` and
      `compression-ml` features; `default` is now empty.
    - `crates/ragent-agent/Cargo.toml`: removed `compression` and
      `compression-ml` feature definitions; `headroom-core` is no longer
      `optional`.
    - `crates/ragent-tui/Cargo.toml`: removed the `compression` feature
      passthrough.
    - `crates/ragent-agent/src/compression/mod.rs`: dropped all
      `#[cfg(feature = "compression")]` gates; `is_available()` now
      always returns `true`.
    - `crates/ragent-agent/src/lib.rs`,
      `crates/ragent-agent/src/session/{mod,history,loop_steps,processor}.rs`:
      removed every `#[cfg(feature = "compression")]` /
      `#[cfg(not(feature = "compression"))]` guard and the dead-code
      markers that existed only to silence the disabled-feature build.
    - `crates/ragent-agent/tests/test_compression_pipeline.rs` and
      `crates/ragent-agent/benches/agent_loop.rs`: removed the
      feature-gated `#[cfg(...)]` attributes on tests and benchmarks.
    - `crates/ragent-config/src/compression.rs`: updated doc comment for
      `CompressorConfig.prose` (no longer references the
      `compression-ml` feature).

## Previous Version: 0.1.0-alpha.128

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.128`.
- **Warning remediation** — Eliminated all 279 compiler warnings across the
  workspace (build, tests, benches, and examples now compile with zero
  warnings under `--all-features`). Fixes applied:
    - Removed ~270 unused imports across `ragent-tui` app submodules (init,
      compress, bench, swarm, research, models, slash, input_handler,
      event_handler, session_ops) left over from the `app.rs` split (M5).
    - Added `///` doc comments to 62 previously-undocumented `pub` /
      `pub(crate)` methods and associated functions across `ragent-tui` app
      submodules and `ragent-agent` `session/processor.rs` to satisfy
      `-W missing-docs`.
    - Gated the `is_token_overflow_error_message` import in
      `session/loop_steps.rs` behind `#[cfg(feature = "compression")]` and
      added `#[allow(unused_variables)]` / `#[allow(unused_mut)]` on
      feature-conditional parameters in `build_turn_chat_messages`.
    - Removed unused `std::sync::Arc` and `clap::Subcommand` imports from
      `src/cli.rs`.
    - Removed unused `ragent_prompt_opt::Completer` import from
      `app/swarm.rs` and `tool::TeamManagerInterface` from
      `app/input_handler.rs`.
    - Deleted dead duplicate `#[cfg(test)]` test functions in
      `app/models.rs` and `app/session_ops.rs` (the canonical `#[test]`
      versions live in `app/tests.rs`).
    - Deleted the dead `test_app` helper in `app/helpers.rs` (the canonical
      copy lives in `app/tests.rs`) and its `#[cfg(test)]` import block.
    - Removed redundant `use super::*;` from `app/tests.rs` and the
      `router_modifiers` inline test file.