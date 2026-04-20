---
title: "DashMap"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:00:47.686138076+00:00"
---

# DashMap

**Type:** technology

### From: coordinator

DashMap is a concurrent associative array implementation used extensively in the Coordinator for tracking job state across asynchronous tasks. Unlike standard HashMap wrapped in Mutex, DashMap provides true lock-free concurrent access through sharded read-write locks and atomic operations, making it ideal for high-contention scenarios where multiple tasks frequently read and write job entries. The Coordinator wraps the DashMap in Arc to enable shared ownership across thread boundaries.

The specific usage pattern in the Coordinator involves storing JobEntry values keyed by job ID strings. The `start_job_async` method inserts new entries, spawned background tasks update status and results, and external callers query state through `get_job_result` and `subscribe_job_events`. This design eliminates the need for explicit locking in user code while maintaining thread safety guarantees. The lock-free nature is crucial for the event streaming pattern, where the events_tx broadcast sender must remain accessible without blocking.

DashMap's API compatibility with standard HashMap makes it a drop-in replacement with minimal code changes, while its performance characteristics—particularly for read-heavy workloads with occasional writes—match the Coordinator's access patterns. The use of `get_mut` for exclusive mutable access to job entries demonstrates awareness of DashMap's interior mutability semantics, ensuring consistent state updates without data races.

## External Resources

- [DashMap GitHub repository and documentation](https://github.com/xacrimon/dashmap) - DashMap GitHub repository and documentation
- [DashMap API reference](https://docs.rs/dashmap/latest/dashmap/struct.DashMap.html) - DashMap API reference

## Sources

- [coordinator](../sources/coordinator.md)
