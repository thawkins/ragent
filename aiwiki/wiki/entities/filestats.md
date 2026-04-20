---
title: "FileStats"
entity_type: "technology"
type: entity
generated: "2026-04-19T16:53:10.506943460+00:00"
---

# FileStats

**Type:** technology

### From: multiedit

FileStats is a private struct used during MultiEditTool execution to track detailed statistics about modifications made to each individual file. It maintains three counters: the number of edit operations applied to the file, the total lines added across all those edits, and the total lines removed. This granularity enables the tool to provide both aggregate statistics and per-file breakdowns in its output, giving users clear visibility into exactly what changed and where.

The struct is employed in Phase 2 of the execution process, where edits are applied to in-memory file contents. As each edit is successfully validated and applied, the corresponding FileStats entry for that file is updated. The lines added and removed are computed by counting newline characters in the old and new strings, providing a meaningful metric of code change volume that correlates with what developers and code review tools typically measure. This approach handles multi-line replacements correctly, accounting for the actual visual impact of changes in source code.

FileStats entries are stored in a HashMap keyed by file path, allowing O(1) lookup when updating statistics for each edit. The struct's fields are simple usize values, making it lightweight and efficient to update frequently. After all edits are applied, the FileStats data is used to build the detailed metadata output, with paths sorted for stable display ordering. This observability feature is particularly valuable in automated workflows where understanding the scope and distribution of changes helps with auditing, rollback decisions, and impact assessment.

## External Resources

- [Rust HashMap for efficient key-value storage](https://doc.rust-lang.org/std/collections/struct.HashMap.html) - Rust HashMap for efficient key-value storage
- [serde_json macro for building JSON output](https://docs.rs/serde_json/latest/serde_json/macro.json.html) - serde_json macro for building JSON output

## Sources

- [multiedit](../sources/multiedit.md)
