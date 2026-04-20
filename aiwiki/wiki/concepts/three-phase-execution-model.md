---
title: "Three-Phase Execution Model"
type: concept
generated: "2026-04-19T16:53:10.509051743+00:00"
---

# Three-Phase Execution Model

### From: multiedit

The three-phase execution model is an architectural pattern employed by MultiEditTool that separates file editing operations into distinct validation, transformation, and persistence phases. This model is fundamental to achieving the tool's atomic guarantees and enables comprehensive error detection before any destructive operations occur. Phase 1 involves reading all target files into memory and validating that every edit operation can be successfully applied. Phase 2 applies the validated edits to the in-memory representations, accumulating changes and tracking statistics. Phase 3 writes only the modified files back to disk, completing the operation.

This phased approach creates natural checkpoints where the operation can be aborted without side effects. If Phase 1 detects any validation failure—such as a missing file, permission error, or non-matching search string—the entire operation terminates before any files are modified. This fail-fast behavior is essential for automated workflows where detecting errors early prevents cascading failures. The separation of validation from execution also enables better error messages, as the context of the original file content is available to explain why an edit failed, rather than encountering errors during partial modification of already-changed content.

Phase 2's in-memory transformation enables sophisticated edit ordering and composition. When multiple edits target the same file, they apply sequentially to the evolving content, with each edit seeing the results of previous edits to that file. This creates intuitive behavior where a series of edits builds upon each other. The phase also collects detailed statistics by comparing line counts before and after each replacement, providing the observability data needed for the comprehensive output reporting. Keeping all modifications in memory during this phase simplifies rollback—if any error occurs, the HashMap entries can simply be discarded without touching the filesystem.

Phase 3's batch writing ensures that all changes become visible simultaneously from the perspective of other processes (subject to operating system caching semantics). The writes occur after all locks are held and all transformations are complete, minimizing the critical section duration. The implementation filters to write only files that actually have modifications, avoiding unnecessary I/O for files that were read but not changed. This optimization, tracked via the file_stats HashMap, is particularly valuable when editing operations specify files redundantly or when optimization eliminates no-op edits. The three-phase model demonstrates how careful separation of concerns enables both safety and efficiency in file manipulation tools.

## External Resources

- [ETL pattern with similar phase separation](https://en.wikipedia.org/wiki/Extract,_transform,_load) - ETL pattern with similar phase separation
- [Rust HashMap for in-memory data management](https://doc.rust-lang.org/std/collections/struct.HashMap.html) - Rust HashMap for in-memory data management

## Related

- [Atomic File Operations](atomic-file-operations.md)

## Sources

- [multiedit](../sources/multiedit.md)
