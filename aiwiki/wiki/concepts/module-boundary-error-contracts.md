---
title: "Module Boundary Error Contracts"
type: concept
generated: "2026-04-19T20:14:19.483304918+00:00"
---

# Module Boundary Error Contracts

### From: error

Module boundary error contracts define the interface stability and abstraction level for error propagation between software components. The ragent-core documentation explicitly contrasts `anyhow::Result` for "internal convenience" with `RagentError` at "module boundaries," illustrating a fundamental architectural pattern in Rust systems. Internal code prioritizes ergonomics and rapid iteration—`anyhow` enables adding context with `.context()` and automatic error conversion without type ceremony. Module boundaries prioritize stability and clarity—`RagentError` variants are a committed contract that downstream code can match against, with semantic stability requirements.

This distinction manages complexity in large systems. Within ragent-core, developers freely refactor error handling, add temporary instrumentation, and evolve internal representations. At the boundary—where ragent-core meets ragent-server, CLI tools, or external consumers—the error type becomes API surface area. Breaking changes require coordinated updates across crates, justifying the upfront design investment in comprehensive variant coverage. The pattern mirrors C++'s `std::exception` hierarchies or Java's checked exceptions, but with Rust's type system enforcing exhaustiveness in `match` expressions and preventing silent error swallowing.

The practical implementation involves conversion layers: internal functions return `anyhow::Result<T>`, boundary functions convert to `Result<T, RagentError>` using `map_err` or `From` implementations. This conversion is the architectural seam where operational decisions crystallize—internal library errors become categorized variants, sensitive details are redacted, and retry/hint information attaches. For agent systems, this boundary likely sits between the core engine (database, LLM clients, tool execution) and transport layers (HTTP APIs, WebSocket handlers, CLI interfaces), ensuring that network-exposed errors are sanitized and categorized while internal diagnostics preserve full fidelity.

## External Resources

- [anyhow context attachment for internal error handling](https://docs.rs/anyhow/latest/anyhow/struct.Context.html) - anyhow context attachment for internal error handling
- [Rust API guidelines on future-proofing and stability](https://rust-lang.github.io/api-guidelines/future-proofing.html) - Rust API guidelines on future-proofing and stability

## Sources

- [error](../sources/error.md)
