---
title: "Unified Diff Format"
type: concept
generated: "2026-04-19T16:01:24.271462318+00:00"
---

# Unified Diff Format

### From: mod

The unified diff format is a standardized textual representation for describing differences between files, serving as the backbone of text change storage in the ragent snapshot module. Developed as an enhancement to the original context diff format by Keith Bostic for the Berkeley Software Distribution in the early 1980s, unified diff has become the de facto standard for patch distribution and code review, recognized by tools from Git to GitHub's pull request interfaces. The format's elegance lies in its human readability combined with unambiguous machine parseability, using minimal metadata to describe complex transformations.

A unified diff consists of hunk headers specifying line ranges and context counts, followed by change lines prefixed with space (context/unchanged), minus sign (removed), or plus sign (added). The ragent module's `make_unified_diff` function generates this format using the `similar` crate's `grouped_ops(3)` configuration, which groups changes with 3 lines of surrounding context—matching the default behavior of GNU diff. This context serves dual purposes: human reviewers can understand change surroundings, and patch application can tolerate minor base file drift by matching context lines. The module's output streamlines this further by omitting explicit hunk headers, generating a simplified format sufficient for internal reconstruction while remaining compatible with standard tools.

The companion `apply_unified_diff` function implements a simplified patch application algorithm that processes this format line-by-line. The implementation recognizes three fundamental operations: context lines advance both output and input pointers while preserving base content; deletion lines advance only the input pointer, effectively dropping content; and insertion lines advance only the output pointer with new content. This state machine approach handles the common case efficiently while accepting limitations—specifically, the absence of hunk header interpretation means the implementation cannot handle overlapping changes or precise line number targeting, instead relying on sequential processing. The fast-path for empty diffs and the handling of remaining base lines after diff exhaustion demonstrate defensive programming against edge cases in real-world file content.

## External Resources

- [Artima explanation of unified diff format](https://www.artima.com/weblogs/viewpost.jsp?thread=164293) - Artima explanation of unified diff format
- [Git documentation on diff formats](https://git-scm.com/docs/diff-format) - Git documentation on diff formats
- [POSIX diff utility specification](https://pubs.opengroup.org/onlinepubs/9699919799/utilities/diff.html) - POSIX diff utility specification

## Related

- [Incremental Delta Encoding](incremental-delta-encoding.md)

## Sources

- [mod](../sources/mod.md)
