# Web source

- URL: https://implexa.ai/blog/claude-code-plugins
- Title: Claude Code plugins: what they are and how to use them
- Author(s): -
- Language: English
- Published (UTC): 2026-06-25T00:00:00+00:00
- Captured (UTC): 2026-10-02T14:08:52.480535711+00:00
- Relevance: Medium - multiple title terms match query


```text
Claude Code plugins are named, versioned installable containers defined by a manifest at `.claude-plugin/plugin.json` that can bundle seven component types—skills, commands, agents, hooks, MCP servers, LSP servers, and monitors—where the plugin name namespaces skills (e.g., `/my-first-plugin:hello`) and all components must sit at the plugin root, not inside `.claude-plugin/`. They are installed through the `/plugin` command by adding a marketplace git repo (e.g., `/plugin marketplace add anthropics/claude-plugins-community`) and running `/plugin install plugin-name@claude-community`; Anthropic maintains the `claude-plugins-official` and `claude-community` marketplaces, and `/plugin marketplace update` refreshes them. To build one, create a directory with `.claude-plugin/plugin.json` plus at least one component, load it with `claude --plugin-dir` (v2.1.128+ also accepts `.zip` and multiple flags), use `/reload-plugins`, scaffold with `claude plugin init`, and validate with `claude plugin validate`; plugins suit sharing/versioning across projects, while standalone `.claude/` configs are for personal iteration, and the article distinguishes session-level plugin governance from an external control plane (implexa) for unattended approval, delivery, and oversight.
```
