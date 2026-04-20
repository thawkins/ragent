---
title: "Surgical Text Replacement Tool for LLM-Based File Editing"
source: "edit"
type: source
tags: [rust, text-processing, file-editing, llm-integration, fuzzy-matching, code-modification, string-algorithms, tool-system]
generated: "2026-04-19T16:58:10.234298439+00:00"
---

# Surgical Text Replacement Tool for LLM-Based File Editing

This document presents a sophisticated Rust implementation of the `EditTool`, a surgical text replacement system designed specifically for LLM-based code editing workflows. The tool implements a multi-pass fuzzy matching algorithm that addresses common discrepancies between how large language models perceive code and how it actually exists in files. The core challenge it solves is that LLMs often generate search strings that don't exactly match file contents due to line ending differences (CRLF vs LF), trailing whitespace that gets silently omitted, or leading indentation that gets stripped when LLMs read line-numbered displays. Rather than requiring perfect matches, the tool progressively relaxes matching criteria through five distinct passes: exact matching, CRLF normalization, trailing whitespace stripping, leading whitespace stripping with automatic re-indentation, and finally collapsed whitespace matching for handling tabs-versus-spaces and irregular spacing.

The architecture demonstrates careful attention to safety and precision. Each matching pass maintains uniqueness guarantees—if multiple matches would occur at any relaxation level, the operation aborts with an error rather than risk ambiguous replacements. The tool integrates with a broader permission system (declaring "file:write" category) and uses file locking to serialize concurrent edits. The implementation includes sophisticated byte-offset mapping to translate between normalized search spaces and actual file positions, preserving the original file's line ending style and structure. The comprehensive test suite validates behavior across edge cases including mixed line endings, varying indentation styles, and complex whitespace scenarios that commonly arise in real-world development environments.

## Related

### Entities

- [EditTool](../entities/edittool.md) — technology
- [find_replacement_range](../entities/find-replacement-range.md) — technology
- [strip_cr](../entities/strip-cr.md) — technology
- [reindent_with](../entities/reindent-with.md) — technology

### Concepts

- [Progressive Relaxation Matching](../concepts/progressive-relaxation-matching.md)
- [Byte-Accurate Text Replacement](../concepts/byte-accurate-text-replacement.md)
- [LLM-Aligned Tool Design](../concepts/llm-aligned-tool-design.md)
- [Defensive Concurrency Control](../concepts/defensive-concurrency-control.md)

