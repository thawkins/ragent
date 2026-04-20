---
title: "Markdown-like Inline Formatting"
type: concept
generated: "2026-04-19T18:45:40.558614620+00:00"
---

# Markdown-like Inline Formatting

### From: office_write

The OfficeWriteTool implements a custom parser for markdown-like inline formatting, enabling rich text styling within Word documents without requiring full markdown processing. This lightweight parser, implemented in the `parse_inline_formatting` function, recognizes three formatting constructs: double asterisks `**` for bold text, single asterisks `*` for italic text, and backticks `` ` `` for code-formatted text. The parser operates as a character-by-character state machine, building segments of text with associated formatting flags. When it encounters opening markers, it finalizes the current plain segment and begins accumulating text for the formatted segment until the closing marker is found. The function returns a vector of tuples containing the text segment and three boolean flags indicating bold, italic, and code formatting respectively. This approach integrates seamlessly with the docx-rust library's `CharacterProperty` builder, where each segment can have its formatting properties set individually before being pushed as a run within a paragraph. The parser handles edge cases such as empty formatted segments and ensures that any remaining text after the last marker is captured. This design choice reflects a pragmatic balance—providing users with familiar formatting syntax while avoiding the complexity of a full markdown parser, which would require handling block-level elements, links, images, and other constructs not relevant to inline document styling.

## External Resources

- [CommonMark specification for markdown](https://commonmark.org/) - CommonMark specification for markdown
- [Markdown basic syntax guide](https://www.markdownguide.org/basic-syntax/) - Markdown basic syntax guide

## Sources

- [office_write](../sources/office-write.md)
