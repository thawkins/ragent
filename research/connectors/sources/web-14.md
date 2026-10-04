# Web source

- URL: https://falconer.com/docs/mcp-and-cli/connect
- Title: Connect MCP
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:27:36.529915339+00:00
- Relevance: High - title matches query


```text
Falconer’s MCP endpoint is `https://falconer.com/api/mcp`; connecting it opens Falconer OAuth in the browser and lets an AI client search, read, create, and update Falconer documents. Falconer supports Claude.ai (custom connector), Claude Code (with `--client-id falconer-claude-code` and `--callback-port 49152`), Cursor app (`.cursor/mcp.json`) and Cursor Agent, and Codex CLI (with `--oauth-client-id falconer-codex-cli` and `--oauth-resource https://falconer.com/api/mcp`). To fully disconnect, revoke access in Personal settings > Connected accounts, then remove Falconer from the MCP client. Troubleshooting includes restarting the client for missing tools, reconnecting for auth/token issues, and calling `list_organizations` to get an `organizationId`; unsupported clients can use the Falconer CLI.
```
