# Web source

- URL: https://github.com/bidule995/dext-codex-connector
- Title: GitHub - bidule995/dext-codex-connector: Unofficial, tenant-safe MCP connector for Dext Data Health and Codex
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:32:09.945867921+00:00
- Relevance: Medium - multiple title terms match query


```text
This is an independent, unofficial Codex plugin/MCP connector for Dext Data Health & Insights whose beta is deliberately fixture-only — its stdio bundle hard-codes three fictional companies and in-memory repositories, excludes the Dext HTTP adapter, PostgreSQL driver, Fastify, and credential decryption code, and cannot connect to a Dext account. It requires Node.js 24 and installs via `codex plugin marketplace add bidule995/dext-codex-connector` and `codex plugin add dext-codex-connector@dext-codex-connector`; `connection_status` should report `providerMode: fixture` and `liveDextRequestsEnabled: false`. Live mode requires explicit flags, PostgreSQL, a tenant connection, an envelope-encrypted credential, and only accepts `https://api.precision.dext.com/`; twelve tools are registered (including `connection_status`, `search`/`fetch`, client and portfolio health tools, and snapshot comparisons), while Commerce's 33 catalogued contracts have no tools or live adapter and Prepare is disabled pending first-party partner contracts. Other implemented features include streamable HTTP MCP at `/mcp`, OAuth bearer verification, PostgreSQL storage with forced row-level security, audit events, a 60-request fixed-window limit, opaque UUID record links, and risk searches capped by `RISK_DETAIL_REQUEST_BUDGET` with portfolio summaries limited to 20 clients and 64 KiB output.
```
