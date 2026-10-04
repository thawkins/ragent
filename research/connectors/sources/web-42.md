# Web source

- URL: https://docs.aembit-eng.com/user-guide/mcp-server/connect/claude-desktop
- Title: Connect with Claude (Desktop/web)
- Author(s): Aembit
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:29:52.443442725+00:00
- Relevance: High - title matches query


```text
Aembit’s docs describe connecting Claude Desktop or Claude on the web to the Aembit MCP Server to query audit logs, authorization events, and workload events from an Aembit Tenant via two methods: the Connectors UI OAuth/SSO method, usable in both Claude Desktop and Claude on the web, which uses an Access Policy, an IdP, a Client Workload redirect URI of `https://claude.ai/*`, and a Server Workload for host `<tenantId>.mcp.useast2.aembit.io` on port 443 with URL path `/mcp`; and a local configuration method for Claude Desktop only, using a static Aembit API Token with an `mcp-remote` stdio-to-HTTP bridge, requiring Node.js/`npx` and `claude_desktop_config.json`. Setup includes an Aembit Access Token Credential Provider with default 900-second lifetime and optional refresh support, after which Claude can call tools such as `get_audit_logs`, `get_audit_events`, and `get_workload_events`. Troubleshooting notes local logs at `~/Library/Logs/Claude/mcp-server-aembit.log` on macOS or `%APPDATA%\Claude\logs\mcp-server-aembit.log` on Windows, with common failures including missing `npx`/Node PATH, 502 errors from MCP Service URL stack mismatch, and 401 errors from expired Aembit API Tokens, which default to a 1-hour lifetime.
```
