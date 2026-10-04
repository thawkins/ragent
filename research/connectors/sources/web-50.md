# Web source

- URL: https://dev.to/aicoding-guide/claude-mcp-add-scope-local-vs-project-vs-user-and-which-to-pick-4ajm
- Title: claude mcp add --scope: local vs project vs user, and which to pick
- Author(s): @
- Language: English
- Published (UTC): 2026-09-24T19:17:24+00:00
- Captured (UTC): 2026-10-02T13:30:15.054935868+00:00
- Relevance: Medium - multiple title terms match query


```text
Claude Code’s `claude mcp add` defaults to `--scope local` (this project, only you), while `--scope project` writes `.mcp.json` at the project root for team sharing via version control, and `--scope user` makes a server available across all your projects; local and user both live in `~/.claude.json`, but local is stored under that project’s path. Project-scoped servers from `.mcp.json` trigger an interactive approval prompt, resettable with `claude mcp reset-project-choices`, though they load without prompting in `claude -p`, Agent SDK/cloud sessions, or bypassPermissions mode with `skipDangerousModePermissionPrompt`; they can be blocked with `disabledMcpjsonServers`, `--setting-sources`, or `--strict-mcp-config`. On duplicate names, Claude Code connects once using the highest-precedence source—local, project, user, plugin-provided servers, then claude.ai connectors—without merging fields, and warns in `claude mcp list` and `/mcp`. Removal requires a scope: `claude mcp remove <name> --scope <scope>`.
```
