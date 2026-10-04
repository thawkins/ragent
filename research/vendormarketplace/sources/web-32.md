# Web source

- URL: https://arsentev.ai/guides/plugins-marketplace
- Title: Claude Code Plugins: an App Store in Your Terminal
- Author(s): Evgenii Arsentev
- Language: English
- Published (UTC): 2026-06-12T00:00:00+00:00
- Captured (UTC): 2026-10-02T14:09:57.320269659+00:00
- Relevance: Medium - multiple title terms match query


```text
Claude Code plugins are prebuilt extension packs—skills, agents, hooks, and MCP server integrations—installed through `/plugin`, which opens a terminal app store; marketplaces are registered stores and plugins are the apps, with Anthropic’s `claude-plugins-official` marketplace pre-registered. Installation can be browse-first via the Discover tab, showing a “Will install” list and context-token cost, then choosing scope (user, project, or local) and running `/reload-plugins`; plugin skills are namespaced, e.g., `/commit-commands:commit`. Recommended first installs for non-programmers are external integrations (`github`, `slack`, `notion`, `linear`, `figma`, `asana`), then `commit-commands` and `security-guidance`, with `explanatory-output-style`, `learning-output-style`, and LSP plugins like `typescript-lsp`/`pyright-lsp` later. Plugins come from the curated official marketplace, the manually added community marketplace (third-party plugins that passed Anthropic’s automated validation and safety screening), or private GitHub repositories; they run with user privileges and Anthropic does not verify third-party plugins, so only trusted sources should be used, and removing a marketplace uninstalls its plugins. Users can build plugins as a folder with `.claude-plugin/plugin.json` plus `skills/`, test with `claude --plugin-dir ./my-first-plugin`, or scaffold via `claude plugin init my-tool`, though beginners are advised to keep plain skills in `.claude/` until sharing.
```
