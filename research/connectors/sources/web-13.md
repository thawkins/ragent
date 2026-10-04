# Web source

- URL: https://www.simplified.guide/codex/docker-mcp-server-add
- Title: How to add the Docker MCP server to Codex
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:27:33.133152895+00:00
- Relevance: Medium - multiple title terms match query


```text
To add Docker-managed MCP tools to Codex, the Docker MCP Toolkit connector saves a stdio MCP server named **MCP_DOCKER** that runs `docker mcp gateway run` as a local process on demand; it requires Docker Desktop with MCP Toolkit enabled or a Docker CLI with the MCP plugin and writes to shared user Codex configuration, so open Codex CLI/IDE sessions must be restarted. Connect with `docker mcp client connect --global codex` (optionally `--profile <profile-name>`), then verify with `codex mcp get MCP_DOCKER --json` or `codex mcp list`, where MCP_DOCKER should show enabled and Auth “Unsupported” (expected for local stdio). Start a new Codex session, confirm via `/mcp`, and send a prompt requiring Docker’s MCP catalog or profile tools; if no catalog servers are added, Docker’s management tools such as catalog search and server add are the first available.
```
