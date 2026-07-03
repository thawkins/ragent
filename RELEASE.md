# Release

## Current Version: 0.1.0-alpha.128

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

## Previous Version: 0.1.0-alpha.127

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.127`.
- **Dead-code removal** — Audited every `#[allow(dead_code)]` site across the
  workspace and removed ~579 net lines of genuinely unreachable code.
  Removed items include: `cleanup_unused_locks` and the whole
  `ragent-agent/src/tool/file_lock.rs` module (a duplicate of the
  `ragent-tools-core` file-lock); `get_attribute` (maven parser);
  `orch_metrics` HTTP handler (route never registered); the deprecated
  `render_status_bar` v1 and `render_plan_approval_dialog` (superseded by
  v2 / widget-based rendering); `GREP_PATTERNS` (predictive); seven unused
  style helpers (`style_healthy`, `style_warning`, `style_error`,
  `style_info`, `style_healthy_bold`, `style_warning_bold`,
  `style_error_bold`); the standalone `AzureFoundry::discover_models`
  method; `FailedToolCall.timestamp`; `FindDiag.pass` / `FindDiag.closest_line`;
  `ShellType::program`; and `resolve_base_url` (research analysis). Stale
  `#[allow(dead_code)]` attributes were also removed from items that are
  actually used (`SAFE_COMMANDS`, `char_wrap`, `push_log_no_agent`).