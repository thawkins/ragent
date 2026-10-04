# Web source

- URL: https://dev.to/danishashko/connect-claude-desktop-to-local-mcp-servers-2ia8
- Title: Connect Claude Desktop to Local MCP Servers
- Author(s): @
- Language: English
- Published (UTC): 2025-11-03T16:09:45+00:00
- Captured (UTC): 2026-10-02T13:28:32.550289802+00:00
- Relevance: Medium - multiple title terms match query


```text
The article explains how to connect Claude Desktop to local MCP servers, specifically a filesystem server, by editing `claude_desktop_config.json` (macOS: `~/Library/Application Support/Claude/`; Windows: `%APPDATA%\Claude\`) to run `npx -y @modelcontextprotocol/server-filesystem` with permitted directories such as Desktop and Downloads. It requires the latest Claude Desktop and Node.js LTS, notes the server runs with the user’s permissions while every file operation needs explicit approval, and says a hammer icon appears after a full restart to show available tools. Troubleshooting covers JSON syntax, absolute/existing paths, logs in `~/Library/Logs/Claude` or `%APPDATA%\Claude\logs`, and Windows `%APPDATA%` env issues. It also points to official MCP servers, custom Python/TypeScript servers, remote servers, and examples including Bright Data and Yahoo Finance MCP Server.
```
