# Web source

- URL: https://dev-docs.moodybeard.com/en/codex/mcp
- Title: Model Context Protocol
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:28:21.010827784+00:00
- Relevance: Medium - partial query match


```text
Model Context Protocol (MCP) connects models to tools and context, and Codex supports MCP servers in both its CLI and IDE extension via STDIO local-process servers or Streamable HTTP servers. Authentication can use bearer tokens or OAuth (`codex mcp login <server-name>`); configuration is stored in `config.toml` (default `~/.codex/config.toml`, or project-scoped `.codex/config.toml` in trusted projects) and shared across CLI and IDE. Servers can be managed with `codex mcp` (e.g., `codex mcp add context7 -- npx -y @upstash/context7-mcp`) or edited under `[mcp_servers.<server-name>]`, with options including command/args/env/cwd/url, bearer token and headers, `startup_timeout_sec` default 10, `tool_timeout_sec` default 60, enabled/required, and enabled/disabled tool lists; static OAuth callback URIs use top-level `mcp_oauth_callback_port`. In the Codex TUI, `/mcp` shows active MCP servers.
```
