# Web source

- URL: https://falconer.com/docs/mcp-and-cli/quickstart
- Title: Quickstart with MCP
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:28:06.368997592+00:00
- Relevance: High - title matches query


```text
Falconer MCP connects compatible AI clients such as Claude, Claude Code, Cursor, Cursor Agent, and Codex CLI to Falconer through the HTTP endpoint `https://falconer.com/api/mcp`. Setup uses OAuth/custom connectors: Claude Code via `claude mcp add --transport http --scope user --client-id falconer-claude-code --callback-port 49152 "falconer" "https://falconer.com/api/mcp"` then `/mcp`; Cursor app via `.cursor/mcp.json` or the MCP settings, Cursor Agent by adding a server named Falconer with that URL and clicking Login; and Codex CLI via `codex mcp add falconer --url "https://falconer.com/api/mcp" --oauth-client-id falconer-codex-cli --oauth-resource "https://falconer.com/api/mcp"`. To verify, ask the client to “List my Falconer organizations,” search for a deployment runbook, or read a Falconer doc; if the account has one organization, `organizationId` can usually be omitted, but with multiple organizations users should list organizations first and include the selected `organizationId` in later requests.
```
