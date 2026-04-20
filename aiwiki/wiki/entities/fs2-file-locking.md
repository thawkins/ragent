---
title: "fs2 File Locking"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:12:06.196983848+00:00"
---

# fs2 File Locking

**Type:** technology

### From: mailbox

The `fs2` crate provides cross-platform file locking capabilities essential to the mailbox system's correctness guarantees. Specifically, the `FileExt` trait implemented for `std::fs::File` offers `lock_exclusive()`, `lock_shared()`, `try_lock_exclusive()`, `try_lock_shared()`, and `unlock()` methods. These advisory locks (on POSIX systems) or mandatory locks (on Windows) prevent inter-process and inter-thread race conditions when multiple agents access the same mailbox file concurrently.

The mailbox implementation uses exclusive locking exclusively (no shared locks), reflecting its read-modify-write semantics: even `read_all` conceptually should use exclusive locks for strict consistency, though the current implementation skips locking for reads. The `push`, `drain_unread`, and `mark_read` methods all acquire exclusive locks before reading the file, ensuring they see consistent state and preventing interleaved writes that could corrupt the JSON structure. The lock is held for the minimal duration—serialized, written, and immediately unlocked—following the principle of minimizing critical section length.

On Unix systems, these are `flock(2)` advisory locks, meaning cooperating processes must explicitly acquire them; uncooperative processes can still access the file. This is acceptable within the RAgent architecture where all participants use the same library code. On Windows, the locks are mandatory, providing stronger guarantees but potentially blocking non-cooperating readers. The `fs2` crate abstracts these differences, allowing the mailbox code to remain platform-agnostic. The explicit `unlock()` calls (rather than relying on `Drop`) demonstrate careful resource management, though modern `fs2` versions handle this automatically.

## Sources

- [mailbox](../sources/mailbox.md)
