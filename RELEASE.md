# Release

## Current Version: 0.1.0-alpha.11

### Added (since 0.1.0-alpha.10)
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
- SPEC.md updated: Skills section marked ✅, implementation details added
- QUICKSTART.md expanded with additional usage guidance
- 781 tests passing (up from 672), including ~110 new skill tests

### Changed (since 0.1.0-alpha.7)
- Message/log panel split: 60/40 (was 70/30)
- Event bus capacity: 2048 (was 256)
- TUI event loop drains all pending events per cycle via `try_recv()`
- Log panel scroll calculation uses wrapped line count for correct auto-scroll

### Fixed (since 0.1.0-alpha.7)
- Log panel missing entries due to incorrect scroll calculation with wrapped lines
- Silent event bus lag dropping tool call log entries during bursts
- Resumed sessions not populating log panel with restored tool calls
- 148 build warnings resolved (missing docs, unused variables, dead code)
