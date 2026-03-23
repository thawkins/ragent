# Skills Implementation Plan

**SPEC Reference:** Section 3.19 Skills  
**Status:** ❌ Not Started  
**Date:** 2026-03-16  

## Overview

Skills enhance the agent's ability to perform specialized tasks by bundling instructions, scripts, and resources into reusable packages. Skills follow a markdown-first format with YAML frontmatter for configuration. They are loaded from `.ragent/skills/`, `~/.ragent/skills/`, and nested project directories for monorepo support.

## Current State

- **No skills module exists** — implementation starts from scratch
- **Agent system is fully functional** — 8 builtin agents with config overrides and prompt assembly
- **TUI slash commands exist** — 13 hardcoded commands with autocomplete; needs extension for dynamic skill commands
- **No YAML parsing support** — only JSON config; need to add `serde_yaml` dependency
- **Prompt assembly is extensible** — `build_system_prompt()` in `agent/mod.rs` can be extended for skill injection
- **Session processor is extensible** — agentic loop in `processor.rs` can be extended for subagent forking
- **`config.instructions` field exists but is unused** — potential integration point

## Architecture

### New Module: `crates/ragent-core/src/skill/`

```
crates/ragent-core/src/skill/
├── mod.rs          # Public API, SkillInfo struct, SkillRegistry
├── loader.rs       # Discovery & loading from filesystem (SKILL.md parsing)
├── args.rs         # Argument substitution ($ARGUMENTS, $N, env vars)
├── context.rs      # Dynamic context injection (!`command` syntax)
├── bundled.rs      # Bundled skill definitions (/simplify, /batch, /debug, /loop)
└── invoke.rs       # Invocation logic, subagent forking for context:fork
```

### Integration Points

| Component | File | Change |
|-----------|------|--------|
| Core lib exports | `lib.rs` | Add `pub mod skill;` |
| Agent config | `config/mod.rs` | Add `skills: Vec<String>` to `AgentConfig` |
| System prompt | `agent/mod.rs` | Inject skill descriptions into prompt assembly |
| Session processor | `session/processor.rs` | Handle skill invocation and subagent forking |
| TUI slash commands | `ragent-tui/src/app.rs` | Register discovered skills as dynamic `/name` commands |
| TUI input | `ragent-tui/src/input.rs` | Route skill slash commands to skill invocation |
| Workspace deps | `Cargo.toml` | Add `serde_yaml` for frontmatter parsing |

---

## Implementation Tasks

### Phase 1: Foundation — Data Model & Dependencies

#### Task 1.1: Add YAML frontmatter parsing dependency
- Add `serde_yaml` to `crates/ragent-core/Cargo.toml`
- Verify workspace builds with new dependency

#### Task 1.2: Define skill data model
- Create `crates/ragent-core/src/skill/mod.rs`
- Define `SkillInfo` struct with all frontmatter fields:
  ```rust
  pub struct SkillInfo {
      pub name: String,
      pub description: Option<String>,
      pub argument_hint: Option<String>,
      pub disable_model_invocation: bool,    // default: false
      pub user_invocable: bool,              // default: true
      pub allowed_tools: Vec<String>,
      pub model: Option<String>,
      pub context: Option<SkillContext>,      // None or Fork
      pub agent: Option<String>,             // subagent type when context=fork
      pub hooks: Option<serde_json::Value>,  // deferred until hooks system exists
      pub source_path: PathBuf,              // absolute path to SKILL.md
      pub skill_dir: PathBuf,               // directory containing SKILL.md
      pub scope: SkillScope,                 // Personal / Project / Enterprise
      pub body: String,                      // markdown body after frontmatter
  }
  ```
- Define `SkillContext` enum (`Fork`)
- Define `SkillScope` enum (`Personal`, `Project`, `Enterprise`)
- Define `SkillRegistry` to hold discovered skills:
  ```rust
  pub struct SkillRegistry {
      skills: HashMap<String, SkillInfo>,
  }
  ```
- Export `pub mod skill;` from `lib.rs`

#### Task 1.3: Implement YAML frontmatter parser
- In `skill/loader.rs`, implement `parse_skill_md(content: &str) -> Result<SkillInfo>`
- Parse `---` delimited YAML frontmatter from markdown content
- Deserialize frontmatter into `SkillFrontmatter` intermediate struct
- Extract markdown body (everything after closing `---`)
- Handle missing/optional fields with sensible defaults

### Phase 2: Discovery & Loading

#### Task 2.1: Implement skill discovery
- In `skill/loader.rs`, implement `discover_skills(working_dir: &Path) -> Result<Vec<SkillInfo>>`
- Search paths in priority order:
  1. Project: `{working_dir}/.ragent/skills/*/SKILL.md`
  2. Personal: `~/.ragent/skills/*/SKILL.md`
- Scan for nested `.ragent/skills/` in subdirectories for monorepo support
- Higher-priority scopes override lower ones when names conflict
- Log discovered skills via tracing

#### Task 2.2: Build SkillRegistry
- Implement `SkillRegistry::new()` and `SkillRegistry::load(working_dir: &Path)`
- Store skills indexed by name
- Provide lookup methods:
  - `get(name: &str) -> Option<&SkillInfo>`
  - `list_user_invocable() -> Vec<&SkillInfo>` (for TUI slash menu)
  - `list_agent_invocable() -> Vec<&SkillInfo>` (for prompt injection)
  - `list_all() -> Vec<&SkillInfo>`

### Phase 3: Argument Substitution & Dynamic Context

#### Task 3.1: Implement argument substitution
- In `skill/args.rs`, implement `substitute_args(body: &str, args: &str, session_id: &str, skill_dir: &Path) -> String`
- Replace variables:
  | Variable | Replacement |
  |----------|-------------|
  | `$ARGUMENTS` | All arguments passed when invoking |
  | `$ARGUMENTS[N]` | Specific argument by 0-based index |
  | `$N` (e.g. `$0`, `$1`) | Shorthand for `$ARGUMENTS[N]` |
  | `${RAGENT_SESSION_ID}` | Current session ID |
  | `${RAGENT_SKILL_DIR}` | Directory containing SKILL.md |
- Parse arguments by splitting on whitespace (respecting quoted strings)

#### Task 3.2: Implement dynamic context injection
- In `skill/context.rs`, implement `inject_dynamic_context(body: &str) -> Result<String>`
- Find all `` !`command` `` patterns in the skill body
- Execute each command via `tokio::process::Command`
- Replace the `` !`command` `` placeholder with command stdout
- Handle command failures gracefully (include error text or skip)
- Apply a reasonable timeout (e.g. 30 seconds per command)

### Phase 4: Skill Invocation ✅

#### Task 4.1: Implement skill invocation logic ✅
- Created `skill/invoke.rs` with `invoke_skill()` and `format_skill_message()`
- `SkillInvocation` struct carries processed content + metadata (forked, model override, allowed tools)
- Full invocation flow: substitute_args → inject_dynamic_context → SkillInvocation
- 8 unit tests covering: simple skills, dynamic context, arg+context combo, fork metadata, session ID substitution, message formatting

#### Task 4.2: Integrate skill invocation into session processor ✅
- In `ragent-tui/src/app.rs`, added skill invocation in the `_ =>` catch-all of `execute_slash_command()`
- Loads `SkillRegistry` on demand, checks if unknown command matches a registered skill
- Validates: user_invocable flag, provider/model configured, session exists
- Applies skill's model override (or falls back to selected model)
- Shows skill invocation as user message in chat, spawns async task for processing
- Forked skills currently run inline with a warning (Phase 5 will add proper subagent support)

### Phase 5: Subagent Forking ✅

#### Task 5.1: Implement subagent execution for `context: fork` skills ✅
- Created `invoke_forked_skill()` in `skill/invoke.rs`
- Creates isolated sub-session via `SessionManager::create_session()`
- Resolves agent from skill's `agent` field (defaults to `"general"`)
- Applies model override (supports both `provider/model` and `provider:model` formats)
- Sets agent mode to `Subagent` for the forked execution
- Runs processed skill content through `SessionProcessor::process_message()` in the new session
- Returns `ForkedSkillResult` with response text and forked session ID
- Added `format_forked_result()` helper for parent conversation injection
- Updated TUI handler to use `invoke_forked_skill()` for forked skills (replaces Phase 4 stub)
- 6 new tests (14 total in invoke.rs): forked result formatting, metadata preservation, agent fallback logic

### Phase 6: System Prompt Integration ✅

#### Task 6.1: Inject skill descriptions into system prompt ✅
- Extended `build_system_prompt()` signature to accept `Option<&SkillRegistry>`
- After AGENTS.md and before Guidelines, injects "## Available Skills" section
- Lists only agent-invocable skills (where `disable_model_invocation` is false)
- Shows skill name, argument hint, and description in markdown list format
- When agent has `skills` configured, filters to only those named skills
- When no skills are configured, shows all agent-invocable skills from registry
- Updated processor.rs to load `SkillRegistry` and pass it to `build_system_prompt()`
- Updated all 4 existing test call sites to pass `None` for backward compatibility
- 6 new tests: with skills, no registry, empty registry, disabled exclusion, agent-specific filter, ordering

#### Task 6.2: Preload agent-specific skills ✅
- Added `skills: Vec<String>` field to `AgentConfig` in `config/mod.rs`
- Added `skills: Vec<String>` field to `AgentInfo` in `agent/mod.rs`
- Updated `AgentInfo::new()` to initialize `skills: Vec::new()`
- Updated all 8 builtin agent struct literals with `skills: Vec::new()`
- Updated `resolve_agent()` to apply skills override from agent config
- 2 new tests: resolve with/without skills config

### Phase 7: TUI Integration ✅

#### Task 7.1: Register skills as dynamic slash commands ✅
- Introduced `SlashMenuEntry` struct (trigger, description, is_skill) to replace index-based matching
- Refactored `SlashMenuState.matches` from `Vec<usize>` (indices into static array) to `Vec<SlashMenuEntry>` (owned entries)
- Extended `update_slash_menu()` to load `SkillRegistry` and append user-invocable skills to autocomplete
- Skills appear in autocomplete with trigger, description, and argument hint
- Builtin commands take priority over skills with the same name
- Skills rendered in yellow in the autocomplete menu to distinguish from builtins
- Updated `/help` command to list skills in a separate "Skills:" section
- Updated `layout.rs` rendering to use `SlashMenuEntry` directly instead of indexing

#### Task 7.2: Handle skill command input in TUI ✅
- Updated `input.rs` Enter handler to use `SlashMenuEntry.trigger` directly (no more `SLASH_COMMANDS[idx]`)
- Removed `SLASH_COMMANDS` import from `input.rs` and `layout.rs` (only used internally in `app.rs`)
- Skill invocation flow: autocomplete selection → `InputAction::SlashCommand` → `execute_slash_command` catch-all → skill registry lookup → `invoke_skill()` (from Phase 4)
- Direct typing also works: `/skillname args` → filter falls through → full text sent as slash command

### Phase 8: Bundled Skills

#### Task 8.1: Implement bundled skill definitions
- In `skill/bundled.rs`, define the 4 bundled skills as embedded SKILL.md content:
  | Skill | Description |
  |-------|-------------|
  | `/simplify` | Reviews recently changed files for code quality, reuse, and efficiency issues |
  | `/batch <instruction>` | Orchestrates large-scale parallel changes across a codebase |
  | `/debug [description]` | Troubleshoots current session by reading debug logs |
  | `/loop [interval] <prompt>` | Runs a prompt repeatedly on an interval |
- Use `include_str!()` or construct `SkillInfo` programmatically
- Bundled skills are always available and have lowest priority (overridable by project/personal skills)

#### Task 8.2: Register bundled skills in SkillRegistry
- Modify `SkillRegistry::load()` to register bundled skills first, then overlay discovered skills
- Bundled skills can be overridden by project or personal skills with the same name

### Phase 9: Config Integration ✅

#### Task 9.1: Extend config schema for skills ✅
- Added `skill_dirs: Vec<String>` to top-level `Config` struct
- Updated `Config::merge()` to append `skill_dirs` (same semantics as `instructions`)
- Extended `discover_skills()` with `extra_dirs` parameter (Personal scope, overridable by Project)
- Extended `SkillRegistry::load()` with `extra_dirs` parameter
- Updated all 5 production call sites (processor + 4 TUI) to load config and pass `skill_dirs`
- Updated all test call sites to pass `&[]` for extra dirs
- Added 7 new tests: 3 loader tests (extra dirs, override, nonexistent), 1 registry test, 3 config tests

### Phase 10: Testing ✅

#### Task 10.1: Unit tests for skill parsing ✅
- Test YAML frontmatter parsing with various field combinations
- Test missing/malformed frontmatter handling
- Test markdown body extraction

#### Task 10.2: Unit tests for argument substitution ✅
- Test `$ARGUMENTS`, `$ARGUMENTS[N]`, `$N` replacement
- Test `${RAGENT_SESSION_ID}` and `${RAGENT_SKILL_DIR}` replacement
- Test edge cases: missing args, empty args, quoted strings

#### Task 10.3: Unit tests for dynamic context injection ✅
- Test `` !`command` `` pattern matching and replacement
- Test command execution and output capture
- Test error handling for failed commands

#### Task 10.4: Unit tests for skill discovery ✅
- Test discovery from project path
- Test discovery from personal path
- Test scope priority (project overrides personal)
- Test monorepo nested discovery

#### Task 10.5: Integration tests for skill invocation ✅
- Test end-to-end skill invocation via slash command
- Test subagent forking for `context: fork` skills
- Test invocation control (disable-model-invocation, user-invocable)
- Test model and tool overrides during skill execution

#### Task 10.6: TUI tests for skill slash commands ✅
- Test skill autocomplete in slash command menu
- Test argument-hint display
- Test filtering by user-invocable flag

---

## Dependency Graph

```
Phase 1 (Foundation)
  ├── 1.1 Add serde_yaml
  ├── 1.2 Define data model
  └── 1.3 Parse frontmatter
        │
Phase 2 (Discovery)
  ├── 2.1 Discover skills (depends on 1.3)
  └── 2.2 Build registry (depends on 2.1)
        │
Phase 3 (Substitution)
  ├── 3.1 Argument substitution (depends on 1.2)
  └── 3.2 Dynamic context injection (depends on 1.2)
        │
Phase 4 (Invocation)
  ├── 4.1 Invoke skill (depends on 2.2, 3.1, 3.2)
  └── 4.2 Processor integration (depends on 4.1)
        │
Phase 5 (Forking)
  └── 5.1 Subagent execution (depends on 4.1)
        │
Phase 6 (Prompt Integration)
  ├── 6.1 Inject descriptions (depends on 2.2)
  └── 6.2 Agent skill preloading (depends on 6.1)
        │
Phase 7 (TUI Integration)
  ├── 7.1 Dynamic slash commands (depends on 2.2)
  └── 7.2 Skill command routing (depends on 4.2, 7.1)
        │
Phase 8 (Bundled Skills)
  ├── 8.1 Define bundled skills (depends on 1.2)
  └── 8.2 Register bundled skills (depends on 2.2, 8.1)
        │
Phase 9 (Config)
  └── 9.1 Extend config schema (depends on 1.2)
        │
Phase 10 (Testing)
  └── 10.1–10.6 (depends on respective phases)
```

## Notes

- **Hooks system is NOT a prerequisite** — the `hooks` field in skill frontmatter is parsed and stored as raw JSON but not acted upon until the hooks system (SPEC §3.17) is implemented. This avoids blocking skills on hooks.
- **Enterprise scope** is deferred — requires managed settings infrastructure not yet built. The field is defined in the enum but discovery only covers Personal and Project scopes.
- **Skill invocation by the agent** (auto-invocation) requires skill descriptions in the system prompt (Phase 6). Without this, only user-initiated `/name` invocation works.
- **The `instructions` field** in `Config` is currently unused. Skills should NOT depend on it — they have their own injection mechanism.
- **Allowed-tools scoping** for active skills requires temporary permission overrides in the permission checker. This is a Phase 4 concern that interacts with the permission system.
