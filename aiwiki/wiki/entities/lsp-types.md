---
title: "lsp-types"
entity_type: "technology"
type: entity
generated: "2026-04-19T18:19:14.085094748+00:00"
---

# lsp-types

**Type:** technology

### From: lsp_definition

The lsp-types crate is a foundational Rust library that provides type definitions for the Language Server Protocol, enabling Rust applications to communicate with LSP-compatible language servers. This crate serves as the official Rust implementation of the LSP specification, maintained as part of the broader LSP community efforts. It defines Rust structs and enums that correspond directly to the JSON-RPC messages defined in the LSP specification, ensuring type-safe communication between clients and servers.

The crate covers the full breadth of the LSP specification, including protocol base types, window and telemetry capabilities, client and server capabilities, document synchronization, language features like completion and hover, workspace operations, and diagnostic reporting. Types used in this implementation include `GotoDefinitionParams` for parameter structuring, `GotoDefinitionResponse` as a tagged union for various response formats, `Position` for zero-based line and character indexing, `TextDocumentPositionParams` for document identification combined with positions, and `Location` for file URI and range specification. The crate's design emphasizes correctness through exhaustive type definitions, with options and nullable fields accurately representing the protocol's optional elements.

Version alignment with the LSP specification is carefully managed, with crate versions tracking protocol versions. The types are designed for serialization with serde, enabling seamless JSON-RPC communication. For Rust developers building LSP clients, servers, or tools that interact with language servers, lsp-types eliminates the error-prone process of manually defining protocol structures and ensures compatibility across the ecosystem. The crate is widely adopted by major Rust LSP implementations including rust-analyzer's own LSP client components.

## Sources

- [lsp_definition](../sources/lsp-definition.md)
