---
title: "Interior Mutability and Shared State"
type: concept
generated: "2026-04-19T21:00:47.688440339+00:00"
---

# Interior Mutability and Shared State

### From: coordinator

The Coordinator's architecture extensively employs interior mutability patterns to enable shared mutable state across asynchronous boundaries without requiring &mut self references. This design is essential for Rust's ownership model when multiple tasks need concurrent access to common resources. The implementation demonstrates sophisticated composition of synchronization primitives appropriate to each access pattern.

The jobs field uses Arc<DashMap<...>>, combining Arc's reference counting with DashMap's sharded locks. DashMap provides fine-grained locking at the bucket level, allowing concurrent access to different keys without contention. The Coordinator clones this Arc for move into spawned tasks, enabling background job execution while retaining access for queries. The get_mut method provides exclusive mutable access when needed, while get offers shared access—both safe through DashMap's internal synchronization.

The metrics field similarly uses Arc<Metrics>, but with AtomicU64 fields providing lock-free counter operations. This is more efficient than DashMap for simple counters where no complex data structures need protection. The atomic fetch_add/fetch_sub operations complete in constant time regardless of contention, making them ideal for hot-path instrumentation.

The policy field demonstrates Optional Trait Object pattern: Option<Arc<dyn ConflictResolver>>. The Arc enables sharing across Coordinator clones, Option provides absence representation, and dyn enables polymorphic behavior without generics. The with_policy constructor uses builder pattern for fluent configuration.

This layered approach—choosing the minimal synchronization necessary for each use case—optimizes for both performance and safety. Relaxed atomic operations suffice for metrics, sharded locks for job map, and single-owner mutation for policy configuration. The Clone derive on Coordinator itself enables cheap duplication: all internal Arcs are cloned, reference counts incremented, but no deep copying occurs.

## External Resources

- [Rust Book: Interior Mutability Pattern](https://doc.rust-lang.org/book/ch15-05-interior-mutability.html) - Rust Book: Interior Mutability Pattern
- [Rustonomicon: Send and Sync traits](https://doc.rust-lang.org/nomicon/send-and-sync.html) - Rustonomicon: Send and Sync traits
- [Async Rust: What is blocking?](https://ryhl.io/blog/async-what-is-blocking/) - Async Rust: What is blocking?

## Related

- [Interior Mutability](interior-mutability.md)

## Sources

- [coordinator](../sources/coordinator.md)
