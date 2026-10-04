# Web source

- URL: https://docs.aembit.io/user-guide/mcp-server/connect/claude-desktop
- Title: Connect with Claude (Desktop/web)
- Author(s): Aembit
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:30:55.264734802+00:00
- Relevance: High - title matches query


```text
The Aembit MCP Server lets Claude Desktop or Claude on the web query Aembit Tenant audit logs, authorization events, and workload events through either a Connectors UI OAuth/SSO flow or a Claude Desktop-only local configuration using a static API Token with the `mcp-remote` stdio-to-HTTP bridge. The OAuth method registers Claude as an Aembit OAuth client, authenticates against the tenant IdP, and issues an auto-refreshed Aembit Access Token; it requires MCP URL `<tenantId>.mcp.useast2.aembit.io`, a Client Workload Redirect URI `https://claude.ai/*` with Enforce SSO, a Server Workload using MCP host/port 443/path `/mcp`, and an Aembit Access Token Credential Provider with a default 900-second lifetime. The local method requires Node.js/`npx` on PATH plus an Aembit API Token and MCP URL, with config at macOS `~/Library/Application Support/Claude/claude_desktop_config.json` or Windows `%APPDATA%\Claude\claude_desktop_config.json`; tools include `get_audit_logs`, `get_audit_events`, and `get_workload_events`, while troubleshooting covers missing `npx`, 502 errors from URL/stack mismatch or missing `/mcp`, and 401 errors from expired API tokens (default 1-hour lifetime), with logs at macOS `~/Library/Logs/Claude/mcp-server-aembit.log` and Windows `%APPDATA%\Claude\logs\mcp-server-aembit.log`.
```
