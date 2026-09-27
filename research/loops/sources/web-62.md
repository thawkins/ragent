# Web source

- URL: https://medium.com/@vovance/loop-engineering-the-skill-thats-replacing-prompting-d429b000489c
- Title: Loop Engineering: The Skill That’s Replacing Prompting
- Author(s): https://medium.com/@vovance
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-13T12:16:41.863439908+00:00
- Relevance: Medium — multiple title terms match query


```text
The article defines "loop engineering" as the practice of building automated systems that do the prompting for you — selecting tasks, sending them to an AI agent, checking output, and deciding whether to retry or proceed without human oversight — a concept that went viral in June 2026 when Peter Steinberger (creator of OpenClaw, now at OpenAI) posted that developers should "design loops that prompt your agents" rather than prompt them directly, a view independently echoed the same week by Boris Cherny (creator of Claude Code at Anthropic) and anticipated by Simon Willison in September 2025. A functioning agentic loop requires two non-negotiables — a trigger and a verifiable exit condition (deterministic or model-based) — plus five components: automations, worktrees, skills, sub-agents (one to generate, one to verify), and state memory that persists outside the conversation window; unlike a cron job, a loop embeds decision-making, which the article says is only now viable because 2026-era LLMs in tools like Claude Code, Codex, and LangChain can evaluate whether goals have been met. The piece warns that loop engineering is expensive by design, citing the FinOps Foundation's State of FinOps 2026 report (1,192 organizations representing over $83 billion in annual tech spend) showing 98% of enterprises now actively manage AI costs, up from 31% two years prior, and recommends spending limits per run, maximum iteration counts, human approval checkpoints, and grader agents that kill failing loops — framing the goal as "controlled autonomy, not unlimited autonomy" applicable beyond developers to any repetitive, multi-step AI work such as content pipelines, support triage, QA, and data enrichment.
```
