---
title: "Byte-Accurate Text Replacement"
type: concept
generated: "2026-04-19T16:58:10.236643467+00:00"
---

# Byte-Accurate Text Replacement

### From: edit

Byte-accurate text replacement is a critical requirement for file editing systems that must preserve unspecified file characteristics while modifying specific content. Unlike line-based or token-based replacement, byte-accurate systems maintain exact control over file encoding, line ending style, and byte positioning. The EditTool implementation achieves this through careful mapping between normalized search spaces and original byte positions. When CRLF normalization or whitespace stripping creates a simplified search space where matches are found, the system must translate those positions back to the original file's coordinate system before performing replacement.

This precision is essential for several practical reasons. Version control systems track file content at the byte level, and replacements that inadvertently change line endings or encoding can create noisy diffs that obscure the semantic change. Files with mixed line endings (common in long-lived projects with heterogeneous contributor environments) must have their existing conventions respected. Character encodings with variable byte widths, particularly UTF-8 with its multi-byte sequences, require byte-accurate indexing rather than character counting to maintain file integrity.

The implementation demonstrates this through functions like norm_to_orig_byte, which walks the original string tracking both normalized and original positions, and byte_offset_of_line, which converts line indices to byte offsets. These utilities enable the higher-level matching logic to work with convenient normalized representations while preserving byte-level precision for the actual file operation. This separation of concerns—semantic matching versus physical replacement—is a hallmark of well-designed text processing systems. The pattern appears in sophisticated editors, patch systems (like unified diff), and refactoring tools where preserving file structure is as important as making correct changes.

## External Resources

- [UTF-8 encoding details for variable-width character handling](https://en.wikipedia.org/wiki/UTF-8) - UTF-8 encoding details for variable-width character handling
- [Unified diff format specification with byte offset handling](https://www.gnu.org/software/diffutils/manual/html_node/Detailed-Unified.html) - Unified diff format specification with byte offset handling

## Sources

- [edit](../sources/edit.md)
