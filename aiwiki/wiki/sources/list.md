---
title: "ListTool Implementation in ragent-core: Directory Tree Listing with File Sizes"
source: "list"
type: source
tags: [rust, filesystem, cli-tool, tree-view, agent-framework, async-trait, serde-json, directory-listing, file-size-formatting, ragent-core]
generated: "2026-04-19T16:51:25.442374079+00:00"
---

# ListTool Implementation in ragent-core: Directory Tree Listing with File Sizes

This document presents the Rust source code for `ListTool`, a utility in the `ragent-core` crate designed to list directory contents in a visually intuitive tree format with file size annotations. The implementation leverages asynchronous traits and JSON schema validation to integrate with a broader agent tooling framework. The tool recursively traverses filesystem hierarchies with configurable depth limits, excludes hidden files and common generated directories (such as `node_modules`, `target`, `.git`, `__pycache__`, `dist`, and `build`), and formats output using UTF-8 box-drawing characters for professional console presentation. The module demonstrates idiomatic Rust patterns including error handling with `anyhow`, serialization with `serde_json`, and careful path manipulation with `std::path` abstractions. The `execute` method processes JSON input to extract path and depth parameters, validates directory existence, and delegates to a recursive helper function that performs the actual filesystem enumeration and formatting. The resulting output includes both human-readable tree text and machine-readable metadata about entry counts and resolved paths, supporting hybrid human-agent consumption patterns.

## Related

### Entities

- [ListTool](../entities/listtool.md) — technology
- [async-trait](../entities/async-trait.md) — technology
- [anyhow](../entities/anyhow.md) — technology
- [serde_json](../entities/serde-json.md) — technology
- [David Tolnay](../entities/david-tolnay.md) — person

### Concepts

- [Tree Visualization in CLI Tools](../concepts/tree-visualization-in-cli-tools.md)
- [Agent Tool Framework Architecture](../concepts/agent-tool-framework-architecture.md)
- [Filesystem Traversal with Filtering](../concepts/filesystem-traversal-with-filtering.md)
- [Human-Readable Size Formatting](../concepts/human-readable-size-formatting.md)

