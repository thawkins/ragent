# Web source

- URL: https://tempreon.com/support/docs/bridges/codex
- Title: Codex (ChatGPT) Setup — Docs | Tempreon™
- Author(s): @tempreonai
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:27:22.623186287+00:00
- Relevance: High - title matches query


```text
Tempreon’s Codex bridge docs state that Codex is OpenAI’s coding-agent product available via the ChatGPT desktop app, IDE extensions (VS Code, Cursor, etc.), and CLI; all read the same MCP config from `~/.codex/config.toml` or a trusted project-local `.codex/config.toml`, and connecting Tempreon via OAuth exposes Core Imprint, Knowledge Vault, preferences, and project context. Setup requires a current Codex install, a Tempreon account with a Bridge slot, and a default browser: get the `/mcp` URL from the Tempreon Bridges dashboard, add `[mcp_servers.tempreon]` with `url = "https://api.tempreon.com/functions/v1/tempreon-mcp/mcp"` (or use `codex mcp add tempreon --url ...` / `codex mcp login tempreon`), authorize in the browser, and verify by asking Codex what Tempreon tools it has. The docs note OpenAI folded the standalone Codex Mac app into the ChatGPT desktop app in July 2026 (as of August 2026 there is no separate Codex app), while config is unchanged; troubleshooting includes direct config edits if the desktop Add server form flickers, fully deleting/re-adding the connector for OAuth disconnects or cached tools, trusting projects for project-local config, and updating Codex for “unknown command” errors tied to older v0.125.0 reconnect-loop bugs. Developer benefits include persistent architecture decisions, project-specific context, automatic session bootstrap, and integrated task management.
```
