# Web source

- URL: https://dev.to/mixture-of-experts/atomics-workflow-sdk-deterministically-extending-coding-agents-29ph
- Title: Atomic&#39;s Workflow SDK: Deterministically Extending Coding Agents
- Author(s): @
- Language: English
- Published (UTC): 2026-05-07T01:35:43+00:00
- Captured (UTC): 2026-10-02T21:29:30.378058064+00:00
- Relevance: Medium - multiple title terms match query


```text
Atomic is an open-source TypeScript SDK (github.com/flora131/atomic) that adds configurable, deterministic workflows around coding agents, preserving their harness (tool-use, context management, sub-agents, permission model) while encoding team guardrails for long-running, ambiguous work. Workflows are plain TypeScript using `defineWorkflow` and `ctx.stage`; each stage is a real coding-agent session in its own tmux pane—Claude Code, Copilot CLI, or opencode interchangeable with a flag—data flows only through explicit transcript reads, topology comes from `await`/`Promise.all`, and `.compile()` freezes the graph. Cited use cases include a UX review gate on every PR, a pre-PR 50-persona parallel feedback gate, support-ticket-to-draft-PR, and production regression triage; workflows run sandboxed by default with permission checks disabled, ship as three GHCR devcontainer features (Claude, Copilot, opencode) with Bun, CLI, playwright-cli and config templates, and should not be run on the host.
```
