---
title: "Reverse-Order Hunk Application"
type: concept
generated: "2026-04-19T16:22:22.987143740+00:00"
---

# Reverse-Order Hunk Application

### From: patch

Reverse-order hunk application is an optimization technique that prevents line number invalidation when applying multiple hunks to the same file, by processing hunks from highest to lowest starting line number rather than in their natural diff order. This approach solves a fundamental problem in line-based patching: when a hunk is applied, it typically changes the file's line count (additions increase it, removals decrease it), which shifts the line numbers of all content below the modification point. If hunks were applied in forward order, each successful application would require recalculating the line number offsets for all subsequent hunks, introducing complexity and potential for arithmetic errors.

The ragent-core implementation achieves this through a concise but critical operation: collecting hunks into a vector and sorting by old_start in descending order using sort_by with a reversed comparison (b.old_start.cmp(&a.old_start)). This ensures that modifications to later parts of the file are applied first, so when earlier hunks are subsequently applied, their target lines haven't shifted due to earlier modifications in the same application pass. The technique assumes that hunks don't overlap in their affected line ranges—a requirement of well-formed unified diffs, where overlapping changes would create ambiguity about the intended final state. When hunks are independent (affecting disjoint line ranges), reverse ordering guarantees that each hunk's old_start remains valid throughout the application process.

This pattern appears in many patch implementations but is often undocumented or implemented implicitly. Its explicit use here, with a clarifying comment about the rationale, demonstrates code maintainability practices. The optimization is particularly important for agentic tools that may process large patches automatically, where manual inspection of hunk interactions is not feasible. Alternative approaches include forward application with dynamic offset tracking (maintaining a running delta of line count changes) or conversion to a line-based edit script representation that abstracts away line numbers entirely. However, the reverse-order approach remains popular for its simplicity and minimal memory overhead, requiring only a sort operation rather than additional data structures or complex offset arithmetic during the application loop.

## External Resources

- [Historical discussion of patch algorithm implementations](https://www.artima.com/articles/the-patch-dolittle) - Historical discussion of patch algorithm implementations

## Sources

- [patch](../sources/patch.md)
