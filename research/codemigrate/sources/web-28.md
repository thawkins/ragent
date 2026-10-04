# Web source

- URL: https://leehanchung.github.io/blogs/2026/05/08/hidden-technical-debt-agent-harness
- Title: Hidden Technical Debt of AI Systems: Agent Harness
- Author(s): Han Lee
- Language: English
- Published (UTC): 2026-05-08T00:00:00+00:00
- Captured (UTC): 2026-10-02T21:26:38.642850169+00:00
- Relevance: Medium - multiple title terms match query


```text
The article defines an agent harness as the orchestration layer between model and environment—system prompts, tool surface/MCP, rollout protocol, context manager, memory, sub-agent topology, guardrails/gates, verifiers/judges, and observability—with agent = harness + foundation model, and argues it is hidden technical debt because most scaffold structure will dissolve as models improve. It contrasts a wide training/research harness (maximal action space, raw low-level tools, failures as signal, KL caps/reward shaping, programmatic verifiers, forkable state, many cheap rollouts) with a narrow production harness (allowlists, scoped credentials, approval tiers, I/O filters, idempotent retries, audit logs, kill switches), arguing the harness should be widest in training and narrowest in deployment with a deliberate audited gap; first-party harnesses usually outperform third-party because labs post-train inside their own harness, though Letta Code reportedly scored 59.1% vs Claude Code’s 41.6% on Opus 4.5 by exploiting memory, while on GPT 5.1 Codex and Gemini 3 Letta was within a few points but did not lead. Citing the Bitter Lesson and Hyung Won Chung’s “add structure… then remove it” rule, it says RAG pipelines (2023), no-code workflow canvases like n8n, tool wrappers, planner-executor scaffolds, memory layers, and elaborate multi-agent topologies are dissolving into model capability, and recommends thin harness/fat skills, treating production harnesses as 90-day artifacts, and investing in durable substrate (training/eval data, environments, tasks, infrastructure). It also warns auto-optimized harnesses are local/overfit and widen the train/prod gap, and frames runtime/harness as unbudgeted bills.
```
