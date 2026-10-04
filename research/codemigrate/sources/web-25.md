# Web source

- URL: https://www.augmentcode.com/guides/ai-code-migration
- Title: AI Code Migration: How Agent Loops Port Codebases Fast
- Author(s): Paula Hingel
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T21:27:24.514632131+00:00
- Relevance: Medium - multiple title terms match query


```text
AI code migration, per the guide, requires a closed LLM-agent loop that translates, compiles, tests, and repairs code rather than one-pass translation, because repository-scale dependencies defeat single-pass methods: GPT-4 resolved only 8.1% of full-project translations and every other tested model scored 0%, while RepoTransAgent reached 32.8%, TRANSREPO-BENCH SOTA LLMs 26.65%, and documented failure modes included 62% API misuse, 19.6% package hallucination, 77.9% Rust errors from misunderstood language differences, and only 30% fully correct web API completions. It names four prerequisites—rulebook, dependency map, characterization tests, and parity gates—and a six-step loop; agentic refinement raised a 79.3% one-shot baseline to 92.1% after 10 cycles with Claude Code, versus C2Rust’s 5/24 tests and one-shot LLM success of 2.1%–47.3%. Production cases cited include Amazon/Amazon Q Developer Java 8/11→17 migration of tens of thousands of apps (>50% upgraded in six months, 79% of auto-generated reviews applied unchanged, 4,500+ developer-years and $260M annual savings), Airbnb’s ~3,500 Enzyme→React Testing Library files (75% in 4 hours, 97% after prompt tuning vs. 1.5-year estimate), Google’s 39 migrations/93,574 edits/3 developers/12 months with ~50% time reduction, and Novacomp’s 10,000+ lines in 50 minutes vs. a 3-week estimate and 60% average debt reduction. It also cites CTO decision data—US federal $83B/79% of FY2025 IT spend on O&M, GAO’s 11 legacy systems 23–60 years old costing ~$754M annually, McKinsey tech debt 20–40% of budgets, and McKinsey/Oxford’s 5,400 projects 45% over budget and 56% less value—and describes Augment Cosmos primitives (Environments, Experts, Sessions), Context Engine, Prism, and Deep Code Review.
```
