---
title: "Message Composition"
type: concept
generated: "2026-04-18T14:47:32.741819933+00:00"
---

# Message Composition

### From: README_LSP_EXPLORATION

Message structure containing multiple MessageParts with ToolCall parts tracking execution state. Tool output stored in ToolCallState.output with full history available for context in subsequent processing loops.

## Sources

- [README_LSP_EXPLORATION](../sources/readme-lsp-exploration.md)

### From: LSP_QUICK_REFERENCE

Structured message format with multiple MessagePart variants: Text, ToolCall (with state tracking), and Reasoning. Supports rich tool result embedding in conversation history
