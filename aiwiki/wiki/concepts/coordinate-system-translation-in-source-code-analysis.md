---
title: "Coordinate System Translation in Source Code Analysis"
type: concept
generated: "2026-04-19T18:26:34.924413110+00:00"
---

# Coordinate System Translation in Source Code Analysis

### From: lsp_references

The translation between different coordinate systems in source code analysis represents a subtle but critical concern that affects correctness and user experience across developer tools. Human-oriented interfaces universally adopt 1-based indexing for line and column numbers—editors display line 1 as the first line, and users intuitively refer to "line 5, column 10." In contrast, most programmatic APIs, including the Language Server Protocol, use 0-based indexing where the first line is line 0. LspReferencesTool demonstrates the importance of handling this translation carefully, using `saturating_sub(1)` to convert input parameters and adding 1 back to LSP responses for output formatting.

The choice of saturating arithmetic rather than unchecked subtraction reveals defensive programming against edge cases. If a user somehow provided line 0 (perhaps through a direct API call bypassing validation), regular subtraction would cause an underflow in unsigned integer types, potentially resulting in `u32::MAX` and causing confusing failures downstream. `saturating_sub` ensures the minimum value is 0, which while still potentially incorrect, won't cause catastrophic behavior. This pattern appears twice in the implementation: once for line numbers and once for character positions. The symmetry suggests intentional design rather than coincidence.

Beyond the 1-based versus 0-based distinction, coordinate systems in code analysis must handle Unicode correctly. LSP's `character` field measures UTF-16 code units, not Unicode code points or visual columns. This distinction matters for text containing characters outside the Basic Multilingual Plane (emojis, some CJK characters, mathematical symbols), where a single code point occupies two UTF-16 code units. LspReferencesTool passes these values through transparently, relying on the LSP client and server to handle encoding correctly. However, the display logic that formats "line {l}:{c}" may produce unexpected results if users expect visual column positions. This represents an inherent complexity in text editor protocols that tool builders must navigate carefully.

## External Resources

- [LSP Position specification with UTF-16 encoding notes](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#position) - LSP Position specification with UTF-16 encoding notes
- [Unicode FAQ on UTF-8, UTF-16, and UTF-32](https://www.unicode.org/faq/utf_bom.html) - Unicode FAQ on UTF-8, UTF-16, and UTF-32

## Related

- [defensive programming](defensive-programming.md)

## Sources

- [lsp_references](../sources/lsp-references.md)
