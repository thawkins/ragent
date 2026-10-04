# Web source

- URL: https://docs.aigateway.cequence.ai/docs/client-config/claude-desktop
- Title: Claude Desktop MCP Configuration Guide | Cequence AI Gateway
- Author(s): -
- Language: English
- Published (UTC): 2026-03-12T23:45:06+00:00
- Captured (UTC): 2026-10-02T13:28:30.038781600+00:00
- Relevance: High - title matches query


```text
Claude Desktop supports remote MCP servers via Custom Connectors, the recommended UI-based method configured under Settings → Connectors (Team/Enterprise org owners must first enable connectors in Admin settings) by adding the Cequence AI Gateway MCP server URL, optionally entering OAuth Client ID/Secret, authenticating, and enabling/disabling tools. The alternative is editing `claude_desktop_config.json` at `~/Library/Application Support/Claude/` on macOS or `%APPDATA%\Claude\` on Windows to add `mcpServers` entries using `npx -y @cequenceai/mcp-remote <your-mcp-server-url>`, then restarting Claude Desktop. Troubleshooting includes validating JSON, checking the MCP server URL/deployment/credentials, and ensuring Node.js/npx is installed.
```
