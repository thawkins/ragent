# Web source

- URL: https://www.octavehq.com/post/how-to-set-up-mcp-servers-in-claude-desktop-complete-guide
- Title: Octave | How to Set Up MCP Servers in Claude Desktop (Complete Guide)
- Author(s): Guest
- Language: English
- Published (UTC): 2026-02-25T00:00:00+00:00
- Captured (UTC): 2026-10-02T13:30:22.526765176+00:00
- Relevance: High - title matches query


```text
This guide explains how to set up Model Context Protocol (MCP) servers in Claude Desktop, an open protocol that standardizes AI connections to external files, databases, APIs, and tools via resources, tools, and prompts. It lists prerequisites—latest Claude Desktop (MCP support added late 2024), Node.js v18+, optional Python 3.10+, and admin access—and shows configuration through `claude_desktop_config.json` at OS-specific paths (macOS `~/Library/Application Support/Claude/`, Windows `%APPDATA%\Claude\`, Linux `~/.config/Claude/`) using `mcpServers` entries with `command`, `args`, and `env`. Setup examples include filesystem, PostgreSQL, SQLite, GitHub, Brave Search, and Slack MCP servers; multiple servers can run simultaneously, and troubleshooting covers JSON syntax, permissions, dependencies, credentials, port conflicts, and logs. For GTM teams, MCP can connect Claude to CRM/prospect data and tools, but Claude Desktop is single-user; the article promotes Octave as a centralized shared context layer for team-wide operationalization.
```
