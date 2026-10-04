# Web source

- URL: https://amie.so/mcp/claude-desktop
- Title: Claude Desktop MCP Setup: Connectors and the Config File
- Author(s): @tryamie
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:30:01.041068614+00:00
- Relevance: High - title matches query


```text
Claude Desktop supports MCP via remote custom connectors in Settings > Connectors (paste a URL and complete OAuth, with no Node, config file, or restart) or local servers via `claude_desktop_config.json` under Settings > Developer > Edit Config, requiring Node and a full app quit/restart rather than just closing the window. The config file is at `~/Library/Application Support/Claude/` on macOS and `%APPDATA%\Claude\` on Windows, with servers defined using commands like `npx -y @some/mcp-server`. The guide recommends connectors for hosted URLs because OAuth replaces API keys and nothing runs locally; Amie’s remote server uses one URL to cover Google Calendar, Outlook, todos, and meeting notes across accounts.
```
