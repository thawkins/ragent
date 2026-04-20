---
title: "Tool Module Consistency"
type: concept
generated: "2026-04-19T17:04:17.241042963+00:00"
---

# Tool Module Consistency

### From: memory_replace

Tool module consistency refers to the architectural principle that similar components in a system should follow identical organizational patterns. In the ragent-core crate, this principle manifests as a strict one-module-per-tool structure where every tool, regardless of implementation complexity, receives its own dedicated file. This consistency provides significant cognitive benefits for developers navigating the codebase—they can predict where to find tool definitions based on the tool name alone. The consistency also extends to how tools are imported and used throughout the system, reducing the mental overhead of remembering different patterns for different tools. While this approach can lead to extremely simple modules like `memory_replace.rs`, the trade-off is justified by the long-term maintainability and reduced onboarding friction for new contributors. The pattern also simplifies automated tooling and documentation generation, as each tool follows a predictable structure.

## External Resources

- [Rust Design Patterns: Single Source of Truth](https://rust-unofficial.github.io/patterns/patterns/structural/single-source-of-truth.html) - Rust Design Patterns: Single Source of Truth

## Sources

- [memory_replace](../sources/memory-replace.md)
