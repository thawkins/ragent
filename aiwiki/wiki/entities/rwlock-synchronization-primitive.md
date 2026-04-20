---
title: "RwLock Synchronization Primitive"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:32:34.191563645+00:00"
---

# RwLock Synchronization Primitive

**Type:** technology

### From: sanitize

`RwLock` (Read-Write Lock) is a synchronization primitive from Rust's standard library that enables multiple concurrent readers or a single writer, but never both simultaneously. In `sanitize.rs`, it protects the `SECRET_REGISTRY` HashSet, allowing efficient concurrent reads during the redaction phase while ensuring exclusive access during secret registration operations. This design choice reflects careful consideration of the expected access patterns: secret registration occurs relatively infrequently (typically at startup or credential rotation events), while redaction operations happen continuously as messages flow through the system. The `RwLock` implementation in Rust's standard library uses operating system primitives on most platforms, with `parking_lot` providing more efficient implementations on some targets. The module's use of `write()` with proper error handling (`if let Ok(mut registry) = ...`) demonstrates defensive programming against poisoned locks, though in practice poison errors typically propagate panics from other threads. The choice of `RwLock` over `Mutex` here is justified by the read-heavy workload pattern and the need to avoid unnecessary contention when multiple threads simultaneously process messages requiring redaction.

## External Resources

- [Rust standard library documentation for RwLock](https://doc.rust-lang.org/std/sync/struct.RwLock.html) - Rust standard library documentation for RwLock
- [The Rustonomicon chapter on race conditions and synchronization](https://doc.rust-lang.org/nomicon/races.html) - The Rustonomicon chapter on race conditions and synchronization

## Sources

- [sanitize](../sources/sanitize.md)
