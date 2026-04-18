---
title: "Agentic Session Processing Loop"
type: concept
generated: "2026-04-18T14:47:32.739996184+00:00"
---

# Agentic Session Processing Loop

### From: README_LSP_EXPLORATION

Core processing flow in session/processor.rs that: stores user message, loads history, builds 7-section system prompt, streams LLM response, handles tool calls with permission checks, embeds output, and loops until finish reason != ToolUse. Finally stores assistant message and publishes MessageEnd.

## Related

- [system prompt](system-prompt.md)

## Sources

- [README_LSP_EXPLORATION](../sources/readme-lsp-exploration.md)
