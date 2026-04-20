---
title: "LspSymbolsTool: LSP Document Symbol Extraction for Code Analysis"
source: "lsp_symbols"
type: source
tags: [rust, lsp, language-server-protocol, code-analysis, static-analysis, developer-tools, async, ai-agents, symbol-extraction, programming-tools]
generated: "2026-04-19T18:28:57.174055107+00:00"
---

# LspSymbolsTool: LSP Document Symbol Extraction for Code Analysis

This document describes the implementation of `LspSymbolsTool`, a Rust-based tool that interfaces with Language Server Protocol (LSP) servers to extract and display structured symbol information from source code files. The tool serves as a bridge between AI agents and the rich semantic understanding provided by language-specific LSP servers, enabling automated code comprehension without requiring line-by-line file reading. The implementation demonstrates sophisticated asynchronous Rust patterns, including the use of `async-trait` for trait-based async methods, proper error handling with the `anyhow` crate, and structured JSON schema validation for tool parameters.

The `LspSymbolsTool` is part of a larger agent framework (ragent) and follows a well-defined tool interface pattern. It accepts a file path parameter, resolves it against a working directory, and communicates with an appropriate LSP server based on the file extension. The tool handles both nested hierarchical symbol structures (where symbols can contain children, such as classes with methods) and flat symbol lists, normalizing both into a consistent output format. This dual-mode handling is crucial because different LSP servers may return symbol information in either format depending on their capabilities and the target language's structure.

The implementation showcases several important software engineering practices: comprehensive error context propagation using `with_context`, permission-based access control through the `lsp:read` category, and rich metadata generation for downstream consumption. The output formatting is carefully designed for human readability while maintaining machine-parseable structure, using consistent column widths and line number references. The tool's architecture enables AI agents to quickly understand code organization, identify relevant symbols for further analysis, and navigate complex codebases with semantic awareness rather than purely syntactic pattern matching.

## Related

### Entities

- [LspSymbolsTool](../entities/lspsymbolstool.md) — technology
- [Language Server Protocol (LSP)](../entities/language-server-protocol-lsp.md) — technology
- [rust-analyzer](../entities/rust-analyzer.md) — technology

### Concepts

- [Document Symbol Extraction](../concepts/document-symbol-extraction.md)
- [Async Tool Interfaces in Rust](../concepts/async-tool-interfaces-in-rust.md)
- [Hierarchical vs Flat Symbol Representations](../concepts/hierarchical-vs-flat-symbol-representations.md)
- [Permission-Based Tool Access Control](../concepts/permission-based-tool-access-control.md)

