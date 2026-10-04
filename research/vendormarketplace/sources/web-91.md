# Web source

- URL: https://github.com/ilderaj/agent-plugin-marketplace
- Title: GitHub - ilderaj/agent-plugin-marketplace: A Git-hosted marketplace that syncs agent plugins from Codex, Claude...
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:13:29.259962471+00:00
- Relevance: Medium - multiple title terms match query


```text
The GitHub repo ilderaj/agent-plugin-marketplace provides a cross-platform plugin sync pipeline that pulls plugins from Codex (github.com/openai/plugins.git), Claude Code (github.com/anthropics/claude-code.git), Cursor (github.com/cursor/plugins.git), and community ASC skills (github.com/rorkai/app-store-connect-cli-skills.git), converts them to VS Code/GitHub Copilot-compatible format, and publishes Git-hosted manifests including `marketplace.json`, `.github/plugin/marketplace.json`, and `.claude-plugin/marketplace.json`. Running `bun install` and `bun run sync` clones upstreams, parses them into a unified PluginIR, generates `plugins/` directories, and uses per-plugin commit SHA tracking in `data/sync-state.json` for incremental syncs. Conversion covers skills, MCP servers, Claude hooks/agents, commands, Codex agents/hooks, and Cursor rules, with `_meta.json` compatibility metadata and unsupported Codex app connectors dropped. Installation options include Copilot CLI (`copilot plugin marketplace add <owner>/agent-plugin-marketplace`), forking/self-hosting with env-var upstream overrides, manual plugin copying, or adding the repo to VS Code `chat.plugins.marketplaces`; the ASC plugin requires the `asc` CLI separately. GitHub Actions run a weekly sync (Friday 03:00 UTC) and manual dispatch that opens PRs with diff summaries and optional Slack/Discord notifications, while CI runs `bun run build` and `bun test` on PRs to main; roadmap milestones v0.4–v0.6 target more IDE adapters, deeper conversion, and bidirectional/community sync.
```
