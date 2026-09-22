# Implementation Plan: Plugin System - Codex and Claude Code/Desktop Plugin Loading

**Spec:** [SPEC.md](SPEC.md) · **Test plan:** [TESTPLAN.md](TESTPLAN.md)

## Approach

Introduce one new crate, `crates/ragent-plugins`, that owns the descriptor model, the
plugin store, the embedded JavaScript runtime wrapper, the host-API bridge, and the
plugin lifecycle (discovery, validation, load, unload, error containment). Surface it
through three integration points:

1. **`ragent-plugins` crate** — `descriptor` (manifest dialect recognition and
   normalisation), `store` (install/remove/scan at `.ragent/plugins/` and
   `~/.config/ragent/plugins/` with path-traversal-safe archive extraction), `runtime`
   (script-context pool, memory ceiling, deadline interruption), `host_api`
   (`ragent` object v1: `register_tool`, `register_command`, `config.get`,
   `message.*`, `log`, `plugin.read_text_file`), `lifecycle` (enable/disable/load/
   unload, error states, telemetry counters), and `harness` (the `/plugins test`
   isolated loader with a stubbed tool context and captured message sink).
2. **Tool-registry bridge** — a `PluginToolAdapter` in the new crate implements the
   existing `Tool` trait, forwards arguments into the plugin's JavaScript handler as
   JSON, converts the returned value into the tool `Output`, and registers under
   `plugin_<pluginid>_<toolname>` so the normal permission engine applies unchanged.
   Slash commands contributed by plugins register through a `PluginCommandAdapter` in
   the TUI command layer.
3. **Command surface** — a `/plugins` slash-command family in
   `crates/ragent-tui/src/app/slash.rs` (parser and dispatch alongside the `/spec`
   family), registered in `SLASH_COMMANDS` in `crates/ragent-tui/src/app/state.rs` and
   documented in `/plugins help`; optional `ragent plugins` CLI parity in
   `src/cli.rs` (FR-021). Configuration gains a `plugins` block in `ragent-config`.

Sandboxing is in-process: no ambient filesystem, network, or process APIs reach the
script context; time and memory budgets are enforced with the engine's interrupt
handler and allocator hooks, and every plugin-facing failure is converted into a Rust
`Result` instead of a panic (FR-026).

### JavaScript engine decision

Open Question 1 is resolved in task T-004 by a spike comparing `rquickjs` (QuickJS)
against Boa on the two capabilities FR-017 depends on: reliable wall-clock interruption
and a hard memory ceiling. The default selection is `rquickjs` because QuickJS exposes
both a `set_memory_limit` on the runtime and a cooperative interrupt handler, while
Boa's memory limiting is coarser. Whichever engine wins, it is wrapped behind the
`runtime` module's own trait so the rest of the crate does not depend on the engine
API directly; swapping engines later touches only that module.

## Requirement Coverage Map

| Requirement | Task(s) |
| ----------- | ------- |
| FR-001 Plugin store discovery | T-005, T-006 |
| FR-002 Manifest recognition and normalisation | T-002, T-003, T-024, T-025 |
| FR-003 Embedded JavaScript execution | T-004, T-007 |
| FR-004 Versioned host API | T-008, T-009 |
| FR-005 Plugin tool contribution | T-010 |
| FR-006 `/plugins` command family | T-012, T-013, T-014 |
| FR-007 `/plugins add` install and validation | T-006, T-011 |
| FR-008 Session-start loading | T-009, T-016 |
| FR-009 `/plugins list` | T-013 |
| FR-010 `/plugins add` source handling | T-006, T-011 |
| FR-011 `/plugins enable` | T-009, T-013 |
| FR-012 `/plugins disable` | T-009, T-013 |
| FR-013 `/plugins test` harness | T-015 |
| FR-014 `/plugins help` and usage | T-014 |
| FR-015 Exception containment | T-007, T-010 |
| FR-016 Disabled-plugin inertness | T-009, T-013 |
| FR-017 Time/memory sandbox | T-007 |
| FR-018 No ambient host access | T-007, T-008 |
| FR-019 API-version refusal | T-003, T-009 |
| FR-020 Permission-gated capabilities | T-008 |
| FR-021 CLI parity | T-017 |
| FR-022 Telemetry counters | T-013, T-016 |
| FR-023 No execution on discovery | T-005, T-006 |
| FR-024 Collision rejection | T-010, T-012 |
| FR-025 Unsupported-capability reporting | T-003, T-013 |
| FR-026 No panics from plugin code | T-007, T-010, T-015 |
| FR-027 Quiet logging posture | T-008 |
| FR-028 Non-JS (skill-only / MCP-only) plugins | T-019 |
| FR-029 Skills bridge (`skills` -> skill discovery) | T-021 |
| FR-030 MCP bridge (`mcpServers` -> McpServerConfig) | T-021 |
| FR-031 Plugin prompt commands | T-022 |
| FR-032 Plugin agents bridge | T-023 |
| FR-033 Plugin hooks bridge | T-023, T-026 |
| FR-010 git-source staging cleanup | T-020 |
| Config schema (`plugins` block) | T-001 |
| Host API surface v1 | T-008 |
| Error handling policy | T-009, T-015 |
| Security and privacy posture | T-006, T-007, T-008 |
| Performance (NFR timing) | T-018 |
| Acceptance criteria 1-9 | T-018 |

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `plugins` configuration block to `ragent-config` | Config schema | S | High | completed | — |
| T-002 | Define `PluginDescriptor` model and dialect recognition rules | FR-002, FR-025 | M | Critical | completed | — |
| T-003 | Implement manifest parsing, normalisation, and host-API version check | FR-002, FR-019, FR-025 | M | Critical | completed | T-002 |
| T-004 | Spike engine choice (rquickjs vs Boa): interruption + memory ceiling | FR-003, FR-017 | M | Critical | completed | — |
| T-005 | Implement plugin store paths, discovery scan, and state ledger (no code execution) | FR-001, FR-023 | M | Critical | completed | T-003 |
| T-006 | Implement `/plugins add` source handling: local dir, zip/tar.gz, https URL with traversal protection and size cap | FR-007, FR-010, FR-023 | L | High | completed | T-005 |
| T-007 | Build runtime module: context pool, memory limit, deadline interruption, exception containment | FR-003, FR-015, FR-017, FR-018, FR-026 | L | Critical | completed | T-004 |
| T-008 | Build host API v1 bridge (`ragent` object) with permission-gated capabilities | FR-004, FR-018, FR-020, FR-027 | L | Critical | completed | T-007 |
| T-009 | Implement lifecycle: enable, disable, load, unload, error states, auto-unload threshold | FR-008, FR-011, FR-012, FR-016, FR-019 | L | Critical | completed | T-005, T-008 |
| T-010 | Implement `PluginToolAdapter` (Tool trait) with collision rejection and JSON marshalling | FR-005, FR-015, FR-024, FR-026 | L | Critical | completed | T-008 |
| T-011 | Wire `/plugins add` and `/plugins remove` store operations | FR-007, FR-010 | M | High | completed | T-006 |
| T-012 | Implement `PluginCommandAdapter` for plugin-contributed slash commands with collision rejection | FR-004, FR-024 | M | High | completed | T-008 |
| T-013 | Implement `/plugins list`, `/plugins enable`, `/plugins disable` with state and telemetry output | FR-009, FR-011, FR-012, FR-016, FR-022, FR-025 | M | Critical | completed | T-009, T-010 |
| T-014 | Register `/plugins help` and usage text; register the family in `SLASH_COMMANDS` and autocomplete | FR-006, FR-014 | S | High | completed | T-013 |
| T-015 | Build `/plugins test` isolated harness: stubbed tool context, captured messages, per-tool sample invocation, `[ ok ]`/`[fail]` report | FR-013, FR-026 | L | High | completed | T-010, T-012 |
| T-016 | Session-start integration: discover, load enabled plugins, record telemetry, deregister on shutdown | FR-008, FR-022 | M | High | completed | T-009 |
| T-017 | Add `ragent plugins` CLI parity subcommands | FR-021 | M | Medium | completed | T-011, T-013, T-015 |
| T-018 | Fixture plugins, acceptance walk, fmt/clippy/test, and NFR timing checks | Acceptance criteria, Performance | L | High | completed | T-013, T-014, T-015, T-016 |
| T-019 | Support non-JS (skill-only / MCP-only) plugin manifests: optional entry point | FR-002, FR-028 | M | High | completed | T-018 |
| T-020 | Prune the whole staging directory after a failed git-subdir install | FR-010 | S | Medium | completed | T-018 |
| T-021 | Bridge plugin `skills` directories into skill discovery and `mcpServers` into `McpServerConfig` | FR-025, FR-029, FR-030 | L | High | completed | T-019 |
| T-022 | Parse Claude `commands/` prompt files and manifest prompt commands; bridge them onto the session command surface with prompt injection and namespaced collisions | FR-004, FR-031 | L | High | completed | T-021 |
| T-023 | Bridge plugin `agents` profiles into agent discovery and `hooks` into the session hook engine | FR-025, FR-032, FR-033 | L | High | completed | T-022 |
| T-024 | Recognise the nested Codex manifest `.codex-plugin/plugin.json` so `openai/plugins` store entries install and load | FR-002 | S | High | completed | T-002 |
| T-025 | Resolve the multi-target nested manifest pair (both `.codex-plugin/plugin.json` and `.claude-plugin/plugin.json`) to Claude instead of refusing it as ambiguous, so `mongodb`-style Claude-store entries install | FR-002 | S | High | completed | T-024 |
| T-026 | Read the Claude `hooks/hooks.json` declaration file and the `{matcher, hooks:[...]}` group shape, honour the matcher and timeout fields, feed the Claude event JSON on stdin with `CLAUDE_PLUGIN_ROOT`, deliver `additionalContext` to the model, and let a blocking Stop hook feed findings back before the turn ends | FR-033 | L | High | completed | T-023 |

## Task Details

### T-001 — Add `plugins` configuration block to `ragent-config`

- Add `pub plugins: Option<PluginsConfig>` to `Config` with fields matching the SPEC
  schema (`enabled`, `max_execution_ms`, `max_entry_ms`, `max_memory_mb`, `store_dir`,
  `permissions`), all with `serde(default)` and skip-when-none serialisation.
- Merge with the same overlay pattern used for other optional sections: overlay section
  wins when present.
- Verify defaults: enabled `true`, 5000 ms tool budget, 10000 ms entry budget, 64 MiB
  memory.

### T-002 — Define `PluginDescriptor` model and dialect recognition rules

- New crate skeleton `crates/ragent-plugins/` with module layout:
  `descriptor.rs`, `store.rs`, `runtime.rs`, `host_api.rs`, `lifecycle.rs`,
  `harness.rs`, `tool_adapter.rs`, `command_adapter.rs`, `error.rs`, `lib.rs` re-exports.
- `PluginDescriptor { id, name, version, dialect: Codex|Claude, entry: PathBuf,
  requested_permissions: Vec<String>, api_version: u32, unsupported_capabilities:
  Vec<String>, manifest_path: PathBuf, root: PathBuf }`.
- Recognition rules (pure functions, unit-testable): a directory is a Codex plugin when
  it contains `codex-plugin.json` or a `plugin.json` with a top-level `"codex"` marker;
  a Claude plugin when it contains `claude-plugin.json` or
  `.claude-plugin/plugin.json`. Exactly one dialect per directory; ambiguous directories
  are `errored` with cause `ambiguous-manifest`.

### T-003 — Manifest parsing, normalisation, and version check

- Parse each dialect's manifest shape into `PluginDescriptor`; tolerate unknown fields;
  extract declared tools/commands arrays when present.
- Record unfulfillable sections (Claude MCP-server transport blocks, Desktop extension
  keys with no ragent equivalent) into `unsupported_capabilities`.
- Implement `check_api_version(declared, host) -> Result<(), VersionMismatch>` refusing
  `declared > host` (FR-019); equal or lower accepted.
- Pure functions with fixture manifests; no I/O outside the plugin directory.

### T-004 — Engine choice spike: rquickjs vs Boa

- Stand up a scratch spike under `target/temp/plugin-spike/` evaluating both engines on:
  (a) interrupt handler stops an infinite `while(1){}` loop within ~50 ms of the
  deadline; (b) memory ceiling aborts an allocation loop cleanly without aborting the
  host process; (c) JSON marshalling ergonomics between Rust and JS values.
- Record findings in the task note; select `rquickjs` unless the spike proves the
  assumptions wrong.
- Behind a `runtime` trait so downstream code is engine-agnostic.

### T-005 — Plugin store paths, discovery scan, state ledger

- `store::store_dirs()` returns project `.ragent/plugins/` then user-global
  `~/.config/ragent/plugins/`, closest-wins on plugin id collision.
- `store::scan()` walks both directories, recognises dialects via T-003 parsing, and
  returns `Vec<ScannedPlugin { descriptor_or_error, enabled }>`. Manifest bytes are
  parsed; no JavaScript executes here (FR-023).
- A per-store `_state.json` (or a storage-crate table) persists enable/disable state and
  telemetry counters so state survives restarts.
- Symlinks followed only when the resolved path stays inside the store directory.

### T-006 — `/plugins add` source handling

- `store::add(source)` accepts: existing local directory (copied), local `.zip` /
  `.tar.gz` (extracted), `https://` URL ending in `.zip`/`.tar.gz` (downloaded to
  `target/temp`-style staging inside the store's temp dir, then extracted).
- Guards: non-https URLs refused; archives larger than 50 MiB refused; archive entries
  with `..` or absolute paths refused; existing plugin id refuses unless `--force`.
- After install, parse and validate the manifest and report id + dialect.
- Files land in the store; the plugin is recorded enabled in the ledger (FR-007) so it
  loads at the next session start, and no code executes during the install (FR-023).

### T-007 — Runtime module: pool, limits, interruption

- `runtime::RuntimePool` owns engine runtimes; `checkout()` returns a fresh context
  pre-configured with the memory ceiling and interrupt handler.
- Deadline interruption: a watchdog flag set from a timer thread (or the engine's own
  deadline hook) causes the interrupt handler to abort execution; the failure surfaces
  as `Err(PluginError::Timeout)`.
- Memory ceiling: engine allocator hook returns null past the ceiling; surfaces as
  `Err(PluginError::MemoryLimit)`.
- Every engine call wrapped so JavaScript exceptions and engine errors convert to
  `Result`; no panic path escapes (FR-026). No `unsafe` beyond what the engine crate
  itself encapsulates.
- The context exposes no globals beyond what `host_api` installs (FR-018): no `print`,
  no `std`, no `os`, no file/network modules.

### T-008 — Host API v1 bridge

- `host_api::install(ctx, plugin_state, permission_gate)` installs the `ragent` object:
  `api_version`, `plugin_id`, `register_tool`, `register_command`, `config.get`,
  `message.info/warn/error`, `log`, `plugin.read_text_file`.
- `register_tool`/`register_command` push definitions into the plugin's pending
  registration list; actual registry insertion happens on successful entry completion.
- `plugin.read_text_file` resolves the path inside the plugin root and refuses escapes.
- Permission gate: capabilities keyed by `plugin:<pluginid>` consult the rule engine;
  ungranted capabilities are simply absent from the injected object (FR-020).
- Logging posture: host-level traces at `debug`/`trace` only; the only `info`-level
  plugin output is what the plugin itself emits through `ragent.log`/`ragent.message.*`
  (FR-027).

### T-009 — Lifecycle

- `lifecycle::enable(id)`: mark enabled, check version, checkout context, install host
  API, execute entry within the entry budget, apply pending tool/command registrations,
  set state `loaded` or `errored(cause)`.
- `lifecycle::disable(id)`: deregister contributed tools/commands, drop context, mark
  disabled, report deregistration counts (FR-012).
- `lifecycle::load_all_enabled()` for session start (FR-008).
- Auto-unload: a plugin whose tool calls fail consecutively past the configured
  threshold (default 3, from `plugins` config) is unloaded and marked `errored`
  (error-handling policy in SPEC).
- Disabled state is fully inert (FR-016).

### T-010 — `PluginToolAdapter`

- Implements the existing `Tool` trait: `name() -> "plugin_<id>_<tool>"`,
  `description()` and `parameters()` from the plugin-declared schema, `execute()`
  marshals the JSON argument into the context, calls the handler with the tool budget,
  converts the result into the standard tool output shape, and maps engine/JS failures
  into tool-call failures (FR-015).
- Name collisions with built-ins or other plugins are rejected at registration time and
  recorded in `errored` cause `name-collision` (FR-024).
- Permission domain `plugin:<pluginid>` with default action `ask` (Assumption A5).

### T-011 — Wire `/plugins add` and `/plugins remove`

- `/plugins add <source> [--force]`: call `store::add`, print installed id, dialect,
  and that the plugin is enabled and loads at the next session start.
- `/plugins remove <pluginid>`: refuse when enabled ("disable first"), then delete the
  plugin directory from the store and report.
- Both flow through the same permission confirmation conventions as other
  mutating slash commands.

### T-012 — `PluginCommandAdapter`

- Plugin-contributed slash commands register into the TUI command surface under their
  declared names; registration fails and records a collision when the name already
  exists in `SLASH_COMMANDS` or in another plugin (FR-024).
- Handler invocation marshals the argument string into the context and prints returned
  text into the message window.

### T-013 — `/plugins list`, `/plugins enable`, `/plugins disable`

- `list`: table rows of id, name, version, dialect, state, contributed tool/command
  names and counts; summary totals line; unsupported-capability notices (FR-009,
  FR-025). `--verbose` adds telemetry counters (FR-022).
- `enable <id>` / `disable <id>`: invoke lifecycle, print resulting state and
  deregistration counts respectively (FR-011, FR-012). Enable prints the plugin's
  declared permissions before completing (security posture).
- Disabled plugins appear with state `disabled` and execute nothing (FR-016).

### T-014 — `/plugins help`, registration, autocomplete

- `help` (and bare `/plugins` or unknown subcommand) prints usage for all six
  subcommands, both source forms for `add`, and creates no files (FR-014).
- Register the family in `SLASH_COMMANDS` in `state.rs` with autocomplete metadata, and
  in the `slash.rs` dispatch arm (FR-006).

### T-015 — `/plugins test` harness

- `harness::test_plugin(id)`: fresh context, stubbed tool context (no live registries),
  captured message sink; step sequence: manifest validate -> version check -> entry
  execute -> per tool: generate schema-valid sample arguments and invoke once -> report.
- Report format: one `[ ok ]`/`[fail]` line per step with wall-clock time; failures
  include the cause (FR-013).
- The harness registers nothing in the live session and unloads afterwards (FR-026).

### T-016 — Session-start integration

- Hook the session bootstrap so after the tool registry is built,
  `lifecycle::load_all_enabled()` runs, plugin tools join the registry, and plugin
  commands join the command surface.
- On session end, unload all loaded plugins cleanly.
- Record telemetry counters into the state ledger (FR-022).

### T-017 — `ragent plugins` CLI parity

- Mirror `list`, `add`, `remove`, `enable`, `disable`, `test`, `help` as
  `ragent plugins <subcommand>` in `src/cli.rs`, sharing the parser with the TUI
  surface (FR-021).
- Non-TUI output is plain text rows and `[ ok ]`/`[fail]` lines.

### T-018 — Fixtures, acceptance walk, quality gates

- Author three fixture plugins under `assets/plugins/`: `fixtures/codex-weather`
  (Codex dialect, one tool returning canned JSON), `fixtures/claude-todo` (Claude
  dialect, one tool + one command), `fixtures/loop-forever` (infinite-loop entry to
  exercise FR-017/acceptance criterion 5), plus an escape-attempt fixture using
  `plugin.read_text_file("../..")` for criterion 6.
- Walk acceptance criteria 1-9 manually per TESTPLAN.md.
- Run `cargo fmt`, `cargo clippy`, and the workspace tests; record NFR timing numbers
  (discovery under 2 s for 50 plugins; test command under 15 s) in the task note.

**Outcome.** Eight fixture plugins now live under `assets/plugins/fixtures/`:
`codex-weather` (Codex, manifest-declared `get_weather`), `claude-todo` (Claude,
manifest-declared `add_todo` + `todo-add` command with runtime handlers),
`loop-forever` (`while (true) {}`), `escape-attempt`
(`ragent.plugin.read_text_file("../../outside.txt")`), `future-api`
(`api_version: 99`), `claude-mcp` (`mcp_servers` unsupported section), `collider`
(duplicate `bash` tool name to force a registry collision), and `bad-manifest`
(truncated JSON).

Automated acceptance coverage is `crates/ragent-plugins/tests/test_fixtures.rs`
(14 tests) plus new master-switch cases in `test_control`, `test_commands`, the
TUI `test_plugins_command`, and the CLI `test_plugins_cli_command`. The walk
surfaced one real gap: `plugins.enabled: false` was only honoured by `test`, so
`PluginSession::start` and the control/store subcommands now short-circuit with a
disabled-subsystem `[err]` report and perform no discovery (acceptance criterion
8, SPEC configuration schema).

NFR timings (debug build, developer workstation, `ragent plugins` CLI):
discovery + list over 50 installed plugins - 0.01-0.02 s (budget 2 s); `/plugins
test` on a 10-tool plugin - 0.01 s (budget 15 s); infinite-loop entry contained
within 0.815 s at an 800 ms entry budget. The harness itself asserts both budgets
(`nfr_discovery_of_fifty_plugins_is_under_two_seconds`,
`nfr_test_of_ten_tool_plugin_is_under_fifteen_seconds`).

## Risks and Mitigations

- **Engine sandbox maturity.** QuickJS/Boa interruption and memory limits are the
  foundation of FR-017 and FR-026. Mitigation: T-004 spike lands before the runtime
  module; if neither engine meets the interruption requirement, fall back to executing
  plugin code on a dedicated worker thread with a cancellation token between host-API
  calls, accepting cooperative (API-boundary) interruption only, and document the
  residual risk.
- **Dialect schema drift.** Codex and Claude plugin manifests are not formally
  standardised and may change. Mitigation: dialect parsing behind small per-dialect
  modules with fixture manifests as regression tests (T-002/T-003); unknown fields are
  tolerated so additive upstream changes do not break loading.
- **Dynamic tool registration safety.** Registering/deregistering tools mid-session is
  newer than the built-in static registry usage. Mitigation: keep registration confined
  to the session bootstrap and enable/disable boundaries; never mutate the registry
  while a tool call is in flight (lifecycle takes a registry lock).
- **Untrusted code in-process.** Plugin code shares the host process address space.
  Mitigation: no ambient capabilities in the context (FR-018), budget enforcement
  (FR-017), documented in SPEC security section; OS-level sandboxing is explicitly out
  of scope for this iteration.
- **Permission UX on tool calls.** Plugin tools default to `ask`, which may prompt
  frequently. Mitigation: documented in the help text and the how-to; YOLO/autopilot
  behaviour is unchanged and applies uniformly.

## Notes on Assumptions

- A1 engine: embedded, not Node.js — resolved by T-004 (default `rquickjs`).
- A2 dialects normalise to one descriptor; unsupported manifest sections are reported,
  not dropped (FR-025).
- A3 discovery/list/add never execute code (FR-023).
- A4 host API is versioned and additive-only; newer-than-host plugins refused (FR-019).
- A5 plugin tools are first-class tools behind the standard permission engine.
- A6 no npm/`node_modules` resolution — self-contained plugins only.
### T-019 — Support non-JS (skill-only / MCP-only) plugin manifests (FR-002, FR-028)

The official Claude marketplace (`anthropics/claude-plugins-official`) ships packages
whose manifest carries no JavaScript entry point — a `skills/` folder of `SKILL.md`
files and/or an `mcpServers` section (for example the `mongodb` plugin). Previously
`parse_claude_manifest` rejected every such package with "manifest has no entry point",
so none of the 310 marketplace plugins could install.

- `descriptor::PluginDescriptor::entry` becomes `Option<PathBuf>`; `None` denotes a
  non-JS plugin. `manifest::resolve_entry` maps an absent declaration to `None`, a
  blank declaration to a `ManifestParse` error, and a real declaration to
  `root.join(entry)`.
- `parse_codex_manifest` / `parse_claude_manifest` accept a manifest with no entry and
  no longer error; the Codex `entry` field becomes `Option<String>`.
- `lifecycle::PluginManager::load_inner` returns an inert `LoadedPlugin` (state
  `Loaded`, no sandbox context, manifest-declared tools/commands only) when the entry is
  absent, so `enable`/session-start loading report `loaded`.
- `harness::run_entry` returns `Ok(None)` for a non-JS plugin; `/plugins test` reports
  the `entry execution` step as a pass with detail `no entry point (non-JS plugin)`.
- The `skills` and `mcpServers` manifest sections are accepted; consuming them is T-021.

Verify: `cargo test -p ragent-plugins`; `tests/test_non_js_plugin.rs` covers install,
load, and harness; `tests/test_manifest.rs` covers the `entry = None` parse cases.

### T-020 — Prune the whole staging directory after a failed git-subdir install (FR-010)

A failed `git+<url>#<ref>:<subpath>` install left a `stage-*` directory behind: the
selected plugin root was a *subdirectory* of the checkout, so `cleanup_staging`'s
`remove_dir_all` on that root left the repository root (and its `.git`) in place, and
the parent `.add-staging` directory could not be pruned.

- `add::cleanup_staging` now removes the entire per-call staging directory (the value
  passed to `add_inner`), not just the extracted plugin root, and then prunes
  `.add-staging` when it becomes empty. It takes `&Path` instead of the `Staging` enum.

Verify: `tests/test_store_git_source.rs::add_failed_git_subdir_install_leaves_no_staging_directory`.

### T-021 — Bridge plugin `skills` and `mcpServers` into the session (FR-025, FR-029, FR-030)

A non-JS (or any) plugin can declare `skills` (a directory of `SKILL.md` packs) and
`mcpServers` (an inline object or an external `mcp.json`). T-019 made such plugins
installable but inert; T-021 consumes the two sections.

- `manifest::extract_skill_dirs` / `extract_mcp_servers` / `map_mcp_server_entry` parse the
  sections into `ParsedManifest::skills` and `ParsedManifest::mcp_servers` (plus
  `raw_mcp`). A section whose shape cannot be bridged retains the FR-025 `UNSUP_MCP` /
  `UNSUP_SKILLS` label; a bridged section is no longer reported unsupported.
- New `bridge` module: `scanned_plugin_skill_dirs` / `scanned_plugin_mcp_servers` resolve
  the contributions of every **enabled** plugin in the project and global stores, with the
  server id prefixed `<plugin-id>.<server>` and external `mcp.json` files read relative to
  the plugin root behind a lexical path-traversal guard.
- `ragent-agent` depends on `ragent-plugins`: `skill::effective_skill_dirs` appends plugin
  skill dirs to the discovery roots, and `SessionProcessor::skill_registry` keys its mtime
  cache on the effective list; `plugin::plugin_mcp_servers` merges plugin servers with the
  configured `mcp` map (configured wins on id collision).
- The binary startup MCP connect block (`src/main.rs`) connects the merged set.

Verify: `tests/test_bridge.rs`, `tests/test_plugin_bridge.rs` (both crates), plus the
existing manifest/harness/fixture suites.

### T-022 — Surface plugin-contributed slash commands (FR-004, FR-031)

The Claude marketplace (e.g. `commit-commands`) ships slash commands as `commands/*.md`
prompt files, and a manifest `commands[]` array may also name prompt files. ragent only
parsed inline `commands[]` objects, so these commands never appeared and — unlike skills
and MCP — the command bridge was never wired to the session surface.

- `manifest` gains `CommandSource` (`Inline` / `File { path, body }`) and `PluginCommandDef`
  (a `Debug` impl redacts the prompt body). The manifest `commands` field is now a tolerant
  `serde_json::Value`; `extract_command_decls` maps an array of strings or objects (an
  object without a `file` stays inline — the historical shape — an object or string with a
  file becomes a prompt command) and `scan_command_dir` discovers `commands/*.md`, taking
  the name from the file stem and the description from the YAML frontmatter (other keys
  ignored). Both dialects' parsers populate `ParsedManifest::commands`; the Claude parser
  merges the manifest array with the directory list.
- `bridge::scanned_plugin_commands` resolves every **enabled** plugin's commands (prompt
  bodies read from disk), and `ragent_agent::plugin::plugin_commands` maps them onto the
  session surface: a free name registers bare, a colliding name registers under
  `plugin:<plugin-id>:<name>` so a plugin can never shadow a built-in, a skill, or another
  plugin.
- The TUI (`App::plugin_commands`, resolved once in `App::new`) lists the commands in the
  `/` menu and `/help`, and `execute_slash_command_inner` resolves a plugin trigger before
  the built-in ladder: a prompt command's body (with `$ARGUMENTS`/`$N` substituted) is
  injected as a user turn; an inline command reports that it needs the plugin host.
- `PluginCommandAdapter` now carries a `PluginCommandDef` and exposes `prompt()`;
  `PluginManager::execute_command` returns a prompt command's body directly (no sandbox) and
  `PluginSession::enable` reports prompt commands so a live enable is consistent.

Verify: `tests/test_manifest.rs` (directory + declaration prompt cases), `test_command_adapter.rs`
(inline dispatch unchanged), `test_plugin_bridge.rs::plugin_commands_*`, and the TUI
`test_plugin_commands.rs`.

### T-023 — Bridge plugin `agents` and `hooks` into the session (FR-025, FR-032, FR-033)

Codex and Claude plugins also ship **subagent profiles** (`agents/`) and **lifecycle hooks**
(`hooks/` or `hooks.json`). Both were previously recorded as unsupported (or silently ignored)
and the two surfaces carry real value for a catalogue that is largely non-JS.

- `manifest` gains `extract_agent_decls` / `scan_agent_dir` (agents) and `extract_hooks`
  (hooks), populating new `ParsedManifest::agents: Vec<String>` and
  `ParsedManifest::hooks: Vec<PluginHook>` fields (`PluginHook = {plugin_id, trigger,
  command, timeout_secs}`). `extract_hooks` accepts an object of trigger→command, an object of
  trigger→entry-array (the Claude `hooks.json` shape: string or `{command|command_path,
  timeout_secs?}`), and a flat array of `{trigger, command, timeout_secs?}`. A section whose
  shape yields no hooks keeps the new `UNSUP_AGENTS` / `UNSUP_HOOKS` FR-025 label; a bridged
  section is no longer reported unsupported.
- `bridge` gains `scanned_plugin_agent_files` / `plugin_agent_files` (resolve declared names,
  a bare name becoming `agents/<name>.md`, and refuse `..`/absolute escapes) and
  `scanned_plugin_hooks` (every enabled plugin's hooks, sorted).
- `ragent-agent` custom-agent loading (`agent::custom::load_custom_agents`) appends the
  plugin-contributed profile files as extra **sources** scanned after the `.ragent/agents/`
  directories (so a project agent wins a name clash); each source may be a directory or a
  single file. The loader's frontmatter parser now tries JSON first and **falls back to
  YAML**, so Claude-dialect `agents/*.md` profiles load unchanged. The mtime cache key and its
  fast-path now cover the plugin files (and compare source-set length, so an empty cache entry
  cannot mask a later non-empty source set).
- `ragent-agent` hooks (`hooks::merge_hook_configs` / `plugin_hook_configs` /
  `HookTrigger::parse`) map plugin triggers (accepting ragent snake_case and Claude PascalCase)
  onto the session hook engine and merge them after the configured hooks, so a user hook fires
  first. `loop_steps::prepare_client` merges the configured hooks with the enabled plugins'
  hooks when the turn's hook configs are built.
- `/plugins list` shows agent paths and hook triggers in its contributions block; `/plugins
  test` reports the agent/hook counts in its manifest-validation detail.

Verify: `crates/ragent-plugins/tests/test_manifest.rs` (agents dir/declaration merge, hook
shapes), `test_bridge.rs` (agent/hook bridge, disabled inert), `crates/ragent-agent/tests/
test_plugin_bridge.rs::plugin_agent_profile_loads_with_yaml_frontmatter`, and
`test_plugin_hooks.rs` (trigger parsing + merge precedence).

### T-024 — Recognise the nested Codex manifest `.codex-plugin/plugin.json` (FR-002)

The official `openai/plugins` catalogue keeps each plugin manifest in
`.codex-plugin/plugin.json`, the Codex counterpart of Claude's
`.claude-plugin/plugin.json`. The recogniser previously accepted only a root
`codex-plugin.json` or a `plugin.json` with a top-level `"codex"` marker, so a
store entry fetched via `git+https://github.com/openai/plugins#main:plugins/<name>`
downloaded but failed to install with `no plugin manifest found`.

- `descriptor` gains the `CODEX_NESTED_MANIFEST = ".codex-plugin/plugin.json"`
  constant (re-exported from the crate root) and `codex_match` now recognises it
  when a `.codex-plugin` directory entry is present and the nested manifest
  reads back, mirroring the existing Claude `.claude-plugin` rule.
- Parsing is unchanged: the nested path flows through
  `parse_manifest` -> `parse_codex_manifest`, so the Codex `name`/`version` shape
  and the `skills` / `mcpServers` / `agents` / `hooks` bridges all apply.

Verify: `crates/ragent-plugins/tests/test_descriptor.rs`
(`recognises_nested_codex_plugin_manifest`,
`nested_codex_marker_without_manifest_file_is_not_a_plugin`) and
`test_add.rs::add_nested_codex_manifest_directory_installs`; live
`ragent plugins add 'git+https://github.com/openai/plugins#main:plugins/linear'`
then `enable`/`test`.

### T-025 — Resolve the multi-target nested manifest pair to Claude (FR-002)

Some upstream trees ship **more than one host manifest in the same folder** — a
`.claude-plugin/plugin.json` *and* a `.codex-plugin/plugin.json` side by side (for
example `mongodb/agent-skills`, whose `plugins/mongodb` and `plugins/mongodb-atlas`
directories each carry `.claude-plugin`, `.codex-plugin`, `.cursor-plugin`,
`.agy-plugin`, and `.grok-plugin`). After T-024 taught the recogniser the nested
Codex manifest, such a directory matched **both** dialects and was refused with
`ambiguous plugin manifest: directory matches both Codex and Claude dialects`, so
the `mongodb` entry in the Claude store browser (`/plugins claude`) would not
install.

- `descriptor::recognise_dialect` no longer treats the *both-nested-manifest*
  pairing as ambiguous: when the Codex match is exactly `.codex-plugin/plugin.json`
  **and** the Claude match is exactly `.claude-plugin/plugin.json`, it returns the
  Claude match. Claude is chosen because its manifest supports every bridged
  contribution surface (skills, MCP servers, commands, agents, hooks), whereas the
  Codex sibling carries no extra ragent-relevant sections for these trees.
- Every **other** both-match is unchanged and still refused:
  `codex-plugin.json` beside `claude-plugin.json`, a `plugin.json` with a top-level
  `"codex"` marker beside a Claude manifest, or a nested Codex manifest beside a
  top-level Claude manifest. Only the exact multi-target pairing is special-cased.
- Parsing, staging, and the FR-023 "no JavaScript at install" guarantee are
  untouched: the resolved Claude manifest flows through `parse_claude_manifest`
  exactly as a Claude-store plugin always has, and the nested Codex manifest is
  simply left inert beside it in the installed tree.

Verify: `crates/ragent-plugins/tests/test_descriptor.rs`
(`multi_target_nested_manifest_pair_resolves_to_claude`,
`nested_codex_manifest_plus_top_level_claude_manifest_is_ambiguous`,
`detect_dialect_resolves_multi_target_nested_pair_on_disk`), with the existing
`ambiguous_directory_is_rejected` / `detect_dialect_reports_ambiguity_on_disk`
still asserting the non-multi-target refusals; live
`ragent plugins add 'git+https://github.com/mongodb/agent-skills#main:plugins/mongodb'`
then `list`/`test`.

### T-026 — Honour the full Claude hook declaration surface (FR-033)

The hooks bridge added in T-023 read a `hooks` inline section and the Claude
argument-array shape, but the **official catalogue's actual layout** was not
consumed: a Claude plugin declares its hooks in a `hooks.json` file (at the plugin
root or under `hooks/`), wrapping the triggers in a top-level `hooks` object, and
each trigger maps to a **group** `{matcher, hooks: [{type, command|command_path,
timeout, if?}]}` rather than a bare command. `security-guidance`, which ships only
a `hooks/` directory, therefore loaded inertly with `Hooks 0`. The delivered
commands were also non-functional: they interpolate `${CLAUDE_PLUGIN_ROOT}` (never
set), read the Claude event JSON from **stdin** (never written), and signal
findings through `hookSpecificOutput.additionalContext` / a blocking `exit 2` —
none of which the bridge honoured.

- `manifest`: new `HOOKS_FILE = "hooks.json"` and
  [`read_plugin_hooks_file`](crate::read_plugin_hooks_file), which reads the root
  and `hooks/` locations, unwraps the top-level `hooks` envelope, and returns each
  `(trigger, entries)` pair. Both dialect parsers merge it with the inline section.
  `extract_hooks` now takes the plugin root, unwraps the group shape (inheriting a
  group `matcher` into its children), accepts the Claude `timeout` (ms -> s) and
  `if` fields, and **skips any entry whose `type` is not `command`**. `PluginHook`
  gains `plugin_root` (the absolute root for `CLAUDE_PLUGIN_ROOT`) and
  `matcher: Option<String>`.
- `ragent-agent` hooks: `HookConfig` gains the same two fields and a
  `matches_tool(tool_name, tool_input)` that treats a `|`/`,`-separated matcher as
  a tool-name list, with `Name(pattern)` guards (`Bash(git commit:*)` = prefix
  match, any other `*` a glob over the tool-input strings).
  `run_pre_tool_use_hooks` / `run_post_tool_use_hooks` filter by it.
- Claude protocol: `claude_event_payload` writes the event JSON
  (`hook_event_name`, `cwd`, `session_id`, `tool_name`, `tool_input`,
  `tool_response`, `tool_success`) to the hook's stdin — only for a
  plugin-contributed hook (a `ragent.json` hook keeps the env-only contract) — and
  `CLAUDE_PLUGIN_ROOT` / `CLAUDE_PROJECT_DIR` join the environment.
- Guidance delivery: a `PostToolUse` hook's `hookSpecificOutput.additionalContext`
  is collected into `PostToolUseResult::Ok::additional_context` and folded into the
  tool's result parts, so the model sees the warning on the next iteration.
- Stop continuation (the Claude `asyncRewake` shape): new
  `hooks::run_stop_hooks` runs the `on_session_end` hooks and collects guidance
  from a blocking `exit 2` (stdout JSON `reason`/`additionalContext`, else stderr),
  an `exit 0` `hookSpecificOutput.additionalContext`, or a top-level
  `decision: "block"`. `processor` now runs it **inside** the loop at the
  no-tool-call exit: non-empty findings are appended as a synthetic user turn (with
  an `AgentNotice`) and the loop continues, bounded by
  `MAX_STOP_CONTINUATIONS = 3` so a hook that always blocks cannot spin. The
  fire-and-forget `OnSessionEnd` dispatch after the loop is removed (the hooks
  already ran).

Verify: `test_manifest.rs` (group shape, matchers, Claude `timeout`,
non-`command` types, `hooks/hooks.json` on disk),
`test_bridge.rs::scanned_plugin_hooks_reads_hooks_json_file`,
`test_plugin_hooks.rs` (matcher filtering, root/matcher round-trip,
`additionalContext` collection, blocking/quiet/context Stop hooks); live
`/plugins list` reports `Hooks 12` for `security-guidance`, and running the
plugin's own `PostToolUse` hook on a `yaml.load` edit emits the expected
security warning.
