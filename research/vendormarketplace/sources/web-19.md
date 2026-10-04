# Web source

- URL: https://brewpirate.github.io/claude-code-docs/plugins/overview
- Title: Plugins Overview
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:08:58.455248359+00:00
- Relevance: Medium - partial query match


```text
The page is a reference for the Claude Code plugin system, covering manifest-driven packaging of commands, skills, agents, hooks, MCP configs, and output styles into installable units; it spans 15 sections and 40 entries. Key material includes the plugin.json manifest with 28 fields (e.g., name, version, commands, skills, agents, hooks, mcpServers, outputStyles, dependencies, gating, autoUpdate), the conventional directory layout (plugin.json plus commands/, skills/, agents/, hooks/, mcp-servers.json, output-styles/), and the 8-phase lifecycle: discovery → install → validate → load → enable → reload → autoupdate → uninstall. It also documents official/custom marketplaces and marketplace.json, managed-only policy from pluginPolicy.ts, bundled plugins from builtinPlugins.ts, CLI subcommands such as claude plugin install/uninstall/list/enable/disable/update/validate/search/info, slash commands /plugin and /reload-plugins, settings keys like enabledPlugins and strictPluginOnlyCustomization, related env vars, and discrepancies including deprecated dual-type, LSP binary requirement, managed-plugin read-only UI, and symlink security.
```
