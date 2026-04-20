---
title: "reindent_with"
entity_type: "technology"
type: entity
generated: "2026-04-19T16:58:10.235900127+00:00"
---

# reindent_with

**Type:** technology

### From: edit

reindent_with is a sophisticated utility function that prepends a specified indentation string to every line of input text, with careful handling of trailing newlines. The function is essential for Pass 4 and Pass 5 matching strategies, where the search successfully matched content with stripped leading whitespace and now needs to apply the original file's indentation to the replacement text. This preserves both the semantic intent of the LLM's edit and the cosmetic formatting conventions of the existing codebase.

The implementation demonstrates attention to edge cases that simpler approaches would mishandle. The function processes lines through the lines() iterator, which handles all Unicode line ending conventions, then maps each line through format! macro to prepend the indentation. The join operation reconstructs with '\n' newlines, matching Rust's convention of normalizing line endings during iteration. Critically, the function then checks if the original string ended with a newline and appends one if so—preserving the original's trailing newline semantics, which can be significant for version control diffs and POSIX text file conventions.

In practice, reindent_with enables the editing system to handle cases where LLMs read code through interfaces that obscure indentation—such as line-numbered displays where content appears after position markers—and subsequently generate edits without that indentation. Rather than rejecting these edits or applying them incorrectly, the system detects the indentation pattern from the matched content and automatically reformats the replacement to match. This tolerance for common LLM output quirks significantly improves the practical usability of automated editing, reducing the manual correction burden on developers. The function's design reflects empirical observation of actual LLM behavior in production editing workflows.

## External Resources

- [POSIX definition of a text file and line termination conventions](https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap03.html#tag_03_206) - POSIX definition of a text file and line termination conventions

## Sources

- [edit](../sources/edit.md)
