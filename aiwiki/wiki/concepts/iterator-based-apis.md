---
title: "Iterator-Based APIs"
type: concept
generated: "2026-04-19T21:06:41.383249505+00:00"
---

# Iterator-Based APIs

### From: wrapper

Iterator-based APIs represent a fundamental Rust design pattern that enables lazy, composable, and zero-cost sequence processing. The generic parameter `I: IntoIterator<Item = (PathBuf, String)>` in `apply_edits_from_pairs` embodies this pattern, accepting any type that can be converted into an iterator of path-content pairs. This design decision has profound implications for flexibility, performance, and API ergonomics that reward understanding.

The `IntoIterator` trait bound, rather than requiring a concrete `Vec` or slice, allows callers to provide data in whatever form is most natural: a `Vec` of pre-collected changes, a `HashMap` drain, a streaming iterator from parsing, or even a generator yielding edits as they're computed. This eliminates forced allocations and enables lazy computation where edits are produced on-demand during application. The pattern aligns with Rust's broader iterator ecosystem, where methods like `map`, `filter`, and `collect` enable expressive data transformations without intermediate allocations.

The tuple type `(PathBuf, String)` represents a common Rust idiom for heterogeneous iterator items, pairing the path (owned, platform-native) with content (owned, UTF-8 String). This choice over a custom struct maintains simplicity while remaining self-documenting through type names. The pattern's zero-cost nature means the generic abstraction compiles to code as efficient as hand-written loops over concrete types. Iterator-based APIs also facilitate testing, as mock data can be provided through simple arrays without complex setup. The prevalence of this pattern in Rust's standard library and ecosystem crates demonstrates its status as a community-accepted best practice for flexible, performant interfaces.

## External Resources

- [Standard library documentation for the Iterator trait](https://doc.rust-lang.org/std/iter/trait.Iterator.html) - Standard library documentation for the Iterator trait
- [Documentation for IntoIterator trait](https://doc.rust-lang.org/std/iter/trait.IntoIterator.html) - Documentation for IntoIterator trait
- [Rust Design Patterns book on related patterns](https://rust-unofficial.github.io/patterns/patterns/behavioural/visitor.html) - Rust Design Patterns book on related patterns

## Related

- [Zero-Cost Abstractions](zero-cost-abstractions.md)

## Sources

- [wrapper](../sources/wrapper.md)
