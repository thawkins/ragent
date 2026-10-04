# Web source

- URL: https://dev.to/teppana88/how-to-build-a-personal-agent-marketplace-for-claude-code-17fp
- Title: How to Build a Personal Agent Marketplace for Claude Code
- Author(s): @teemupiirainen
- Language: English
- Published (UTC): 2026-09-27T13:06:00+00:00
- Captured (UTC): 2026-10-02T14:09:25.957294056+00:00
- Relevance: Medium - multiple title terms match query


```text
Teemu Piirainen describes building a personal Claude Code plugin marketplace after accumulating more than 40 agents and skills, with about five plugins used across every project. Using the awave-agents marketplace and its aw-review plugin (install ID `aw-review@awave-agents`; skill `/aw-review:review-code`), the article outlines a three-layer structure—marketplace `marketplace.json`, plugin `plugin.json`, and components in `skills/`, `agents/`, `hooks/`, and `scripts/`—with deterministic work handled by dependency-free Node 18+ scripts and `SubagentStop` hooks. It recommends stable names, bundling dependencies via `${CLAUDE_PLUGIN_ROOT}` and persistent data via `${CLAUDE_PLUGIN_DATA}`, testing local and hosted installations, versioning in `plugin.json`, and notes that third-party marketplaces have background auto-update disabled by default.
```
