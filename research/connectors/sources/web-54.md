# Web source

- URL: https://www.mcpgee.com/tutorials/claude-integration
- Title: Connect MCP Servers to Claude Desktop &amp; Claude Code
- Author(s): MCPgee Team
- Language: English
- Published (UTC): 2026-10-01T21:13:34.731+00:00
- Captured (UTC): 2026-10-02T13:31:01.543099654+00:00
- Relevance: High - title matches query


```text
The page is an intermediate guide to integrating Model Context Protocol (MCP) servers with Anthropic’s Claude Desktop and Claude Code. Claude Desktop acts as an MCP client connecting to multiple stdio-based servers as child processes, configured in `claude_desktop_config.json` at platform-specific paths (macOS `~/Library/Application Support/Claude/`, Windows `%APPDATA%\Claude\`, Linux `~/.config/Claude/`), with fields for `command`, `args`, and `env`; Python servers must set `PYTHONUNBUFFERED=1`. Claude Code uses `claude mcp add`, `claude mcp list`, and `claude mcp remove`, with project scope in `.claude/settings.json` and user scope in `~/.claude/settings.json`. Servers must use stdio transport, log only to stderr, handle initialization and SIGINT/SIGTERM, and provide clear tool descriptions; troubleshooting includes validating JSON, using absolute paths, fully restarting Claude Desktop, and checking logs.
```
