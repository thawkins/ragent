---
title: "Pluralization Logic"
type: concept
generated: "2026-04-19T16:38:35.796817907+00:00"
---

# Pluralization Logic

### From: format

The module implements consistent pluralization handling across five counting utilities: `format_line_count`, `format_file_count`, `format_match_count`, `format_entry_count`, and within `format_edit_summary`. Each follows the same fundamental pattern: checking for the singular case (count == 1) and returning the singular noun form, otherwise appending 's' for plural. This simple English-centric approach suffices for the crate's scope while remaining extensible through the shared pattern structure.

The `format_edit_summary` function elevates pluralization to a more sophisticated level with four distinct branches handling the matrix of old_lines/new_lines combinations. When both values equal 1, it produces the concise "replaced 1 line"; when they differ, it explicitly shows the transformation with "replaced X lines with Y lines" or variants preserving singular forms. This contextual pluralization—where the same numeric value might produce "line" or "lines" depending on sentence position—demonstrates how formatting logic must adapt to linguistic context.

Edge case handling reveals careful design: zero consistently pluralizes as "0 lines" following English conventions where zero is grammatically plural. The test suite explicitly validates these edge cases, including the zero case often overlooked in naive implementations. While the current implementation handles English only, the centralized pattern suggests future internationalization could replace these functions with locale-aware equivalents. The type system enforces correctness through `usize` parameters, preventing negative counts that would complicate pluralization logic.

## External Resources

- [ICU MessageFormat for internationalized pluralization](https://unicode-org.github.io/icu/userguide/format_parse/messages/) - ICU MessageFormat for internationalized pluralization
- [Unicode CLDR plural rules specification](https://github.com/unicode-org/cldr/blob/main/docs/ldml/tr35-numbers.md) - Unicode CLDR plural rules specification

## Related

- [Content Format Patterns](content-format-patterns.md)
- [Human-Readable Formatting](human-readable-formatting.md)

## Sources

- [format](../sources/format.md)
