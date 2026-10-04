# Web source

- URL: https://github.com/akeyless-community/claude-akeyless-connector
- Title: GitHub - akeyless-community/claude-akeyless-connector: a connector for akeyless and claude
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:30:47.192095918+00:00
- Relevance: High - title + snippet match query


```text
The GitHub repo `akeyless-community/claude-akeyless-connector` provides an SDK-based MCP connector that brings Akeyless Agentic Runtime Authority (ARA) to Claude Desktop without the Akeyless CLI, using the official Akeyless Node.js SDK (`akeyless` npm package) and exposing `list-secrets`, `query-db`, `service-execute`, and `list-sub-tools`. It ships in two modes: a Claude Desktop extension (`.mcpb`) with a settings UI for Gateway URL, Authentication Method, Access ID/Key, UID Token File, JWT, and Agent ID (default `claude-desktop`), with sensitive values stored in the OS keychain; and a Claude plugin (`akeyless-ara`) for marketplace/Anthropic directory listing using `AKEYLESS_*` env vars, since standalone MCPB listings are no longer accepted in Anthropic’s public directory. Requirements include Node.js 18+, Claude Desktop >=1.0.0, an Akeyless Gateway with ARA enabled, and a role with ARA Allow Access; the npm package is `@akeyless-community/claude-connector`. It is secretless (credentials resolved and used by the Gateway, only query/action results returned), RBAC-scoped by `ara_allow_access`, and audited as ARA sessions with agent ID and MCP ID; it runs locally, has no telemetry, does not send conversation content to Akeyless, and is MIT licensed.
```
