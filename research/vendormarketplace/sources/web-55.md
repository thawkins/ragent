# Web source

- URL: https://pi.dev/packages/pi-claude-marketplace?name=Claude
- Title: Pi Coding Agent
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:11:23.423398310+00:00
- Relevance: Medium - partial query match


```text
**pi-claude-marketplace** (v0.19.2, published Sep 25, 2026, by acolomba, MIT license, ~2,655 downloads/month) is a Pi Coding Agent extension, installed via `pi install npm:pi-claude-marketplace`, that installs Claude plugin-marketplace plugins supporting commands, skills, agents, hooks, and MCP servers. It provides a `/claude:plugin` command (mirroring Claude Code's `/plugin`) for browsing, installing, updating, and uninstalling plugins and marketplaces, with a desired-state config in `~/.pi/agent/claude-plugins.json` (user) or `<cwd>/.pi/claude-plugins.json` (project), plus a gitignored `claude-plugins.local.json` override; project scope inherits user scope and user-scope plugins take precedence. Partially supported plugins (unmappable hooks, LSP servers, themes) require a `--partial` flag, generated pi-subagents agent files (named `pi-claude-marketplace-<plugin>-<agent>`) are regenerated on install/update and customizable only via pi-subagents `agentOverrides`, and remote plugin repos are fetched on demand. The author notes the project was developed with AI agent engineering practices using the Open GSD spec-driven development system.
```
