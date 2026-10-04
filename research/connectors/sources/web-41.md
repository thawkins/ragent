# Web source

- URL: https://usingclaude.com/en/claude-code/mcp/how-to-connect-mcp
- Title: How to Connect &amp; Use MCP with Claude: 4 Ways to Add Connectors
- Author(s): Using Claude Editorial Team
- Language: English
- Published (UTC): 2026-06-02T18:23:54.167589+00:00
- Captured (UTC): 2026-10-02T13:29:41.925477442+00:00
- Relevance: High - title matches query


```text
The page outlines four main ways to connect MCP to Claude: directory connectors (easiest), custom remote MCP servers, local MCP servers via Claude Desktop’s config file, and Claude Code CLI. Directory connectors such as Google Drive, Slack, GitHub, and Notion use OAuth and work on claude.ai, Claude Desktop, and iOS/Android across all plans including Free, while custom connectors require a server URL and auth, connect from Anthropic’s cloud, must be publicly reachable (not behind firewall/VPN), and support Streamable HTTP. Local servers run over stdio using `claude_desktop_config.json` on macOS/Windows/Linux and are unavailable on claude.ai web or Cowork; Claude Code uses `claude mcp add --transport http <name> <url>`, with OAuth 2.0 for auth and `/mcp` for 401/403 errors. The article warns that custom/unverified servers should only be used from trusted sources, notes Free allows one custom connector versus more on paid plans, and mentions troubleshooting for JSON errors, full app restart, and `claude mcp add` not found requiring `npm install -g @anthropic-ai/claude-code`.
```
