# Web source

- URL: https://www.rapidevelopers.com/mcp-tutorial/how-to-connect-mcp-to-claude-desktop
- Title: Connect MCP to Claude Desktop | Setup Guide | RapidDev
- Author(s): RapidDev Engineering Team
- Language: English
- Published (UTC): 2026-03-28T10:59:13.417254+00:00
- Captured (UTC): 2026-10-02T13:29:47.482897666+00:00
- Relevance: Medium - multiple title terms match query


```text
Claude Desktop, MCP’s first AI host and reference implementation, is configured by editing `claude_desktop_config.json`—located at `~/Library/Application Support/Claude/` on macOS or `%APPDATA%\Claude\` on Windows—with an `mcpServers` object whose entries specify `command`, `args`, optional `env`, or `url` for remote Streamable HTTP servers. It launches configured servers as child processes; after a full restart (`Cmd+Q` on macOS), connected tools appear via a hammer icon, Claude asks permission before using them, and servers that fail to start are silently skipped. Key cautions include valid JSON, absolute paths, using `env` for secrets, `-y` for `npx`, testing with MCP Inspector, and that the npm GitHub MCP server was deprecated in April 2025 in favor of the Docker image `ghcr.io/github/github-mcp-server`; there is no hard-coded server limit, though 5–10 servers work well in practice.
```
