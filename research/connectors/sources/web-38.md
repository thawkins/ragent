# Web source

- URL: https://mcppedia.org/setup
- Title: How to Set Up MCP Servers - MCPpedia
- Author(s): @MCPpedia
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:29:32.655533630+00:00
- Relevance: High - title matches query


```text
MCPpedia’s setup page is a visual step-by-step guide for configuring any MCP server in Claude Desktop, Cursor, Claude Code, or VS Code. It troubleshoots common failures: fully quit and reopen the app, validate JSON for trailing commas, place Claude Desktop config at `~/Library/Application Support/Claude/claude_desktop_config.json` on macOS or `%APPDATA%\Claude\claude_desktop_config.json` on Windows, merge `mcpServers` alongside existing keys, and install Node.js LTS v18+ so `npx` works. Remote servers can be set up natively with `url`/`transport: streamable-http` or through the `mcp-remote` proxy; OAuth failures may require clearing `~/.mcp-auth`, and multiple servers are added as separate keys under `mcpServers`.
```
