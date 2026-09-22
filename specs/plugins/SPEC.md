---
status: draft
audit:
  - { time: 1789883252, from: "none", to: "draft", actor: "system" }
---
# Specification: Plugin System - Codex and Claude Code/Desktop Plugin Loading with a JavaScript Runtime

## Overview

This specification defines a **plugin system** for ragent that discovers, loads, and
executes third-party plugins written for **OpenAI Codex** and **Claude Code / Claude
Desktop**, and runs their **JavaScript plugin code** inside ragent under a sandboxed
JavaScript runtime. Loaded plugins are presented with a stable **ragent plugin API**
(host API) that exposes a deliberately narrow set of capabilities: contributing tools to
the agent's tool registry, contributing slash commands, reading plugin-scoped
configuration, emitting messages to the message window, and writing structured log
entries.

The system is managed through a single slash-command family with six subcommands:

| Subcommand         | Purpose                                                        |
| ------------------ | -------------------------------------------------------------- |
| `/plugins list`    | List discovered plugins and their state (enabled/disabled/errored). |
| `/plugins add`     | Install a plugin from a local path or URL into the plugin store. |
| `/plugins remove`  | Uninstall a plugin from the plugin store.                      |
| `/plugins enable`  | Enable a disabled plugin (loads it on next session or immediately). |
| `/plugins disable` | Disable an enabled plugin (unloads it without deleting files).  |
| `/plugins help`    | Show usage for all `/plugins` subcommands.                     |
| `/plugins test`    | Load a plugin in an isolated test harness and report the result. |

### Why this exists

Codex and Claude Code/Desktop have growing third-party plugin ecosystems. Users who have
already installed or authored plugins for those tools currently gain nothing from them
inside ragent. A compatibility loading layer lets ragent reuse that ecosystem instead of
forcing authors to write ragent-native plugins from scratch, and lets ragent-native
plugins be written once in JavaScript against a documented host API.

### Worked examples

```text
/plugins help
/plugins add ./vendor-plugins/codex-weather               # from a local folder
/plugins add https://example.com/plugins/claude-todo.zip  # from a URL
/plugins list
/plugins enable codex-weather
/plugins test claude-todo
/plugins disable codex-weather
/plugins remove claude-todo
```

## Assumptions and interpretation (read before implementing)

The feature prompt leaves several decisions open. This specification fixes them
explicitly so the work is testable; each is also listed under `## Open Questions` so a
reviewer can overturn it deliberately.

- **A1 - JavaScript engine.** Plugin code runs on an embedded JavaScript engine
  integrated as a Rust crate, not on an external Node.js process. Rust-first engines
  (Boa) or QuickJS bindings (rquickjs) are the candidates; the choice is made in
  PLAN.md task T-004. Rationale: ragent ships a single statically-linked binary with
  zero runtime dependencies, so shelling out to Node.js is rejected.
- **A2 - Two manifest dialects, one internal model.** Codex plugins are recognised by a
  `codex-plugin.json` (or `.codex-plugin/plugin.json`, the official `openai/plugins`
  layout, or `plugin.json` with a `codex` marker) manifest; Claude
  Code/Desktop plugins are recognised by a `claude-plugin.json` (or
  `.claude-plugin/plugin.json`) manifest. Both dialects are normalised into one
  internal `PluginDescriptor`. A directory shipping *both* nested manifests
  (`.codex-plugin/plugin.json` and `.claude-plugin/plugin.json`) is a multi-target
  plugin, not an ambiguous one: it resolves to Claude, whose manifest carries the
  richer bridged surface. Fields that have no ragent equivalent (for example
  Claude Desktop's MCP-server transport sections) are recorded as
  `unsupported_capabilities` and reported, not silently dropped.
- **A3 - Plugins never execute until enabled and permitted.** Discovery and `/plugins
  list` parse manifests only; JavaScript code executes only for an **enabled** plugin
  during session start, or inside `/plugins test`. Disabling takes effect without a
  restart.
- **A4 - The host API is versioned and additive.** The API object presented to plugin
  code is `ragent` with `api_version: 1`. Version 1 is additive-only: fields and
  functions may be added, never removed or retyped. Plugin code declares the API
  version it was written against; ragent refuses plugins whose declared version is
  newer than the host's.
- **A5 - Plugin tools are first-class tools.** Tools contributed by a plugin are
  registered in the same tool registry as built-in tools under the name
  `plugin_<pluginid>_<toolname>`, are visible in `/tools`, and are subject to the
  normal permission system (default action: `ask`).
- **A6 - No npm ecosystem.** The plugin runtime does not resolve `node_modules`,
  does not fetch npm packages, and does not provide CommonJS `require` of arbitrary
  packages. A plugin must be self-contained (single file or a folder of files it can
  load through the host API's plugin-relative read).

## Background - existing machinery to reuse

- **Slash-command surface.** Commands are declared as `SlashCommandDef` entries in
  `crates/ragent-tui/src/app/state.rs` (`SLASH_COMMANDS`) and dispatched in
  `crates/ragent-tui/src/app/slash.rs`. The `/spec` family is the precedent for a
  multi-subcommand surface with `help`, validation, and usage text.
- **Tool registry.** Tools implement the `Tool` trait (`name`, `description`,
  `parameters` JSON schema, `execute`) and are registered in the session's tool
  registry; plugin-contributed tools will be registered under a `PluginToolAdapter`
  that marshals arguments into the JavaScript runtime and marshals the plugin's return
  value back into a tool result.
- **Permission system.** Tool calls pass through the allow/deny/ask rule engine
  (see `README.md` permission section). Plugin tools inherit this by registering as
  ordinary tools with permission domain `plugin:<pluginid>`.
- **Skills system precedent.** The skills system is the closest existing analogue for
  "loadable packs that inject capabilities into a session"; the plugin system's
  enable/disable lifecycle and discovery-path precedence (project > user > bundled)
  mirrors it.
- **Config layering.** Plugin store location and per-plugin enable state follow the
  existing config precedence: project `.ragent/` overrides user-global
  `~/.config/ragent/` (and legacy `~/.ragent/`).

## Definitions

| Term                    | Meaning                                                                                                          |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------- |
| **Plugin**              | A folder (or zip expanded to a folder) containing a recognised plugin manifest and, for a JavaScript plugin, one or more JavaScript files. A manifest that declares no entry point is a non-JS (skill-only / MCP-only) plugin. |
| **Plugin manifest**     | A JSON manifest in the Codex dialect (`codex-plugin.json` / `.codex-plugin/plugin.json` / marked `plugin.json`) or Claude dialect (`claude-plugin.json` / `.claude-plugin/plugin.json`). |
| **Plugin descriptor**   | The normalised internal representation of a manifest: id, name, version, dialect, optional entry-point file, permissions requested, declared host-API version, unsupported capabilities. |
| **Plugin store**        | The directory tree where added plugins live: project `.ragent/plugins/`, falling back to user-global `~/.config/ragent/plugins/`. |
| **Host API**            | The `ragent` object injected into the plugin's JavaScript global scope exposing versioned capabilities (tools, commands, config, messaging, logging). |
| **Plugin lifecycle**    | discovered -> validated -> enabled -> loaded -> unloaded; a plugin in any failed state is `errored` with a recorded cause. |
| **Test harness**        | The isolated load-and-exercise environment used by `/plugins test`: a throwaway session with a stubbed tool context and a captured message sink. |

## Requirements

### Ubiquitous requirements

**FR-001** — The system shall maintain a plugin store under the project `.ragent/plugins/`
directory, falling back to the user-global `~/.config/ragent/plugins/` directory, and
shall scan both locations on session start to discover plugins.

**FR-002** — The system shall recognise a plugin manifest in the Codex dialect
(`codex-plugin.json`, `.codex-plugin/plugin.json`, or `plugin.json` containing a `"codex"`
marker field) and in the
Claude Code/Desktop dialect (`claude-plugin.json`, or `.claude-plugin/plugin.json`), and
shall normalise either dialect into a single internal plugin descriptor carrying the
plugin id, name, version, dialect, requested permissions, declared host-API version, and
any unsupported capabilities. The descriptor's entry-point file is optional: a manifest
declaring no entry point normalises to an absent entry point (FR-028). A directory that
carries **both** nested dialect manifests (`.codex-plugin/plugin.json` and
`.claude-plugin/plugin.json`, the layout a single upstream tree ships when it targets
several hosts at once) shall not be rejected as ambiguous: it shall resolve
deterministically to the Claude dialect, whose manifest supports every bridged
contribution surface. Any other both-dialect match (mixed top-level
`codex-plugin.json`/`claude-plugin.json`, or a Codex-marked `plugin.json` beside a Claude
manifest) remains ambiguous and is refused.

**FR-003** — The system shall execute enabled plugins' JavaScript entry points on an
embedded JavaScript engine integrated as a Rust crate within the ragent binary, and
shall not require an external Node.js, Deno, or Bun installation.

**FR-004** — The system shall inject a versioned host-API object named `ragent` into each
plugin's JavaScript global scope, exposing at minimum: `api_version`, `register_tool`,
`register_command`, `config.get`, `message.info` / `message.warn` / `message.error`,
`log`, and `plugin.read_text_file` (plugin-relative file read).

**FR-005** — The system shall register tools contributed by an enabled plugin in the
session tool registry under names of the form `plugin_<pluginid>_<toolname>` with the
JSON parameter schema supplied by the plugin, and shall route invocations of those tools
through the plugin's JavaScript handler with arguments marshalled from JSON and results
marshalled back to JSON.

**FR-006** — The system shall provide the `/plugins` slash-command family with the six
subcommands `list`, `add`, `remove`, `enable`, `disable`, `help`, and `test`, registered
in the `SLASH_COMMANDS` registry, reachable through the slash-command dispatch arm,
listed in the autocomplete menu, and documented in `/plugins help`.

**FR-007** — When `/plugins add <source>` succeeds, the system shall install the plugin
files into the plugin store, shall validate the manifest, shall record the plugin as
**enabled** in the store ledger (so it loads at the next session start), and shall report
the plugin id and dialect discovered; no plugin JavaScript executes during the install.

### Event-driven requirements

**FR-008** — When a session starts, the system shall load each enabled plugin by
validating its descriptor, checking its declared host-API version against the host's,
allocating a fresh script context from the sandbox pool, injecting the host API, and
executing its entry point, and shall record the plugin's lifecycle state as `loaded` or
`errored` with the cause.

**FR-009** — When `/plugins list` is invoked, the system shall print one row per
discovered plugin showing id, name, version, dialect, state (`disabled`, `enabled`,
`loaded`, `errored`), and the count of contributed tools, commands, skills, agents, and
hooks, shall print the detailed names/sections of those contributions (skill names,
agent-profile paths, hook triggers), and shall print a summary line with totals and a
notice of any plugin that requested unsupported capabilities.

**FR-010** — When `/plugins add <source>` is invoked, the system shall treat `<source>`
as a local directory, a local `.zip`/`.tar.gz` package file, or an `https://` URL
pointing at a package file, shall refuse non-`https` URLs, and shall refuse installation
when a plugin with the same id already exists unless `--force` is supplied.

**FR-011** — When `/plugins enable <pluginid>` is invoked, the system shall mark the
plugin enabled, shall immediately attempt to load it into the current session using the
same procedure as session-start loading, and shall report the resulting state.

**FR-012** — When `/plugins disable <pluginid>` is invoked, the system shall mark the
plugin disabled, shall unload it from the current session, shall deregister every tool
and command the plugin contributed, and shall confirm how many tools and commands were
deregistered.

**FR-013** — When `/plugins test <pluginid>` is invoked, the system shall load the plugin
into an isolated test harness, shall execute its entry point with a captured message
sink and a stubbed host API, shall invoke each contributed tool once with schema-valid
sample arguments generated from the plugin's JSON schemas, shall report per-step
`[ ok ]` / `[fail]` results including wall-clock time, and shall then unload the harness
without touching the live session.

**FR-014** — When `/plugins help` is invoked, or `/plugins` is invoked with no
subcommand or an unrecognised one, the system shall display usage text documenting all
six subcommands, their arguments, and the accepted URL and local source forms, and shall
create or modify no files.

**FR-015** — When a plugin's JavaScript code raises an uncaught exception during entry
execution or during a tool invocation, the system shall catch the exception, record the
plugin state as `errored` (entry failure) or the tool call as failed with the exception
message (tool failure), and shall keep ragent itself running and stable.

### State-driven requirements

**FR-016** — While a plugin is disabled, the system shall not execute its code, shall not
register its tools or commands, and shall still list it in `/plugins list` with state
`disabled`.

**FR-017** — While a plugin's code is executing, the system shall enforce a resource
sandbox consisting of a wall-clock execution budget (default 5 seconds per tool
invocation, 10 seconds per entry execution, configurable via `plugins.max_execution_ms`
and `plugins.max_entry_ms`), a memory ceiling (default 64 MiB per script context,
configurable via `plugins.max_memory_mb`), and an interruption mechanism that terminates
execution when either budget is exceeded.

**FR-018** — While executing plugin code, the system shall expose no ambient filesystem,
network, or process APIs to the script context beyond the host API; all file access
shall be restricted to the plugin's own directory through `plugin.read_text_file`.

**FR-019** — While a plugin's declared host-API version is newer than the host's
`api_version`, the system shall refuse to load that plugin and shall report the version
mismatch.

### Optional requirements

**FR-020** — Where a plugin requests a permission it has declared (for example
`network.outbound`), the system may consult the normal permission rule engine with
permission domain `plugin:<pluginid>`; where the user has not granted the permission,
the capability shall be absent from that plugin's host-API instance.

**FR-021** — The system may expose a `ragent plugins` CLI subcommand family mirroring the
slash commands so the plugin store can be managed without launching the TUI.

**FR-022** — The system may persist plugin execution telemetry (invocation counts,
cumulative execution time, last error) per plugin and surface it in `/plugins list`
verbose mode (`/plugins list --verbose`).

### Unwanted requirements

**FR-023** — The system shall not execute a plugin's JavaScript code during discovery,
during `/plugins list`, or during `/plugins add`; manifest parsing only.

**FR-024** — The system shall not allow plugin JavaScript code to obtain the host API of
another plugin, to contribute a tool whose name collides with a built-in tool or with a
tool of another plugin, or to register a slash command whose name collides with an
existing slash command; collisions shall be rejected and recorded.

**FR-025** — The system shall not silently ignore a manifest containing capabilities the
host API cannot satisfy; it shall record them as unsupported capabilities and report them
in `/plugins list` and `/plugins test` output. A `skills` or `mcpServers`/
`mcp_servers` section that is bridged (FR-029, FR-030) is **not** an unsupported
capability; only a section whose shape has no bridged equivalent retains the label.

**FR-026** — The system shall never panic or terminate the ragent process because of
plugin code, including infinite loops, memory exhaustion, thrown exceptions, malformed
JSON returned from tool handlers, or re-entrant host-API calls; every such failure shall
be contained within the plugin sandbox and reported.

**FR-027** — The system shall not log, trace, or print the contents of plugin-internal
files or returned tool results at `info` level or above unless the plugin itself
explicitly calls `ragent.log` or `ragent.message.*`.

**FR-028** — The system shall recognise a plugin manifest that declares no JavaScript
entry point (no `entry`, `main`, or `server.entry`) as a **non-JS plugin** — the
skill-only / MCP-only shape the official Claude marketplace ships. A non-JS plugin shall
install, list, and load, and `/plugins test` shall report it as passing, without executing
any entry point: it contributes no tools or commands and holds no live sandbox. A manifest
that declares an entry field which is present but blank shall remain a parse error
(FR-002).

**FR-029** — The system shall bridge a plugin's `skills` section into the session's skill
discovery. Each declared skill directory (a string, or a list of strings, resolved relative
to the plugin root) of every **enabled** plugin in the project and user-global plugin
stores shall be appended to the skill-discovery roots, so a plugin's `SKILL.md` packs are
loaded by `SkillRegistry::load` exactly as a user skill directory is. A disabled plugin
contributes nothing (FR-016); a declared path that escapes the plugin root is dropped.

**FR-030** — The system shall bridge a plugin's `mcpServers`/`mcp_servers` section into the
session's MCP server set. Each server entry (an inline `{command, args, env, url, headers,
type}` object, or a top-level string naming an external MCP-config file — the Claude
marketplace `"mcpServers": "./mcp.json"` shape — resolved relative to the plugin root) of
every **enabled** plugin shall be mapped to a `McpServerConfig` with a plugin-qualified id
`<plugin-id>.<server>`, and connected at startup exactly like a server configured in
`ragent.json`. A server id colliding with a configured server shall be dropped (the
configured server wins); an unbridgeable entry shape retains the FR-025 unsupported label.

**FR-031** — The system shall surface a plugin's contributed slash commands on the session
command surface. A plugin declares commands in two forms: an **inline** command (a manifest
`commands[]` object with no `file`, or `ragent.register_command` at load time) dispatched
into the plugin's JavaScript sandbox, and a **prompt** command (a `commands/<name>.md` file
in the plugin root — the Claude marketplace shape — or a manifest object carrying a `file`)
whose YAML-frontmatter `description` is the menu text and whose body, after `$ARGUMENTS` /
`$N` substitution, is injected into the session as an ordinary user turn. A prompt command's
body is stripped of its frontmatter (all keys other than `description` ignored). Every
**enabled** plugin's commands are resolved when the session starts; a command whose name is
free registers under that bare name, and a colliding name registers under the namespaced
`plugin:<plugin-id>:<name>` trigger so a plugin can never shadow a built-in slash command, a
skill, or another plugin. The commands appear in the `/` autocomplete menu and in `/help`. A
disabled plugin contributes nothing (FR-016); a plugin whose manifest cannot be parsed
contributes nothing. An inline command requires the live plugin sandbox, so a surface without
one reports that it must be invoked through the plugin host rather than acting.

**FR-032** — The system shall bridge a plugin's `agents` section into the session's agent
discovery. Each declared agent profile (a string, or a list of strings, resolved relative to
the plugin root — a bare name resolving to `agents/<name>.md` — plus every `.md`/`.json` file
found directly under the plugin's `agents/` directory) of every **enabled** plugin in the
project and user-global plugin stores shall be loaded by the custom-agent loader exactly as a
user-provided profile under `.ragent/agents/`. A plugin-contributed profile may use ragent's
JSON frontmatter or the Claude-dialect YAML frontmatter; in both cases the markdown body
becomes the system prompt. Plugin-contributed profiles are scanned **after** the
`.ragent/agents/` directories, so an explicitly authored project agent always wins a name
clash. A disabled plugin contributes nothing (FR-016); a declared path that escapes the plugin
root is dropped; a profile that fails validation is reported as a diagnostic and skipped.

**FR-033** — The system shall bridge a plugin's `hooks` section into the session hook engine.
Each declared hook (an object keyed by trigger name with a command string or an array of
entries, each entry a string command or an object `{command|command_path, timeout_secs?}`, or a
flat array of `{trigger, command, timeout_secs?}` objects) of every **enabled** plugin shall be
normalised to a session hook (trigger + shell command) and merged with the hooks configured in
`ragent.json`. The trigger name is matched case- and separator-insensitively against the
session's triggers, accepting both ragent's snake_case spellings (`pre_tool_use`) and the Claude
plugin dialect's PascalCase spellings (`PreToolUse`); Claude's `UserPromptSubmit` maps onto
`on_turn_start`, `Stop`/`SessionEnd` onto `on_session_end`, and `PreCompact` onto
`on_compaction`. A hook whose trigger is not recognised is dropped. Configured hooks run before
plugin hooks at the same trigger, so a user's own hooks always take precedence. A disabled
plugin contributes nothing (FR-016); a `hooks` section whose shape yields no hook retains the
FR-025 unsupported label.

## Configuration schema

The top-level `Config` struct gains an optional `plugins` object:

```jsonc
{
  "plugins": {
    "enabled": true,                 // master switch; default true
    "max_execution_ms": 5000,          // per-tool wall-clock budget
    "max_entry_ms": 10000,             // entry-point wall-clock budget
    "max_memory_mb": 64,               // per-context memory ceiling
    "store_dir": null,                // optional override of the plugin store path
    "permissions": {                  // optional per-plugin permission grants
      "codex-weather": ["network.outbound"]
    }
  }
}
```

- Loaded and merged with the same precedence as other config sections (project overrides
  user-global).
- `plugins.enabled: false` makes the entire subsystem inert: no discovery, no loading,
  and `/plugins` subcommands other than `help` report that the system is disabled.

## Host API surface (version 1)

```text
ragent.api_version                          -> 1
ragent.plugin_id                            -> string
ragent.register_tool(def)                   -> { name, description, parameters(JSON schema), handler(fn) }
ragent.register_command(def)                -> { name, description, usage, handler(fn) }
ragent.config.get(key)                      -> JSON value from plugin-scoped config, or undefined
ragent.message.info(text) / .warn(text) / .error(text)
ragent.log(level, text)
ragent.plugin.read_text_file(relative_path) -> string (restricted to the plugin directory)
```

Marshalling rules: arguments cross the boundary as JSON values; handler return values may
be a JSON value or `{ "content": string, "isError"?: boolean }`; anything else is
serialised with `JSON.stringify` and, on `JSON.stringify` throwing, the tool call fails
with a serialisation error reported to the agent message window.

## Error handling

- Manifest parse failures mark the plugin `errored` with cause `manifest-parse` and the
  JSON error position; the plugin never loads.
- Entry-point execution failures mark the plugin `errored` with cause `entry` and the
  JavaScript exception message; the plugin's tools are deregistered if partial
  registration occurred.
- Tool-invocation failures (exception, timeout, memory ceiling, bad return value) fail
  the individual tool call and are recorded against the plugin's telemetry; they do not
  change the plugin's lifecycle state unless a configurable consecutive-failure threshold
  (default 3) is reached, at which point the plugin is auto-unloaded and marked
  `errored`.
- `/plugins` subcommand failures (unknown plugin id, existing id on add without
  `--force`, malformed arguments) are reported in the message window with the `[err]`
  marker and change no state.

## Security and privacy

- Plugin code runs with no access to the ragent process's filesystem, network stack,
  environment variables, or credentials other than what the host API explicitly exposes.
- The host API never exposes API keys, provider credentials, memory database handles, or
  session internals.
- Plugin store scanning follows symbolic links only within the store directory itself.
- `/plugins add` from a URL downloads over `https` only, to a size cap of 50 MiB, and
  extracts archives with path-traversal protection (no `..` or absolute entries).
- Third-party plugin code is untrusted: enabling a plugin is a trust decision and the
  enable path states the plugin's declared permissions before completing.

## Performance

- Plugin discovery and manifest parsing for up to 50 installed plugins shall complete in
  under 2 seconds on a developer workstation during session start (excluding enabled
  plugins' entry execution).
- An enabled plugin's entry execution within budget shall add at most 200 ms amortised to
  session start per plugin (budget separately bounded by FR-017).
- `/plugins test` on a plugin contributing up to 10 tools shall complete in under 15
  seconds excluding user permission prompts.

## Scope

In scope:

- Plugin store layout, discovery, manifest recognition of the Codex and Claude
  Code/Desktop dialects, and normalisation to one descriptor.
- Non-JS (skill-only / MCP-only) plugin packages: recognised, installed, listed, loaded
  inertly, and reported by `/plugins test` without executing an entry point (FR-028).
- Embedded JavaScript runtime with sandboxing (time, memory, interruption) and the
  versioned host API v1 (tools, commands, config, messaging, logging, plugin-relative
  file read).
- Tool and slash-command contribution with collision rejection and deregistration.
- Bridging a plugin's `skills` (FR-029), `mcpServers` (FR-030), `commands` (FR-031),
  `agents` (FR-032), and `hooks` (FR-033) sections into the session's skill, MCP,
  command, agent-discovery, and hook surfaces.
- The six `/plugins` subcommands on the TUI surface, plus optional CLI parity (FR-021).
- `/plugins test` isolated harness with per-tool sample-invocation reporting.
- Configuration schema, security posture, telemetry counters, and documentation.

Out of scope:

- Executing non-JavaScript plugin languages (TypeScript without a pre-built bundle,
  Python, Wasm) in the first iteration.
- npm/package-manager dependency resolution, `node_modules`, or CommonJS `require` of
  arbitrary packages.
- Running Claude Desktop's MCP-server sections as real MCP servers (recorded as
  unsupported capabilities only).
- Hot code reloading of an already-loaded plugin's files (disable/enable cycle achieves
  this).
- A graphical plugin marketplace or remote registry browser.
- Sandboxing at the OS-process level (seccomp, jails); the sandbox is in-process.

## Acceptance criteria

1. Installing the two fixture plugins (a Codex-dialect fixture and a Claude-dialect
   fixture) via `/plugins add` reports the correct id and dialect for each, and
   `/plugins list` shows both with state `enabled` and names and versions matching the
   fixtures.
2. `/plugins enable <codex-fixture>` followed by `/plugins list` shows state `loaded`
   and lists the fixture's tool; invoking that tool from the agent message loop returns
   the fixture's expected JSON result.
3. `/plugins disable <codex-fixture>` deregisters the fixture's tool (the tool no longer
   appears in the tool list and a direct invocation attempt reports "unknown tool").
4. `/plugins test <claude-fixture>` reports `[ ok ]` for discovery, manifest validation,
   entry execution, and every contributed tool's sample invocation, without registering
   anything in the live session.
5. A fixture whose JavaScript entry point contains an infinite loop is contained by the
   entry budget: enable fails with cause `entry` after roughly the configured entry
   timeout and the TUI remains responsive.
6. A fixture that attempts filesystem access outside its own directory is denied and the
   attempt is reported; no file outside the plugin directory is read.
7. `/plugins help`, `/plugins` with no arguments, and `/plugins bogus` all print the
   usage block and create no files.
8. With `plugins.enabled: false` in config, `/plugins list` reports the subsystem
   disabled and no plugin code executes.
9. A plugin declaring `api_version: 99` is refused at enable/test time with a version-
   mismatch report.

## Open Questions

1. **JavaScript engine choice** — Boa (pure Rust, easiest to embed, smaller spec
   coverage) vs `rquickjs` (QuickJS, near-full ES2020+, excellent interrupt/memory
   hooks). PLAN.md T-004 selects one; the deciding factor is the quality of
   memory-ceiling and deadline-interruption hooks, which FR-017 depends on. Default
   assumption: `rquickjs` with the `rquickjs::Runtime` memory limit and interrupt
   handler.
2. **Exact Codex manifest schema** — Codex plugin packaging is not yet formally
    standardised; this specification treats the Codex dialect as `codex-plugin.json`
    (or the official `.codex-plugin/plugin.json` layout) with `name`, `version`, `entry`,
    and optional `tools[]`/`commands[]` declarations.
    If OpenAI publishes a different schema, the recogniser in FR-002 is updated to match
    before implementation starts.
3. **Claude manifest variants** — Claude Code plugins and Claude Desktop extensions have
   slightly different manifests (`.claude-plugin/plugin.json` vs extension bundles).
   Should both be first-class, or should Desktop extensions be treated purely as
   unsupported-capability carriers? Default assumption: both recognised, MCP-server
   sections recorded as unsupported.
4. **Restart semantics for enable/disable** — default assumption: both take effect
   immediately in the current session (no restart), because the registry supports
   dynamic deregistration. If that proves unsafe, fall back to "takes effect on next
   session" and adjust FR-011/FR-012.
5. **Plugin-scoped configuration storage** — should `ragent.config.get` read from a
   per-plugin section of `ragent.json` (default assumption) or from a per-plugin
   `config.json` inside the plugin directory?
