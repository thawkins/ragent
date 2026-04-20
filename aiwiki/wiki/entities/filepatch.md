---
title: "FilePatch"
entity_type: "technology"
type: entity
generated: "2026-04-19T16:22:22.985330996+00:00"
---

# FilePatch

**Type:** technology

### From: patch

FilePatch is a private struct that represents the intermediate parsing result for a single file's worth of changes within a larger unified diff. It serves as the container that bridges the gap between raw text parsing and hunk-level application, capturing the essential metadata needed to locate and modify a target file. The struct's design reflects the hierarchical nature of unified diff format, where a single diff may contain changes to multiple files, each with its own path and collection of hunks. By maintaining this structure as a distinct type rather than operating directly on raw strings, the implementation enables sophisticated operations like path resolution override and multi-file transaction management.

The path field stores the target file path as parsed from the +++ line of the diff header, with normalization already applied to handle git's a/ and b/ prefixes as well as timestamp suffixes from traditional Unix diff output. This normalization is crucial for interoperability, as different tools produce slightly different header formats—the struct abstracts these variations away from downstream consumers. The hunks vector maintains ownership of all Hunk instances for this file, and their ordering within the vector preserves the original diff's sequence, which matters for both human readability and for the reverse-order application optimization performed during the execution phase.

FilePatch instances are constructed during the parse_unified_diff function's state machine execution, which scans for ---/+++ header pairs and accumulates hunks until the next file boundary or end of input is reached. The Debug derive implementation enables structured logging and diagnostics, which is particularly valuable when patches fail to apply and developers need to inspect the parsed representation to understand how the parser interpreted the diff text. The struct's privacy (private to the module) reflects its role as an implementation detail rather than a public API, allowing the internal representation to evolve without breaking changes to Tool consumers.

## External Resources

- [GNU diffutils documentation on unified diff format](https://www.gnu.org/software/diffutils/manual/html_node/Unified-Format.html) - GNU diffutils documentation on unified diff format

## Sources

- [patch](../sources/patch.md)
