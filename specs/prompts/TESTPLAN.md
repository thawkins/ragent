---
status: draft
---
# Manual Test Plan — /prompt Agent System Prompt Inspector

Spec: `specs/prompts/SPEC.md`

All test cases are executed by hand in the ragent TUI. No automated test code
is referenced here; the goal is to verify the rendered behaviour a human sees.

## Prerequisites

Before running the test cases:

1. Build the debug binary and start the TUI from a project directory that
   contains an `AGENTS.md` and a README (the ragent repository itself is
   suitable — `~/Projects/ragent`):

   ```bash
   cd ~/Projects/ragent
   cargo build
   target/debug/ragent
   ```

2. Confirm the TUI home screen shows the working directory and that a
   provider/model is selectable. The `/prompt` command makes no LLM calls, so
   no API key is required; a local provider (Ollama) or a configured cloud
   provider both work, and an unconfigured provider is acceptable for every
   test case in this plan.
3. Ensure no other session activity (streaming response, autopilot run,
   background agent) is in progress while testing, so status-bar
   observations are unambiguous.
4. Note the currently selected agent shown on the home screen / status bar
   (default: `coder` preset family agent as configured in `ragent.json`).
   Record it before starting so the `/prompt primary` header can be compared.
5. Optional contrast cases: temporarily set `tool_visibility.teams` (or any
   family) to `false` in `ragent.json` for TC-008, and have a project-local
   custom agent in `.ragent/agents/` for TC-009. Restore the config afterwards
   (see Cleanup).

## Test Cases

### TC-001 — Bare `/prompt` shows the help page

Preconditions: TUI running, active session established.

Steps:
1. Type `/prompt` in the message input. Observe the autocomplete menu offers
   `prompt` (with the parameter hint) as you type `/pro`.
2. Press `Tab` to accept the suggestion, then `Enter`.
3. Read the assistant output bubble.

Test data: input `/prompt` (bare, no subcommand).

Expected results:
- A fresh assistant bubble appears with the prefix `From: /prompt` containing
  the help page.
- The help page lists every subcommand: `help`, `primary [agent]`,
  `subagent [agent]`, `list`, and the agent-name alias, each with a one-line
  description.
- No prompt body text (no `## Working Directory`, no `## Available Tools`)
  is displayed.
- The status bar shows a `prompt`-related status (e.g. `prompt: help`).

### TC-002 — `/prompt help` alias routes to the same help page

Preconditions: TUI running, active session established.

Steps:
1. Type `/prompt help` and press `Enter`.
2. Compare the rendered output with the TC-001 output (scroll back with the
   log panel if needed).
3. Repeat with `/prompt --help` and `/prompt -h`.

Test data: `/prompt help`, `/prompt --help`, `/prompt -h`.

Expected results:
- All three spellings render the identical help page (same content as TC-001).
- None of them renders a prompt body.

### TC-003 — `/prompt primary` renders the current agent's primary prompt with tools

Preconditions: TUI running, active session, current agent known (recorded in
Prerequisites step 4).

Steps:
1. Type `/prompt primary` and press `Enter`.
2. Read the header summary line at the top of the output.
3. Scroll through the body: confirm the agent's own opening prompt text, the
   `## Working Directory`, `## Project Structure` (file tree), `## Project
   Guidelines (AGENTS.md)`, `## Git Context`, and `## README` sections appear
   with plausible live content (the file tree should resemble this repo).
4. Locate the `## Available Tools` section and spot-check familiar tool names
   (`bash`, `read`, `edit`, `grep`, `codeindex_search`) appear.
5. Count a few listed tools and compare the total against the tool count in
   the header summary.

Test data: `/prompt primary`.

Expected results:
- The header summary states the agent name (matching the currently selected
  agent), source badge (`built-in` or `custom`), mode (`primary`), an effective
  tool count greater than zero, and the rendered size in characters.
- The prompt body contains the live context sections with current content.
- The `## Available Tools` section is present and the number of listed tools
  equals the header tool count.
- No `## Sub-Agent Completion Protocol` section appears anywhere in the body.

### TC-004 — `/prompt subagent` renders the subagent variant with detailed tools

Preconditions: TUI running, active session.

Steps:
1. Type `/prompt subagent` and press `Enter`.
2. Read the header summary line.
3. Search the body for the `## Sub-Agent Completion Protocol (MANDATORY -
   HARD REQUIREMENT)` heading and confirm its content demands
   `agent_complete(summary)` as the final action.
4. Confirm the `## Sub-Agent Spawning` section is ABSENT.
5. Inspect the tool reference section: confirm it is the detailed form — each
   tool listed as a `### \`tool-name\` heading with parameter bullets
   (`name` (`type`) — description) rather than the compact one-line list.
6. Confirm neither `ask_user` nor `question` appears anywhere in the tool
   reference.

Test data: `/prompt subagent`.

Expected results:
- The header summary shows mode `subagent`.
- The completion-protocol section is present; the spawning section is absent.
- The tool reference is detailed (per-tool headings + parameter bullets) and
  contains no interactive tools.
- The tool count in the header matches the number of `###` tool headings.

### TC-005 — `/prompt primary <tool-free-agent>` shows `(no tools)`

Preconditions: TUI running, active session.

Note: tool-free is defined by the assembler's own tool gate
(`agent/mod.rs:2533-2536`): an agent with `max_steps <= 1` renders no tool
sections. No built-in agent satisfies this today (all declare
`max_steps: 1024`), so this test uses a tool-free custom agent — e.g. a
`.ragent/agents/one-shot.md` profile with `"max_steps": 1` in the frontmatter
(restart ragent or re-run `/agents reload` so the custom agent loads). The
built-in `ask` agent is NOT tool-free by this definition: production wires the
full tool surface to it (`processor.rs:1719`), so `/prompt primary ask`
truthfully reports a tool count.

Steps:
1. Install a tool-free custom agent (see note) and run `/prompt list`
   (TC-007) to confirm its name appears in the roster.
2. Type `/prompt primary <tool-free-agent-name>` and press `Enter`.
3. Read the header summary.
4. Scroll the entire body and confirm there is no `## Available Tools`
   section.

Test data: `/prompt primary <tool-free-agent-name>` (custom agent with
`max_steps: 1`).

Expected results:
- The header summary shows the tool-free agent and states `(no tools)` instead
  of a tool count.
- The body contains no tool reference section.
- The command completes without a warning or error.

### TC-006 — Agent-name alias routes to the primary renderer

Preconditions: TUI running, active session; a known agent name from TC-003.

Steps:
1. Type `/prompt <agent-name>` where `<agent-name>` is the agent used in
   TC-003 (e.g. `/prompt general` or whatever TC-003's header showed), using
   a different capitalisation if the name has uppercase letters.
2. Press `Enter` and compare the output with TC-003.

Test data: `/prompt <agent-name>` (case-varied spelling of a valid agent).

Expected results:
- The output is equivalent to `/prompt primary` for that agent (same header
  fields, same body), proving the alias routes to the primary renderer.
- Case-insensitive matching resolves the agent (no unknown-agent warning).

### TC-007 — `/prompt list` renders the agent roster

Preconditions: TUI running, active session.

Steps:
1. Type `/prompt list` and press `Enter`.
2. Verify one line appears per built-in agent (at minimum: `ask`, `general`,
   `build`, `plan`, `explore`, `title`, `summary`).
3. Check each line carries a mode badge (`primary` or `subagent`) and a source
   badge (`built-in` or `custom`), plus a truncated one-line description.
4. Cross-check TC-005: the agent you used there is listed with its mode badge
   matching the `(no tools)` behaviour.

Test data: `/prompt list`.

Expected results:
- The roster includes all built-in agents and any loaded custom agents.
- Every line has both badges; descriptions are single-line (truncated if long).
- No prompt bodies are rendered by this subcommand.

### TC-008 — Effective tool surface honours `tool_visibility`

Preconditions: TUI running, active session; a `tool_visibility` switch set to
`false` in `ragent.json` (e.g. `"teams": false`); ragent restarted after the
config change.

Steps:
1. Run `/prompt primary` and record the tool count and a couple of tool names
   from the hidden family (e.g. `team_create`).
2. Flip the switch to `true` in `ragent.json`, restart ragent, re-establish a
   session, and run `/prompt primary` again.
3. Compare the two outputs.

Test data: `ragent.json` with `"tool_visibility": { "teams": false }` and then
`"teams": true`.

Expected results:
- With the switch off, the hidden family's tools are absent from the
  `## Available Tools` section and the header tool count is lower.
- With the switch on, the tools reappear and the count increases.
- The header count always matches the rendered list length.

### TC-009 — Custom agents appear in the roster and resolve

Preconditions: TUI running; a project-local custom agent exists at
`.ragent/agents/<name>.json` (or `.md`), ragent restarted.

Steps:
1. Run `/prompt list` and confirm the custom agent appears with the `custom`
   source badge.
2. Run `/prompt primary <custom-agent-name>` and read the header summary.
3. Optionally run `/prompt subagent <custom-agent-name>` if the custom agent
   declares subagent mode.

Test data: one custom agent definition file in `.ragent/agents/`.

Expected results:
- The roster line for the custom agent shows source `custom`.
- `/prompt primary <custom-agent-name>` renders that agent's own prompt
  template (not a built-in's) with its effective tool surface.
- Case-insensitive name matching works.

### TC-010 — Unknown subcommand shows a usage correction

Preconditions: TUI running, active session.

Steps:
1. Type `/prompt frobnicate` and press `Enter`.
2. Read the output.

Test data: `/prompt frobnicate`.

Expected results:
- A usage correction is rendered listing the valid subcommands (`help`,
  `primary`, `subagent`, `list`, agent-name alias).
- No prompt body content is displayed.
- No crash; the TUI remains responsive.

### TC-011 — Unknown agent name is rejected without prompt output

Preconditions: TUI running, active session.

Steps:
1. Type `/prompt primary nosuchagent` and press `Enter`.
2. Read the output.
3. Repeat with a bare unknown name that is not a subcommand but is not an
   agent either (e.g. `/prompt nosuchagent`) to confirm it takes the
   unknown-agent path rather than unknown-subcommand (TC-010).

Test data: `/prompt primary nosuchagent`, `/prompt nosuchagent`.

Expected results:
- The warning names the unmatched argument (`nosuchagent`).
- The available agent names are listed (the roster from TC-007).
- No prompt content is displayed in either case.

### TC-012 — Output is read-only: no session history, cost, or state change

Preconditions: TUI running, active session; `/cost` and the Context panel
observable.

Steps:
1. Run `/cost` (or observe the status bar token counters) and note the token
   usage; note the message count shown in the Context panel.
2. Run `/prompt primary`, then `/prompt subagent`, then `/prompt list`.
3. Run `/cost` again and compare. Check the Context panel message count.
4. Run any `/prompt` subcommand three times in a row and compare the outputs
   byte-for-byte (scroll back through the log).

Test data: three consecutive `/prompt primary` invocations.

Expected results:
- Token usage is unchanged by `/prompt` (no LLM calls).
- The Context panel message count is unchanged (command output is
  display-only and not persisted into session history).
- The three consecutive outputs are identical (determinism), apart from
  position in the chat log.

### TC-013 — Non-blocking execution under a large working tree

Preconditions: TUI running, active session, ragent launched in a large
repository (e.g. the ragent workspace itself).

Steps:
1. Type `/prompt primary` and press `Enter`.
2. Immediately after pressing Enter, observe the status bar and attempt to
   scroll or type in the input.
3. Wait for the output to land and check the final status.

Test data: `/prompt primary` in a large repository.

Expected results:
- The status bar shows `[wait] prompt` (or equivalent wait indicator) while
  the work runs.
- The UI remains responsive during the wait (no frozen input).
- When the output lands, the status shows the rendered state (e.g.
  `prompt: primary`) and then auto-expires to `ready`.
- The output completes within 5 seconds.

### TC-014 — Oversized render truncates with an explicit marker

Preconditions: TUI running, active session, and — if achievable — a working
directory whose AGENTS.md/README/file tree pushes the assembled primary prompt
above the 100,000-character cap. If the natural working directory cannot reach
the cap, temporarily append a large generated block (~150 KB of markdown) to
the project `AGENTS.md` for this test and remove it afterwards (see Cleanup).

Steps:
1. Run `/prompt primary`.
2. Inspect the very end of the rendered output.
3. Inspect the header summary.

Test data: an `AGENTS.md` enlarged by ~150 KB of filler markdown.

Expected results:
- The displayed body stops at the size cap and the last line is the explicit
  truncation marker stating the shown and total character counts.
- The header summary appears above the body (not truncated away).
- After removing the filler, `/prompt primary` renders untruncated again.

### TC-015 — Slash-menu autocomplete and `/help` integration

Preconditions: TUI running, empty input.

Steps:
1. Type `/p` and observe the autocomplete menu; `prompt` should be listed.
2. Continue typing `/prompt` and press `Tab`; observe the parameter hint
   (e.g. `[help|primary [agent]|subagent [agent]|list]`) offered.
3. Type `/prompt ` (with trailing space) and observe the subcommand
   suggestions (`help`, `primary`, `subagent`, `list`).
4. Clear the input, type `/help`, and press `Enter`; confirm `/prompt` appears
   in the command index with its one-line description.
5. Select `primary` from the subcommand suggestions and press `Enter` to
   complete the command; confirm it executes as TC-003.

Test data: typed prefixes `/p`, `/prompt`, `/prompt `; `/help`.

Expected results:
- `prompt` appears in the slash menu from a `/p` prefix.
- Tab completion accepts the trigger; a parameter hint is shown.
- Subcommand suggestions for `/prompt ` list `help`, `primary`, `subagent`,
  `list`.
- The `/help` index includes the `prompt` row.

### TC-016 — Session gate: `/prompt` requires an active session

Preconditions: TUI freshly started with no session open (home screen, no
prior message).

Steps:
1. Without sending any user message, type `/prompt help` and press `Enter`.
2. Observe the behaviour (expected: the standard session-gate prompt, same as
   other non-exempt commands).
3. Send any short user message to establish a session (e.g. `hi`), wait for
   the response to settle.
4. Run `/prompt help` again.

Test data: `/prompt help` before and after session creation.

Expected results:
- Before a session exists, `/prompt` behaves like every other session-gated
  command (prompts to start a session / is refused per the standard
  `ensure_session` behaviour) and does NOT render the help page.
- After the session exists, the help page renders normally.

## Cleanup

1. If TC-008 toggled `tool_visibility` switches, restore `ragent.json` to its
   original state and restart ragent once to confirm restoration.
2. If TC-009 added a custom agent file, remove it from `.ragent/agents/` and
   restart ragent (or leave it if it is a permanent fixture of the project).
3. If TC-014 appended filler to `AGENTS.md`, remove the filler block and
   verify with `git diff AGENTS.md` that the file is byte-identical to HEAD.
4. Close the TUI. Optional: run `/config list` or inspect `ragent.json` to
   confirm no configuration was modified by any test step.
5. No other teardown is required — `/prompt` performs no writes by design
   (FR-012).