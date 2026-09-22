---
status: draft
audit:
  - { time: 1789142671, from: "none", to: "draft", actor: "system" }
---
# /prompt — Agent System Prompt Inspector

## Overview

Every ragent agent runs with an assembled system prompt. The primary agent's
prompt is assembled by
`build_system_prompt_with_storage_and_memory_and_config`
(`crates/ragent-agent/src/agent/mod.rs:2462`) and gains its per-turn tool
reference in the session loop (`crates/ragent-agent/src/session/loop_steps.rs`,
via `build_tool_reference_from_defs` / `build_detailed_tool_reference_from_defs`
in `session/prompt_builders.rs`). Sub-agent runs execute the same assembler with
`AgentMode::Subagent`, which switches two mode gates: the `## Sub-Agent
Spawning` section is rendered only in primary mode (`agent/mod.rs:2635`) and
the `## Sub-Agent Completion Protocol` section is rendered only in subagent
mode (`agent/mod.rs:2816`).

There is no way today to inspect any of this from inside ragent. This spec
defines `/prompt`, a read-only TUI slash command that renders:

- `/prompt primary` — the assembled primary-agent system prompt for the
  currently selected agent preset, followed by the effective tool surface
  (the same `## Available Tools` reference block the model receives),
- `/prompt subagent` — the subagent variant of the same assembly: prompt with
  the subagent mode gates applied plus the detailed tool reference filtered
  exactly as a spawned sub-agent would see it (interactive tools removed),
- `/prompt list` — the agent roster (built-in + custom) with mode badges,
- `/prompt <agent-name>` — optional per-agent override for `primary`,
- `/prompt help` — the command's usage reference.

The output mirrors what the LLM actually receives at the start of a session,
without making any LLM call and without modifying any state.

## Scope

In scope:

- `/prompt` and `/prompt help` — usage reference.
- `/prompt primary [agent]` — assembled primary-mode system prompt + compact
  tool reference + header summary (agent, mode, tool count, size).
- `/prompt subagent [agent]` — assembled subagent-mode system prompt + detailed
  tool reference with interactive tools filtered out.
- `/prompt list` — agent roster with built-in/custom and primary/subagent
  badges.
- `/prompt <agent-name>` — optional alias for `primary` with a named agent.
- Rendering the effective tool surface with the same filtering the session loop
  applies: `tool_visibility` hidden tools, agent `allowed_tools` allowlist, and
  (subagent mode only) the interactive-tool filter.
- Non-blocking execution so the TUI never stalls while the prompt is assembled.
- A size cap with an explicit truncation marker to protect the TUI from
  pathological prompt sizes.

Out of scope:

- HTTP/REST endpoints and CLI subcommand surfaces (TUI-only; may be added later).
- Session-specific prompt sections that only exist mid-run (goal-loop section,
  loop tool-set restriction, compaction/TOON sections, initiatives section) —
  `/prompt` shows the canonical start-of-session assembly.
- Editing prompts (`/prompt` is strictly read-only).
- Rendering raw `ToolDefinition` JSON schemas — the tool reference is shown in
  the same markdown form the model receives, not as raw JSON.
- Exempting `/prompt` from the dispatcher's session gate (it follows the
  standard `ensure_session` requirement like every other command).

## Background

Verified types and APIs the implementation will reuse (no re-implementation):

- `ragent_agent::agent::build_system_prompt_with_storage_and_memory_and_config`
  — canonical assembler (`agent/mod.rs:2462`); takes the resolved `AgentInfo`,
  working dir, file tree, skills, git status, README, AGENTS.md, storage,
  memory config/section, and `ragent_config::Config`.
- `ragent_agent::agent::builtin_agents()` — the Arc-cached built-in roster
  (`agent/mod.rs:1136`); custom agents load via `agent::custom` (loaded in
  `agent/mod.rs:2651`). Roster names must come from these sources — README
  preset names (`coder`, `architect`, `debug`, ...) do not all exist as
  built-ins.
- `build_tool_reference_from_defs` / `build_detailed_tool_reference_from_defs`
  — `crates/ragent-agent/src/session/prompt_builders.rs:172/203`; the compact
  and detailed tool-reference renderers. `build_tool_reference_from_defs` is
  currently `pub(crate)`; the implementation SHALL widen it to `pub` (one-line
  visibility change) rather than duplicating the renderer in `ragent-tui`.
- Tool-surface filtering, mirroring `loop_steps.rs:454-529`:
  1. `tool_registry.definitions()` minus tools hidden by `tool_visibility`
     (`ragent_config::Config::effective_hidden_tools` /
     `ToolRegistry::set_hidden`, applied via
     `App::sync_tool_visibility_from_config` — `app/models.rs:2192`),
  2. filtered by the agent's `allowed_tools` allowlist
     (`tool::build_allowed_tool_set` + `tool::is_allowed_tool`,
     `tool/mod.rs:1157/1189`) when non-empty,
  3. subagent mode only: interactive tools
     (`session::permissions::is_interactive_tool`) removed.
- Mode gates inside the assembler: `## Sub-Agent Spawning` rendered only for
  primary mode (`agent/mod.rs:2635`), `## Sub-Agent Completion Protocol` only
  for subagent mode (`agent/mod.rs:2816`); the Question Tool Usage section is
  injected by the session loop for primary mode only (`loop_steps.rs:537`).
- Tool-free agents take the early-return gate in the assembler
  (`agent/mod.rs:2533-2536`): an agent with `max_steps <= 1` gets an assembled
  prompt containing no tool sections at all, and the session processor sends
  zero tools on the wire for it (`processor.rs:1719`). Note: no built-in agent
  declares `max_steps <= 1` today (all built-ins are `1024`); the built-in
  `ask` agent's prompt *says* it has no tools, but production still wires the
  full tool surface to it, so `/prompt` reports `ask` as tool-carrying.
  Tool-free behaviour is exercised with custom agents declaring
  `"max_steps": 1`.

Slash-command plumbing (following the `/toolchain` precedent exactly):

- Registry entry in `SLASH_COMMANDS`
  (`crates/ragent-tui/src/app/state.rs:694`, alphabetical position).
- Suggestions arm in `get_command_suggestions`
  (`crates/ragent-tui/src/app/slash.rs:141-337`) and a parameter hint in the
  hint table (precedent `app/session_ops.rs:1016`).
- Dispatch arm `"prompt" => self.handle_prompt_command(args)` in
  `execute_slash_command_inner`'s `match cmd` (`slash.rs:1821`).
- Handler as `impl App` methods mirroring `handle_toolchain_command`
  (`slash.rs:1194-1300`): lowercased first-word router, `"" | help | --help |
  -h` help route (via the standard `is_help_args` helper, `slash.rs:119`),
  unknown-subcommand correction route.
- Output through `self.append_assistant_text(&report)` with a `From: /prompt
  ...` prefix (`app/event_handler.rs:2489`); the dispatcher's
  `force_new_message = true` (`slash.rs:1807`) guarantees a fresh bubble.
- Long work on `tokio::task::block_in_place` with `[wait] prompt` status set
  first (the `execute_slash_command` wrapper defers the Finished log line while
  status starts with `[wait]`, `slash.rs:1780-1793`).
- Tab-completion regression pattern: `crates/ragent-tui/tests/test_slash_help.rs`
  (toolchain cases at lines 300/819/876).

## Requirements

### FR-001 — Slash command registration (Ubiquitous)

The system SHALL register a `prompt` slash command in the TUI `SLASH_COMMANDS`
table, in the autocomplete suggestion map, in the parameter-hint table, and in
the `/help` command index, so that `/prompt` is recognised by
`execute_slash_command_inner` and appears in the slash menu.

### FR-002 — Dispatch routing (Event-driven)

WHEN the user submits a `/prompt` command, the system SHALL parse the first
whitespace-delimited argument as a subcommand and route: `""`, `help`,
`--help`, `-h` to the help page; `primary` to the primary-mode renderer;
`subagent` to the subagent-mode renderer; `list` to the roster renderer; a
known agent name to the primary-mode renderer for that agent; anything else to
the unknown-subcommand correction.

### FR-003 — Help page (Ubiquitous)

The system SHALL render a `/prompt help` page (also shown for bare `/prompt`)
listing every subcommand, its arguments, and a one-line description, so the
available options are discoverable without reading documentation.

### FR-004 — Primary-mode prompt rendering (Event-driven)

WHEN the user runs `/prompt primary` (or `/prompt <agent-name>`), the system
SHALL assemble the primary-mode system prompt for the target agent by calling
the canonical assembler with the agent's own prompt template, the current
working directory, live file tree, git status, README, AGENTS.md content,
skills, memory, and resolved config, and SHALL display it prefixed by a header
summary stating: agent name, prompt source (built-in or custom), agent mode
(primary), tool count of the effective tool surface, and the rendered size in
characters.

### FR-005 — Subagent-mode prompt rendering (Event-driven)

WHEN the user runs `/prompt subagent`, the system SHALL render the same
assembly with `AgentMode::Subagent` forced, so that:

- the `## Sub-Agent Completion Protocol (MANDATORY - HARD REQUIREMENT)` section
  is present, and
- the `## Sub-Agent Spawning` section is absent, and
- the tool reference is the detailed form
  (`build_detailed_tool_reference_from_defs`) with interactive tools
  (`ask_user`, `question`) excluded, and
- the header summary shows agent mode (subagent).

### FR-006 — Optional agent-name override (Optional)

WHERE a subcommand accepts `[agent]` (i.e. `primary` and `subagent`) or a bare
agent name is used as the subcommand, the system SHALL resolve the named agent
from the built-in roster and the loaded custom agents; the resolution SHALL be
case-insensitive.

### FR-007 — Roster listing (Optional)

WHERE the user requests `/prompt list`, the system SHALL render one line per
available agent (built-ins plus customs) showing the agent name, its mode
badge (`primary`/`subagent`), its source badge (`built-in`/`custom`), and a
truncated one-line description, so the names accepted by FR-006 are visible.

### FR-008 — Effective tool-surface composition (Ubiquitous)

The tool reference rendered by `/prompt` SHALL be composed from the registered
tool registry using the same filters the session loop applies, in this order:
(a) exclude tools hidden by `tool_visibility` configuration; (b) restrict to
the agent's `allowed_tools` allowlist when the allowlist is non-empty; (c) in
subagent mode, exclude interactive tools. The displayed tool count in the
header summary SHALL equal the number of tools in this effective surface.

### FR-009 — Tool-free agent handling (State-driven)

WHILE the resolved agent is tool-free (the assembler's tool gate yields no tool
sections — an agent with `max_steps <= 1`), the renderer SHALL display the
prompt body without any `## Available Tools` section and SHALL state
`(no tools)` in the header summary instead of a tool count.

### FR-010 — Unknown subcommand (Unwanted)

IF the first argument matches none of the known subcommands, help aliases, or
resolvable agent names, THEN the system SHALL render a usage correction
listing the valid subcommands and SHALL NOT display any prompt content.

### FR-011 — Unknown agent name (Unwanted)

IF an agent name argument cannot be resolved against the built-in and custom
roster, THEN the system SHALL render a warning naming the unmatched argument,
list the available agent names, and SHALL NOT display any prompt content.

### FR-012 — Read-only, no-LLM guarantee (Ubiquitous)

The `/prompt` command SHALL make no LLM request, SHALL not write to any file,
SQLite store, or configuration, and SHALL NOT persist its rendered output into
the session message history (output is TUI-display only, following the
`/toolchain` channel pattern), so repeated invocations leave the session and
token usage untouched.

### FR-013 — Output size cap (State-driven)

WHILE the assembled render exceeds a 100,000-character cap, the renderer SHALL
truncate the displayed text at the cap and SHALL append an explicit marker
(`... [truncated: showing N of M characters — use /prompt subagent|primary
with a lighter agent for the full text]`) so the user is never shown a silent
cut-off, and the header summary SHALL always precede the body.

### FR-014 — Non-blocking execution (Event-driven)

WHEN the renderer performs its disk reads and assembly, the system SHALL run
the work off the TUI hot path (blocking-thread execution, following the
`/toolchain` pattern) with the status indicator set to `[wait] prompt` before
the work starts and the rendered status (`prompt: primary` / `prompt:
subagent` / `prompt: list`) after the output lands.

## Non-Functional Requirements

### NFR-001 — Performance

A `/prompt primary` or `/prompt subagent` render SHALL complete within 5
seconds on a typical workstation (the assembly is local disk reads and string
building only; no network, no LLM).

### NFR-002 — Fidelity

The displayed prompt body SHALL be the same string the corresponding session
would send to the LLM as its system prompt (before per-turn session-specific
sections), and the tool reference SHALL be the same generated block the model
receives — no re-formatted or hand-maintained copy.

### NFR-003 — Determinism

Repeated `/prompt` invocations in the same working directory and configuration
SHALL produce identical output (barring concurrent file/git changes feeding
the context sections).

### NFR-004 — No new duplication

The tool-reference rendering SHALL reuse `prompt_builders` functions (widening
visibility where required); the renderer SHALL NOT duplicate the assembly or
filtering logic from `loop_steps.rs`.

## Acceptance Criteria

- `/prompt` and `/prompt help` render the help page listing `help`, `primary`,
  `subagent`, `list`, and the agent-name alias.
- `/prompt primary` renders the current agent's primary-mode prompt with a
  header summary and an `## Available Tools` section whose tool count matches
  the header.
- `/prompt subagent` renders the completion-protocol section, omits the
  spawning section, shows the detailed tool reference without `ask_user` or
  `question`, and is labelled subagent in the header.
- A tool-free agent (custom definition with `"max_steps": 1`) renders its
  prompt with `(no tools)` in the header and no tool reference section. The
  built-in `ask` agent is tool-carrying in production (full wire surface,
  `processor.rs:1719`) and is truthfully reported with a tool count.
- `/prompt list` renders the built-in + custom roster with mode badges.
- `/prompt nosuchthing` warns with valid subcommands; `/prompt primary
  nosuchagent` warns with valid agent names; neither displays prompt content.
- No LLM call, no state change, and no session-history mutation occurs for any
  `/prompt` invocation.