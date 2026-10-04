# Web source

- URL: https://aikickstart.com.au/courses/agentic-workflows/skill-to-plugin
- Title: From skill to plugin: build your own tooling | AI Kick Start
- Author(s): -
- Language: English
- Published (UTC): 2026-06-05T00:00:00+00:00
- Captured (UTC): 2026-10-02T14:09:15.501339150+00:00
- Relevance: Medium - partial query match


```text
The AI Kick Start lesson “Skill to Plugin” presents a capability ladder—Prompt → Template → Skill → Plugin → MCP server → Internal app—arguing value-per-effort falls as you climb and that plugins are the sharing rung for turning one person’s skill into a team-installable tool. It says that by 2026 both Claude Code and OpenAI Codex converged on manifest-driven plugin directories bundling skills, connectors/MCP servers, sub-agents, hooks, and commands for one-command, zero-config installation: Claude Code’s `.claude-plugin/plugin.json` requires only `name` with components auto-discovered at the plugin root, while Codex uses `.codex-plugin/plugin.json` with `name`, `version`, and `description` plus `skills/`, `.mcp.json`, and `.app.json`. Distribution is via marketplaces—Claude Code uses `.claude-plugin/marketplace.json` and `claude plugin marketplace add`/`install` (Anthropic curates Knowledge Work, Financial Services, Legal, and Life Sciences), while Codex uses `codex plugin marketplace add`, `/plugins`, or “Add to Codex”—with Codex self-serve publishing still “coming soon” as of mid-2026. The lesson stresses shipping plugins like software: explicit semver (must bump to push updates) or omitted version so git commit SHA drives updates, install scopes (user/project/project-local/managed), workspace trust gates, admin marketplace restrictions and Codex `enabled = false`, plus least-privilege connectors, `defaultEnabled: false`, `userConfig` with `sensitive: true` keychain storage, and a trust note, because bundled local MCP servers run with program-level permissions; it also warns Codex manifest/publishing details are provisional and several Claude Code fields are client-version-gated as of June 2026.
```
