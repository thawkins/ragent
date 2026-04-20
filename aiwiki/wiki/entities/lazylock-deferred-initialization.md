---
title: "LazyLock Deferred Initialization"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:32:34.191871659+00:00"
---

# LazyLock Deferred Initialization

**Type:** technology

### From: sanitize

`LazyLock` is a thread-safe lazy initialization primitive stabilized in Rust 1.80, replacing the popular `lazy_static` crate for most use cases. It defers expensive initialization until first access, then caches the result for subsequent uses. In `sanitize.rs`, `LazyLock` serves two critical purposes: first, it delays regex compilation (a computationally expensive operation) until the first call to `redact_secrets`, improving application startup time; second, it provides thread-safe initialization of the `SECRET_REGISTRY` without requiring separate synchronization for the initialization itself. The `LazyLock::new` constructor accepts a closure that computes the initial value, which is guaranteed to execute exactly once even under concurrent access from multiple threads. This pattern eliminates the need for `OnceCell` or manual double-checked locking patterns that were previously common in Rust. The stabilization of `LazyLock` in the standard library represents a significant milestone in Rust's concurrency primitives, providing zero-cost abstractions that match the performance characteristics of hand-rolled initialization while maintaining complete memory safety.

## External Resources

- [Official Rust documentation for LazyLock](https://doc.rust-lang.org/std/sync/struct.LazyLock.html) - Official Rust documentation for LazyLock
- [Rust 1.80 release notes announcing LazyLock stabilization](https://blog.rust-lang.org/2024/07/25/Rust-1.80.0.html) - Rust 1.80 release notes announcing LazyLock stabilization

## Sources

- [sanitize](../sources/sanitize.md)
