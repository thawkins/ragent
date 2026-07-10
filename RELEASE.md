# Release

## Current Version: 0.1.0-alpha.143

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.143`. Fixes to scroll optimizations and continued edit/multi-edit reliability improvements.

## Previous Version: 0.1.0-alpha.142

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.142`. Fix Edit/MultiEdit tool matcher reliability.

## Previous Version: 0.1.0-alpha.141

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.141`. Update research to add titles to findings.

## Previous Version: 0.1.0-alpha.137

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.137`. Follow-up to
  `0.1.0-alpha.136`, which added web source publication dates to the
  `/research` slash command (`RESEARCH.md` References Index **Published**
  column, per-finding `**Source date range:**`, and the
  `ragent_research::extract_published_at` helper).

## Previous Version: 0.1.0-alpha.136

### Added

- **Research source publication dates** — The `/research` slash command now
  captures the publication date of each web source and surfaces it in the
  `RESEARCH.md` output and references. A new `**Source date range:**` line
  under each finding summarises the earliest–latest publication dates of its
  cited web sources, so the relative age of the evidence is visible at a
  glance. Dates are parsed best-effort from JSON-LD `datePublished`, article
  meta tags (`article:published_time`, `pubdate`, `dc.date`, etc.), `<time>`
  elements, and a visible-text fallback; any failure leaves the date as `—`
  without aborting the research run. The References Index table gained a
  **Published** column, supporting files show `Published (UTC)`, and the new
  `ragent_research::extract_published_at` helper is re-exported for
  `ragent-agent`'s best-effort raw-HTML fetch. Older `RESEARCH.md` files
  remain loadable via `#[serde(default)]` on the new optional field.

## Previous Version: 0.1.0-alpha.135

### Fixed
- **Workspace version** — Bumped to `0.1.0-alpha.135`.
- **fix ci** — Resolved GitHub Actions "Check and Test" failure caused by the `0.1.0-alpha.132` scrollbar drag math regression in Memory/TODO panels. Reverted the `top_based` inversion in `apply_scrollbar_drag()` and updated the Memory panel tests to use the bottom-based offset convention consistent with Messages/Log/Profile.

## Previous Version: 0.1.0-alpha.134

### Fixed
- **Scrollbar drag math regression** — Reverted the `top_based` inversion introduced for Memory/TODO panels in `0.1.0-alpha.132` and updated the Memory panel tests to match the bottom-based offset convention used by the rest of the TUI.

## Previous Version: 0.1.0-alpha.133

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.133`.
- **fix tests that depend on untracked files** — Pointed `ragent-specs` real-project integration tests at the self-contained fixture under `crates/ragent-specs/tests/fixtures/testspec` so they no longer rely on the untracked `specs/` directory.

## Previous Version: 0.1.0-alpha.132

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.132`.
- **fix thumb scrolls** — TUI thumb/srollbar scrolling improvements.

## Previous Version: 0.1.0-alpha.131

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.131`.
- **TODO panel** — Implemented a third side panel (Alt+T) in `ragent-tui`
  that renders the session's TODO items from `ragent-storage`. The panel
  follows the existing log/profile side-panel pattern with mutual
  exclusion, text selection, scrollbar drag, and a `/todo` slash alias.
  All 12 plan tasks (T-001…T-012) and 8 acceptance criteria from the
  `todopanel` spec are satisfied.
- **Agentic-loop performance upgrade** — Implemented all six milestones
  (A–F) of `PERFPLAN.md`, covering 26 findings (P-1…P-26) plus 5
  measurement/gating tasks (F-1…F-5). Highlights:
  - Deleted inline nudge recomputation; single `set_step` call; verified
    empty-buffer stall guard (`handle_no_tool_decision`).
  - `LoopState.chat_messages` is now `Arc<Vec<ChatMessage>>` with
    `Arc::make_mut` for cheap clones; tool-definition bytes cached on
    `SessionProcessor`; one `ToolContext` per step; hoisted reusable Vecs;
    `text_buffer` moved via `mem::take`.
  - `get_messages` routed through `storage_op`; cached config keyed by
    file mtimes; `build_turn_chat_messages` returns the context window;
    `TaskManager.has_pending_background` AtomicBool skips drain scans;
    interim-save hash uses `serde_json::to_vec` bytes.
  - `ToolsSent` published only on step 1; added `Event::ToolCallBatch` +
    `ToolCallBatchEntry` and SSE forwarding; tool-result preview scan
    capped at 400 bytes.
  - Consolidated emergency-compression call sites; verified async history
    reads; short-circuit when `last_reported_input_tokens > 0`; added
    `cached_spec_section` to `SystemPromptCache` keyed by
    `(spec_id, spec.modified_at)` with `/spec activate` invalidation.
  - Added `MockLlmClient`/`MockLlmScript` in `ragent-bench`, criterion
    `agent_loop` benchmarks, baseline report, `/perf` TUI alias, and
    `scripts/check-bench-regression.sh` CI guard wired into `pre-flight.sh`.

## Previous Version: 0.1.0-alpha.129
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