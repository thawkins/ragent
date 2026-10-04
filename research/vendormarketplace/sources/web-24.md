# Web source

- URL: https://skills.cat/skills/daymade/claude-code-skills/daymade-claude-code-claude-skills-troubleshooting
- Title: claude-skills-troubleshooting | AI Agent Skill | SkillsCat
- Author(s): daymade
- Language: English
- Published (UTC): 2026-04-30T10:57:15.883+00:00
- Captured (UTC): 2026-10-02T14:09:23.117992821+00:00
- Relevance: Medium - multiple title terms match query


```text
The SkillsCat skill **daymade/claude-skills-troubleshooting** provides systematic debugging workflows for Claude Code plugin and skill configuration issues, using **scripts/diagnose_plugins.py** to check installed-vs-enabled mismatches, missing **enabledPlugins** entries in **settings.json**, stale marketplace cache, and invalid plugin configurations. It documents known bug **GitHub #17832**, where plugins are added to **~/.claude/plugins/installed_plugins.json** but not automatically added to **enabledPlugins** in **~/.claude/settings.json**, and covers fixes such as **claude plugin enable plugin@marketplace**, updating marketplace cache, and checking **marketplace.json** or missing **SKILL.md**. It also distinguishes auto-activated **Skills** in **skills/** from explicitly invocable **Commands** in **commands/**, noting that a command file may be needed if a skill should be explicitly invocable.
```
