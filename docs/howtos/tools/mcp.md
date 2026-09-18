# Tools — MCP

Bridge to external Model Context Protocol (MCP) servers with auto-discovery
of known server types and stdio clients.

| Tool | Description |
|------|-------------|
| `mcp_tool` | Bridge to external Model Context Protocol servers. |

MCP server tools surface dynamically as `mcp_<server>_<tool>` at runtime
after discovery via the `/mcp discover` TUI command.

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

TUI commands: `/mcp discover` (probe for known servers), `/mcp list`,
`/mcp call`.
