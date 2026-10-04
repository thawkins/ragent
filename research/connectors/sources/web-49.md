# Web source

- URL: https://itecsonline.com/post/how-to-set-up-model-context-protocol-mcp-in-claude
- Title: How to Set Up MCP in Claude: Current Guide | ITECS
- Author(s): ITECS Team
- Language: English
- Published (UTC): 2026-08-05T14:26:12.088+00:00
- Captured (UTC): 2026-10-02T13:30:18.902959743+00:00
- Relevance: High - title matches query


```text
As of August 15, 2026, the article describes MCP as an open standard for connecting Claude to external data, tools, and workflows, with setup paths including remote connectors configured through Claude’s Connectors settings with OAuth; Anthropic-reviewed or organization-approved local Desktop Extensions packaged as `.mcpb` files; Claude Code using `claude mcp add` for remote HTTP or local stdio servers and verifying with `/mcp`; and manual `claude_desktop_config.json` only for unpackaged local servers. It recommends reviewed remote connectors and OAuth for cloud services, Desktop Extensions or narrowly scoped stdio servers for local tools, and notes that Team/Enterprise owners approve organization connectors while members connect their own identities. Security guidance includes verifying publishers and transports, minimizing OAuth scopes and permissions, expecting prompt-injection risk, approving actions deliberately, using separate identities, and logging/reassessing connections; troubleshooting covers missing local servers, remote connectivity failures, denied tools, and unexpected Claude actions.
```
