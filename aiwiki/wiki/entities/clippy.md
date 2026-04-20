---
title: "Clippy"
entity_type: "technology"
type: entity
generated: "2026-04-19T14:54:44.380851007+00:00"
---

# Clippy

**Type:** technology

### From: ref:AGENTS

Clippy is Rust's official linter tool that provides comprehensive static analysis beyond the compiler's built-in warnings, catching common mistakes, style violations, and potential performance issues in Rust code. The document mandates cargo clippy as the standard linting command, integrating it into the development workflow alongside formatting checks. Clippy's lint categories cover correctness, suspicious patterns, style conventions, complexity metrics, and performance optimizations, with the guidelines specifically noting a cognitive complexity threshold of ≤30 and warnings for missing documentation as configured requirements.

The integration of Clippy into these agent guidelines reflects its established position in the Rust ecosystem as an essential quality assurance tool. Clippy's ability to detect anti-patterns such as unnecessary clones, inefficient iterator usage, and non-idiomatic Rust constructs aligns with the document's emphasis on best practices and clean, scalable code. The cognitive complexity limit of 30 points specifically references Clippy's complexity lint which measures how difficult code is to understand based on nesting depth, branching, and other structural factors. This metric helps maintain codebase maintainability as projects scale.

The prohibition against wildcard imports (use crate::module::*) is another Clippy-enforceable rule mentioned in the guidelines, promoting explicit dependency declaration and reducing namespace pollution. Clippy's documentation lints support the mandate for comprehensive docblock coverage of public APIs. When combined with cargo fmt for automatic formatting, Clippy creates a two-phase quality gate that can be automated in continuous integration pipelines, ensuring consistent code standards across distributed development teams and AI agent contributions.

## External Resources

- [Clippy linting tool official documentation](https://doc.rust-lang.org/clippy/) - Clippy linting tool official documentation
- [Complete list of Clippy lints with explanations](https://rust-lang.github.io/rust-clippy/master/index.html) - Complete list of Clippy lints with explanations

## Sources

- [ref:AGENTS](../sources/ref-agents.md)
