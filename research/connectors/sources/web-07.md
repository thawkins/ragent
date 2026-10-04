# Web source

- URL: https://learn.arm.com/install-guides/codex-cli
- Title: Codex CLI: Install Guide
- Author(s): Joe Stech, @ArmSoftwareDev
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:27:02.806387457+00:00
- Relevance: High - title matches query


```text
Arm’s guide (author Joe Stech, last updated 7 Sep 2026) documents installing OpenAI’s Codex CLI, a local terminal coding agent, on macOS and Arm Linux. It requires an OpenAI account—ChatGPT Plus/Pro/Team/Edu/Enterprise or an API key—and Node.js 18+; install via `npm install -g @openai/codex`, or on macOS via `brew install --cask codex`, then verify with `codex --version` (example output `codex-cli 0.153.4`). On Arm Linux, prerequisite curl and Node.js (e.g., NodeSource setup_22.x, node v22.23.2, npm 10.9.8) are installed before global npm install, authentication uses `codex` ChatGPT sign-in or `OPENAI_API_KEY`, and config is stored in `~/.codex/config.toml`. The guide also configures the Arm MCP Server via Docker (`armlimited/arm-mcp:latest`, added under `[mcp_servers.arm-mcp]` with `startup_timeout_sec = 60` and a `/workspace` volume mount, or via `codex mcp add arm-mcp ...`), with alternatives Podman, Finch, Colima, and Rancher Desktop, and verification through `/mcp`; it recommends Arm prompt files such as `arm-migration.md` for x86-to-Arm migration and lists `mcpserver@arm.com` for support.
```
