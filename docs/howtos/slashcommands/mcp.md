# /mcp
> MCP servers: /mcp [status] | /mcp discover | /mcp connect <id> | /mcp disconnect <id> | /mcp help

## Overview

`/mcp` reports the state of Model Context Protocol servers and launches
discovery. The bare form (the `status` token lands in the same arm) prints a
server table from the current config and runtime state; `/mcp discover` scans
for available servers and opens an interactive selection dialog.

Be aware of the implementation boundaries: only `help`, `discover`,
`connect <id>`, and `disconnect <id>` subcommands exist. `connect <id>` is a
stub that reports "not yet implemented" after validating the id, and
`disconnect <id>` is always a stub. There is no `/mcp call` subcommand - it
is mentioned in some README text but the dispatcher does not implement it, so
do not document or attempt it. Tool calls to MCP servers go through the
`mcp_tool` tool in chat instead.

## Syntax

```
/mcp                      # server status table (status token behaves the same)
/mcp status               # same table
/mcp discover             # scan for MCP servers, open selection dialog
/mcp connect <id>         # stub: validates the id, then "not yet implemented"
/mcp disconnect <id>      # stub: always reports "not yet implemented"
/mcp help                 # usage help
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/mcp` | Print the MCP server table |
| `/mcp status` | Same as the bare form |
| `/mcp discover` | Run `McpClient::discover()` and open the interactive dialog |
| `/mcp connect <id>` | Look up `<id>` in config; found -> reports a not-yet-implemented stub |
| `/mcp disconnect <id>` | Always reports a not-yet-implemented stub |
| `/mcp help` | Print usage help |

Not implemented (do not use):

| Form | Status |
| --- | --- |
| `/mcp call <server> <tool>` | No such subcommand in the dispatcher |

## Examples

Show the server table:

```
/mcp
```

Scan for servers and pick one interactively:

```
/mcp discover
```

Validate a server id (expect the stub message):

```
/mcp connect filesystem
```

Expect a stub message even for a valid id:

```
/mcp disconnect filesystem
```

Print the usage help:

```
/mcp help
```

Call an MCP tool from chat (the supported path):

```
mcp_tool(server: "filesystem", tool: "read_file", arguments: {"path": "README.md"})
```

## Output

Bare form (From: /mcp, MCP Servers):

- With no servers configured, an advisory line suggests running
  `/mcp discover` to scan for available servers and then adding them to the
  `mcp` section of `ragent.json` to activate them.
- Otherwise one line per server: the server id padded to a fixed width,
  a status glyph (rendered as an emoji in the terminal - described here as
  connected, disabled, needs auth, or failed with the error text), and
  `tools: <n>` when the server exposes tools.
- A summary line: `<connected>/<total> server(s) connected`.
- A footer listing the subcommands:
  `/mcp discover`, `/mcp connect <id>`, `/mcp disconnect <id>`.

`/mcp discover` produces no transcript output; it opens an interactive
dialog (numbered selection list with a feedback line) fed by
`McpClient::discover()`.

`/mcp connect <id>`:

- Missing id: `Usage: /mcp connect <id>`
- Id not in config: `MCP '<id>' not found in config`
- Id found: `MCP connect not yet implemented for '<id>'`

`/mcp disconnect <id>`:

```
MCP disconnect not yet implemented for '<id>'
```

## Related

- `mcp_tool` - the chat tool for calling MCP server tools
- `/reload mcp` - rescan MCP configuration after editing `ragent.json`
- `mcp` section in `ragent.json` - server definitions and enablement
- `/doctor` - includes MCP connectivity in its diagnostics