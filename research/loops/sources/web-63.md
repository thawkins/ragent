# Web source

- URL: https://rakiabensassi.substack.com/p/everyones-building-ai-loops-most
- Title: Everyone&#x27;s Building AI Loops. Most of Them Are Going to Regret It.
- Author(s): Rakia Ben Sassi
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-13T12:17:03.602320268+00:00
- Relevance: Medium — multiple title terms match query


```text
Rakia Ben Sassi’s post defines “loop engineering”—designing automated loops that prompt AI coding agents rather than prompting them manually, per OpenClaw author Peter Steinberger—and lists Addy Osmani’s five building blocks (automations like cron jobs/webhooks, isolated git worktrees, skills files, MCP plugins, and independent sub-agent reviewers) plus a sixth: persistent memory via an external state file. Loops deliver on stable, repeatable, verifiable tasks: GitHub’s automated issue-triage loop achieved 62% token savings across 109 production runs by pre-fetching only relevant files, caching the system prompt, and limiting the agent to three tools, but loops fail quietly on live incidents with rapidly changing context and on undocumented, chaotic codebases, where they “automate the chaos at scale.” Cost scenarios derived from Anthropic pricing and Vantage.sh data range from $3–6 per engineer/day for standard Claude Code use (~$3,000–6,000/month for 50 engineers), to ~$18/day raw ($10–11 with effective caching; $10,000–18,000/month) for active loop engineering, to StrongDM’s “dark factory” benchmark of $1,000/engineer/day—a figure that covers inference only, excludes their Digital Twin infrastructure, and comes from a three-person team (Justin McCarthy, Jay Taylor, Navan Chauhan); the author estimates a realistic starting budget of $15,000–25,000/month. A key hidden cost is broken prompt caching: cache reads cost 10% of normal input price, but any dynamic content (timestamps, user IDs, run counters) in the cached prefix forces full-price writes, a leak detectable via the `cache_read_input_tokens` field, with the caveat that the default cache TTL is only five minutes, limiting benefit for low-volume loops.
```
