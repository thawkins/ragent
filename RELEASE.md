# Release

## Current Version: 0.1.0-alpha.12

### Added (since 0.1.0-alpha.11)
- **OpenSkills support** — extended skill loader to support OpenSkills-format skill files
- **Output file support for `/simplify` skill** — skill now accepts optional output path argument to save findings to a markdown file

### Fixed (since 0.1.0-alpha.11)
- **Overly aggressive read function** — improved large file handling with section maps

### Carried from 0.1.0-alpha.11
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
