# Web source

- URL: https://www.firecrawl.dev/blog/loop-engineering
- Title: Loop Engineering: Should You Stop Prompting Agents and Start Designing Loops
- Author(s): Hiba Fathima, @firecrawl
- Language: English
- Published (UTC): 2026-06-11T00:00:00+00:00
- Captured (UTC): 2026-09-13T14:48:17.281174653+00:00
- Relevance: Medium — multiple title terms match query


```text
The article explains "loop engineering," a term that went viral after Peter Steinberger's June 7, 2026 post (2.2M views)—"You shouldn't be prompting coding agents anymore. You should be designing loops that prompt your agents"—and a similar on-stage comment two days earlier by Claude Code creator Boris Cherny. Loop engineering means writing a program (a shell loop, hosted task, or script) that autonomously dispatches tasks to a model, grades results, and iterates—an "act, observe, reason, repeat" cycle that, per Addy Osmani's framing, sits one level above prompt engineering and agent-harness engineering. Tasks qualify if they are repetitive, reviewable, and valuable; production loops require a trigger with a stop condition, git-worktree isolation for concurrent agents, codified "skills," writer/reviewer sub-agent splits, MCP connectors, plugins, and persistent memory, with Shann Holmberg's open-vs-closed distinction favoring closed loops in production. Key risks are runaway token spend—countered by iteration caps, no-progress diff checks, and dollar budgets (Uber capped engineers at $1,500/person/tool/month for Claude Code and Cursor after burning its annual AI budget in four months)—plus "comprehension debt" (Osmani) and the need for taste in rubrics (Greg Brockman). Finally, the article pitches Firecrawl's search, scrape, crawl, and monitor endpoints—usable via its MCP server or CLI, with 1,000 free monthly credits—as the web-feedback layer that keeps such loops grounded in live data.
```
