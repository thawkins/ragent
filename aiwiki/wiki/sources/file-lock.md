---
title: "Per-File Locking Mechanism for Concurrent File Editing in Rust"
source: "file_lock"
type: source
tags: [rust, concurrency, async, file-locking, tokio, synchronization, race-condition-prevention, systems-programming, ragent-core]
generated: "2026-04-19T16:08:40.088351196+00:00"
---

# Per-File Locking Mechanism for Concurrent File Editing in Rust

This Rust source code implements a sophisticated per-file locking system designed to serialize concurrent edit operations on files, preventing race conditions in multi-threaded or asynchronous environments. The module uses a global registry of per-file mutexes, implemented through a combination of Tokio's async synchronization primitives and standard Rust collections. At its core, the system maintains a `LazyLock`-initialized `RwLock`-protected `HashMap` that maps canonical file paths to `Arc<Mutex<()>>` instances, enabling fine-grained locking where different files can be edited in parallel while operations on the same file are automatically serialized.

The primary entry point is the `lock_file` function, which returns an `OwnedMutexGuard<()>` that must be held throughout the entire read-modify-write sequence. This design ensures that even if multiple `edit` or `multiedit` tool calls target the same file simultaneously, they will execute sequentially rather than interleaving their operations and potentially corrupting the file. The function first canonicalizes the path to handle symbolic links and relative paths consistently, then either retrieves an existing mutex or creates a new one atomically. The `cleanup_unused_locks` function provides optional maintenance by removing entries from the HashMap when no references remain, preventing unbounded growth during long-running sessions without leaking resources.

The implementation demonstrates several advanced Rust patterns for concurrent programming, including the use of `Arc` for shared ownership across async boundaries, `RwLock` for efficient read-heavy access to the registry, and `OwnedMutexGuard` for guard-based resource management. The choice of Tokio's synchronization primitives over standard library alternatives reflects the async runtime requirements of the broader application, while the `LazyLock` initialization ensures thread-safe, on-demand creation of the global registry without requiring explicit initialization code.

## Related

### Entities

- [Tokio](../entities/tokio.md) — technology
- [Rust Programming Language](../entities/rust-programming-language.md) — technology
- [ragent-core](../entities/ragent-core.md) — product

### Concepts

- [Per-File Locking](../concepts/per-file-locking.md)
- [Read-Modify-Write Pattern](../concepts/read-modify-write-pattern.md)
- [Reference Counting for Resource Management](../concepts/reference-counting-for-resource-management.md)
- [Canonical Path Resolution](../concepts/canonical-path-resolution.md)

