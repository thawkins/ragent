---
title: "Async Caching Strategies"
type: concept
generated: "2026-04-19T22:07:14.369433078+00:00"
---

# Async Caching Strategies

### From: predictive

The PrefetchCache implementation demonstrates sophisticated async caching strategies tailored for high-concurrency, I/O-bound workloads typical of LLM agent systems. The core challenge addressed is providing fast, thread-safe access to cached file contents while supporting concurrent reads and writes from multiple async tasks. The solution employs a two-layer wrapping strategy: `Arc<RwLock<HashMap<...>>>` where the `Arc` enables shared ownership across async boundaries and the `RwLock` provides reader-writer locking semantics. This design allows multiple concurrent readers to access cached data without blocking, while ensuring exclusive access for cache updates. The choice of `RwLock` over `Mutex` specifically optimizes for the expected read-heavy workload where cache hits should be extremely fast.

The eviction strategy, while noted as simplistic, implements the essential semantics of bounded cache operation. When inserting a new entry at capacity, the code acquires an iterator over keys, removes the first key returned, then inserts the new entry—all within a single write lock critical section. This approach maintains consistency but sacrifices optimality, as arbitrary removal may evict frequently accessed entries. The explicit TODO for LRU replacement indicates awareness of this limitation and provides a clear path for production hardening. The capacity of 100 entries represents a heuristic based on typical file working sets in development workflows, with the `with_capacity` constructor enabling domain-specific tuning.

Beyond the basic operations, the cache design integrates with the broader async ecosystem through careful use of `Arc<String>` for values. This enables zero-copy sharing where multiple callers can receive references to the same cached content without cloning potentially large strings. The `Option<Arc<String>>` return type from `get` elegantly handles cache misses while the `cloned()` operation on the Option produces a new Option containing a cloned Arc pointer—extremely cheap compared to string cloning. The async method signatures throughout enforce that callers must `.await` lock acquisition, preventing blocking in async contexts and enabling the tokio runtime to schedule other tasks during contention. These patterns exemplify idiomatic async Rust caching that could be extracted into a reusable crate for similar applications.

## External Resources

- [Tokio RwLock documentation and performance characteristics](https://docs.rs/tokio/latest/tokio/sync/struct.RwLock.html) - Tokio RwLock documentation and performance characteristics
- [Moka - a fast, concurrent cache library for Rust](https://github.com/moka-rs/moka) - Moka - a fast, concurrent cache library for Rust

## Related

- [Speculative Execution in LLM Applications](speculative-execution-in-llm-applications.md)

## Sources

- [predictive](../sources/predictive.md)
