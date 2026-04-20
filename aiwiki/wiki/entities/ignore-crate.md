---
title: "ignore crate"
entity_type: "technology"
type: entity
generated: "2026-04-19T18:55:31.955234998+00:00"
---

# ignore crate

**Type:** technology

### From: search

The ignore crate is a Rust library that provides fast, recursive, glob-enabled file system traversal with built-in support for respecting various ignore patterns. Developed by Andrew Gallant as part of the ripgrep ecosystem, it serves as the foundation for gitignore-aware file walking in tools that need to process project directories while excluding irrelevant files. The crate implements efficient matching against .gitignore, .ignore, .git/info/exclude, and global git ignore files, using optimized data structures that can handle large directory trees without excessive memory consumption.

In SearchTool, the ignore crate appears in two critical capacities: WalkBuilder for directory traversal and OverrideBuilder for custom glob-based filtering. WalkBuilder provides a fluent API for configuring traversal behavior including hidden file handling (disabled by default), gitignore respect (enabled by default), and symbolic link following. The builder pattern enables clean configuration of complex traversal policies without verbose constructor arguments. SearchTool specifically enables hidden file exclusion and all three gitignore sources, ensuring that searches behave consistently with developer expectations from command-line tools.

The OverrideBuilder interface allows dynamic addition of include patterns that restrict which files are searched. This differs from gitignore patterns in that overrides specify what to include rather than exclude, with non-matching files being skipped. The crate's glob syntax supports standard shell patterns including * for wildcards, ? for single characters, and ** for recursive directory matching. Error handling in the override building process catches invalid glob patterns early, providing descriptive error messages through anyhow's context system. The crate's performance is achieved through parallel directory traversal using thread pools and efficient pattern matching algorithms that minimize per-file overhead.

## External Resources

- [ignore crate documentation with examples and API reference](https://docs.rs/ignore/latest/ignore/) - ignore crate documentation with examples and API reference
- [WalkBuilder struct documentation for directory traversal](https://docs.rs/ignore/latest/ignore/struct.WalkBuilder.html) - WalkBuilder struct documentation for directory traversal
- [OverrideBuilder for custom include pattern filtering](https://docs.rs/ignore/latest/ignore/overrides/struct.OverrideBuilder.html) - OverrideBuilder for custom include pattern filtering

## Sources

- [search](../sources/search.md)
