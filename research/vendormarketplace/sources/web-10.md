# Web source

- URL: https://dev.to/nagell/build-your-own-claude-code-marketplace-scaffold-structure-and-auto-updates-4n3f
- Title: Build Your Own Claude Code Marketplace: Scaffold, Structure, and Auto-Updates
- Author(s): @
- Language: English
- Published (UTC): 2026-06-14T16:52:08+00:00
- Captured (UTC): 2026-10-02T14:08:14.431213005+00:00
- Relevance: Medium - multiple title terms match query


```text
A Claude Code marketplace is a public GitHub repository containing a `.claude-plugin/marketplace.json` registry and one subdirectory per plugin, each minimally defined by `plugins/<name>/.claude-plugin/plugin.json`; plugins can ship skills (`SKILL.md`, auto-loaded or invoked as `/plugin:skill`), hooks, agents, and MCP servers via `.mcp.json`, while a plugin-level `CLAUDE.md` is ignored. Users install via `/plugin marketplace add <username>/<repo>` and `/plugin install <plugin>@<marketplace>`, and auto-updates can be enabled in the `/plugin` Marketplaces UI or via `settings.json`’s `extraKnownMarketplaces` with `autoUpdate: true`. The starter template `github.com/Nagell/claude-marketplace-template` pre-wires GitHub Actions/Release Please auto-versioning from conventional commits (`feat:` minor, `fix:` patch) and keeps `marketplace.json` synced, with `github.com/Nagell/claude-marketplace` as a fuller reference.
```
