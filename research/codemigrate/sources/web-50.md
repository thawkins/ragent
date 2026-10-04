# Web source

- URL: https://www.humanlayer.dev/blog/skill-issue-harness-engineering-for-coding-agents
- Title: Skill Issue: Harness Engineering for Coding Agents
- Author(s): Kyle
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T21:29:39.336888543+00:00
- Relevance: Medium - multiple title terms match query


```text
HumanLayer’s post argues that coding-agent failures are typically a harness/configuration problem, not a model problem, where the harness is the model plus its configuration surface (CLAUDE.md/AGENTS.md, MCP servers, skills, sub-agents, hooks, and back-pressure) and harness engineering—coined by Viv—is a subset of Dex’s context engineering. It cites an ETH Zurich study of 138 agentfiles finding LLM-generated files hurt performance while costing 20%+ more and human-written files helped only ~4%, with agents spending 14–22% more reasoning tokens; it recommends concise (<60-line) instruction files, few MCP tools (Anthropic’s experimental MCP tool search), CLI alternatives such as a Linear CLI that saved thousands of tokens, skills for progressive disclosure, sub-agents as context firewalls (backed by Chroma’s 18-model context-rot research), hooks for deterministic control/verification, and back-pressure via typechecks/tests with silent success. The post also notes post-training/harness coupling (Codex’s apply_patch added to OpenCode; Opus 4.6 ranked #33 in Claude Code vs #5 in a different harness on Terminal Bench 2.0) and advises biasing toward shipping, adding configuration only after real failures, and avoiding upfront ideal harnesses, dozens of just-in-case skills/MCPs, full 5+ minute test suites, and sub-agent tool micro-optimization.
```
