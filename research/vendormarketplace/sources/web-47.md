# Web source

- URL: https://chris-ayers.com/posts/agent-skills-plugins-marketplace
- Title: Agent Skills, Plugins and Marketplace: The Complete Guide
- Author(s): Chris Ayers
- Language: English
- Published (UTC): 2026-03-26T00:00:00+00:00
- Captured (UTC): 2026-10-02T14:10:44.788666653+00:00
- Relevance: Medium - multiple title terms match query


```text
GitHub Copilot’s extensibility now centers on Agent Skills (introduced December 2025), Plugins, and Marketplaces: Skills are folders with a `SKILL.md` file using YAML frontmatter and optional scripts/templates that load on demand, following the open agentskills.io standard shared across Copilot CLI, VS Code, Claude Code, Codex CLI, and Gemini CLI. Plugins package agents, skills, hooks, MCP servers, and LSP servers into installable units via a `.github/plugin.json` manifest (with `.claude-plugin/plugin.json` for Claude Code compatibility), while Marketplaces are Git repos with `.github/plugin/marketplace.json` registries that can reference external plugins pinned to versions, managed through `copilot plugin` CLI commands and VS Code settings. As of early 2026 the features are in preview, with limitations including user-level-only VS Code marketplace settings, possible dev-container failures, silent manifest errors, aggressive caching, and no direct branch installs; security relies on folder trust, tool permissions, `skipPermission`, and MCP allowlists.
```
