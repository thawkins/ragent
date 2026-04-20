---
title: "Coordinate System Translation"
type: concept
generated: "2026-04-19T18:19:14.087475997+00:00"
---

# Coordinate System Translation

### From: lsp_definition

Coordinate system translation is a critical implementation detail in text editor and protocol integrations, addressing the fundamental mismatch between user-facing conventions and protocol requirements. Human-readable text editing interfaces universally use 1-based indexing for line and column numbers—lines start at 1, and the first character on a line is column 1. However, the Language Server Protocol and most programming APIs use 0-based indexing, where the first line is line 0 and the first character is character 0. This implementation must perform careful bidirectional translation to present familiar numbers to users while communicating correctly with LSP servers.

The translation occurs at multiple points in the execution flow. Input parameters from users arrive as 1-based coordinates, which the tool converts to 0-based before constructing LSP `Position` structs using `saturating_sub(1)`. This defensive subtraction prevents underflow on invalid zero inputs, though such inputs would indicate user error. After receiving LSP responses with 0-based locations, the tool adds 1 to convert back to 1-based for user display in the formatted output string and JSON metadata. This consistent translation ensures users see conventional line numbers while the LSP protocol receives correct indices.

The column/character coordinate requires additional consideration because its interpretation depends on encoding. LSP specifies positions in UTF-16 code units by default, meaning multi-byte characters may occupy multiple positions. Rust's string handling uses UTF-8, creating potential mismatches. The `lsp_types::Position` uses `u32` for both line and character, with the character representing the offset in UTF-16 code units. Language servers are responsible for handling encoding conversions, but client implementations must be aware of these semantics. This implementation delegates to the LSP client for document handling, which manages the encoding complexities internally.

## External Resources

- [LSP Position specification with encoding notes](https://microsoft.github.io/language-server-protocol/specifications/specification-current/#position) - LSP Position specification with encoding notes

## Sources

- [lsp_definition](../sources/lsp-definition.md)
