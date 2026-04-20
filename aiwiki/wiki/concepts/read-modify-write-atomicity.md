---
title: "Read-Modify-Write Atomicity"
type: concept
generated: "2026-04-19T21:19:36.865851857+00:00"
---

# Read-Modify-Write Atomicity

### From: task

Read-modify-write atomicity is the fundamental concurrency pattern ensuring consistent state updates in the TaskStore, where each mutating operation reads the entire task list, applies modifications in memory, and writes back a complete replacement under exclusive lock. This pattern appears in distributed systems literature as a optimistic concurrency control variant, though here implemented with pessimistic locking (exclusive file locks) rather than version vectors. The atomicity guarantee is that observers either see the complete pre-state or complete post-state, never partial modifications.

The implementation details reveal careful attention to durability and crash safety. The `write_locked()` function performs operations in a specific order: serialize to pretty-printed JSON string (memory operation), truncate file to zero length, seek to start, write all bytes, then flush. The `set_len(0)` and `seek(Start(0))` combination effectively clears the file before writing new content, ensuring no trailing garbage from longer previous content. The `flush()` call requests OS-level sync, though without `sync_all()` the data may still be in OS buffers. The exclusive lock held throughout prevents readers from observing the truncated state.

This pattern has important implications for scalability and consistency. The coarse granularity—locking the entire file for any modification—creates serialization that limits throughput but guarantees serializable isolation. For ragent's use case of human-paced task management (seconds between operations), this is entirely appropriate. The pattern also enables simple recovery: on startup, any JSON parseable file is valid state, and corrupted files (from crashes outside the locked section) can be reset to empty. Alternative implementations might use append-only logs or database transactions, but the read-modify-write approach matches the project's constraints of minimal dependencies and human-readable persistence format.

## External Resources

- [Martin Kleppmann on serializability and consistency models](https://martin.kleppmann.com/2014/11/05/serializability-vs-linearizability.html) - Martin Kleppmann on serializability and consistency models
- [Rust OpenOptions for configuring atomic file operations](https://doc.rust-lang.org/std/fs/struct.OpenOptions.html) - Rust OpenOptions for configuring atomic file operations

## Related

- [Advisory File Locking](advisory-file-locking.md)

## Sources

- [task](../sources/task.md)
