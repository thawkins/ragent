# Web source

- URL: https://zitniklab.hms.harvard.edu/ToolUniverse/guide/building_ai_scientists/codex_cli.html
- Title: Codex CLI
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:26:54.725449640+00:00
- Relevance: Medium - multiple title terms match query


```text
Codex supports MCP servers via shared CLI/IDE configuration, and ToolUniverse can be set up by asking Codex to read `https://aiscientist.tools/setup.md`, or manually with `codex mcp add tooluniverse --env PYTHONIOENCODING=utf-8 -- uvx --refresh tooluniverse`; run `/mcp` in Codex to confirm the server is listed. MCP configuration is stored by default in `~/.codex/config.toml`, or project-scoped `.codex/config.toml` for trusted projects, under `[mcp_servers.tooluniverse]` with command `uvx`, args `["--refresh","tooluniverse"]`, and env `PYTHONIOENCODING="utf-8"`. Validate with `uvx --refresh tooluniverse --help`; if Codex cannot find `uvx`, use its absolute path, found via `where uvx` on Windows or `which uvx` on macOS/Linux.
```
