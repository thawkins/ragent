# Implementation Plan — /prompt Agent System Prompt Inspector

Spec: `specs/prompts/SPEC.md`

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Widen `build_tool_reference_from_defs` to `pub` (no logic change) | FR-008, NFR-004 | S | High | completed | — |
| T-002 | Create `app/prompt.rs` module with report data structs | FR-004, FR-005 | M | High | completed | — |
| T-003 | Implement agent resolution (case-insensitive, built-in + custom) with miss warning | FR-006, FR-011 | S | High | completed | T-002 |
| T-004 | Implement effective-tool-surface filter chain (visibility, allowlist, subagent interactive) | FR-008, FR-005 | M | Critical | completed | T-001, T-002 |
| T-005 | Implement primary-mode assembly call + header summary renderer | FR-004 | M | Critical | completed | T-002, T-004 |
| T-006 | Implement subagent-mode rendering (mode-forced assembly, detailed reference) | FR-005 | M | Critical | completed | T-005 |
| T-007 | Implement tool-free agent path with `(no tools)` marker | FR-009 | S | High | completed | T-005 |
| T-008 | Implement output size cap + explicit truncation marker | FR-013 | S | Medium | completed | T-005 |
| T-009 | Implement `/prompt list` roster renderer with mode + source badges | FR-007 | S | Medium | completed | T-002 |
| T-010 | Implement `/prompt help` page | FR-003 | S | High | completed | T-002 |
| T-011 | Implement unknown-subcommand usage-correction renderer | FR-010 | S | High | completed | T-010 |
| T-012 | Register `prompt` in `SLASH_COMMANDS`, suggestions, parameter-hint, and `/help` | FR-001 | S | High | completed | — |
| T-013 | Add `prompt` dispatch arm in `execute_slash_command_inner` + subcommand router | FR-002 | S | High | completed | T-012 |
| T-014 | Wire output channel (`append_assistant_text`, `From: /prompt` prefix, status strings) | FR-012 | S | High | completed | T-013 |
| T-015 | Non-blocking execution with `[wait] prompt` status | FR-014, NFR-001 | M | High | completed | T-013 |
| T-016 | Verify read-only/no-LLM guarantee (no writes, no session-history mutation) | FR-012 | S | High | completed | T-014 |
| T-017 | Unit tests: resolution, filter chain, mode gates, roster, truncation | FR-005–FR-009, FR-011, FR-013 | M | High | completed | T-003, T-006, T-007, T-008, T-009 |
| T-018 | Regression tests: registration, autocomplete, tab-completion pass | FR-001, FR-002 | S | Medium | completed | T-012, T-013 |
| T-019 | Manual test pass against `TESTPLAN.md` | FR-001–FR-014 | S | Medium | completed | T-010, T-011, T-013, T-017 |
## Notes

- All dispatch/UI work follows the `/toolchain` precedent exactly:
  registry entry in `crates/ragent-tui/src/app/state.rs` (`SLASH_COMMANDS`,
  ~line 694, alphabetical position), suggestions arm in
  `get_command_suggestions` (`slash.rs:141-337`), dispatch arm in the
  `match cmd` at `slash.rs:1821`, handler as `impl App` methods mirroring
  `handle_toolchain_command` (`slash.rs:1194-1300`).
- The rendering engine lives in a new focused module
  `crates/ragent-tui/src/app/prompt.rs` (registered in `app.rs` alongside
  `pub mod toolchain;`) so assembly, filtering, and rendering are testable
  independently of the TUI.
- `ragent-tui` already depends on `ragent-agent` (it calls
  `ragent_agent::Config::load()` and drives the session processor), so
  `build_system_prompt_with_storage_and_memory_and_config`, `builtin_agents()`,
  and the tool-filter helpers are available without a new crate edge. The only
  `ragent-agent` change is the T-001 visibility widening of
  `build_tool_reference_from_defs` (`pub(crate)` → `pub`).
- Subagent-mode fidelity requires forcing the mode gate exactly like a spawned
  run: the resolved `AgentInfo` passed to the assembler must carry
  `AgentMode::Subagent` (this is what drives the `## Sub-Agent Spawning` /
  `## Sub-Agent Completion Protocol` gates at `agent/mod.rs:2635/2816`), and
  the tool filter chain must apply `is_interactive_tool` only in subagent
  mode, mirroring `loop_steps.rs:454-529`.
- Tool-free agents (e.g. `ask`) are covered by the assembler's own gate
  (`agent/mod.rs:2533-2536`); the renderer only needs to detect the empty tool
  section and report `(no tools)`.
- The context inputs (file tree, git status, README, AGENTS.md) are read from
  the live working directory exactly as the session loop does; this makes
  `/prompt` output match the model's actual first-turn system prompt (NFR-002)
  and is why NFR-001's 5 s budget is comfortable.
- Session-specific sections (goal loop, loop tool-set, initiatives) are
  intentionally excluded — `/prompt` renders the canonical start-of-session
  assembly, not a mid-run snapshot.
- FR-012's no-session-history rule follows the `/toolchain` channel pattern:
  output lands in the chat window via `append_assistant_text` but is not
  persisted into the session message store, so `/cost` and token usage are
  unaffected.
- No HTTP/CLI surface in this spec; the module boundary (pure resolve/filter/
  render functions taking plain inputs) keeps a later `ragent prompt` CLI or
  REST endpoint cheap to add.