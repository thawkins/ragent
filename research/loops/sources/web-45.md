# Web source

- URL: https://simonwillison.net/2025/Sep/30/designing-agentic-loops
- Title: Designing agentic loops
- Author(s): Simon Willison, @simonw
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-13T14:49:26.571018486+00:00
- Relevance: Medium — multiple title terms match query


```text
In this September 30, 2025 post, Simon Willison argues that "designing agentic loops"—which he defines as running tools in a loop to achieve a goal—is the critical skill for getting value from coding agents like Anthropic's Claude Code (first released February 2025) and OpenAI's Codex CLI, which act as brute-force problem solvers when given a clear goal and iterative tools. He advocates using "YOLO mode" (auto-approving all commands) despite three key risks—destructive shell commands, data exfiltration via prompt injection, and attacks using the machine as a proxy—mitigating them via sandboxes (Docker, Apple's container tool), "someone else's computer" (his preferred option, via GitHub Codespaces), or simply accepting risk; Anthropic's own docs recommend using `--dangerously-skip-permissions` only in a container without internet access. He recommends giving agents shell commands documented in an AGENTS.md file (e.g., a one-line `shot-scraper` example) rather than relying on MCP, issuing tightly scoped credentials to test/staging environments with spending caps (he gave a Fly.io API key limited to a dedicated organization with a $5 budget), and applying agentic loops to problems with clear success criteria and trial-and-error solutions—debugging, performance optimization, dependency upgrades, and container-size optimization—noting that a clean automated test suite massively amplifies their value.
```
