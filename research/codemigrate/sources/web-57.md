# Web source

- URL: https://github.com/codeaudit/agentic-harness-engineering
- Title: GitHub - codeaudit/agentic-harness-engineering: Agentic Harness Engineering
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T21:30:36.922161435+00:00
- Relevance: Medium - multiple title terms match query


```text
AHE (Agentic Harness Engineering) is an open observability system for automatically evolving the harness around a coding agent while holding the base model fixed; what evolves includes system prompts, tool descriptions/implementations, middleware, skills, sub-agents, and long-term memory. It has three observability layers: Component observability via NexAU, which decomposes the harness into seven orthogonal, git-tracked file-level components; Experience observability via Agent Debugger, which distills ~10M-token raw traces into layered, sourced reports (optimizer reads digests by default but can drill to raw traces); and Decision observability via Evolve Agent, which proposes evidence-backed edits, predicts impact, and is falsified by the next iteration’s flipped tasks. The outer loop runs evaluate→analyze→improve, using harbor to benchmark, distilling traces, and having Evolve Agent rewrite components until target pass rate or iteration cap; the Agent Debugger release is only partially open-sourced and cannot be fully open-sourced due to company strategy. Setup requires cloning Curry09/agentic-harness-engineering, running uv sync with GITHUB_TOKEN pull access to private NexAU and harbor-LJH, and env vars including LLM_API_KEY/LLM_BASE_URL, E2B_API_KEY, GITHUB_TOKEN, and SERPER_API_KEY; rollouts run in E2B sandboxes—SaaS default has a per-account concurrency cap tied to tier, while self-hosted E2B via E2B_API_URL/E2B_DOMAIN has no shared cap but hardware limits—and templates are built once per dataset with scripts/build_templates.py. Experiments launch via ./scripts/evolve.sh, with base.yaml defaults target_pass_rate 0.95 and max_iterations 100; the repo is MIT-licensed.
```
