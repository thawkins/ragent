---
title: "HunkLine"
entity_type: "technology"
type: entity
generated: "2026-04-19T16:22:22.985970225+00:00"
---

# HunkLine

**Type:** technology

### From: patch

HunkLine is a private enum that classifies individual lines within a hunk according to their role in the diff operation, providing type-safe distinction between context lines that must match exactly, lines to be removed from the original file, and lines to be added to the result. Each variant carries a String payload containing the actual line content (without the diff prefix character), enabling direct comparison and collection operations. The enum's design reflects the three fundamental line types in unified diff syntax: space-prefixed context lines, minus-prefixed removal lines, and plus-prefixed addition lines, with a fourth case of backslash-prefixed metadata lines that are explicitly ignored.

The Context variant represents lines present in both old and new file versions, serving as the anchor points that enable the matching algorithm to locate where a hunk should be applied even when line numbers have shifted. Context lines are the basis for the fuzz tolerance feature, which progressively trims context from hunks when exact matches fail, allowing patches to apply successfully after minor edits to surrounding code. The Remove and Add variants represent the actual modifications, with Remove indicating content that must exist in the original file and will be deleted, and Add indicating new content that will appear in the modified file. The structural separation of these concerns prevents logic errors that could arise from string prefix manipulation, such as accidentally treating a removed line as context due to off-by-one slicing errors.

HunkLine implements Clone to support the parallel old_lines and new_lines construction in parse_hunk, where a single source line may need to appear in both vectors with different classifications. This occurs for context lines (which appear identically in both) and for certain edge cases in the parser's handling of bare lines that lack leading whitespace. The Debug derive enables detailed inspection of parsed hunks during development and troubleshooting, with each variant clearly labeled in debug output. The enum's privacy ensures that its representation can be modified without affecting the public Tool API, while its comprehensiveness (handling all possible line prefixes including the bare-line fallback) demonstrates the robustness required for production-grade diff parsing.

## External Resources

- [Git documentation on diff format and line prefixes](https://git-scm.com/docs/diff-format) - Git documentation on diff format and line prefixes

## Sources

- [patch](../sources/patch.md)
