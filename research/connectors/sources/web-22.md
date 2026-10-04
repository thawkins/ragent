# Web source

- URL: https://glama.ai/mcp/servers/jonwadsworth/codex-bridge
- Title: Codex Bridge by jonwadsworth
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:28:09.969834454+00:00
- Relevance: High - title matches query


```text
Codex Bridge is a self-hosted MCP server that lets claude.ai and other MCP clients delegate tasks over HTTPS/OAuth 2.1 to OpenAI Codex CLI, billed through the operator’s existing ChatGPT subscription rather than per-token API usage. It uses Python/FastMCP with Streamable HTTP, wraps `codex exec` as detached subprocesses tracked in SQLite, exposes `codex_delegate`, `codex_status`, `codex_result`, and `codex_cancel`, and confines jobs to a working-directory allowlist. Installation targets a root-controlled Linux/Ubuntu VPS: an idempotent installer sets up uv, Node, Codex CLI, `codex login`, env config, and a systemd service on 127.0.0.1:8427, after which the operator must add a reverse proxy/tunnel and a claude.ai custom connector at `https://<domain>/mcp`. Security includes OAuth-authenticated access, rate/concurrency limits, Codex CLI’s `--sandbox` default `workspace-write` (never `--dangerously-bypass-approvals-and-sandbox`), protected `~/.codex/auth.json`, and a forced-command CI deploy key; v1 limitations include single-operator/no multi-tenancy, no `codex_review` or session resume, headless-login caveats, missing exit code if the service restarts mid-job, and no job-history retention policy.
```
