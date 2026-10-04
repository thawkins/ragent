# Web source

- URL: https://docs.hyperstack.cloud/docs/libraries/mcp-server/claude-desktop-setup
- Title: Claude Desktop Setup - Hyperstack Docs
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:29:27.499508897+00:00
- Relevance: Medium - multiple title terms match query


```text
The Hyperstack docs describe connecting Claude Desktop to the Hyperstack API MCP Server via either the hosted endpoint `https://console.hyperstack.cloud/ai/mcp`, which uses a native custom HTTP connector and browser-based Hyperstack account authentication with no Node.js/mcp-remote install, or a self-hosted endpoint `http://127.0.0.1:8080/mcp`, supported for local testing only and requiring Node.js v18.0.0+, globally installed `mcp-remote`, and a `claude_desktop_config.json` entry with the Node executable path, `proxy.js` path, and server URL, authenticated by the server’s API key. The docs warn that this MCP server can create, modify, and delete real Hyperstack resources and may incur charges, with NexGen Cloud disclaiming liability, and recommend validation prompts such as “Show me all hyperstack tools” for hosted and “Show me the available tools” for self-hosted. Claude Code can also connect directly over HTTP without Node.js or the `mcp-remote` proxy.
```
