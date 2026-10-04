# Web source

- URL: https://platform.claude.com/docs/en/agents-and-tools/mcp-connector
- Title: MCP connector
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:29:15.955743151+00:00
- Relevance: High - title matches query


```text
Claude's MCP connector lets the Messages API connect directly to remote MCP servers—without a separate MCP client—supporting tool calling, allowlist/denylist and per-tool configuration, OAuth bearer tokens, and multiple servers per request; only tool calls from the MCP spec are supported, and servers must be publicly exposed over HTTP (Streamable HTTP or SSE, not local STDIO). Configuration uses an `mcp_servers` array (URL, auth) plus one `MCPToolset` per server in the `tools` array, with merging precedence of tool-specific `configs` > `default_config` > system defaults; validation requires every server to be referenced by exactly one toolset, while unknown tool names only log a backend warning. The `mcp-client-2026-09-15` beta header pins a server's tool list via `mcp_tool_listing` blocks (superseding `mcp-client-2025-11-20`, which replaces the deprecated `mcp-client-2025-04-04` header that kept tool config inside the server definition), and `inline-tools-2026-09-15` allows adding a server mid-conversation. The connector is available on the Claude API, Claude Platform on AWS, and Microsoft Foundry (all beta), works with the Message Batches API at the same pricing, is not covered by ZDR arrangements, and offers SDK helpers for converting MCP tools, prompts, and resources (throwing `UnsupportedMCPValueError` on unsupported values).
```
