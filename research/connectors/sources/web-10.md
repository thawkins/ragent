# Web source

- URL: https://claw.aguidetocloud.com/openai/codex-cli/mcp
- Title: MCP integration — wiring tools into Codex · Claw Planet
- Author(s): Sush (Susanth Sutheesh)
- Language: English
- Published (UTC): 2026-05-15T00:00:00+00:00
- Captured (UTC): 2026-10-02T13:27:08.628839751+00:00
- Relevance: Medium - multiple title terms match query


```text
Codex CLI’s MCP support connects agents to external tools/data via the Model Context Protocol, exposing filesystems, git repos, Figma, browsers (Playwright/Chrome DevTools), Sentry/GitHub/Slack, and document search. It fully supports STDIO and Streamable HTTP (with bearer token or OAuth), not older HTTP+SSE, and servers can be added via experimental `codex mcp add` commands or the stable `~/.codex/config.toml` path, which allows timeouts (`startup_timeout_sec` default 10, `tool_timeout_sec` default 60), `enabled`/`required`, tool allow/deny lists, and OAuth (`codex mcp login figma`, callback port/URL). As of Codex 0.130.0, OpenAI Docs MCP is on by default and Memories MCP is enabled via `[features] memories = true`; documented third-party examples include Context7, Figma, Playwright, Chrome DevTools, Sentry, and GitHub, and `/mcp` shows active servers, tools, and startup errors. Codex can also run as an experimental MCP server via `codex mcp-server` for other MCP clients, while cautions note MCP adds startup latency, `required = true` helps CI fail fast, tool gating reduces context/tool-choice risk, and Codex 0.129.0+ truncates large MCP outputs. Cross-vendor reuse is possible across Claude Code, Cursor, and VS Code GitHub Copilot, though ChatGPT Apps need extra widget metadata (`text/html;profile=mcp-app`, `_meta.ui.*`) beyond the plain MCP tool contract.
```
