# Web source

- URL: https://mcpverdict.com/mcp/clients/claude-desktop
- Title: Claude Desktop MCP: Transport Support, Extensions, Limits (2026)
- Author(s): -
- Language: English
- Published (UTC): 2026-06-28T13:45:18+00:00
- Captured (UTC): 2026-10-02T13:29:35.763252726+00:00
- Relevance: High - title + snippet match query


```text
Claude Desktop, the first MCP client from Anthropic’s November 2024 launch, supports local servers via stdio and remote servers via Streamable HTTP Custom Connectors (Pro/Max/Team/Enterprise; traffic routes through Anthropic’s cloud and requires public reachability), lacks native SSE support (requiring the mcp-remote bridge), fully supports tools but only limited resources/prompts, and offers three install paths: one-click .mcpb Desktop Extensions (formerly .dxt), manual JSON “mcpServers” config, or Custom Connectors. It has no hard server limit but a practical ceiling of 3–5 servers because each server’s tool definitions consume 500–2,000 context tokens—one user ran 32 servers/473 tools using 140k–150k of a 200k-token window—and uses per-call approval with “Always approve”; full MCP requires Claude Pro at $20/month, while free users get only pre-built Anthropic-managed connectors. Compared with Cursor (40-tool ceiling, no per-tool permissions) and Claude Code (all three primitives, four transports, granular allow/deny/ask rules), Claude Desktop is best for conversational and non-developer workflows; MCP has been governed since December 2025 by the Agentic AI Foundation under the Linux Foundation, co-founded by Anthropic, Block, and OpenAI.
```
