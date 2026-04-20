---
title: "Version-Based Cache Invalidation"
type: concept
generated: "2026-04-19T15:52:20.010136448+00:00"
---

# Version-Based Cache Invalidation

### From: cache

Version-based cache invalidation is a distributed systems pattern employed throughout the ragent-core cache implementation to coordinate cache freshness across multiple concurrent components without requiring complex dependency tracking or message passing between cache instances. This approach uses a monotonically increasing global counter—implemented as an `AtomicU64`—that serves as a logical timestamp for cache generations. When any component needs to invalidate caches system-wide, it simply increments this counter; individual cache entries store the version number at which they were computed and compare against the current global version to determine validity.

The pattern offers several advantages over traditional time-to-live (TTL) or explicit invalidation approaches. Unlike TTL-based caching, version invalidation doesn't require predicting how long data remains valid, eliminating the trade-off between cache staleness and unnecessary recomputation. Compared to dependency graphs or observer patterns, version-based invalidation dramatically reduces coordination overhead—cache consumers need only check a single atomic value rather than traversing complex dependency structures or receiving invalidation messages. This is particularly valuable in the LLM session context where system prompt components have interdependent but loosely coupled lifecycles.

The implementation in cache.rs demonstrates careful attention to memory ordering semantics. The `CACHE_VERSION` static uses `Ordering::SeqCst` (sequentially consistent) ordering, the strongest memory ordering guarantee, ensuring that version updates are immediately visible across all CPU cores. While this has higher overhead than weaker orderings, it provides correctness guarantees essential for cache consistency. Individual cache entries use the `Cached<T>` struct which stores both a version number and a generation counter; the generation enables debugging and potential future enhancements like cache statistics while the version enables the actual validity check. This pattern scales well because version checking is O(1) and lock-free, while only cache updates require mutex acquisition for the specific component being modified.

## External Resources

- [Rust Atomics and Locks: The Rustonomicon](https://doc.rust-lang.org/nomicon/atomics.html) - Rust Atomics and Locks: The Rustonomicon
- [Cache invalidation strategies in computer science](https://en.wikipedia.org/wiki/Cache_invalidation) - Cache invalidation strategies in computer science
- [CppCon talk: Designing for Cache Friendly C++](https://www.youtube.com/watch?v=WDIkqP4JbkE) - CppCon talk: Designing for Cache Friendly C++

## Sources

- [cache](../sources/cache.md)
