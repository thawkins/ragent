# Web source

- URL: https://dev.to/arifulislamat/harness-engineering-101-how-coding-agents-actually-work-4247
- Title: Harness Engineering 101: How Coding Agents Actually Work
- Author(s): @
- Language: English
- Published (UTC): 2026-09-24T20:52:28+00:00
- Captured (UTC): 2026-10-02T21:31:15.542623283+00:00
- Relevance: Medium - multiple title terms match query


```text
The article argues that coding-agent outcomes are decided mostly by the “harness” around a model—Agent = Model + Harness, per Birgitta Böckeler of Thoughtworks—citing an August arXiv paper, “Same Model, Different Harness,” in which changing only the harness on 169 SWE-bench Verified bug-fixing tasks moved performance from 43 to 72 while weights, tasks, and context stayed the same (the gap nearly disappears with a 262K window). It describes harness components including context management (compaction, truncation, memory files such as CLAUDE.md/AGENTS.md, sub-agents, and full context reset, adopted by Anthropic after it found compaction alone insufficient and models displayed “context anxiety”), guardrails (Cline asks before every action; Claude Code/Cursor use classifier review; Codex CLI defaults to an OS sandbox, workspace only, network off; Pi uses no sandbox/no prompts, “full YOLO mode”), and verification via guides/sensors—Anthropic’s planner-generator-evaluator setup with Playwright took 6 hours and $200 versus a solo agent’s 20 minutes and $9 but produced a far better result. It notes Claude Code creator Boris Cherny said early RAG with a local vector database was replaced by plain agentic search, and Vercel cut an internal data agent to a single bash tool and reported 3.5x faster on 37% fewer tokens across only five test queries; Artificial Analysis’s Coding Agent Index measured $0.07–$2.26 per task, so the article recommends judging cost per completed task, success on your own tasks, long-task completion, safety, and fit, while expecting the permission boundary to remain a human decision.
```
