---
title: "Ragent-Core Reference Parser: @-Mention Detection and Classification"
source: "parse"
type: source
tags: [rust, parser, text-processing, agent-systems, file-references, context-management, lexer, ragent-core]
generated: "2026-04-19T20:32:08.268981856+00:00"
---

# Ragent-Core Reference Parser: @-Mention Detection and Classification

This document presents the `parse.rs` source file from the `ragent-core` crate, which implements a specialized parser for detecting and classifying `@` references in user input text. The module serves as a critical component in an agent-based system, enabling users to reference files, directories, URLs, and fuzzy-named entities within natural language prompts. The parser employs a state-machine approach using byte-level iteration to identify `@` symbols while avoiding false positives from email addresses and other contexts where `@` appears mid-word.

The implementation defines a clear taxonomy of reference types through the `FileRef` enum, distinguishing between concrete file paths, directory references, web URLs, and fuzzy names that require project-wide matching. The `ParsedRef` struct captures not only the classified reference but also its exact byte position (span) within the original input, enabling precise text manipulation and replacement operations. The classification logic follows hierarchical rules: URLs are detected by their scheme prefixes, directories by trailing slashes, file paths by separators or extensions, with remaining entries falling through to fuzzy matching.

Comprehensive test coverage validates the parser's behavior across edge cases including email address rejection, multiple adjacent references, relative paths, dotfiles, and span correctness verification. The module demonstrates idiomatic Rust patterns including exhaustive pattern matching, zero-copy parsing where possible, and defensive programming against malformed input. This parser likely feeds into a larger context assembly system where detected references are resolved to actual file contents or metadata for language model consumption.

## Related

### Entities

- [ParsedRef](../entities/parsedref.md) — technology
- [FileRef](../entities/fileref.md) — technology
- [parse_refs Function](../entities/parse-refs-function.md) — technology
- [classify_ref Function](../entities/classify-ref-function.md) — technology
- [ragent-core](../entities/ragent-core.md) — product

