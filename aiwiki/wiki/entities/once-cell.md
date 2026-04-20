---
title: "once_cell"
entity_type: "technology"
type: entity
generated: "2026-04-19T22:05:36.472131753+00:00"
---

# once_cell

**Type:** technology

### From: intern

The `once_cell` crate provides thread-safe lazy initialization primitives for Rust, addressing a common need for global variables that are expensive to compute but only needed on first access. Prior to its stabilization in the Rust standard library, `once_cell` was the de facto solution for lazy static initialization without requiring unsafe code. The crate offers both unsynchronized `Lazy` types for single-threaded contexts and synchronized variants for thread-safe usage.

In this module, `once_cell::sync::Lazy` is specifically used to wrap the global `Mutex<StringInterner>`, ensuring that the interner is only created when first accessed and that this initialization happens exactly once even across multiple threads. This pattern eliminates the need for eager initialization at program startup, improving startup performance for applications that may not immediately need interning services. The `once_cell` crate has been so influential that its core functionality was incorporated into `std::sync::LazyLock` in Rust 1.80, though many projects continue using the crate for broader compatibility.

## External Resources

- [Documentation for the once_cell crate](https://docs.rs/once_cell/) - Documentation for the once_cell crate
- [Rust 1.80 release notes on LazyLock stabilization](https://blog.rust-lang.org/2024/07/25/Rust-1.80.0.html#lazycell-and-lazylock) - Rust 1.80 release notes on LazyLock stabilization

## Sources

- [intern](../sources/intern.md)
