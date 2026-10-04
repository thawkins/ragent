# Web source

- URL: https://policylayer.com/integrations/claude-desktop
- Title: How to Add MCP Servers to Claude Desktop: Config Guide
- Author(s): PolicyLayer, @PolicyLayer
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:29:04.412905152+00:00
- Relevance: Medium - multiple title terms match query


```text
PolicyLayer’s Claude Desktop integration page details MCP troubleshooting and a gateway-based policy layer. Claude Desktop reads config only at startup, so edits require a full quit/relaunch; config files are `~/Library/Application Support/Claude/claude_desktop_config.json` on macOS and `%APPDATA%\Claude\claude_desktop_config.json` on Windows, PATH issues require absolute `npx`/`node` paths, and logs live in `~/Library/Logs/Claude/` or `%APPDATA%\Claude\logs\` (`mcp.log`, `mcp-server-NAME.log`). To enforce policy, register a filesystem MCP in PolicyLayer, mint a grant, and point Claude Desktop at `https://proxy.policylayer.com/mcp/<server-uuid>/` via `mcp-remote` with an `Authorization: Bearer <grant-token>` header, so every call is evaluated before reaching upstream. The gateway can cap calls per minute/hour/day, allow/deny/conditionally gate tools by arguments, count usage and deny over budget, and record grant, tool, argument keys, and deciding rule; an example policy defaults to deny, allows `read_file`/`list_directory`, and denies `write_file` to `^/Users/[^/]+/(\.ssh|\.aws|secrets)`. It also notes remote MCP custom connectors with OAuth, `.mcpb` desktop extensions, and compliance mapping for SOC 2, HIPAA, and GDPR.
```
