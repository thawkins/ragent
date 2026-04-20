---
title: "ragent-core: Core Library for the ragent AI Coding Agent"
source: "lib"
type: source
tags: [rust, ai-agent, coding-assistant, llm, lsp, mcp, orchestration, core-library, modular-architecture, agent-system]
generated: "2026-04-19T22:08:52.631230244+00:00"
---

# ragent-core: Core Library for the ragent AI Coding Agent

The ragent-core library serves as the foundational crate for the ragent AI coding agent system, providing essential modules for agent orchestration, configuration management, LLM integration, and tool execution. Written in Rust, this crate establishes the architectural backbone that enables intelligent code assistance through modular design patterns. The library encompasses approximately 28 public modules spanning core concerns from agent lifecycle management to specialized features like Language Server Protocol (LSP) integration, Model Context Protocol (MCP) support, and GitHub/GitLab integrations. The codebase demonstrates sophisticated software engineering practices by separating concerns into discrete, well-documented modules that handle everything from permission management and input sanitization to resource limits and auto-update capabilities.

The architecture reveals several innovative approaches to AI agent design, including a sub-agent task management system for spawning and tracking multiple agents, team coordination capabilities with shared task lists and mailboxes, and a comprehensive skill discovery and invocation framework. Security considerations are deeply embedded throughout, with dedicated modules for bash command allowlisting/denylisting, input sanitization with secret redaction, and a configurable "YOLO mode" for development flexibility. The reference to "SPEC §3.34" for file reference parsing indicates adherence to formal specifications, suggesting this is part of a larger, well-documented system. The crate also demonstrates operational maturity through features like lifecycle hooks, process resource limits for bounded concurrency, and automatic binary updates from GitHub releases.

## Related

### Entities

- [ragent AI Coding Agent](../entities/ragent-ai-coding-agent.md) — product
- [Language Server Protocol (LSP)](../entities/language-server-protocol-lsp.md) — technology
- [Model Context Protocol (MCP)](../entities/model-context-protocol-mcp.md) — technology

### Concepts

- [Agent Orchestration Architecture](../concepts/agent-orchestration-architecture.md)
- [Skill Framework and Dynamic Capability Loading](../concepts/skill-framework-and-dynamic-capability-loading.md)
- [Security-First Agent Design: Permission Layers and Input Sanitization](../concepts/security-first-agent-design-permission-layers-and-input-sanitization.md)
- [Reference Resolution and Fuzzy Matching for Code Intelligence](../concepts/reference-resolution-and-fuzzy-matching-for-code-intelligence.md)

