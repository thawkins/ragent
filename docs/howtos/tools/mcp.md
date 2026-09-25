# Tools — MCP

Bridge to external Model Context Protocol (MCP) servers with auto-discovery
of known server types and stdio clients.

| Tool | Description |
|------|-------------|
| `mcp_tool` | Bridge to external Model Context Protocol servers. |

MCP server tools surface dynamically as `mcp_<server>_<tool>` at runtime
after discovery via the `/mcp discover` TUI command, or after `/mcp connect
<id>` enables and connects a server live. Servers bridged from an enabled
plugin's `mcpServers` section connect as `<plugin-id>.<server>` at startup.

---

## mcp_tool

The `McpToolWrapper` calls a tool hosted on a connected MCP server.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `server` | string | yes | MCP server name (as configured/discovered) | `"filesystem"` |
| `tool` | string | yes | Tool name exposed by the server | `"read_file"` |
| `arguments` | object | no | JSON payload forwarded to the server tool | `{"path":"/tmp/x"}` |

**Example:**
```text
mcp_tool server="github" tool="create_issue" arguments={"title":"Bug report"}
```

TUI commands: `/mcp` / `/mcp status` (server table), `/mcp discover` (probe for
known servers), `/mcp connect <id>` and `/mcp disconnect <id>` (enable/disable
and connect/disconnect live, persisted globally in `<state dir>/mcp_state.json`).
The wrapper refuses to call a disabled server's tools even when the tool is
still registered, and names the command that re-enables it.
