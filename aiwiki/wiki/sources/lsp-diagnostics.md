---
title: "LspDiagnosticsTool: LSP Diagnostics Extraction Tool for AI Agents"
source: "lsp_diagnostics"
type: source
tags: [rust, lsp, language-server-protocol, diagnostics, compiler-errors, tooling, ai-agent, async-rust, code-analysis, ragent-core]
generated: "2026-04-19T18:22:18.100125560+00:00"
---

# LspDiagnosticsTool: LSP Diagnostics Extraction Tool for AI Agents

The `lsp_diagnostics.rs` source file implements `LspDiagnosticsTool`, a Rust-based tool designed to surface compiler errors, warnings, and hints from connected Language Server Protocol (LSP) servers within the ragent-core framework. This tool acts as a bridge between LSP servers and AI agents, enabling programmatic access to diagnostic information that would typically be displayed in an IDE. The implementation leverages the `lsp-types` crate for standardized LSP communication, `serde_json` for parameter and output serialization, and `anyhow` for robust error handling. The tool supports filtering diagnostics by file path and severity level, making it highly configurable for different use cases in automated code analysis workflows.

The architecture of `LspDiagnosticsTool` follows a trait-based design pattern, implementing the `Tool` trait that defines the contract for all tools in the ragent system. This design enables consistent integration with the broader tool execution framework. The `execute` method orchestrates the diagnostic retrieval process: it parses input parameters, validates the LSP manager configuration, resolves file paths, retrieves accumulated diagnostics from the LSP manager's internal state, filters them according to user-specified criteria, and formats the results for human-readable output while also providing structured JSON metadata. The tool handles edge cases such as missing LSP managers, unresolved file paths, and empty diagnostic results with appropriate error messages and graceful degradation.

The source code demonstrates several sophisticated Rust programming patterns including async/await for non-blocking I/O operations, pattern matching for severity parsing and ranking, and careful memory management with owned and borrowed string types. The severity filtering system is particularly notable for its intelligent ranking mechanism that maps LSP's inverse severity scale (where lower integers indicate higher severity) to intuitive filter behavior. The URI shortening functionality enhances output readability by converting absolute file paths to relative paths when files reside within the current working directory, making diagnostic output more concise and relevant to the user's context.

## Related

### Entities

- [LspDiagnosticsTool](../entities/lspdiagnosticstool.md) — technology
- [ragent-core](../entities/ragent-core.md) — technology
- [Language Server Protocol (LSP)](../entities/language-server-protocol-lsp.md) — technology

### Concepts

- [Diagnostic Severity Filtering](../concepts/diagnostic-severity-filtering.md)
- [Async Tool Execution in Rust](../concepts/async-tool-execution-in-rust.md)
- [JSON Schema Parameter Validation](../concepts/json-schema-parameter-validation.md)
- [URI Shortening and Path Normalization](../concepts/uri-shortening-and-path-normalization.md)
- [AI Agent Tool Design Patterns](../concepts/ai-agent-tool-design-patterns.md)

