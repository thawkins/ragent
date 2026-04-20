---
title: "Error Handling Patterns"
type: concept
generated: "2026-04-19T14:54:44.382848550+00:00"
---

# Error Handling Patterns

### From: ref:AGENTS

The error handling patterns in these guidelines represent a mature, stratified approach to Rust's Result-based error propagation, distinguishing between application-level and library-level error requirements. The core strategy employs anyhow::Result for main functions and application code where error type specificity is unnecessary, while using thiserror for deriving custom error types when library consumers need programmatic error discrimination. This dual-crate approach acknowledges the fundamental tension in error handling: ergonomics and flexibility versus precision and stability. The ? operator serves as the primary propagation mechanism, with type inference determining appropriate conversions based on the Result types in scope.

The anyhow crate's role in application code eliminates boilerplate for error types that would never be matched programmatically, while preserving error context through its context attachment methods. For library code, thiserror-derived enums provide stable variants that downstream code can match against, with automatic implementations of Display, Error, and From traits. The guidelines' explicit type preference recommendation—using explicit types and type aliases for complex signatures—prevents inference failures that could propagate confusing errors from deep in the crate graph.

The prohibition of panic-based error handling (implied by the Result-centric recommendations) and the specific logging integration with tracing create a comprehensive error management strategy. Errors become structured data that can be logged with full context, propagated with semantic meaning, and converted between representations as they cross application/library boundaries. This pattern language reflects Rust ecosystem evolution from early panic-heavy code toward sophisticated error handling that maintains performance while providing observability. The specific crate selections—anyhow and thiserror both by David Tolnay—indicate preference for well-maintained, widely-deployed solutions over bespoke implementations.

## External Resources

- [Rust Book error handling fundamentals](https://doc.rust-lang.org/book/ch09-00-error-handling.html) - Rust Book error handling fundamentals
- [Sled database author's philosophy on Rust error handling](https://sled.rs/errors.html) - Sled database author's philosophy on Rust error handling

## Sources

- [ref:AGENTS](../sources/ref-agents.md)
