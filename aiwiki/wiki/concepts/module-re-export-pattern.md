---
title: "Module Re-export Pattern"
type: concept
generated: "2026-04-19T17:04:17.240591282+00:00"
---

# Module Re-export Pattern

### From: memory_replace

The module re-export pattern is a fundamental code organization technique in Rust that allows a single implementation to be exposed through multiple module paths. In this case, the `MemoryReplaceTool` is implemented in `memory_write.rs` but made available through a dedicated `memory_replace.rs` module. This approach serves several important purposes: it maintains API consistency by giving each conceptual tool its own module path, it avoids code duplication by not copying the implementation, and it allows the internal organization to differ from the public structure. The pattern is particularly valuable in large crates where consumers expect a predictable module hierarchy. By using `pub use`, the module creates an alias that is transparent to downstream users—they cannot tell from the public API whether the type is defined locally or re-exported. This abstraction allows maintainers to refactor internal organization without breaking downstream code.

## External Resources

- [Rust Reference: Use declarations for re-export patterns](https://doc.rust-lang.org/reference/items/use-declarations.html) - Rust Reference: Use declarations for re-export patterns
- [Rust API Guidelines: Naming conventions and module structure](https://rust-lang.github.io/api-guidelines/naming.html) - Rust API Guidelines: Naming conventions and module structure

## Sources

- [memory_replace](../sources/memory-replace.md)
