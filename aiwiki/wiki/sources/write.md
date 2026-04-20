---
title: "Ragent Core Write Tool Implementation"
source: "write"
type: source
tags: [rust, async, file-io, agent-framework, tool-system, serde-json, tokio, anyhow, security, directory-traversal-protection]
generated: "2026-04-19T16:56:33.669075807+00:00"
---

# Ragent Core Write Tool Implementation

This document contains the Rust source code implementation of the `WriteTool` struct in the ragent-core crate, which provides file writing capabilities for an agent-based system. The `WriteTool` is designed to write string content to files while automatically creating parent directories when they don't exist, making it a foundational utility for file system operations within the agent framework. The implementation demonstrates several important Rust programming patterns including error handling with the `anyhow` crate, asynchronous I/O operations using `tokio::fs`, JSON schema definition with `serde_json`, and the trait-based tool architecture that allows for consistent interfaces across different tools.

The code reveals a security-conscious design through the use of `check_path_within_root` to prevent directory traversal attacks, ensuring that write operations are constrained within a designated working directory. The tool implements a comprehensive lifecycle through the `Tool` trait, providing metadata such as its name ("write"), description, JSON schema for parameter validation, and permission categorization ("file:write") for access control. The asynchronous `execute` method handles the complete workflow from parameter extraction and path resolution to directory creation, file writing, and result reporting with detailed metadata including byte count and line count statistics.

## Related

### Entities

- [WriteTool](../entities/writetool.md) — product
- [Tokio](../entities/tokio.md) — technology
- [Serde JSON](../entities/serde-json.md) — technology

### Concepts

- [Directory Traversal Protection](../concepts/directory-traversal-protection.md)
- [Async Trait Pattern in Rust](../concepts/async-trait-pattern-in-rust.md)
- [JSON Schema for Tool Interfaces](../concepts/json-schema-for-tool-interfaces.md)
- [Error Context Propagation](../concepts/error-context-propagation.md)

