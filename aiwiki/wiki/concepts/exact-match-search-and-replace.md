---
title: "Exact Match Search and Replace"
type: concept
generated: "2026-04-19T16:53:10.508681503+00:00"
---

# Exact Match Search and Replace

### From: multiedit

Exact match search and replace is a strict text manipulation paradigm that requires search strings to match their target content precisely, without pattern matching, regular expressions, or fuzzy matching. MultiEditTool embodies this philosophy with its requirement that each old_str must appear exactly once in its target file. This constraint eliminates ambiguity in replacement operations and ensures that edits have predictable, reproducible effects. While more flexible search methods might seem convenient, they introduce significant risks in automated editing scenarios where unintended matches could corrupt files silently.

The validation logic enforces this strictness through the find_replacement_range function, which returns specific errors for the two failure modes: NotFound when the string doesn't exist, and MultipleMatches when it appears more than once. This granular error reporting is essential for usability, as the remedies differ significantly—NotFound suggests checking for typos or whether the file already contains the desired change, while MultipleMatches encourages adding surrounding context to disambiguate which occurrence should be replaced. The error messages specifically advise users to 'add more context to make it unique,' guiding them toward the tool's intended usage pattern.

This approach contrasts with more permissive editing tools that might replace all occurrences, use heuristics to choose one match, or apply fuzzy matching. Such approaches, while sometimes convenient for interactive use, become hazardous in automated workflows where the user is not present to verify results. The exact match requirement forces users to be explicit about their intentions, typically by including sufficient surrounding context in the old_str to make it unique. This practice has the beneficial side effect of making edits self-documenting—a well-crafted old_str serves as both search pattern and documentation of what was expected at that location in the file.

The implementation handles multi-line strings correctly, counting lines for statistics and performing byte-level operations for replacements. This enables editing code blocks, function definitions, or any structured text where newlines are significant. The exact match semantics apply to the full string including internal newlines, making it suitable for refactoring operations that need to match and replace entire code constructs. The design prioritizes safety and predictability over convenience, reflecting lessons learned from production systems where overly permissive text manipulation has caused significant incidents.

## External Resources

- [Search and replace concepts in text editing](https://en.wikipedia.org/wiki/Search_and_replace) - Search and replace concepts in text editing
- [Rust regex crate for pattern matching (contrast with exact match)](https://docs.rs/regex/latest/regex/) - Rust regex crate for pattern matching (contrast with exact match)

## Related

- [Atomic File Operations](atomic-file-operations.md)

## Sources

- [multiedit](../sources/multiedit.md)
