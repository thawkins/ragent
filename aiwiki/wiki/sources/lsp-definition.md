---
title: "LSP Definition Tool Implementation in Rust"
source: "lsp_definition"
type: source
tags: [rust, lsp, language-server-protocol, code-navigation, async, tool, agent, goto-definition, lsp_types]
generated: "2026-04-19T18:19:14.083963655+00:00"
---

# LSP Definition Tool Implementation in Rust

This document presents the implementation of `LspDefinitionTool`, a Rust-based tool that enables code navigation by finding symbol definitions using the Language Server Protocol (LSP). The tool is part of a larger agent system and provides a bridge between high-level agent commands and low-level LSP server interactions. The implementation demonstrates sophisticated error handling, asynchronous programming patterns, and careful coordinate system translations between 1-based user-facing coordinates and 0-based LSP protocol coordinates.

The tool follows a structured execution flow that begins with parameter validation and path resolution, proceeds through LSP client acquisition and document preparation, executes the actual definition request, and finally formats the results for human-readable output and structured metadata. The design handles multiple response formats from LSP servers, including scalar locations, arrays of locations, and location links, ensuring compatibility with diverse language server implementations. The permission categorization as "lsp:read" indicates this is a read-only operation that doesn't modify source code.

The implementation showcases several Rust best practices including the use of `anyhow` for ergonomic error handling with context propagation, `async_trait` for defining asynchronous trait implementations, and careful resource management with read locks on shared state. The `uri_to_display` helper function demonstrates defensive programming by attempting multiple parsing strategies to produce user-friendly file paths from LSP URIs, falling back gracefully when URL parsing fails.

## Related

### Entities

- [LspDefinitionTool](../entities/lspdefinitiontool.md) — technology
- [lsp-types](../entities/lsp-types.md) — technology
- [async-trait](../entities/async-trait.md) — technology

### Concepts

- [Language Server Protocol (LSP)](../concepts/language-server-protocol-lsp.md)
- [Goto Definition](../concepts/goto-definition.md)
- [Coordinate System Translation](../concepts/coordinate-system-translation.md)
- [Asynchronous Tool Execution](../concepts/asynchronous-tool-execution.md)

