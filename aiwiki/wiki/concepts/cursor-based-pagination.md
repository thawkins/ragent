---
title: "Cursor-Based Pagination"
type: concept
generated: "2026-04-19T18:51:38.684047158+00:00"
---

# Cursor-Based Pagination

### From: pdf_write

Cursor-based pagination is a layout technique where a virtual cursor tracks position through content stream, triggering page breaks when content would exceed available space. This approach, implemented in pdf_write.rs through the `Cursor` struct, contrasts with pre-pagination (calculating all breaks before rendering) and constraint-solving (optimizing breaks globally). The technique originates in early text processing systems with limited memory, processing documents as streams rather than random-access structures. Its simplicity makes it robust and debuggable—position state is explicit, break decisions are local, and behavior is reproducible. The primary limitation is suboptimal breaks: without look-ahead, the algorithm cannot avoid widows (single lines at page top) or orphans (single lines at page bottom) without additional complexity.

The implementation demonstrates sophisticated handling despite simple foundations. Y-coordinate tracking uses millimeters for intuitive constants, converted to PDF's native points (1/72 inch) through the printpdf abstraction. The `needs_new_page` predicate includes safety margin logic—requesting space not just for content height but for subsequent spacing, preventing elements from crowding margins. Multi-page documents emerge from the `flush_page` closure capturing pages vector and cursor, appending completed pages while resetting cursor state for continuation. This pattern of stateful iteration with periodic reset appears in streaming parsers, network protocols, and other sequential processing domains.

Modern alternatives include TeX's box-and-glue model with global optimization, browser engines' layout trees with constraint solving, and machine learning approaches trained on professional typesetting examples. However, for agent-generated content where speed and predictability outweigh typographic perfection, cursor-based methods remain appropriate. The technique's extensibility is demonstrated by image handling: when scaled images exceed remaining page space, the algorithm can either force page break or rescale to fit, with the implementation choosing the latter for user experience.

## External Resources

- [CSS Paged Media Module Level 3](https://www.w3.org/TR/css-page-3/) - CSS Paged Media Module Level 3
- [LuaTeX: the engine - modern TeX pagination approach](https://www.tug.org/TUGboat/tb35-3/tb111thanh.pdf) - LuaTeX: the engine - modern TeX pagination approach

## Sources

- [pdf_write](../sources/pdf-write.md)
