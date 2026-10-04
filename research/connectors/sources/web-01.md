# Web source

- URL: https://www.verdent.ai/guides/codex-cli-mcp-setup-guide
- Title: Codex CLI MCP: Tool Connectivity
- Author(s): Rui Dai
- Language: English
- Published (UTC): 2026-05-15T05:58:56.050+00:00
- Captured (UTC): 2026-10-02T13:26:49.169651600+00:00
- Relevance: High - title matches query


```text
Codex CLI’s MCP support lets Codex act as a client to external MCP servers—via stdio for local child processes or Streamable HTTP for remote services with optional OAuth/bearer auth—configured in `~/.codex/config.toml` or a trusted project’s `.codex/config.toml` (project config takes precedence), using commands like `codex mcp add`, `codex mcp list`, `codex mcp login`, and `/mcp` to verify tools. The guide cites GitHub’s endpoint `https://api.githubcopilot.com/mcp/` with `bearer_token_env_var = "GITHUB_PAT"` and use cases including GitHub, Notion, Linear, Sentry, Vercel, internal tools, and CI/CD so Codex can fetch issues/PRs, specs, error traces, deployment logs, and pipeline output directly. It lists failure modes: missing bearer-token env vars, stdio executables needing absolute paths, default 10s startup and 60s tool timeouts (overridable via `startup_timeout_sec`/`tool_timeout_sec`), OAuth not working headless, and project-scoped config requiring trusted directories.
```
