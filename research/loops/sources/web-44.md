# Web source

- URL: https://claudecertificationguide.com/learn/1-agentic-architecture/1-1-agentic-loops
- Title: 1.1 Agentic Loops — Claude Certification Guide
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-13T14:49:18.385767274+00:00
- Relevance: Medium — multiple title terms match query


```text
An agentic loop—the core execution cycle of Claude-based agents, defined as deterministic control flow in code—follows a four-step lifecycle via the Messages API: send the conversation history, inspect the `stop_reason` field, and either execute requested tools and append their results to the history when `stop_reason` is `"tool_use"` (continuing the loop) or terminate when it is `"end_turn"`; `stop_reason` is the only reliable loop-control signal, and failing to append tool results is the most common breakage point. The live API (verified July 2026) returns additional values a production loop must handle—`pause_turn`, `max_tokens`, `stop_sequence`, `refusal`, and `model_context_window_exceeded`—so anything other than `end_turn` should be treated as "not finished, check why." Loops rely on model-driven decision-making (Claude chooses which tool to call from context), except where deterministic compliance is required (financial, security, or regulatory operations), in which case programmatic enforcement takes precedence (Task Statement 1.4). Three anti-patterns should be avoided as primary stopping mechanisms: parsing natural-language completion signals, relying on arbitrary iteration caps (acceptable only as a runaway-prevention safety net, e.g., a 20-iteration maximum), and checking for text content, since Claude can return text alongside `tool_use` blocks.
```
