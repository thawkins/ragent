# Web source

- URL: https://docs.ability.ai/guides/abilities-marketplace
- Title: Trinity — Autonomous Agent Orchestration and Infrastructure
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T14:09:08.265428443+00:00
- Relevance: Medium - partial query match


```text
Ability.ai’s `abilities` marketplace (github.com/abilityai/abilities, connected via Claude Code’s `/plugin marketplace add`) is a versioned plugin registry for the agent lifecycle and, as of 2026-09-14, had 5 active plugins plus one deprecated pointer stub. Its main plugin, trinity v2.9.0, has 7 skills—`start-here`, `connect`, `onboard`, `sync`, `loop`, `create-dashboard`, `deploy-new-instance`—for deploying/operating agents on Trinity, using repository-first `/trinity:onboard` (interactive, `analyze`, or `in-place`) and git-based multi-remote `/trinity:sync` with `.trinity-remote.yaml`, and it exposes Trinity MCP tools (`list_agents`, `chat_with_agent`, `deploy_local_agent`, `run_agent_loop`); declared `template.yaml` plugins are reinstalled on every boot. The docs define plugins, skills (`SKILL.md`), and playbooks—callable as `/playbook-name [args]` by schedules, agents, orchestrators, or pipelines—and state that agents should delegate only via named playbook calls, with gated skills requiring a `--autonomous` mode for unattended cron runs.
```
