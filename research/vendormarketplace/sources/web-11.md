# Web source

- URL: https://developers.openai.com/plugins/build/plugins
- Title: Package your plugin – Plugins | OpenAI Developers
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:08:17.696804380+00:00
- Relevance: Medium - multiple title terms match query


```text
OpenAI plugins are packaged with a portable root `plugin.json` using the Agent Plugins schema, plus optional `skills/`, `mcp.json` for bundled MCP servers, assets, and lifecycle hooks; OpenAI-specific presentation, registered MCP mappings, and hook settings go under `extensions.com.openai`, while `.codex-plugin/plugin.json` remains a compatibility fallback. The built-in `@plugin-creator` is the fastest OpenAI-specific path, scaffolding the Codex-compatible layout and optionally a local marketplace entry. Marketplaces are JSON catalogs, with repo scope at `$REPO_ROOT/.agents/plugins/marketplace.json` and personal scope at `~/.agents/plugins/marketplace.json`, managed via `codex plugin marketplace add/list/upgrade/remove`; local plugins install to `~/.codex/plugins/cache/$MARKETPLACE_NAME/$PLUGIN_NAME/$VERSION/`. Public plugins are published once to the universal directory shared by ChatGPT and Codex, while workspace publishing is admin-only, restricted to the workspace, and can be disabled with `features.plugin_sharing = false`. Bundled MCP servers use root `mcp.json` with the Agent Plugins MCP schema, and plugin hooks receive `PLUGIN_ROOT` and `PLUGIN_DATA` (plus `CLAUDE_PLUGIN_ROOT`/`CLAUDE_PLUGIN_DATA`) but are skipped until the user reviews and trusts them.
```
