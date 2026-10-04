# Web source

- URL: https://spybara.com/openai/codex/history/docs/en/2026-07-16-2057..2026-07-17-2257/build-plugins
- Title: build-plugins.md — Spybara
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:13:40.929260843+00:00
- Relevance: Medium - partial query match


```text
The page documents building Codex/ChatGPT plugins: a plugin can bundle skills, an MCP-backed app, or both, with the required manifest at `.codex-plugin/plugin.json` and optional `skills/`, `hooks/`, `.app.json`, `.mcp.json`, and `assets/`. Authors can use `@plugin-creator` (Work mode) or `$plugin-creator` (Codex) to scaffold the manifest and local marketplace, or create plugins manually; marketplaces are JSON catalogs stored at `$REPO_ROOT/.agents/plugins/marketplace.json` (repo) or `~/.agents/plugins/marketplace.json` (personal), and can be managed with `codex plugin marketplace add|list|upgrade|remove` for GitHub, Git, sparse-checkout, and local sources. The ChatGPT desktop app reads marketplaces from official, repo, legacy `.claude-plugin`, and personal locations, installs plugins into `~/.codex/plugins/cache/$MARKETPLACE_NAME/$PLUGIN_NAME/$VERSION/`, stores enable/disable state in `~/.codex/config.toml`, and requires users to trust non-managed plugin hooks, which receive `PLUGIN_ROOT` and `PLUGIN_DATA`. Plugins can be shared within a ChatGPT workspace without public publishing—admins can disable sharing via `features.plugin_sharing = false` in `requirements.toml`—and public publishing is submitted through the plugin submission portal.
```
