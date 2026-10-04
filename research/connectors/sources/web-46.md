# Web source

- URL: https://handsonai.info/builder-setup/mcp-connectors-setup
- Title: MCP Connectors Setup Guide
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:30:03.663004841+00:00
- Relevance: High - title matches query


```text
Claude connects to external tools via built-in Integrations (GitHub, Google Drive, Gmail, Google Calendar; available on Free, Pro, Max, Team, and Enterprise), Custom Connectors for remote MCP servers (Claude web/Desktop; Pro, Max, Team, Enterprise), and Desktop Extensions for local MCP servers (Claude Desktop only). Setup is through the chat `+` button or Settings → Connectors/Extensions, with Team/Enterprise org owners needing to enable integrations in Admin settings → Connectors; custom connectors require a remote MCP server URL and optional OAuth, while custom local MCP servers require editing `claude_desktop_config.json` at macOS `~/Library/Application Support/Claude/` or Windows `%APPDATA%\Claude\` and restarting Claude Desktop. Per-tool permissions can be set to Always allow, Ask each time, or Never; read-only tools like `notion-fetch`, `notion-search`, and `notion-get-users` are often allowed, while write/delete tools like `notion-create-pages`, `notion-update-database`, and `notion-move-pages` are often set to Ask each time. Security notes include trusting only known server organizations, reviewing OAuth permission requests, encrypted data transfers, integrations only in private projects, and not sharing chats with synced content; troubleshooting covers plan/owner enablement, MCP URL/OAuth, valid JSON/absolute paths, complete restart, and macOS logs at `~/Library/Logs/Claude/mcp.log`.
```
