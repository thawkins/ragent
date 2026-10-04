# Web source

- URL: https://dev.to/cloudeval-ai/give-cursor-claude-code-or-codex-your-azure-architecture-over-mcp-1b31
- Title: Give Cursor, Claude Code or Codex your Azure architecture over MCP
- Author(s): @
- Language: English
- Published (UTC): 2026-10-02T08:00:33+00:00
- Captured (UTC): 2026-10-02T13:27:41.102804338+00:00
- Relevance: Medium - multiple title terms match query


```text
Prateek Singh’s 2 October 2026 post (checked against @ganakailabs/cloudeval-cli 0.38.5) explains how to connect Cursor, Claude Code, or Codex to Azure architecture via the Cloudeval CLI’s local stdio MCP server using `npx -y @ganakailabs/cloudeval-cli mcp serve --toolset readonly`, Node.js 20+, and a project-scoped, 30-day MCP Read-only key stored as `CLOUDEVAL_ACCESS_KEY`. The default readonly toolset lets agents read project architecture graphs, saved cost and Well-Architected reports, and the validation catalogue, but prevents deploying, changing, or remediating Azure resources and excludes work-starting tools like report runs or agent profiles. It is distinct from Microsoft’s Azure MCP Server, which queries live Azure resources and can run alongside Cloudeval; setup includes client-specific MCP configs and prompts/Agent Profiles that require cited evidence.
```
