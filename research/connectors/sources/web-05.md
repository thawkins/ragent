# Web source

- URL: https://hussamahmed.com/ai/codex/codex-mcp
- Title: MCP in Codex: Client, Server &amp; Tool Governance
- Author(s): Hussam Ahmed
- Language: English
- Published (UTC): 2026-06-13T00:00:00+00:00
- Captured (UTC): 2026-10-02T13:26:58.099069799+00:00
- Relevance: Medium - multiple title terms match query


```text
Codex integrates with MCP as both a client—adding stdio servers via a command after `--` or HTTP servers via `--url`, configured in `~/.codex/config.toml`—and a server via `codex mcp-server`, which exposes `codex` and `codex-reply` tools and uses `structuredContent.threadId` for multi-turn continuation. Management uses `codex mcp list/get/remove/login/logout`, with streamable-HTTP OAuth login/logout and credential storage via `auto`, `keyring`, or `file`. Tool governance uses `enabled_tools` (allow-list) and `disabled_tools` (deny-list applied after), plus `default_tools_approval_mode` and per-tool `approval_mode` (`auto`, `prompt`, `approve`); default timeouts are 10s startup and 60s per-tool. In managed environments, `requirements.toml` at `/etc/codex/requirements.toml` (Linux/macOS) or `%ProgramData%\OpenAI\Codex\requirements.toml` (Windows) acts as a strict MCP allow-list keyed by command or URL, with an empty table disabling all MCP servers.
```
