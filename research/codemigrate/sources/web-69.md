# Web source

- URL: https://www.chatprd.ai/how-i-ai/workflows/how-to-systematically-reduce-technical-debt-using-ai-agents
- Title: How to Systematically Reduce Technical Debt Using AI Agents | AI Workflows
- Author(s): ChatPRD, @chatprd
- Language: English
- Published (UTC): 2026-01-08T23:21:53.456+00:00
- Captured (UTC): 2026-10-02T21:32:21.838667085+00:00
- Relevance: Medium - multiple title terms match query


```text
ChatPRD outlines a five-step workflow for reducing technical debt with AI agents: capture a dated baseline log by running the normal test command and recording test count, failures, and total log lines; give the log to Claude to group and quantify repeated warnings, identify worst files, and separate legitimate defects from harmless output; save a tiered Markdown migration checklist in the repo so Cursor, Devin, or another coding agent can take one small task, fix the cause within named files, run relevant tests, and report before/after warning counts; then review against the baseline and merge through the normal path. Good outcomes are a shrinking test log while tests still pass, tasks that name files and a measurable expected reduction, and one agent completing a task without doing the entire migration. If logs are too large, process in chunks and aggregate by warning signature; forbid blanket suppression by requiring root-cause explanation and before/after counts; split broad cleanup tasks by warning type or file group; and if noise drops because tests stopped exercising behavior, compare test counts/failures with the baseline and inspect deleted assertions, mocks, or setup code.
```
