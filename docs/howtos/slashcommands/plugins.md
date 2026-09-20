# /plugins
> Plugin management: /plugins list [--verbose] | add <source> [--force] | remove <pluginid> | enable <pluginid> | disable <pluginid> | test <pluginid> | help

## Overview

`/plugins` manages third-party plugins written for OpenAI Codex and Claude
Code/Desktop. Plugin JavaScript runs on an embedded, budget-sandboxed engine
(`rquickjs`) inside the ragent binary - no external Node.js, Deno, or Bun
installation is required. Enabled plugins contribute tools (registered as
`plugin_<id>_<tool>`) and slash commands through a versioned `ragent` host API.

A bare `/plugins`, `/plugins help`, and any unrecognised subcommand all print the
same usage block and create or modify no files.

The same operations are available from a shell via the `ragent plugins <sub>`
CLI parity surface; the two surfaces share one help text and one set of command
handlers.

## Syntax

```
/plugins list [--verbose]      # list discovered plugins
/plugins add <source> [--force]  # install a plugin (stays disabled)
/plugins remove <pluginid>     # uninstall (refused while enabled)
/plugins enable <pluginid>     # enable, load, register tools/commands
/plugins disable <pluginid>    # unload, deregister tools/commands
/plugins test <pluginid>       # isolated harness: load + invoke each tool once
/plugins help                  # usage block
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/plugins list [--verbose]` | One row per discovered plugin showing id, name, version, dialect, state (`disabled`/`enabled`/`loaded`/`errored`), and contributed tool/command names, plus a totals summary. `--verbose` (or `-v`) appends per-plugin telemetry counters. |
| `/plugins add <source> [--force]` | Install a plugin and validate its manifest; reports the plugin id and dialect. The plugin stays disabled until enabled. Refuses a duplicate id unless `--force` is given. |
| `/plugins remove <pluginid>` | Uninstall a plugin from the store. Refused while the plugin is enabled. |
| `/plugins enable <pluginid>` | Mark the plugin enabled, load it into the current session, and register its tools and commands. Reports the declared permissions and the load outcome. |
| `/plugins disable <pluginid>` | Mark the plugin disabled, unload it, and deregister every tool and command it contributed, confirming how many of each were removed. Files are not deleted. |
| `/plugins test <pluginid>` | Load the plugin in an isolated harness (never touching the live session), invoke each contributed tool once with schema-derived sample arguments, and report per-step `[ ok ]`/`[fail]` results with wall-clock time. |
| `/plugins help` | Print the usage block. A bare `/plugins` or an unknown subcommand does the same. |

### Sources accepted by `/plugins add`

- a local directory containing a plugin manifest;
- a local `.zip` or `.tar.gz` package file;
- an `https://` URL pointing at a `.zip`/`.tar.gz` package (non-`https` URLs are refused).

## Examples

List everything with telemetry:

```
/plugins list --verbose
```

Install a plugin from a local directory and enable it:

```
/plugins add ./my-plugin
/plugins enable my-plugin
```

Test a plugin without touching the live session:

```
/plugins test my-plugin
```

Unload without deleting files:

```
/plugins disable my-plugin
```

## Output

Reports are prefixed with a `From: /plugins <sub>` attribution line. `list`
renders a fixed-width table:

```
|ID |Name |Version |Dialect |State    |Tools|Commands|
```

followed by `Contributions:`, `Unsupported capabilities:`, and `Errors:`
sections and a summary line (`Total: N plugin(s) - X loaded, X enabled, ...`).

`test` renders one line per harness step:

```
[ ok ] discovery (2 ms)
[ ok ] manifest (0 ms)
[ ok ] entry (5 ms)
[ ok ] tool ping (1 ms)
```

and ends by confirming the harness was unloaded with the live session untouched.

## Configuration

The `plugins` block in `ragent.json` controls the subsystem (`enabled`,
`max_execution_ms`, `max_entry_ms`, `max_memory_mb`, `store_dir`, and
per-plugin `permissions`). With `plugins.enabled: false`, every subcommand
other than `help` reports the subsystem is disabled and no plugin code executes.
Plugins are discovered under `.ragent/plugins/` (project), falling back to
`~/.config/ragent/plugins/` (user-global).

See [`docs/howtos/config.md`](../config.md) §7.37 and
[`specs/plugins/SPEC.md`](../../../specs/plugins/SPEC.md).

## Related

- [`docs/howtos/plugins.md`](../plugins.md) - the full plugin-system manual
- `/reload` - reload customizations after plugin files change
- `/tools` - toggle tool visibility (plugin tools are always advertised while enabled)
- `ragent plugins <sub>` - CLI parity for the same subcommands
- `specs/plugins/SPEC.md` - the full plugin-system specification
