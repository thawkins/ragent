# Release

## Current Version: 0.1.0-alpha.49

### Added
- **`/model show` slash command** — Added an in-chat model metadata display for the active provider/model, including capabilities, context window, output limits, and cost information.
- **Slash-command regression coverage** — Added tests for `/model show` behavior and a dedicated `test_slash_menu_escape.rs` coverage file for escape-key handling.

### Changed
- **Slash menu keyboard UX** — Pressing `Esc` now closes the slash-command menu without discarding partially typed input and keeps the cursor position safely clamped.
- **Slash command help and spec text** — Updated `/model` help text, slash-command descriptions, and SPEC.md to reflect model metadata display and the refined escape behavior.

## Previous: 0.1.0-alpha.48

### Changed
- **Permission workflow and config updates** — Refined permission handling, TUI countdown behavior, and config parsing diagnostics.
- **Codeindex permissions** — Hardwired codeindex tools to bypass prompts for read-only local analysis.
- **Workspace maintenance** — Added new crate-level tests and refreshed specification/supporting reports for the current release.
