---
title: "Reference Kinds in Static Analysis"
type: concept
generated: "2026-04-19T17:22:24.438711360+00:00"
---

# Reference Kinds in Static Analysis

### From: codeindex_references

Reference kinds represent a fundamental concept in static program analysis, categorizing how symbols participate in program structure and execution. The `CodeIndexReferencesTool` exposes three kinds—`call`, `type`, and `field_access`—though the underlying index likely supports additional granularity. Understanding these distinctions enables precise code navigation and refactoring: a `call` reference indicates invocation of a function or method, crucial for finding all execution paths; a `type` reference covers declarations, annotations, casts, and generic parameters, essential for inheritance analysis; while `field_access` captures struct member usage, important for data flow tracking. These categories emerge from programming language semantics. In Rust specifically, `call` encompasses function calls, method invocations, and trait implementations; `type` includes struct/enum definitions, type aliases, and generic constraints; `field_access` covers both direct field usage and automatic dereferencing. More sophisticated indices might distinguish `read` from `write` access for mutable analysis, separate `import` references for dependency tracking, or identify `override` relationships for polymorphism. The choice of granularity involves trade-offs: finer distinctions enable more precise queries but increase index size and build complexity. The `CodeIndexReferencesTool`'s approach of exposing basic kinds while potentially storing more detail allows future enhancement without breaking changes. This concept connects to academic research in program slicing, impact analysis, and code clone detection, where reference relationships form the graph structure underlying algorithmic analysis.

## External Resources

- [Overview of program analysis techniques](https://en.wikipedia.org/wiki/Program_analysis) - Overview of program analysis techniques
- [LLVM/Clang's declaration reference kinds as industry precedent](https://clang.llvm.org/doxygen/classclang_1_1Decl.html) - LLVM/Clang's declaration reference kinds as industry precedent

## Sources

- [codeindex_references](../sources/codeindex-references.md)
