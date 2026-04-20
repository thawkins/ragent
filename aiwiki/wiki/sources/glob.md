---
title: "GlobTool: Parallel Glob Pattern File Discovery for Rust AI Agents"
source: "glob"
type: source
tags: [rust, file-system, glob-patterns, parallel-computing, ai-agents, tool-system, rayon, async-trait, serde-json, directory-walking]
generated: "2026-04-19T16:44:43.006569765+00:00"
---

# GlobTool: Parallel Glob Pattern File Discovery for Rust AI Agents

This document presents the implementation of `GlobTool`, a file discovery component designed for AI agent systems written in Rust. The tool enables recursive directory traversal with glob pattern matching, allowing agents to locate files based on patterns such as `**/*.rs` or `src/**/*.ts`. The implementation demonstrates sophisticated software engineering practices including parallel processing via Rayon for performance optimization, careful error handling through the anyhow crate, and JSON-based parameter schemas for tool interoperability. The `GlobTool` struct implements a `Tool` trait, indicating its role within a larger agent framework where tools are composable units with standardized interfaces for name, description, parameter validation, permission categorization, and execution. The design addresses practical concerns for codebases of varying sizes by implementing parallel directory walking when subdirectory counts exceed a threshold, while also incorporating safeguards such as a 1,000-match limit and automatic filtering of hidden files and common generated directories like `node_modules`, `target`, `__pycache__`, `dist`, and `build`. The tool's permission categorization as `file:read` reflects a security-conscious design that enables fine-grained access control within agent systems. The implementation showcases modern Rust patterns including the use of `Arc` for shared ownership across parallel iterations, `async_trait` for asynchronous trait implementation, and careful memory management to prevent unbounded growth of result collections.

## Related

### Entities

- [GlobTool](../entities/globtool.md) — technology
- [Rayon](../entities/rayon.md) — technology
- [globset](../entities/globset.md) — technology
- [anyhow](../entities/anyhow.md) — technology

