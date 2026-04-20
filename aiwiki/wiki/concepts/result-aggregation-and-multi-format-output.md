---
title: "Result Aggregation and Multi-Format Output"
type: concept
generated: "2026-04-19T18:26:34.925374625+00:00"
---

# Result Aggregation and Multi-Format Output

### From: lsp_references

The processing of LSP references results in LspReferencesTool illustrates sophisticated patterns for data transformation that serve multiple consumers simultaneously. The raw LSP response—`Option<Vec<Location>>`—contains rich information about each reference including URI and precise range, but this structure isn't optimal for human readability or for certain programmatic use cases. The implementation performs a multi-stage transformation: filtering empty results, grouping by file for readability, converting 0-based to 1-based coordinates, and generating both formatted text and structured JSON outputs.

The grouping operation using `BTreeMap<String, Vec<(u32, u32)>>` demonstrates intentional data structure selection. A `BTreeMap` provides sorted iteration over file paths, ensuring consistent and predictable output ordering that aids both human review and test assertions. The alternative `HashMap` would provide faster insertion but non-deterministic ordering. The nested structure—files mapping to lists of line-column pairs—enables the hierarchical output format where references are organized under their containing files, dramatically improving readability when a symbol has dozens of usages across many files.

The dual-output design through `ToolOutput { content, metadata }` anticipates different consumption patterns. The `content` string provides immediate human-readable information suitable for chat interfaces or log files, with careful formatting including the reference count header, file indentation, and line:column positioning. The `metadata` JSON preserves structured information including the total count and an array of location objects with file, line, and column fields. This separation enables downstream processing: the metadata can be consumed by other tools, stored in databases, or used to generate IDE-style navigation, while the content serves immediate user communication. The pattern demonstrates API design that doesn't force a choice between human and machine readability but provides both.

## External Resources

- [Rust BTreeMap documentation](https://doc.rust-lang.org/std/collections/struct.BTreeMap.html) - Rust BTreeMap documentation

## Sources

- [lsp_references](../sources/lsp-references.md)
