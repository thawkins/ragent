---
title: "Tracing"
entity_type: "technology"
type: entity
generated: "2026-04-19T14:54:44.381172306+00:00"
---

# Tracing

**Type:** technology

### From: ref:AGENTS

Tracing is a Rust framework for instrumenting programs with structured, context-aware diagnostics, representing a significant advancement over traditional logging approaches. The guidelines mandate tracing as the exclusive logging solution, explicitly prohibiting println! and eprintln! macros at all development phases. This requirement reflects tracing's capabilities for structured logging with key-value fields, span-based context propagation, and level-based filtering that supports both development debugging and production observability. The document specifies particular usage patterns: debug!() for non-performance-critical paths, trace!() for detailed debug scenarios, and the critical maintenance requirement to remove all debug! and tracing::debug! calls after issue resolution.

Tracing's architecture separates event recording from output formatting, enabling flexible configuration without code changes. The framework supports multiple subscriber implementations for different output destinations and formats, from human-readable console output to machine-parseable JSON for log aggregation systems. The structured nature of tracing events—with explicitly typed fields rather than interpolated strings—enables powerful downstream querying and analysis, particularly valuable in distributed systems where logs must be correlated across service boundaries. The span concept allows tracking logical units of work through the system, with automatic propagation across async boundaries when using tokio::test and compatible async runtimes.

The prohibition against println! represents a deliberate shift away from ad-hoc debugging output toward professional observability practices. The cleanup mandate for debug calls prevents log noise accumulation that plagues long-running projects, ensuring that production logs contain only actionable information. Tracing's integration with the broader Rust async ecosystem, including compatibility with tokio and other runtimes, makes it particularly suitable for modern Rust applications that the guidelines appear to target. The performance characteristics of tracing—with compile-time level filtering and efficient macro implementations—support its use even in latency-sensitive code paths where traditional logging would be prohibitive.

## External Resources

- [Tokio project's tracing documentation and tutorials](https://tokio.rs/tokio/topics/tracing) - Tokio project's tracing documentation and tutorials
- [Tracing crate API documentation](https://docs.rs/tracing/latest/tracing/) - Tracing crate API documentation

## Sources

- [ref:AGENTS](../sources/ref-agents.md)
