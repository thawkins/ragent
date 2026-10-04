# Web source

- URL: https://aitechconnect.in/tips/migrate-legacy-codebase-ai-coding-agents-2026
- Title: Migrating a Large Legacy Codebase with AI Coding Agents
- Author(s): PremKumar
- Language: English
- Published (UTC): 2026-06-30T05:30:00+00:00
- Captured (UTC): 2026-10-02T21:24:47.129330182+00:00
- Relevance: High - title matches query


```text
As of June 2026, the article recommends against asking an AI agent to “migrate everything” because a 100,000-file repository cannot fit in any agent’s context window, causing lost cross-file invariants and silent behavioural changes (e.g., rounding, DST, null-vs-empty) that pass casual review. Its safe-at-scale playbook is to index/dependency-map first, lock current behaviour with Michael Feathers’s characterisation (golden-master/approval) tests taken from untouched legacy code and made a hard merge gate, then migrate module-by-module through five human-checkpointed phases—inventory, pilot, parallel waves, integration, cutover—using a steering document (AGENTS.md/CLAUDE.md), per-module subagents in isolated git worktrees/branches, and an audit log. Agents should do mechanical syntactic transforms, scaffolding, and test generation, while humans keep architectural judgement, ambiguous business logic, and cross-cutting concerns like concurrency, transaction boundaries, locking, and security invariants. McKinsey QuantumBlack reportedly estimates generative AI can accelerate technology-modernization timelines by roughly 40–50% and reduce tech-debt costs by around 40%, with a top-15 global insurer case citing >50% code-modernization efficiency improvement, though these are engagement-specific estimates; the article also includes a Verified Builder anecdote from Aisha (London, UK) about a 15-year-old billing-service port where a single-agent first attempt was wrong in three staging-found places, while a second attempt using two days of golden-master tests and per-module worktree agents caught regressions at the moment of change.
```
