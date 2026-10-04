# Web source

- URL: https://spybara.com/openai/codex/history/docs/en/2026-06-19-2357..2026-06-20-0358/mcp
- Title: mcp.md — Spybara
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:28:17.349226491+00:00
- Relevance: Medium - partial query match


```text
Codex supports MCP servers in both the CLI and IDE extension via STDIO (local process) and Streamable HTTP transports, with bearer-token and OAuth authentication (`codex mcp login <server-name>`); it reads the server’s initialization `instructions` as server-wide guidance and recommends keeping the first 512 characters self-contained. Configuration is stored in `~/.codex/config.toml` or project-scoped `.codex/config.toml` (trusted projects only), is shared across CLI and IDE, and can be managed via `codex mcp add` (e.g., `codex mcp add context7 -- npx -y @upstash/context7-mcp`) or edited using `[mcp_servers.<name>]` tables with options including command/args/env/env_vars/cwd, URL/bearer_token_env_var/http_headers/env_http_headers, startup and tool timeouts (defaults 10s and 60s), enabled/required flags, tool allow/deny lists, and approval modes (`auto`, `prompt`, `approve`, with per-tool overrides). OAuth options include top-level `mcp_oauth_callback_port` and `mcp_oauth_callback_url`; installed plugins can bundle MCP servers controlled under `plugins.<plugin>.mcp_servers.<server>`.
```
