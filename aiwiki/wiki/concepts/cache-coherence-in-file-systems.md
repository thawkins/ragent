---
title: "Cache Coherence in File Systems"
type: concept
generated: "2026-04-19T20:09:27.053913416+00:00"
---

# Cache Coherence in File Systems

### From: read

Cache coherence in file systems addresses the fundamental challenge of maintaining consistency between cached data and the underlying storage when multiple processes or components may modify files. ReadTool's implementation demonstrates a specific solution tailored to single-process agent environments: using file modification timestamps (mtime) as cache invalidation triggers. The `CacheKey` struct's design as `(PathBuf, SystemTime)` embodies this approach, where any change to a file's metadata timestamp—typically updated on write operations—automatically invalidates previous cache entries. This ensures that agents always receive current file contents even when tools within the same process or external processes modify files during agent execution.

The trade-offs in this coherence strategy reflect real-world constraints and usage patterns. Timestamp-based invalidation is efficient and portable across operating systems, requiring only a metadata check rather than content hashing or distributed coordination protocols. However, it has well-known limitations: timestamps may not update on certain filesystems with subsecond precision disabled, network filesystems may exhibit clock skew, and some advanced scenarios like hardlinked files can result in shared timestamps across logically distinct paths. The implementation mitigates these risks through conservative defaults—returning `UNIX_EPOCH` when metadata is unavailable ensures cache misses rather than potentially stale hits, and the use of absolute paths prevents aliasing issues from relative path navigation. For the intended use case of AI agent file access, where files are typically modified through well-behaved tools and coherence requirements are session-scoped, these trade-offs are acceptable.

The cache's process-wide scope through `OnceLock` initialization creates a coherence boundary at the process level. This means that file modifications by external processes are detected on next access (through mtime changes), but modifications by other threads or async tasks within the same process are immediately visible to subsequent cache lookups. The `Arc<String>` sharing model ensures that in-flight operations using cached data are not affected by subsequent invalidations—each holder of an `Arc` retains valid access to the snapshot they received. This snapshot semantics approach is appropriate for agent workflows where file contents are typically consumed immediately and not held across suspension points where modifications might occur. The 256-entry capacity limit provides an additional coherence mechanism through LRU eviction, ensuring that very old entries are naturally phased out even if their timestamps somehow fail to reflect underlying changes.

## External Resources

- [File system cache coherence and mtime reliability discussion](https://apenwarr.ca/log/20181113) - File system cache coherence and mtime reliability discussion
- [Rust std::fs::Metadata documentation including modified() method](https://doc.rust-lang.org/std/fs/struct.Metadata.html) - Rust std::fs::Metadata documentation including modified() method

## Sources

- [read](../sources/read.md)
