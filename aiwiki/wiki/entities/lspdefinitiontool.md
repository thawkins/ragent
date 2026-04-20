---
title: "LspDefinitionTool"
entity_type: "technology"
type: entity
generated: "2026-04-19T18:19:14.084587261+00:00"
---

# LspDefinitionTool

**Type:** technology

### From: lsp_definition

LspDefinitionTool is a Rust struct that implements the Tool trait to provide go-to-definition functionality for an intelligent agent system. This tool serves as a bridge between agent commands and Language Server Protocol servers, enabling automatic code navigation and symbol resolution. The tool's primary purpose is to locate where symbols are defined in source code, returning file paths and precise line numbers for discovered definitions.

The implementation demonstrates sophisticated integration with the broader LSP ecosystem. It requires an LSP manager to be configured in the agent's context, which maintains connections to language-specific servers. When executed, the tool performs coordinate translation from 1-based line and column numbers (user-friendly) to 0-based indices (LSP protocol standard), canonicalizes file paths for consistent handling, and manages document lifecycle through opening and tracking. The tool handles multiple response formats from LSP servers: single scalar locations, arrays of locations for overloaded symbols, and location links, ensuring broad compatibility with different language server implementations.

The tool's permission category of "lsp:read" classifies it as a non-destructive read operation, appropriate for safe execution in agent workflows. Error handling is comprehensive, covering missing parameters, unresolvable paths, unavailable LSP servers, document opening failures, and request execution errors. Results are formatted in both human-readable text and structured JSON metadata, supporting both direct user presentation and downstream programmatic processing.

## Diagram

```mermaid
flowchart TD
    subgraph Input["Input Processing"]
        A[Extract path, line, column] --> B[Convert to 0-based LSP coordinates]
        B --> C[Canonicalize file path]
    end
    
    subgraph LSP["LSP Interaction"]
        D[Acquire LSP manager] --> E[Get client for file type]
        E --> F[Open document]
        F --> G[Build GotoDefinitionParams]
        G --> H[Send textDocument/definition request]
    end
    
    subgraph Output["Response Handling"]
        I{Response type?} -->|Scalar| J[Single location]
        I -->|Array| K[Multiple locations]
        I -->|Link| L[Convert links to locations]
        I -->|None| M[Empty result]
        J --> N[Format output]
        K --> N
        L --> N
        M --> O[Return 'not found']
    end
    
    C --> D
    H --> I
    N --> P[Return ToolOutput with content and metadata]
```

## External Resources

- [Official Language Server Protocol specification](https://microsoft.github.io/language-server-protocol/) - Official Language Server Protocol specification
- [lsp-types crate documentation for Rust LSP types](https://docs.rs/lsp-types/latest/lsp_types/) - lsp-types crate documentation for Rust LSP types
- [async-trait crate for async trait implementations](https://crates.io/crates/async-trait) - async-trait crate for async trait implementations

## Sources

- [lsp_definition](../sources/lsp-definition.md)
