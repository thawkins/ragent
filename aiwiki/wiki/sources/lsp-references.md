---
title: "LspReferencesTool: LSP-Based Symbol Reference Finder for Rust Agent Systems"
source: "lsp_references"
type: source
tags: [rust, lsp, language-server-protocol, code-analysis, agent-systems, async-rust, symbol-references, tool-architecture, anyhow, serde_json]
generated: "2026-04-19T18:26:34.922126480+00:00"
---

# LspReferencesTool: LSP-Based Symbol Reference Finder for Rust Agent Systems

This document presents the implementation of `LspReferencesTool`, a Rust-based tool that enables intelligent code analysis agents to discover all usages of a symbol across a workspace using the Language Server Protocol (LSP). The tool bridges the gap between agent systems and language-specific semantic analysis by delegating symbol reference queries to appropriate LSP servers, abstracting away the complexity of parsing and analyzing source code directly. The implementation demonstrates sophisticated error handling, asynchronous execution patterns, and careful coordinate system translation between 1-based user-facing positions and 0-based LSP positions.

The architecture follows a clean separation of concerns where the tool itself acts as a coordinator between the agent framework and LSP infrastructure. It validates input parameters, resolves file paths against a working directory, acquires the appropriate LSP client for the file's language, ensures the document is open in the LSP server, constructs properly formatted LSP requests, and post-processes results into human-readable and machine-parseable formats. The grouping of references by file with sorted output enhances usability for both human operators and downstream automation.

Key technical decisions include the use of saturating arithmetic for coordinate conversion to prevent underflow on edge cases, the optional inclusion of symbol declarations in results, and the dual-output format that provides both formatted text for immediate consumption and structured JSON metadata for programmatic processing. The tool exemplifies modern Rust patterns including the use of `async_trait` for async trait methods, `anyhow` for ergonomic error handling with context, and careful ownership management with `to_string_lossy()` for path conversion. Permission categorization as "lsp:read" enables fine-grained access control in security-conscious agent deployments.

## Related

### Entities

- [LspReferencesTool](../entities/lspreferencestool.md) — product
- [Microsoft Language Server Protocol (LSP)](../entities/microsoft-language-server-protocol-lsp.md) — technology
- [rust-analyzer](../entities/rust-analyzer.md) — technology

### Concepts

- [Coordinate System Translation in Source Code Analysis](../concepts/coordinate-system-translation-in-source-code-analysis.md)
- [Agent Tool Architecture and Permission Categories](../concepts/agent-tool-architecture-and-permission-categories.md)
- [Result Aggregation and Multi-Format Output](../concepts/result-aggregation-and-multi-format-output.md)
- [URI Handling and Path Conversion in Cross-Platform Tools](../concepts/uri-handling-and-path-conversion-in-cross-platform-tools.md)

