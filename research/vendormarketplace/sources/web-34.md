# Web source

- URL: https://eliteaiadvantage.com/how-to/claude-code-plugin-marketplace
- Title: How Do I Install and Publish Claude Code Plugins from the Marketplace? | Elite AI Advantage
- Author(s): Jake McCluskey
- Language: English
- Published (UTC): 2026-04-24T18:33:02.970+00:00
- Captured (UTC): 2026-10-02T14:10:08.539102900+00:00
- Relevance: Medium - multiple title terms match query


```text
Jake McCluskey’s April 24, 2026 guide (updated May 6, 2026) explains that Claude Code 1.5+ plugins package reusable Skills, MCPs, hooks, and agents as installable GitHub-based marketplace packages, requiring Git and a GitHub account. Installing clones a plugin into `~/.claude/plugins/<name>/` and merges it into active config—Skills appear as `/` commands, MCPs register in `mcp-config.json`, and hooks activate—while publishing needs a `plugin.json` manifest, `SKILL.md`, and `README` (the minimum viable plugin) and can be listed via a PR to `anthropics/claude-code-plugins`. It warns that unvetted plugins can write files, run hook scripts, call MCPs, require secrets, or gain unexpected network access, so inspect `SKILL.md`/hook scripts, check name collisions, and pin versions in `CLAUDE.md` or a plugin-lock file. Private installs can use any Git URL (e.g., GitHub Enterprise or Gitea), and verification includes fresh-machine/teammate installs, clean `claude plugin remove`, and `claude plugin list`.
```
