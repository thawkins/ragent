# Web source

- URL: https://simonwillison.net/2025/Sep/30/designing-agentic-loops
- Title: Designing agentic loops
- Author(s): Simon Willison, @simonw
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-13T12:15:51.787951902+00:00
- Relevance: Medium — multiple title terms match query


```text
Simon Willison (Sept 30, 2025) argues that "designing agentic loops" is a critical new skill for getting real value from coding agents like Anthropic's Claude Code and OpenAI's Codex CLI, which he defines as tools that run in a loop to achieve a goal by brute-forcing solutions through iteration. He discusses "YOLO mode" (auto-approving all agent commands), whose risks include destructive shell commands, data exfiltration, and proxy attacks, recommending mitigations such as sandboxing with Docker or Apple's container tool—echoing Anthropic's own guidance to use `--dangerously-skip-permissions` only in an internet-isolated container—or running agents on "someone else's computer," with GitHub Codespaces as his preferred option. He advises favoring shell commands over MCP for tool design, documenting them in an AGENTS.md file (e.g., a `shot-scraper` example), and issuing tightly scoped credentials, illustrated by his Fly.io cold-start investigation where he gave Claude Code an API key limited to a dedicated organization with a $5 budget. Agentic loops suit problems with clear success criteria requiring trial-and-error—debugging, SQL performance optimization, dependency upgrades, and shrinking Docker images—with a solid automated test suite massively amplifying their value; Willison notes the skill is brand new, as Claude Code only launched in February 2025.
```
