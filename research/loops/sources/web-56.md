# Web source

- URL: https://www.firecrawl.dev/blog/loop-engineering
- Title: Loop Engineering: Should You Stop Prompting Agents and Start Designing Loops
- Author(s): Hiba Fathima, @firecrawl
- Language: English
- Published (UTC): 2026-06-11T00:00:00+00:00
- Captured (UTC): 2026-09-13T12:16:26.556203912+00:00
- Relevance: High — title + snippet match query


```text
This Firecrawl blog post defines "loop engineering"—the practice of designing systems that autonomously prompt coding agents—popularized by Peter Steinberger's June 7, 2026 tweet (which drew 2.2 million views) arguing developers should "design loops that prompt your agents" rather than prompt agents directly, an idea Boris Cherny (Claude Code's creator) echoed days earlier. A loop is a program (shell script, hosted task, or TypeScript) that picks tasks, dispatches them to a model, grades results, and iterates on an "act, observe, reason, repeat" cycle until a stopping condition; Addy Osmani frames it as a third abstraction layer above prompt engineering and agent harness engineering. Working loops require triggers, git worktree isolation, codified context (SKILL.md files), writer/reviewer sub-agent splits, MCP connectors, and persistent memory, with closed loops (fixed paths and rubrics) preferred in production over open loops; key risks include runaway token spend (Uber capped Claude Code/Cursor use at $1,500 per person per tool per month after burning its annual AI budget in four months), "comprehension debt" (Osmani's term for code nobody understands), and the need for three guards: an iteration cap, a diff check, and a spend cap. The post positions Firecrawl as the web-feedback layer for such loops via its search, scrape, crawl, and monitor endpoints (available through an MCP server and CLI), offering 1,000 free monthly credits, and cites Greg Brockman's point that as models improve, taste—not capability—becomes the bottleneck.
```
