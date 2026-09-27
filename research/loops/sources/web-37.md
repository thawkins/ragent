# Web source

- URL: https://dev.to/mmmattos/youre-not-writing-code-anymore-youre-designing-agents-2m08
- Title: You’re Not Writing Code Anymore — You’re Designing Agents
- Author(s): @mmmattos
- Language: English
- Published (UTC): 2026-04-30T22:24:29+00:00
- Captured (UTC): 2026-09-13T14:47:50.971170652+00:00
- Relevance: High — title matches query


```text
This dev.to article by mmmattos argues that agentic coding represents the next abstraction in software engineering—not over infrastructure but over development itself—shifting senior engineers from writing code to designing systems that write, run, and fix code via a "Goal → Generate → Execute → Observe → Fix → Repeat" loop that doesn't stop until the system works. The author demonstrates this with an experiment building the same REST API notes app autonomously in three ecosystems—Python (FastAPI), Go (net/http), and TypeScript (Express)—using OpenAI's gpt-4.1-mini model with up to five retry iterations, automatic dependency installation (pip, go get, npm), and error-feedback regeneration; full code is at github.com/mmmattos/agentic-coding-demo. The key insight is that failures were environmental (missing dependencies, modules, runtime mismatches, process lifecycle) rather than code errors, meaning "the agent is not fixing code—it is fixing the system." The article also introduces a "Minions vs Stripes" vocabulary (Minions handle the WHAT—running code, installing dependencies, writing files; Stripes handle the HOW—retry loops, error handling, decision flow) and previews future topics including multi-file agents, test-driven agentic development, and multi-agent architectures.
```
