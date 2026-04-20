---
title: "Hunk"
entity_type: "technology"
type: entity
generated: "2026-04-19T16:22:22.985676277+00:00"
---

# Hunk

**Type:** technology

### From: patch

Hunk represents the fundamental unit of change within unified diff format, capturing a contiguous range of modifications that can be applied as an atomic operation. The struct encapsulates the complex state required to match against existing file content and produce modified output, including the starting line number for matching (old_start) and parallel vectors representing how lines are classified in both the old and new file contexts. This dual representation is necessary because unified diff interleaves context, removal, and addition lines, and the matching algorithm needs to know which lines must exist in the original file versus which should appear in the result.

The old_start field uses 1-based indexing as specified by the unified diff format specification, requiring careful conversion to 0-based indexing when interacting with Rust's Vec operations. The old_lines and new_lines vectors maintain parallel structure through the HunkLine enum, with Context variants appearing in both, Remove variants only in old_lines, and Add variants only in new_lines. This design enables the apply_hunk function to construct search patterns from old_lines and replacement content from new_lines through simple filtering operations. The struct's relatively small size makes it efficient to clone and manipulate during the fuzzy matching process, where multiple match attempts may be made with different context trimmings.

The Hunk type plays a critical role in the tool's safety guarantees: each hunk must match exactly (within fuzz tolerance) before any modifications are written, and hunks are sorted by old_start in descending order before application to prevent line number shifting from invalidating subsequent hunks. This sorting operation consumes the original ordering but preserves the essential dependency relationships between overlapping modifications. The Debug derive enables detailed error messages that include hunk line counts and classifications, significantly improving the debugging experience when patches fail to apply due to content drift or malformed input.

## External Resources

- [Wikipedia explanation of unified diff hunk format](https://en.wikipedia.org/wiki/Diff#Unified_format) - Wikipedia explanation of unified diff hunk format

## Sources

- [patch](../sources/patch.md)
