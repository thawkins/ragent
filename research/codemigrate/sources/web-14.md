# Web source

- URL: https://dev.to/iacobandrei/how-i-documented-a-broken-codebase-without-losing-my-mind-74m
- Title: How i documented a Broken codebase without losing my mind.
- Author(s): @
- Language: English
- Published (UTC): 2026-09-20T13:29:29+00:00
- Captured (UTC): 2026-10-02T21:25:05.878654465+00:00
- Relevance: Medium - multiple title terms match query


```text
The article describes a parallel-agents documentation workflow, inspired by a Meta article, for a broken, undocumented codebase with AI-era tech debt—duplicated business logic, zombie tables/columns, and manually tracked downstream effects—which the author says is common and in 99% of cases leaves no time to fully refactor. The `/map-domain` skill defines four phases—Analyse, Critique, Fix, and Sweep check—plus a Route facts step, spawning one agent per domain (e.g., Orders, payments, billing); Analyse reads `CLAUDE.md` and writes “compass” files answering five questions and extracting Quick Commands, 3–5 Key Files, non-obvious patterns, and See Also, while Critique grades claims as CONFIRMED/WRONG/OVERSTATED/UNCITED without editing, Fix returns applied/rejected/validatorClean/lines, and Sweep check catches thin files, contradictions, missing compasses, duplicated facts, and useless links. After a couple of days, the author reported fixing and implementing several database changes in one shot without manually listing constraints/gotchas, calling it not a silver bullet but the best shovel for AI tech debt.
```
