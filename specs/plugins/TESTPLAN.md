---
status: draft
---

# Manual Test Plan: Plugin System - `/plugins` Slash-Command Family and JavaScript Plugin Runtime

**Spec:** [SPEC.md](SPEC.md) · **Plan:** [PLAN.md](PLAN.md)

This is a **manual** test plan. Every case is executed by a human in a real terminal
session. Where a case needs a fixture plugin, a configured LLM provider, or network
access, the prerequisite is listed before the steps. This document contains no automated
test code.

## Prerequisites

1. **Build the binary.** Run `cargo build` (debug is sufficient; allow up to 1000
   seconds). Confirm `./target/debug/ragent --version` runs and reports the current
   version.
2. **Configure an LLM provider.** Acceptance criteria involving tool invocation from the
   agent loop need a model. Set the API key for your configured provider in the
   environment, or point ragent at a local Ollama model. Confirm a plain
   `ragent run "say hello"` returns a response.
3. **Prepare a scratch root.** Create `~/scratch/plugins-tests/` and run every TUI
   session from inside it so the project plugin store is `~/scratch/plugins-tests/.ragent/plugins/`.
4. **Stage the fixture plugins.** Copy the fixture plugins from the repository into the
   scratch root as *add sources* (do not pre-install them):
   - `~/scratch/plugins-tests/fixtures-src/codex-weather/` — Codex dialect
     (`codex-plugin.json`), entry `main.js`, one tool `get_weather` returning canned
     JSON, declared permission list empty.
   - `~/scratch/plugins-tests/fixtures-src/claude-todo/` — Claude dialect
     (`.claude-plugin/plugin.json`), entry `index.js`, one tool `add_todo` and one
     slash command `todo-add`.
   - `~/scratch/plugins-tests/fixtures-src/loop-forever/` — Codex dialect whose entry
     contains `while (true) {}`.
   - `~/scratch/plugins-tests/fixtures-src/escape-attempt/` — Codex dialect whose tool
     handler calls `ragent.plugin.read_text_file("../../outside.txt")`.
   - `~/scratch/plugins-tests/fixtures-src/future-api/` — Codex dialect whose manifest
     declares `api_version: 99`.
   - `~/scratch/plugins-tests/fixtures-src/bad-manifest/` — a folder with a
     syntactically invalid `codex-plugin.json` (truncated JSON).
   - `~/scratch/plugins-tests/fixtures-src/claude-mcp/` — Claude dialect with an
     MCP-server section in the manifest (unsupported-capability fixture).
   - `~/scratch/plugins-tests/fixtures-src/collider/` — Codex dialect declaring a tool
     named `bash` (built-in collision fixture).
5. **Prepare a packaged fixture.** Zip `codex-weather` into
   `~/scratch/plugins-tests/fixtures-src/codex-weather.zip` and tar `claude-todo` into
   `~/scratch/plugins-tests/fixtures-src/claude-todo.tar.gz`.
6. **Prepare a poisoned archive.** Build `evil.zip` containing one entry whose path is
   `../../escape.js` to exercise archive path-traversal protection.
7. **For URL add cases (TC-009, offline-safe alternative provided):** serve
   `codex-weather.zip` over a local HTTPS endpoint, or use any reachable HTTPS URL to a
   small zip. If you cannot produce an HTTPS URL, skip TC-009 and record it as not run.
8. **Preserve config.** Back up `~/scratch/plugins-tests/.ragent/ragent.json` (if it
   exists) before starting so temporary flags can be reverted in Cleanup.

## Test Cases

### TC-001 — `/plugins help` and usage fallbacks

**Requirement:** FR-006, FR-014

**Preconditions:** ragent TUI running from `~/scratch/plugins-tests/`.

**Steps:**
1. Type `/plugins ` into the message input and observe the autocomplete menu.
2. Confirm `list`, `add`, `remove`, `enable`, `disable`, `help`, and `test` appear;
   press `Esc` to dismiss the menu.
3. Type `/plugins help` and press `Enter`; read the usage text.
4. Type `/plugins` (no subcommand) and press `Enter`; read the usage text.
5. Type `/plugins bogus` and press `Enter`; read the usage text.

**Test data:** the literal strings `/plugins help`, `/plugins`, `/plugins bogus`.

**Expected results:**
- All six subcommands appear in the autocomplete menu and in the usage block.
- The usage block documents: `list` (with `--verbose`), `add <source> [--force]`,
  `remove <pluginid>`, `enable <pluginid>`, `disable <pluginid>`, and `test <pluginid>`,
  plus the accepted `<source>` forms (local directory, local zip/tar.gz, https URL).
- All three invocations print the same usage block, and a second terminal shows
  `ls -la ~/scratch/plugins-tests/` unchanged — no files or directories created.
- Output contains no non-ASCII characters and includes a `From: /plugins` prefix or the
  equivalent session attribution, matching `/spec` family conventions.

### TC-002 — Add a Codex plugin from a local directory

**Requirement:** FR-002, FR-007, FR-010, FR-023

**Preconditions:** TUI running; `fixtures-src/codex-weather/` staged; the project plugin
store empty.

**Steps:**
1. Note the current time; this is a no-code-execution check.
2. Type `/plugins add ~/scratch/plugins-tests/fixtures-src/codex-weather` and press
   `Enter`.
3. Approve any permission prompt shown.
4. Read the report line in the message window.
5. In a second terminal, run `ls -R ~/scratch/plugins-tests/.ragent/plugins/`.
6. Run `/plugins list`.

**Test data:** source path `~/scratch/plugins-tests/fixtures-src/codex-weather`.

**Expected results:**
- The report names plugin id `codex-weather`, dialect `codex`, version matching the
  fixture manifest, and states the plugin is enabled and loads at the next session start.
- The store now contains `.ragent/plugins/codex-weather/` with `codex-plugin.json` and
  `main.js`, and `.ragent/plugins/_state.json` records `codex-weather` enabled.
- `/plugins list` shows one row: id `codex-weather`, state `enabled`, 1 tool and 0
  commands contributed.
- No evidence of code execution: the fixture's entry contains an immediate
  `ragent.message.info("entry ran")` line, and no such message appears in the message
  window at any point in this case.

### TC-003 — Add a Claude plugin from a packaged archive

**Requirement:** FR-002, FR-007, FR-010

**Preconditions:** TUI running; `claude-todo.tar.gz` staged.

**Steps:**
1. Type `/plugins add ~/scratch/plugins-tests/fixtures-src/claude-todo.tar.gz` and press
   `Enter`.
2. Read the report line.
3. Run `/plugins list`.
4. Inspect `.ragent/plugins/claude-todo/.claude-plugin/plugin.json` exists.

**Test data:** source path `~/scratch/plugins-tests/fixtures-src/claude-todo.tar.gz`.

**Expected results:**
- The report names id `claude-todo`, dialect `claude`.
- `/plugins list` now shows two rows: `codex-weather` (enabled) and `claude-todo`
  (enabled), with `claude-todo` showing 1 tool and 1 command.
- The extracted directory contains the `.claude-plugin/` manifest path.

### TC-004 — Refusal cases for `/plugins add`

**Requirement:** FR-010, FR-026

**Preconditions:** `codex-weather` already installed from TC-002; `evil.zip` and
`bad-manifest/` staged.

**Steps:**
1. Type `/plugins add ~/scratch/plugins-tests/fixtures-src/codex-weather` and press
   `Enter` (duplicate id, no `--force`).
2. Type `/plugins add http://insecure.example.com/plugin.zip` and press `Enter`
   (non-https URL).
3. Type `/plugins add ~/scratch/plugins-tests/evil.zip` and press `Enter` (poisoned
   archive).
4. Type `/plugins add ~/scratch/plugins-tests/fixtures-src/bad-manifest` and press
   `Enter` (invalid manifest JSON).
5. After each, run `/plugins list`.
6. Confirm `~/scratch/plugins-tests/escape.js` and `~/scratch/escape.js` do not exist.

**Expected results:**
- Step 1 refuses, naming the existing plugin id and suggesting `--force`; store
  unchanged.
- Step 2 refuses with a message that only `https://` URLs are accepted; no download
  attempted.
- Step 3 refuses with a path-traversal message naming the offending entry; no file
  outside the store is written (verified in step 6).
- Step 4 fails with a manifest-parse report including the JSON error; `bad-manifest` may
  appear in `/plugins list` with state `errored`, may not appear at all, but in no case
  executes code.
- The ragent process remains responsive throughout.

### TC-005 — Enable a Codex plugin and invoke its tool from the agent loop

**Requirement:** FR-004, FR-005, FR-008, FR-011, FR-016

**Preconditions:** `codex-weather` installed and enabled (TC-002); LLM provider
configured.

**Steps:**
1. Type `/plugins enable codex-weather` and press `Enter`.
2. Read the report: expected state `loaded`, a declared-permissions note, and the tool
   name `plugin_codex-weather_get_weather`.
3. Run `/plugins list` and confirm the row now shows state `loaded` and tool
   `plugin_codex-weather_get_weather`.
4. Type the agent prompt: `Use the plugin_codex-weather_get_weather tool with city set
   to "Helsinki" and tell me what it returned.` Press `Enter`.
5. When the permission prompt for `plugin:codex-weather` appears, approve it.
6. Read the tool-result block in the message window.

**Test data:** tool argument JSON `{ "city": "Helsinki" }` (via the natural-language
prompt).

**Expected results:**
- Enable completes without a restart and reports `loaded`.
- The tool executes; the message window shows the fixture's canned JSON result
  containing `"city": "Helsinki"` and the fixture's fixed conditions string.
- The invocation appears in the log panel as a step-numbered tool call with the
  marshalled JSON arguments.
- `claude-todo` remains `enabled` (from its install) and contributes nothing until
  loaded.

### TC-006 — Disable a plugin mid-session and verify deregistration

**Requirement:** FR-012, FR-016

**Preconditions:** `codex-weather` loaded from TC-005.

**Steps:**
1. Type `/plugins disable codex-weather` and press `Enter`.
2. Read the report; note the deregistration counts.
3. Run `/plugins list`.
4. Type the agent prompt: `Call plugin_codex-weather_get_weather with city "Oslo".`
   Press `Enter`.

**Expected results:**
- The disable report states 1 tool and 0 commands deregistered.
- `/plugins list` shows `codex-weather` with state `disabled`.
- The agent reports the tool as unavailable / unknown; no plugin code executes and no
  fixture JSON appears.

### TC-007 — Plugin-contributed slash command

**Requirement:** FR-004, FR-012, FR-024

**Preconditions:** `claude-todo` installed (TC-003).

**Steps:**
1. Type `/plugins enable claude-todo` and press `Enter`; confirm state `loaded` and one
   command reported.
2. Type `/todo-` into the message input and confirm the autocomplete menu offers
   `/todo-add`.
3. Type `/todo-add buy milk` and press `Enter`.
4. Read the message emitted by the command handler.
5. Type `/plugins disable claude-todo`, then type `/todo-` again.

**Test data:** command arguments `buy milk`.

**Expected results:**
- A new slash command `/todo-add` is contributed while the plugin is enabled and appears
  in autocomplete.
- Invoking it prints the fixture's confirmation text containing `buy milk`.
- After disable, `/todo-add` no longer appears in autocomplete and invoking it falls
  back to the unknown-command behaviour.

### TC-008 — `/plugins test` isolated harness

**Requirement:** FR-013, FR-026

**Preconditions:** `claude-todo` installed; note the tool list before the test (run
`/plugins list`).

**Steps:**
1. Type `/plugins test claude-todo` and press `Enter`.
2. Read the step table as it completes.
3. After the report finishes, run `/plugins list` again and compare with the earlier
   snapshot.
4. Type `/todo-add test` and confirm the command is still unknown.

**Expected results:**
- The report contains one `[ ok ]` line each for: manifest validation, version check,
  entry execution, and sample invocation of `add_todo`, plus a summary line, each with a
  wall-clock time in milliseconds.
- The live session is untouched: `claude-todo`'s state is unchanged afterwards, and
  `/todo-add` remains as it was before the run.
- The sample invocation used schema-valid generated arguments (shown in the report).

### TC-009 — Add from an HTTPS URL (skip if offline; then record as not run)

**Requirement:** FR-010

**Preconditions:** an HTTPS URL serving `codex-weather.zip`; the plugin not currently
installed (remove it first via `/plugins remove codex-weather` if needed).

**Steps:**
1. Type `/plugins add https://<host>/codex-weather.zip` and press `Enter`.
2. Read the report and confirm id `codex-weather`, dialect `codex`.
3. Run `/plugins list` and inspect the store directory.

**Expected results:**
- Download and extraction succeed; the store contains the unpacked plugin; the report
  and `/plugins list` row match TC-002.
- If the URL is unreachable, the command fails with a network error report, changes no
  state, and leaves no partial directory under `.ragent/plugins/`.

### TC-010 — Sandbox: infinite-loop entry is contained

**Requirement:** FR-017, FR-026

**Preconditions:** `loop-forever` fixture installed; config default `max_entry_ms`
(10000) in effect.

**Steps:**
1. Note the wall clock.
2. Type `/plugins enable loop-forever` and press `Enter`.
3. While waiting, press `Esc`-safe keys, scroll the message window, and confirm the TUI
   remains responsive (cursor moves, status bar updates).
4. Wait for the run to terminate (expect roughly 10-15 seconds).
5. Read the terminal report; run `/plugins list`.

**Expected results:**
- Enable fails with cause class `entry` / timeout, at approximately the configured entry
  budget (within a few seconds of 10 s), not indefinitely.
- The TUI stayed usable for the whole wait.
- `/plugins list` shows `loop-forever` as `errored` with a timeout cause.
- The ragent process does not crash, hang permanently, or consume unbounded CPU after
  termination (check `top` in another terminal).

### TC-011 — Sandbox: filesystem escape attempt is denied

**Requirement:** FR-018

**Preconditions:** `escape-attempt` installed and enabled; a sentinel file
`~/scratch/plugins-tests/outside.txt` exists containing `SENTINEL-DO-NOT-READ`.

**Steps:**
1. Type `/plugins enable escape-attempt`, confirm `loaded`.
2. Prompt the agent: `Call plugin_escape-attempt_read_outside.` and approve the
   permission prompt.
3. Read the tool result block.

**Expected results:**
- The tool call fails with a denied/escape error naming the path-restriction rule.
- The sentinel string `SENTINEL-DO-NOT-READ` never appears in the message window or the
  tool result.
- The plugin remains loaded (a single denied call is a tool failure, not an entry
  failure).

### TC-012 — Host-API version refusal

**Requirement:** FR-019

**Preconditions:** `future-api` fixture installed (declares `api_version: 99`).

**Steps:**
1. Type `/plugins enable future-api` and press `Enter`; read the report.
2. Type `/plugins test future-api` and press `Enter`; read the step table.

**Expected results:**
- Enable refuses with a version-mismatch report naming declared `99` and host `1`; the
  plugin does not load.
- The test report shows `[fail]` at the version-check step, skips entry execution, and
  ends with a failure summary.

### TC-013 — Unsupported capabilities are reported, not dropped

**Requirement:** FR-025

**Preconditions:** `claude-mcp` fixture installed.

**Steps:**
1. Type `/plugins list` and read the row and summary for `claude-mcp`.
2. Type `/plugins test claude-mcp` and read the report's notices.

**Expected results:**
- `/plugins list` shows a notice that `claude-mcp` requested unsupported capabilities,
  naming the MCP-server section.
- The test report repeats the notice; the plugin still loads/tests for its supported
  surface.

### TC-014 — Tool-name collision is rejected

**Requirement:** FR-024

**Preconditions:** `collider` fixture installed (declares a tool named `bash`).

**Steps:**
1. Type `/plugins enable collider` and press `Enter`.
2. Read the report; run `/plugins list`.
3. Prompt the agent to run a normal built-in shell command (for example `run ls in the
   current directory`); approve and confirm the built-in `bash` tool behaves normally.

**Expected results:**
- Enable completes but the colliding tool is rejected; either the plugin is `errored`
  with cause `name-collision` or loads with the colliding tool recorded as rejected —
  either way the built-in `bash` tool is untouched and functions.
- `/plugins list` reports the collision.

### TC-015 — Master switch `plugins.enabled: false`

**Requirement:** Config schema (master switch), FR-016

**Preconditions:** `.ragent/ragent.json` backed up (Prerequisite 8).

**Steps:**
1. Exit the TUI.
2. Edit `~/scratch/plugins-tests/.ragent/ragent.json` to contain
   `{ "plugins": { "enabled": false } }`.
3. Relaunch the TUI from the scratch root.
4. Type `/plugins list` and press `Enter`.
5. Type `/plugins enable codex-weather` and press `Enter`.
6. Type `/plugins help` and press `Enter`.

**Expected results:**
- `list` reports the plugin subsystem is disabled and performs no discovery (rows are
  not shown).
- `enable` refuses with the same disabled-subsystem notice; no code executes.
- `help` still prints usage.

### TC-016 — CLI parity `ragent plugins`

**Requirement:** FR-021

**Preconditions:** shell at `~/scratch/plugins-tests/`; config restored to enabled.

**Steps:**
1. Run `./target/debug/ragent plugins list` (adjust the binary path to the repo build).
2. Run `./target/debug/ragent plugins disable codex-weather` if it was left enabled,
   then `./target/debug/ragent plugins list` again.
3. Run `./target/debug/ragent plugins enable codex-weather` then
   `./target/debug/ragent plugins test codex-weather`.
4. Run `./target/debug/ragent plugins help`.

**Expected results:**
- Each CLI subcommand prints the same information as its TUI counterpart (table rows and
  `[ ok ]`/`[fail]` lines), operates on the same store, and exits 0 on success, non-zero
  on refusal.
- State changes made from the CLI are visible in a subsequent TUI session's
  `/plugins list`.

### TC-017 — Session-start loading of enabled plugins

**Requirement:** FR-008

**Preconditions:** `codex-weather` and `claude-todo` both enabled (via CLI or TUI);
exit the TUI.

**Steps:**
1. Launch a fresh TUI session from `~/scratch/plugins-tests/`.
2. Immediately run `/plugins list` before doing anything else.
3. Invoke `plugin_codex-weather_get_weather` via an agent prompt (city `Tallinn`) and
   approve the permission.

**Expected results:**
- Both plugins show state `loaded` on session start without any manual enable step.
- The tool works immediately in the fresh session.

### TC-018 — Non-JS (skill-only / MCP-only) plugin installs, loads, and tests clean

**Requirement:** FR-028

**Preconditions:** a Claude-dialect plugin directory with a manifest that carries no
`entry`/`main`/`server.entry` (for example `{"name":"mongodb","version":"1.2.1",
"skills":"./skills/","mcpServers":"./mcp.json"}`), plus a `skills/` folder with at least
one `SKILL.md` and an `mcp.json`.

**Steps:**
1. `ragent plugins add <path-to-skill-only-plugin>`.
2. `ragent plugins list`.
3. `ragent plugins enable mongodb` (or `/plugins enable mongodb`), then `/plugins list`.
4. `/plugins test mongodb`.

**Expected results:**
- The add succeeds; no "manifest has no entry point" error is shown.
- The list row shows the plugin with version `1.2.1`; the `skills` and `mcpServers`
  sections are bridged (FR-029, FR-030) so no unsupported-capability note is shown for
  them.
- Enable reports state `loaded` with zero tools and zero commands; no JavaScript runs.
- With the plugin enabled, its `skills/<name>/SKILL.md` packs appear in `/skills` (or the
  skill catalog), and its `mcpServers` entry appears as `mongodb.mongodb` in `/mcp`.
- `/plugins test` passes with an `entry execution` step reading
  `no entry point (non-JS plugin)`.

### TC-019 — A failed git subdir install leaves no staging directory

**Requirement:** FR-010

**Preconditions:** a public repository whose selected subpath is not a plugin, or a local
`file://` mirror built for the test.

**Steps:**
1. `ragent plugins add 'git+<https-url>#<ref>:<subpath-that-is-not-a-plugin>'`.
2. Inspect the store's `.add-staging` directory.

**Expected results:**
- The add fails with a "no plugin manifest" refusal.
- `.add-staging` no longer exists: the transient git checkout (and its `.git` above the
  subpath) has been pruned.

### TC-020 — Enabled plugin skills and MCP servers reach the session

**Requirement:** FR-029, FR-030

**Preconditions:** a Claude-dialect non-JS plugin with `"skills":"./skills/"` and
`"mcpServers":"./mcp.json"` installed but not enabled.

**Steps:**
1. With the plugin disabled, inspect the skill catalog and the MCP server list.
2. `ragent plugins enable <pluginid>`.
3. Inspect the skill catalog (`/skills`) and the MCP server list (`/mcp`) again.

**Expected results:**
- While disabled the plugin contributes nothing (no skills, no MCP servers).
- After enable, each `skills/<name>/SKILL.md` is a registered skill and each
  `mcpServers` entry is present as `<plugin-id>.<server>`.
- A configured `ragent.json` `mcp` entry with the same id wins over the plugin's entry.

### TC-021 — Plugin prompt commands surface as slash commands

**Requirement:** FR-031

**Preconditions:** the Claude-dialect `commit-commands` plugin installed (its
`.claude-plugin/plugin.json` declares no `commands`, but it ships `commands/commit.md`,
`commands/clean_gone.md`, and `commands/commit-push-pr.md`) and enabled.

**Steps:**
1. `/plugins enable commit-commands`, then open the `/` autocomplete menu and type `/commit`.
2. Run `/help`.
3. Invoke `/commit` with a short argument in a git repository.
4. Install a plugin whose command name collides with a built-in (e.g. `spec`) and enable it.

**Expected results:**
- `/commit`, `/clean_gone`, and `/commit-push-pr` appear in the autocomplete menu, each with
  the description from its frontmatter and a `(plugin commit-commands)` suffix; `/help`
  lists the same under a `Plugin commands:` section.
- `/commit` injects the command file's body (with `$ARGUMENTS` substituted) into the session
  as a user turn and the agent acts on it; the invocation is not reported as an unknown
  command.
- The colliding command registers under its namespaced `plugin:<plugin-id>:<name>` trigger
  (listed in `/help` and the menu) rather than shadowing the built-in; the built-in still
  runs when invoked by its bare name.
- `/plugins list` reports the contributed command names for the plugin.

### TC-022 — Plugin agents and hooks reach the session

**Requirement:** FR-032, FR-033

**Preconditions:** a Claude-dialect plugin installed and enabled that declares an `agents/`
directory containing one `agents/<name>.md` profile with YAML frontmatter (`name`,
`description`, `mode: subagent`) plus a markdown body, and a `hooks` section mapping
`PreToolUse` to a shell command.

**Steps:**
1. `/plugins list` and read the plugin's contributions block.
2. Run `/agents` (or open the agent picker) and confirm the plugin's agent profile is listed.
3. Spawn the plugin agent via `new_agent <name> ...` and confirm it runs with the profile body
   as its system prompt.
4. Disable the plugin, then repeat step 2.
5. From a turn, invoke a tool and confirm the plugin hook fires (e.g. the hook command writes
   a sentinel line); add a `ragent.json` hook at the same trigger and confirm it runs first.
6. Add a project agent at `.ragent/agents/<same-name>.md` and confirm it wins over the
   plugin profile.

**Expected results:**
- `/plugins list` shows `agents [agents/<name>.md]` and `hooks [PreToolUse]` for the plugin.
- `/agents` lists the plugin's profile; `new_agent <name>` resolves it and the session uses
  the profile body as the system prompt (YAML frontmatter is accepted).
- After disabling, the plugin's agent no longer appears and its hook no longer fires.
- The plugin hook fires at `pre_tool_use`; where a configured hook shares the trigger, the
  configured hook runs first.
- The project-local profile of the same name shadows the plugin profile.

### TC-023 — Codex nested manifest (`.codex-plugin/plugin.json`) installs and loads

**Requirement:** FR-002

**Preconditions:** network access to `github.com/openai/plugins` (skip and record as
not run if offline).

**Steps:**
1. `ragent plugins add 'git+https://github.com/openai/plugins#main:plugins/linear'`.
2. `ragent plugins list` and read the `linear` row.
3. `ragent plugins enable linear`.
4. `ragent plugins test linear`.

**Expected results:**
- Step 1 reports `Installed plugin 'linear' (dialect: codex, version: 5.0.1).` with no
  `no plugin manifest found` error; the manifest was found at
  `.codex-plugin/plugin.json`.
- Step 2 shows `linear` with dialect `codex` and version `5.0.1`.
- Step 3 reports the plugin loaded (0 tools, 0 commands - a non-JS bundle whose
  `mcpServers` bridge connects as `linear.linear`).
- Step 4 passes all four harness steps, with manifest validation naming dialect `codex` and
  the entry execution step reporting `no entry point (non-JS plugin)`.
- Cleanup: `ragent plugins remove linear`.

### TC-024 — Multi-target nested manifests resolve to Claude (both `.codex-plugin/plugin.json` and `.claude-plugin/plugin.json`)

**Requirement:** FR-002

**Preconditions:** network access to `github.com/mongodb/agent-skills` (skip and record as
not run if offline).

**Steps:**
1. `ragent plugins add 'git+https://github.com/mongodb/agent-skills#main:plugins/mongodb'`.
2. `ragent plugins list` and read the `mongodb` row.
3. `ragent plugins test mongodb`.

**Expected results:**
- Step 1 reports `Installed plugin 'mongodb' (dialect: claude, version: 1.2.1).` with no
  `ambiguous plugin manifest` error, even though the directory carries both
  `.codex-plugin/plugin.json` and `.claude-plugin/plugin.json`.
- Step 2 shows `mongodb` with dialect `claude` and its 7 skill directories listed in the
  contributions line (the Claude manifest is the one parsed).
- Step 3 passes the four harness steps, with manifest validation naming dialect `claude` and
  the entry execution step reporting `no entry point (non-JS plugin)`.
- A directory matching both dialects in any *other* combination (e.g. a top-level
  `codex-plugin.json` beside `claude-plugin.json`) still reports
  `ambiguous plugin manifest`; only the both-nested pairing resolves.
- Cleanup: `ragent plugins disable mongodb` then `ragent plugins remove mongodb`.

## Cleanup

1. In the TUI, run `/plugins disable` for every enabled plugin, then
   `/plugins remove codex-weather`, `/plugins remove claude-todo`, and equivalent removes
   for every other fixture installed during the run; confirm `/plugins list` shows an
   empty store.
2. Restore the backed-up `~/scratch/plugins-tests/.ragent/ragent.json` (or delete the
   scratch copy if it was created only for these tests).
3. Delete the sentinel file `~/scratch/plugins-tests/outside.txt`.
4. Delete the scratch tree: `rm -rf ~/scratch/plugins-tests/` (confirm this only removes
   the scratch directory).
5. Confirm no files leaked outside the store during the archive tests:
   `ls ~/scratch/escape.js` and `ls escape.js` in the scratch root must report "No such
   file".
6. If TC-009 used a local HTTPS server, stop it.
