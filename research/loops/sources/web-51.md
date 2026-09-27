# Web source

- URL: https://claudecertificationguide.com/learn/1-agentic-architecture/1-1-agentic-loops
- Title: 1.1 Agentic Loops — Claude Certification Guide
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-13T12:15:43.929504386+00:00
- Relevance: Medium — multiple title terms match query


```text
This page from a Claude certification study guide explains the **agentic loop**, the core execution cycle for Claude-based agents: a deterministic, code-defined cycle of (1) sending a request via the Messages API with full conversation history, (2) inspecting the response's `stop_reason` field, (3) executing requested tools and appending tool results to history when `stop_reason` is `"tool_use"`, and (4) terminating on `"end_turn"`—with the critical caveat that tool results must be appended or Claude cannot reason about them on the next iteration. The `stop_reason` field is presented as the only reliable loop-control signal; beyond the two exam values, production loops should also handle `pause_turn`, `max_tokens`, `stop_sequence`, `refusal` (current models such as "Fable 5" can refuse on a 200 response), and `model_context_window_exceeded`, treating any non-`end_turn` value as "not finished, check why" (verified against Messages API docs, July 2026). Agentic loops rely on model-driven decision-making—Claude selects tools from context rather than following hard-coded sequences—except when deterministic compliance is required (financial operations, security, regulatory logic, per Task Statement 1.4). The guide flags three termination anti-patterns: parsing natural language completion signals, using arbitrary iteration caps as the primary stopping mechanism (acceptable only as a safety net), and checking for text content via `response.content[0].type == "text"`, which causes premature termination because Claude can return text alongside `tool_use` blocks. A hands-on exercise reinforces this by building a two-tool loop (calculator plus web-search stub) that checks `stop_reason` each iteration, appends `tool_result` messages, and adds a safety cap of 20 iterations with a warning log that should never trigger in normal use.
```
