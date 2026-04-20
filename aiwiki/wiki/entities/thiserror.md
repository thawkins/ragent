---
title: "Thiserror"
entity_type: "technology"
type: entity
generated: "2026-04-19T14:54:44.381770506+00:00"
---

# Thiserror

**Type:** technology

### From: ref:AGENTS

Thiserror is a procedural macro crate for deriving the standard library's Error trait, recommended in these guidelines as the complement to anyhow for library error handling. While anyhow provides flexibility for application code, thiserror enables precise, discriminable error types essential for library APIs where callers need to programmatically distinguish failure modes. The crate eliminates the repetitive boilerplate typically required for Error trait implementation, including Display formatting, source error chaining, and backtrace capture. Through derive macros and field attributes, thiserror generates efficient implementations that maintain full compatibility with Rust's error handling ecosystem.

The guidelines' error handling architecture—anyhow::Result for main and thiserror for custom errors—reflects the Rust community's evolved understanding of error handling stratification. Library errors require stability guarantees and variant-level matching, which thiserror facilitates without the manual implementation burden. The derive macro supports custom Display messages through #[error("...")] attributes, automatic source error detection for wrapped errors, and transparent passthrough for error types that simply delegate to an inner error. This expressive power enables rich error types with minimal code overhead.

Thiserror's compile-time code generation produces optimal implementations with no runtime overhead compared to hand-written Error trait implementations. The crate's design by David Tolnay, who also created anyhow and serde among other foundational Rust tools, ensures consistency with ecosystem conventions and long-term maintenance quality. Integration with tracing's error recording capabilities allows seamless conversion of thiserror-derived errors into structured log events. The explicit distinction between application and library error handling in these guidelines, with thiserror serving the latter, demonstrates sophisticated understanding of API design tradeoffs in the Rust ecosystem.

## External Resources

- [Thiserror crate documentation with derive macro details](https://docs.rs/thiserror/latest/thiserror/) - Thiserror crate documentation with derive macro details
- [Thiserror GitHub repository with implementation examples](https://github.com/dtolnay/thiserror) - Thiserror GitHub repository with implementation examples

## Sources

- [ref:AGENTS](../sources/ref-agents.md)
