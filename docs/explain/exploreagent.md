# Explore Agent

The **Explore** agent is a lightweight, read‑only sub‑agent built into **ragent**. It is designed for fast, low‑cost code‑base exploration and information gathering without making any changes to the filesystem.

## What it does
- **Stateless** – each invocation starts with a fresh view of the repository. No context is retained between calls.
- **Read‑only** – the agent can only read files, run LSP queries, search with `grep`/`rg`, and list directories. It never writes, deletes, or executes code.
- **Fast & cheap** – uses the cheapest LLM model configured for the project and avoids expensive tooling, making it ideal for quick look‑ups.
- **Parallelizable** – multiple explore agents can be spawned in the same turn with `background: true`. They run concurrently (up to 4 at a time) and you can wait for them with `wait_tasks`.

## When to use it
| Situation | Reason |
|-----------|--------|
| Understanding a new module or crate | Quickly summarise files, list symbols, or locate definitions without building.
| Finding usages of a symbol across the code‑base | Use LSP `references` or a regex `grep` through the explore agent.
| Gathering architectural over‑view | Ask the agent to describe directory layout, public APIs, or event flow.
| Preparing for a larger change | Get a concise report before spawning the heavier `build` or `general` agents.

## How to invoke
```json
{"agent":"explore","task":"<your question or task>","background":false}
```
- **`background: false`** – only when you need the answer immediately and you are spawning a single explore task.
- **`background: true`** – when launching several independent explore tasks in parallel. After spawning, call `wait_tasks` (or `list_tasks` to poll) to collect the results.

### Example: Parallel exploration of three areas
```json
{"agent":"explore","task":"Summarise the architecture of crates/ragent-core/src/agent/","background":true}
{"agent":"explore","task":"List all tools defined under crates/ragent-core/src/tool/","background":true}
{"agent":"explore","task":"Explain the provider stack in crates/ragent-core/src/provider/","background":true}
```
Then wait for all results:
```json
{"tool":"wait_tasks","task_ids":null,"timeout_secs":300}
```

## Best‑practice guidelines
1. **Batch questions** – include every piece of information you need in a single `task` string; the explore agent does not retain context.
2. **Limit parallelism** – never spawn more than 4 background explore agents at once.
3. **Do not duplicate work** – once an explore call has returned information about a file, do not read the same file again with `read`.
4. **Read‑only guarantee** – the agent cannot invoke `write`, `edit`, `rm`, or any build commands. If you need to modify code, use the `build` or `general` agents instead.
5. **Use LSP when possible** – for Rust files, prefer `lsp_*` tools (e.g., `lsp_symbols`, `lsp_hover`, `lsp_references`) inside the explore task for accurate, language‑aware results.

## Interaction flow
1. **Lead (you)** creates an explore task via `new_task` or directly in a prompt.
2. The explore agent runs, collecting information using its read‑only toolbox.
3. Results are returned to the lead as a JSON response.
4. The lead may decide to spawn additional agents (e.g., `build`) based on the findings.

---
*This document lives in `docs/explain/exploreagent.md` and follows the project’s documentation conventions.*
