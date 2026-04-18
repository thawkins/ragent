---
title: "Ragent LSP Integration - Quick Reference Guide"
source: "LSP_QUICK_REFERENCE"
type: source
tags: [LSP, Language Server Protocol, Rust, AI assistant, developer tools, code intelligence, architecture, integration guide, TUI, MCP]
generated: "2026-04-18T15:03:50.968544233+00:00"
---

# Ragent LSP Integration - Quick Reference Guide

This document provides a comprehensive technical reference for integrating Language Server Protocol (LSP) support into the Ragent AI assistant framework. It details the system architecture, which consists of three main crates: ragent-core (business logic), ragent-tui (terminal UI), and ragent-server (HTTP API). The document outlines four approaches to adding LSP functionality: native tools, MCP servers, slash commands, and skills, each with their respective trade-offs. It provides extensive code examples and file locations for key components including the tool execution flow, system prompt injection, event system, configuration loading, permission system, and message composition. The guide includes step-by-step integration recommendations, a testing checklist, and a development workflow for implementing LSP capabilities in Ragent.

## Related

### Entities

- [Ragent](../entities/ragent.md) — product
- [ragent-core](../entities/ragent-core.md) — technology
- [ragent-tui](../entities/ragent-tui.md) — technology
- [ragent-server](../entities/ragent-server.md) — technology
- [rust-analyzer](../entities/rust-analyzer.md) — product
- [MCP (Model Context Protocol)](../entities/mcp-model-context-protocol.md) — technology
- [processor.rs](../entities/processor-rs.md) — product
- [lsp-types](../entities/lsp-types.md) — technology
- [jsonrpc](../entities/jsonrpc.md) — technology

### Concepts

- [Tool System](../concepts/tool-system.md)
- [Event-Driven Architecture](../concepts/event-driven-architecture.md)
- [System Prompt Injection](../concepts/system-prompt-injection.md)
- [Session Processing](../concepts/session-processing.md)
- [Permission System](../concepts/permission-system.md)
- [Configuration Precedence](../concepts/configuration-precedence.md)
- [Message Composition](../concepts/message-composition.md)
- [Slash Commands](../concepts/slash-commands.md)
- [LSP Integration Patterns](../concepts/lsp-integration-patterns.md)
- [Skill System](../concepts/skill-system.md)

