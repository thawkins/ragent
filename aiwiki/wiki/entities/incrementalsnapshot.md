---
title: "IncrementalSnapshot"
entity_type: "product"
type: entity
generated: "2026-04-19T16:01:24.270435588+00:00"
---

# IncrementalSnapshot

**Type:** product

### From: mod

The `IncrementalSnapshot` struct represents a sophisticated optimization in the ragent snapshot system, implementing delta encoding to dramatically reduce memory footprint for tracking file changes across agent session messages. Unlike its full-capture counterpart, this structure stores only the differences relative to a base `Snapshot`, making it feasible to maintain fine-grained versioning history without prohibitive storage costs. The struct contains seven fields: standard identification fields (`id`, `session_id`, `message_id`, `created_at`) parallel to `Snapshot`, a critical `base_id` referencing the parent snapshot, plus three specialized collections—`diffs` for unified diff strings of modified text files, `added` for full content of new files (including binary changes), and `deleted` as a vector of removed file paths.

The architectural elegance of `IncrementalSnapshot` lies in its tiered storage strategy that adapts to file content characteristics. For text files with modifications, it leverages the space-efficient unified diff format through the `similar` crate's advanced diffing algorithms, capturing only changed lines rather than complete files. Binary files and entirely new additions bypass diff generation entirely—their full content is stored in the `added` map, acknowledging that binary deltas would require specialized algorithms beyond the module's scope. Deleted files are simply tracked by path, requiring minimal storage while enabling complete reconstruction of file system state.

The `to_full` method embodies the reconstruction capability that makes incremental storage viable, applying stored diffs against base content through a carefully sequenced process: first removing deleted entries, then patching modified files using custom diff application logic, and finally inserting new additions. This method handles edge cases including empty diffs (indicating binary files carried forward unchanged) and missing base files with appropriate error propagation. The implementation demonstrates sophisticated string processing with UTF-8 conversion and lossy handling, ensuring robustness across the boundary between raw bytes and text operations. By supporting round-trip conversion between incremental and full representations, the module provides flexibility for storage optimization while maintaining the simplicity of full snapshots for active operations.

## Diagram

```mermaid
classDiagram
    class IncrementalSnapshot {
        +String id
        +String base_id
        +String session_id
        +String message_id
        +HashMap~PathBuf, String~ diffs
        +HashMap~PathBuf, Vec~u8~~ added
        +Vec~PathBuf~ deleted
        +DateTime~Utc~ created_at
        +to_full(base: Snapshot) Result~Snapshot~
    }
    IncrementalSnapshot ..> Snapshot : reconstructs to
    IncrementalSnapshot ..> "similar" : uses for diffs
```

## External Resources

- [Similar crate for text diffing and comparison](https://docs.rs/similar/latest/similar/) - Similar crate for text diffing and comparison
- [Unified diff format specification on Wikipedia](https://en.wikipedia.org/wiki/Diff#Unified_format) - Unified diff format specification on Wikipedia
- [Anyhow error handling for Rust](https://docs.rs/anyhow/latest/anyhow/) - Anyhow error handling for Rust

## Sources

- [mod](../sources/mod.md)
