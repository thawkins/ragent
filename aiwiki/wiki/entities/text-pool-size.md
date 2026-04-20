---
title: "TEXT_POOL_SIZE"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:22:01.570270197+00:00"
---

# TEXT_POOL_SIZE

**Type:** technology

### From: pool

TEXT_POOL_SIZE is a compile-time constant that defines the maximum number of pooled String objects per thread, set to 256 in this implementation. This constant serves as a critical tuning parameter balancing memory consumption against reuse opportunities. The value 256 represents a reasonable default for typical message processing workloads—large enough to capture reuse opportunities during traffic spikes, but bounded to prevent unbounded memory growth in pathological cases. The constant is used in the Drop implementation of PooledString to conditionally return strings to the pool (`if pool.len() < TEXT_POOL_SIZE`), and in pool initialization where `Vec::with_capacity(TEXT_POOL_SIZE)` pre-allocates capacity to avoid reallocations. The choice of 256 as a power-of-two may also provide minor optimization benefits in certain allocator implementations, though this is secondary to the semantic meaning. This per-thread limit means that a system with N threads can hold up to N * 256 strings in pools, which operators must consider when sizing deployments. The module exposes this limit through `PoolStats::string_pool_capacity`, allowing monitoring systems to track utilization against this theoretical maximum. The constant could be made configurable through environment variables or build-time features in future iterations, but the hardcoded value provides predictable behavior and avoids configuration complexity for the common case.

## Sources

- [pool](../sources/pool.md)
