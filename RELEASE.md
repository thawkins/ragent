# Release

## Current Version: 0.1.0-alpha.13

### Added (since 0.1.0-alpha.12)
- **LSP test prompts** — 5 test prompts for LSP server integration testing (hover, go-to-definition, find-references, list-symbols, diagnostics) targeting Rust language servers
- **Office 365 test prompts** — 5 test prompts for Office document read/write testing (Word, Excel, PowerPoint, multi-sheet Excel, output formats)
- **LSP workspace folders support** — updated LSP client to use `workspace_folders` parameter instead of deprecated `root_uri`

### Fixed (since 0.1.0-alpha.12)
- **Unused imports and dead code** — removed unused `LspDiscoverState` import and dead `get_json_str`/`get_json_u64` helper functions
- **Missing documentation** — added doc comments for `ExperimentalFlags` struct and `VsCodeExtension` variant field

### Carried from 0.1.0-alpha.12
- OpenSkills support for extended skill file formats
- Output file support for `/simplify` skill
- Skills system fully implemented (SPEC §3.19) — 10 phases complete
  - YAML frontmatter skill definitions in `SKILL.md` files
  - Scope-priority registry (Bundled < Enterprise < Personal < Project)
  - Argument substitution (`$ARGUMENTS`, `$N`, `$ARGUMENTS[N]`, `${RAGENT_*}`)
  - Dynamic context injection via `` !`command` `` patterns
  - Forked subagent execution context for isolated skill runs
  - 4 bundled skills: simplify, batch, debug, loop
  - `/skills` slash command with formatted table display
  - Skill autocomplete in TUI slash menu (skills shown in yellow)
  - Config `skill_dirs` for extra skill search paths
  - `release` project skill for automated version bump, push, and tag
- 781+ tests passing
