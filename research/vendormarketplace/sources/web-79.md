# Web source

- URL: https://www.codex-marketplace.com/docs
- Title: Documentation — Codex Plugin Marketplace
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:12:56.360365027+00:00
- Relevance: Medium - multiple title terms match query


```text
The Codex Marketplace docs describe a CLI (`npx codex-marketplace`) that installs one artifact class at a time from GitHub—plugins, standalone skills, or hook packages—using `--plugins`/`--skills`/`--hooks` (or singular `--plugin`/`--skill`/`--hook` for direct paths) and `--project`/`--global` scope, or an interactive prompt. Plugins require `.codex-plugin/plugin.json` and may bundle skills, `.mcp.json`, `.app.json`, and `hooks.json`; skills use `SKILL.md` under `skills/`, and hooks require `hooks.json` under `hooks/`. Project installs put skills in `$REPO/.codex/skills/<name>` and hooks in `$REPO/.codex/hooks.json` plus `$REPO/.codex/hooks/<name>`, while global installs use `~/.agents/skills/<name>` and `$CODEX_HOME/hooks.json`/`$CODEX_HOME/hooks/<name>`; plugins provision `~/.codex/plugins/cache/` and `~/.codex/config.toml`. Publishing uses `marketplace.json` at repo or personal paths, submission is via the Submit Plugin page with a GitHub repo/tree URL for automated/manual review, approved plugins install with `npx codex-marketplace add your-org/your-plugin --plugin --project`, and MCP is not a standalone marketplace type but stays bundled in plugins via optional `.mcp.json`.
```
