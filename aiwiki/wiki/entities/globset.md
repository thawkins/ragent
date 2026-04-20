---
title: "globset"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:24:51.864085911+00:00"
---

# globset

**Type:** technology

### From: mod

The globset crate is a high-performance Rust library for glob pattern matching that serves as a critical dependency for the ragent-core permission system. It provides optimized algorithms for matching file paths against wildcard patterns, supporting both standard glob syntax and set-based operations for matching against multiple patterns simultaneously. Within the permission module, globset is used to compile pattern strings into `GlobMatcher` instances that can be efficiently evaluated against resource paths. The crate's design emphasizes performance through compiled matchers and fast rejection heuristics, making it suitable for security-critical paths that may be evaluated frequently during agent operation. The `PermissionChecker` stores compiled matchers in its `always_grants` HashMap, avoiding recompilation overhead for frequently-checked patterns. Globset's pattern syntax supports features like `*` for any sequence of characters, `?` for single character matching, `[...]` for character classes, and `**` for recursive directory traversal, providing sufficient expressiveness for complex resource access policies. This dependency choice reflects the module's focus on both security correctness and runtime performance, essential characteristics for AI agent systems operating at scale.

## External Resources

- [globset crate on crates.io](https://crates.io/crates/globset) - globset crate on crates.io
- [globset API documentation](https://docs.rs/globset/) - globset API documentation

## Sources

- [mod](../sources/mod.md)
