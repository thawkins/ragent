---
title: "SessionProcessor: Agentic Conversation Loop Implementation"
source: "processor"
type: source
tags: [rust, agent-system, llm-integration, async-concurrency, message-processing, tool-calling, session-management, event-driven, mcp, lsp]
generated: "2026-04-19T15:58:32.775531511+00:00"
---

# SessionProcessor: Agentic Conversation Loop Implementation

This document details the implementation of `SessionProcessor`, a core component of the ragent agent system that orchestrates the agentic conversation loop for individual sessions. The processor manages the complete lifecycle of message processing, from receiving user input through streaming LLM responses, executing tool calls, and iterating until task completion or step limits are reached. The implementation demonstrates sophisticated handling of asynchronous operations, with careful attention to avoiding Tokio runtime stalls through dedicated blocking threads for storage operations.

The architecture reveals a well-structured dependency injection pattern using `std::sync::OnceLock` to manage circular dependencies with components like `TaskManager`, `LspManager`, `TeamManager`, and `McpClient`. This allows lazy initialization while maintaining clean separation of concerns. The processor implements multiple guidance systems for steering LLM behavior, including specialized prompts for Ollama models, LSP code intelligence tools, and codebase indexing. These guidance sections use strong directive language to prevent common failure modes like tool hallucination or inappropriate tool selection.

The implementation shows production-ready patterns for error handling, with comprehensive error publishing to an event bus that ensures the TUI remains responsive even when failures occur. The system supports advanced features including multimodal inputs (handling `MessagePart::Image` alongside text), team context resolution for multi-agent scenarios, permission checking for tool invocations, and automatic memory extraction for candidate generation. The streaming architecture with configurable timeouts, retries, and backoff demonstrates resilience engineering appropriate for production LLM integrations.

## Related

### Entities

- [SessionProcessor](../entities/sessionprocessor.md) — technology
- [PendingToolCall](../entities/pendingtoolcall.md) — technology
- [Ollama](../entities/ollama.md) — technology
- [MCP (Model Context Protocol)](../entities/mcp-model-context-protocol.md) — technology

### Concepts

- [Agentic Conversation Loop](../concepts/agentic-conversation-loop.md)
- [Guidance Engineering for LLM Tool Use](../concepts/guidance-engineering-for-llm-tool-use.md)
- [Context Window Management](../concepts/context-window-management.md)
- [Event-Driven Architecture for Agent Systems](../concepts/event-driven-architecture-for-agent-systems.md)
- [Resilience Patterns for LLM Streaming](../concepts/resilience-patterns-for-llm-streaming.md)

