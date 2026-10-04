# Web source

- URL: https://www.grizzlypeaksoftware.com/library/integrating-mcp-with-claude-desktop-ipczfkci
- Title: Integrating MCP with Claude Desktop - Library - Grizzly Peak Software
- Author(s): Shane Larson, @PeakGrizzly
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:30:09.331147573+00:00
- Relevance: Medium - multiple title terms match query


```text
This guide explains that Claude Desktop, Anthropic’s standalone app, acts as a full MCP host with a built-in client: it spawns configured MCP servers as child processes and communicates via JSON-RPC 2.0 over stdio, exposing server tools in conversations after user approval. Configuration lives in `claude_desktop_config.json` (macOS: `~/Library/Application Support/Claude/`; Windows: `%APPDATA%\Claude\`; Linux: `~/.config/Claude/`) using an `mcpServers` map with `command`, `args`, and `env`; prerequisites include Node.js 18+, npm, and JSON familiarity. It covers official/community servers such as `@modelcontextprotocol/server-filesystem` (tools like `read_file`, `write_file`, `list_directory`, `search_files`), `server-github`, and `server-postgres`, plus custom servers built with `@modelcontextprotocol/sdk` and `zod`, tested via MCP Inspector at `http://localhost:5173`. Key operational/security points include restarting Claude Desktop after config changes, using `console.error` rather than `console.log`, lazy-loading resources, setting timeouts, restricting filesystem access and database credentials to read-only, and debugging via Claude Desktop MCP logs.
```
