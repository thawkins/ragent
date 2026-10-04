# Web source

- URL: https://mcpplaygroundonline.com/blog/how-to-setup-mcp-claude-desktop
- Title: How to Set Up MCP in Claude Desktop (Complete 2026 Guide)
- Author(s): Nikhil Tiwari
- Language: English
- Published (UTC): 2026-01-12T00:00:00+00:00
- Captured (UTC): 2026-10-02T13:29:56.950651626+00:00
- Relevance: Medium - multiple title terms match query


```text
An April 2026 guide explains setting up MCP servers in Claude Desktop either via one-click Desktop Extensions (.dxt) dragged into Settings → Extensions or manual editing of claude_desktop_config.json, located at `~/Library/Application Support/Claude/claude_desktop_config.json` on macOS, `%APPDATA%\Claude\claude_desktop_config.json` on Windows, and `~/.config/Claude/claude_desktop_config.json` on Linux; prerequisites are Node.js 18+ for npm servers and Python 3.10+ for Python servers. Configs use an `mcpServers` object with `command`, `args`, and `env` fields, with examples including the filesystem server via `npx @modelcontextprotocol/server-filesystem`, GitHub’s official Docker server (`ghcr.io/github/github-mcp-server`, requiring Docker Desktop and `GITHUB_PERSONAL_ACCESS_TOKEN`), Supabase’s official npx server (`--read-only`, `--project-ref`, `SUPABASE_ACCESS_TOKEN`), and Microsoft Playwright’s `npx @playwright/mcp@latest`. After saving, users must fully restart Claude Desktop and verify tools via the hammer (🔨) icon; multiple servers can run simultaneously. Remote servers are added through Settings → Connectors → Add custom connector using Streamable HTTP (MCP protocol 2025-11-25) with OAuth, available on Pro, Max, Team, and Enterprise plans, while SSE is deprecated; troubleshooting emphasizes full executable paths due to minimal PATH, and security recommends limiting file access, minimal token scopes, reviewing server code, and not sharing configs.
```
