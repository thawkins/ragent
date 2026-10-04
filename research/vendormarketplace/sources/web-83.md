# Web source

- URL: http://developers.openai.com/codex/plugins/build
- Title: Package your plugin – Plugins | OpenAI Developers
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:13:07.977582307+00:00
- Relevance: Medium - partial query match


```text
The page explains how to package OpenAI/Codex plugins as portable Agent Plugins: a root `plugin.json` declares the schema and identity, with optional `skills/`, root `mcp.json` for bundled MCP servers, assets, and lifecycle hooks; OpenAI-specific presentation, MCP mappings, and hooks go under `extensions.com.openai`, while `.codex-plugin/plugin.json` remains a compatibility fallback. `@plugin-creator` scaffolds a `.codex-plugin/plugin.json` compatibility manifest and local marketplace entry, whereas manual portable packaging uses root `plugin.json` and `mcp.json` with Agent Plugins schemas (the portable MCP format requires a transport type per server). Public plugins are submitted once to the universal ChatGPT/Codex directory; local and repo marketplaces are separate authoring/testing/team-distribution sources, configured via JSON catalogs at `$REPO_ROOT/.agents/plugins/marketplace.json` or `~/.agents/plugins/marketplace.json`, with CLI commands like `codex plugin marketplace add/list/upgrade/remove`. Workspace admins can publish local plugins to a workspace—not the universal public directory—and plugin-bundled hooks are non-managed, so Codex skips them until the user reviews and trusts them.
```
