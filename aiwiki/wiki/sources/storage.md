---
title: "Ragent Core Memory Storage: File-Based Block Storage Implementation"
source: "storage"
type: source
tags: [rust, file-storage, memory-management, persistence, atomic-writes, markdown, yaml-frontmatter, ragent, block-storage, trait-abstraction]
generated: "2026-04-19T21:36:42.614235047+00:00"
---

# Ragent Core Memory Storage: File-Based Block Storage Implementation

This document describes the `storage.rs` module in the ragent-core crate, which implements a file-based persistence layer for memory blocks. The module defines the `BlockStorage` trait, an abstract interface for memory block operations, and `FileBlockStorage`, a concrete implementation that stores blocks as Markdown files with YAML frontmatter. The storage system supports two scopes: project-local (`.ragent/memory/` directory) and global (`~/.ragent/memory/` in the user's home directory). Key features include atomic writes using temp-and-rename patterns, content limit enforcement, legacy MEMORY.md backward compatibility, and comprehensive error handling with the anyhow crate. The implementation emphasizes data integrity through atomic file operations, where content is first written to a temporary `.md.tmp` file before being renamed to the final `.md` filename, preventing data corruption in case of crashes during write operations.

The module provides a rich set of operations including loading individual blocks by label and scope, saving blocks with automatic directory creation, listing all valid block labels in a scope, and deleting blocks. The `load_all_blocks` function enables bulk loading across both scopes with graceful error handling that skips unparseable files while logging warnings. The `load_legacy_memory` function provides migration support for older MEMORY.md files that lack YAML frontmatter, treating them as blocks with a fixed "MEMORY" label. The test suite validates core functionality including atomic write guarantees, content limit enforcement, cross-scope operations, and legacy file handling, using temporary directories to ensure test isolation.

## Related

### Entities

- [FileBlockStorage](../entities/fileblockstorage.md) — technology
- [BlockStorage](../entities/blockstorage.md) — technology

### Concepts

- [Atomic File Writes](../concepts/atomic-file-writes.md)
- [BlockScope](../concepts/blockscope.md)
- [Legacy Memory Migration](../concepts/legacy-memory-migration.md)
- [YAML Frontmatter](../concepts/yaml-frontmatter.md)

