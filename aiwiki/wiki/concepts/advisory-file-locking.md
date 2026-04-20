---
title: "Advisory File Locking"
type: concept
generated: "2026-04-19T21:19:36.864880550+00:00"
---

# Advisory File Locking

### From: task

Advisory file locking is a concurrency control mechanism employed throughout the TaskStore implementation to coordinate access among multiple ragent processes sharing a task list. Unlike mandatory locking, advisory locks require cooperating processes to explicitly check and respect locks—there's no kernel enforcement preventing uncoordinated processes from accessing the file. The ragent system uses POSIX-style advisory locks via the fs2 crate, where `lock_exclusive()` creates a lock that other processes attempting `lock_exclusive()` will block on, while `lock_shared()` (not used here) would allow concurrent readers.

The implementation pattern in TaskStore demonstrates proper advisory lock hygiene: locks are always paired with unlock calls, even on error paths (via implicit Drop or explicit unlock), and lock scope is minimized to the actual critical section. The `write_locked()` helper function encapsulates the atomic write pattern—truncate, seek to start, write, flush—ensuring that readers never see partial writes. This addresses the classic problem where a process crashes mid-write, leaving corrupted data. With advisory locking, a crashed process's lock is automatically released by the kernel, allowing recovery by subsequent processes.

Advisory locking contrasts with alternatives like file-based mutexes (creating/deleting lock files) which suffer from race conditions during lock acquisition, and database transactions which would require external infrastructure. The trade-off is that advisory locks only work when all participants cooperate—malicious or buggy processes can ignore locks. For ragent's threat model of cooperative agents, this is acceptable. The locking granularity is at file level (coarse), which simplifies reasoning but may reduce concurrency compared to record-level locking; this is mitigated by the typically small size of task lists and the quick in-memory operations within critical sections.

## External Resources

- [Wikipedia article on file locking mechanisms and advisory vs mandatory locks](https://en.wikipedia.org/wiki/File_locking) - Wikipedia article on file locking mechanisms and advisory vs mandatory locks
- [Avery Pennarun's analysis of file locking challenges and edge cases](https://apenwarr.ca/log/20101213) - Avery Pennarun's analysis of file locking challenges and edge cases

## Sources

- [task](../sources/task.md)
