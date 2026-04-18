---
title: "Ragent LSP Integration Planning Guide"
source: "LSP_INTEGRATION_GUIDE"
type: source
tags: [LSP, Language Server Protocol, Ragent, Rust, AI coding assistant, multi-agent system, MCP, tool system, event-driven architecture, integration guide]
generated: "2026-04-18T15:03:19.526758364+00:00"
---

# Ragent LSP Integration Planning Guide

This document provides a comprehensive technical guide for integrating Language Server Protocol (LSP) capabilities into Ragent, a Rust-based multi-agent AI coding assistant. Ragent features a modular architecture organized into three crates (ragent-core, ragent-tui, ragent-server) with well-defined systems for tool registration, event-driven communication, and session processing. The guide details the existing tool system architecture including the core Tool trait, ToolContext, ToolRegistry, and 17+ built-in tools for file operations, shell execution, and web access. It also covers the event bus architecture with 30+ event types for session lifecycle, message streaming, tool calls, permissions, and agent switching.

For LSP integration, the document outlines three strategic approaches: implementing LSP as a native built-in tool, wrapping LSP functionality as an MCP (Model Context Protocol) server, or using a hybrid approach. The integration should leverage existing patterns including tool registration, event publishing for LSP-specific events, and system prompt injection to make agents aware of code intelligence capabilities. The guide provides specific file paths for core modules, implementation patterns from the session processor's main loop, and a complete checklist covering configuration, client library implementation, tool registration, slash commands, event publishing, and testing with real LSP servers like rust-analyzer.

## Related

### Entities

- [Ragent](../entities/ragent.md) — product
- [ragent-core](../entities/ragent-core.md) — technology
- [ragent-tui](../entities/ragent-tui.md) — technology
- [ragent-server](../entities/ragent-server.md) — technology
- [rust-analyzer](../entities/rust-analyzer.md) — technology
- [pylsp](../entities/pylsp.md) — technology
- [MCP](../entities/mcp.md) — technology
- [lsp-types](../entities/lsp-types.md) — technology
- [jsonrpc](../entities/jsonrpc.md) — technology

### Concepts

- [Tool System Architecture](../concepts/tool-system-architecture.md)
- [Event-Driven Architecture](../concepts/event-driven-architecture.md)
- [Agent System Prompt Building](../concepts/agent-system-prompt-building.md)
- [Session Processing Loop](../concepts/session-processing-loop.md)
- [LSP Integration Strategies](../concepts/lsp-integration-strategies.md)
- [Permission System](../concepts/permission-system.md)
- [Skill System](../concepts/skill-system.md)

