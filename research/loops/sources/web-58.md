# Web source

- URL: https://tosea.ai/blog/loop-engineering-ai-agents-complete-guide-2026
- Title: What Is Loop Engineering? A Complete Guide from Prompt to Harness Engineering (2026)
- Author(s): Tosea Team
- Language: English
- Published (UTC): 2026-06-16T00:00:00+00:00
- Captured (UTC): 2026-09-13T12:16:19.151499745+00:00
- Relevance: High — title + snippet match query


```text
Loop engineering—designing the repeating cycle (act, observe feedback, decide, repeat until a termination condition) that drives an AI agent rather than prompting it by hand—became the dominant framing in agentic AI in June 2026, when Peter Steinberger's June 7 post (which drew roughly 6.5 million views) argued the key skill had shifted from prompting agents to designing their loops, and Google engineer Addy Osmani published a "Loop Engineering" essay the next day defining its anatomy (automations, worktrees, skills, connectors, sub-agents, external state); Anthropic's Boris Cherny captured the shift with "I don't prompt Claude anymore." The article presents loop engineering as the fourth layer in a nested progression—prompt engineering (2022–2024), context engineering (2025, defined by Shopify's Tobi Lütke and endorsed by Andrej Karpathy), harness engineering (2026), and now loop engineering—descended from research patterns including ReAct (Yao et al., 2022), Reflexion (Shinn et al., 2023), Plan-and-Execute, Evaluator-Optimizer, and Orchestrator-Workers. A reliable loop requires a testable goal, real-environment tools, context management (to prevent "context rot"), layered termination logic (verifier, step caps, budgets, no-progress detection), and trustworthy verification—ideally deterministic checks like tests and type checkers rather than the agent's self-report—with common failure modes including reward hacking (e.g., deleting a failing test to turn CI green), hallucinated success, compounding errors, and cost blowup. The piece concludes that leverage is shifting from prompt authorship to orchestration, enabling parallel, unattended "agents that run while you sleep," and links the concept to Tosea.ai's document-to-PPT pipeline, which applies the same generate-verify-refine loop to keep slides anchored to their source documents.
```
