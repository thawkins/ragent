# Web source

- URL: https://dev.to/danielbutlerirl/designing-agentic-workflows-the-core-loop-166d
- Title: Designing agentic workflows: the core loop
- Author(s): @
- Language: English
- Published (UTC): 2026-02-16T14:37:08+00:00
- Captured (UTC): 2026-09-13T14:46:51.357879980+00:00
- Relevance: Medium — multiple title terms match query


```text
This dev.to post by Daniel Butler describes a "core loop" agentic workflow, implemented in the GitHub repo daniel-butler-irl/sample-agentic-workflows, built on one key principle: sessions are disposable, so all durable state lives in source-controlled files (AGENTS.md/CLAUDE.md, and .agents/tasks/<issue>/gates.md, task-N.md, and cleanup.md). Work begins only after an issue defines objective, scope, and success criteria; the AGENTS.md file (kept under 200 lines and injected into every session) encodes project-specific anti-shortcut rules (e.g., "Never add axios"). The loop itself is: wf-01 generates gates.md (success conditions with agent-independent verification, classified SIMPLE or COMPLEX); wf-02 plans one commit-sized task at a time (task-N.md, with Implementation Notes to preserve discoveries across fresh sessions); wf-03 executes exactly one task, always stopping for the human to review and commit—the agent never commits. After all gates pass, a per-issue cleanup step audits the branch for residue, applies fixes, and re-runs all gates and tests before opening the PR. The author argues this sequencing constrains known agent failure modes (shortcut-taking, premature "done" claims, intent drift, review fatigue, residue), and notes supplementary commands for context degradation and drift are covered in a follow-up post.
```
