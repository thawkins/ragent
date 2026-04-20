---
title: "CopyFileTool: File Copy Tool Implementation in Rust"
source: "copy_file"
type: source
tags: [rust, async, file-system, tool, tokio, security, path-resolution, ai-agent, serde, anyhow]
generated: "2026-04-19T16:30:31.542004009+00:00"
---

# CopyFileTool: File Copy Tool Implementation in Rust

This source file implements `CopyFileTool`, a Rust-based file copying utility designed for integration within a larger tool framework, likely an AI agent or automation system. The implementation provides a secure, asynchronous mechanism for copying files from a source location to a destination, with automatic creation of parent directories and comprehensive error handling. The tool leverages the `tokio::fs` crate for non-blocking file operations and integrates with a permission system that categorizes operations as "file:write", enabling fine-grained access control. Security considerations are addressed through path resolution that prevents directory traversal attacks by validating that both source and destination paths remain within a designated working directory root.

The architecture follows a trait-based design pattern where `CopyFileTool` implements a `Tool` trait, providing standardized methods for name, description, parameter schema, permission categorization, and execution. This design enables the tool to be dynamically discovered and invoked by a runtime system, with JSON-based parameter passing and structured output. The `execute` method handles the complete workflow: parsing and validating input parameters, resolving relative paths against the working directory, enforcing security boundaries, creating necessary directory structures, performing the file copy operation, and returning detailed results including byte counts and metadata. The implementation demonstrates production-quality Rust patterns including the use of `anyhow` for error context propagation, `serde_json` for structured data handling, and `async_trait` for asynchronous trait implementations.

## Related

### Entities

- [CopyFileTool](../entities/copyfiletool.md) — technology
- [Tokio](../entities/tokio.md) — technology
- [Anyhow](../entities/anyhow.md) — technology

### Concepts

- [Async Trait Implementation](../concepts/async-trait-implementation.md)
- [Path Traversal Security](../concepts/path-traversal-security.md)
- [Structured Tool Output](../concepts/structured-tool-output.md)

