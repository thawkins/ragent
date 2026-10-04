# Web source

- URL: https://amux.io/guides/legacy-code-modernization-ai-agents
- Title: AI Agents for Legacy Code Modernization: The Developer's Week-by-Week DIY Guide (2026)
- Author(s): amux
- Language: English
- Published (UTC): 2026-05-24T00:00:00+00:00
- Captured (UTC): 2026-10-02T21:24:21.953319961+00:00
- Relevance: Medium - multiple title terms match query


```text
An amux.io May 2026 guide presents a four-week DIY legacy-code modernization playbook using AI agents (e.g., Claude Code/Opus 4.6 and amux fleets) for 100K–2M+ line codebases, citing typical rewrite/consultancy figures of 12–18 months, $150,000–500,000+, and 50–70% failure versus roughly $400–1,600 in DIY tooling and 40–60 human hours. The workflow maps the codebase in Week 1, generates characterization tests toward 60–80% coverage in Week 2, runs 4–8 parallel Strangler Fig refactors in isolated git worktrees in Week 3, then integrates, validates with CI/Semgrep/Snyk, and ships in Week 4. It claims agents excel at pattern-based work—language/framework/dependency upgrades, dead-code removal, test generation, API modernization, documentation—but struggle with architectural redesigns, business logic rewrites, database schema migrations, and cross-system integration; it lists anti-patterns such as big-bang rewrite, refactoring without tests, unreviewed agent output, modernizing everything at once, and no merge-order discipline, while noting scaling via repeated cycles above 500K lines, regulated-industry guardrails, and a comparison to Morgan Stanley’s 9M-line conversion saving 280,000 developer hours.
```
