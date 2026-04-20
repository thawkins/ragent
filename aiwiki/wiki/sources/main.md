---
title: "ragent CLI Binary - Main Entry Point"
source: "main"
type: source
tags: [rust, cli, ai-agent, async, tokio, clap, tracing, multi-agent, orchestration, mcp, tui, session-management, memory-management, anthropic, openai, ollama]
generated: "2026-04-19T14:56:28.623783262+00:00"
---

# ragent CLI Binary - Main Entry Point

This document presents the main entry point for the ragent CLI application, a Rust-based AI coding agent designed for terminal use. The source file `main.rs` serves as the orchestration layer that parses command-line arguments using the clap crate, initializes the comprehensive infrastructure including tracing/logging, storage backends, event bus, and various registries for providers and tools, then dispatches execution to the appropriate sub-command handlers. The application supports multiple operational modes including an interactive TUI (Terminal User Interface), headless execution, HTTP server mode, session management, memory import/export, and a demonstration of multi-agent orchestration capabilities.

The architecture demonstrates sophisticated Rust patterns including dependency injection through Arc-wrapped shared state, async/await throughout with tokio as the runtime, and careful initialization ordering to resolve circular dependencies. Notable features include platform-appropriate data directory resolution, flexible configuration loading with CLI overrides, secret registry seeding for automatic credential redaction in logs, and MCP (Model Context Protocol) server integration for extended tool capabilities. The code shows production-ready concerns like graceful error handling, structured logging with tracing, rate limiting preparation for server mode, and support for multiple AI providers including Anthropic, OpenAI, and Ollama.

The multi-agent orchestration example embedded in the code illustrates how the Coordinator pattern enables job distribution to registered agents based on capability matching, with responders handling payloads asynchronously. Session management provides persistence for conversation history with import/export capabilities supporting multiple formats (ragent native, Cline, and Claude Code). The authentication system supports provider-specific API key storage with fallback to environment variables, while the model listing functionality can discover available models from both local Ollama instances and remote Ollama Cloud services.

## Related

### Entities

- [ragent](../entities/ragent.md) — product
- [tokio](../entities/tokio.md) — technology
- [clap](../entities/clap.md) — technology
- [Model Context Protocol](../entities/model-context-protocol.md) — technology
- [SQLite](../entities/sqlite.md) — technology

### Concepts

- [Multi-Agent Orchestration](../concepts/multi-agent-orchestration.md)
- [Session-Based Architecture](../concepts/session-based-architecture.md)
- [Secret Registry and Log Sanitization](../concepts/secret-registry-and-log-sanitization.md)
- [Capability-Based Provider Resolution](../concepts/capability-based-provider-resolution.md)
- [Async Initialization Patterns](../concepts/async-initialization-patterns.md)

