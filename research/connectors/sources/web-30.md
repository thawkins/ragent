# Web source

- URL: https://dev.to/dennis-ddev/how-to-set-up-claude-desktop-with-mcp-servers-2026-guide-2fc7
- Title: How to Set Up Claude Desktop with MCP Servers (2026 Guide)
- Author(s): @
- Language: English
- Published (UTC): 2026-06-13T00:44:25+00:00
- Captured (UTC): 2026-10-02T13:28:43.266722637+00:00
- Relevance: High - title matches query


```text
This guide explains how to set up Claude Desktop with MCP (Model Context Protocol) servers to give Claude access to files, GitHub, databases, web search, and screenshot tools via `claude_desktop_config.json` (Settings > Developer > Edit Config; config paths vary by OS, and Anthropic has no official Linux build, recommending Claude Code CLI there). It notes Claude Desktop requires at least a $20/month Pro subscription for full usage, recommends Sonnet 4.6 as the daily driver, and details eight MCP servers: GitHub, Brave Search, Context7, Filesystem, Playwright, SnapRender, Figma, and Supabase, plus combined JSON configs, one-click Desktop Extensions in `.dxt`/`.mcpb` format, and remote servers using Streamable HTTP with OAuth 2.1. The suggested starter stack is Filesystem, Brave Search ($5/month credits, ~1,000 queries), and GitHub; SnapRender offers 200 free screenshots/month and paid plans from $9/month for 2,000, while troubleshooting emphasizes full restarts, valid JSON, API keys, Docker running, and logs, and best practices advise starting with 1–2 servers, using read-only modes, keeping secrets out of version control, and watching quotas.
```
