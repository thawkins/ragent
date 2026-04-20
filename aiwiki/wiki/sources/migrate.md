---
title: "Memory Migration Module for Ragent Core"
source: "migrate"
type: source
tags: [rust, migration, markdown, memory-management, ragent, data-transformation, file-processing, refactoring]
generated: "2026-04-19T21:41:18.175793721+00:00"
---

# Memory Migration Module for Ragent Core

This document describes the `migrate.rs` module in the ragent-core crate, which provides functionality for migrating flat MEMORY.md files into structured memory blocks. The module implements a safe, opt-in migration process that analyzes Markdown content, extracts sections based on headings, and transforms them into named blocks with proper labels. A key design principle is the dry-run capability: by default, the migration only analyzes and reports what would happen, requiring explicit user confirmation before any files are modified. The original MEMORY.md is preserved as a backup, ensuring no data loss during migration.

The migration process involves several sophisticated steps. First, the `analyse_memory_md` function parses Markdown content to identify top-level (`##`) headings, using the heading text to generate valid block labels through a `slugify_heading` function that normalizes text to lowercase, replaces special characters with hyphens, and ensures labels start with letters. For content without headings, the module gracefully falls back to creating a single "general" block. The `migrate_memory_md` function then orchestrates the actual migration, checking for existing blocks to avoid overwrites, and constructing a detailed `MigrationPlan` that tracks what would be created versus skipped.

The module demonstrates Rust best practices including comprehensive unit testing, error handling with `anyhow`, and clean separation between analysis and execution phases. The `MigrationPlan` struct provides transparency by returning detailed metadata about each section including line counts, byte counts, and existence status. This design enables users to review and approve migrations before committing changes, making it suitable for production environments where data integrity is critical.

## Related

### Entities

- [MigrationPlan](../entities/migrationplan.md) — product
- [SectionInfo](../entities/sectioninfo.md) — technology
- [FileBlockStorage](../entities/fileblockstorage.md) — technology

### Concepts

- [Markdown Content Migration](../concepts/markdown-content-migration.md)
- [Slugification](../concepts/slugification.md)
- [Dry-Run Migration Pattern](../concepts/dry-run-migration-pattern.md)
- [Memory Block Architecture](../concepts/memory-block-architecture.md)
- [Defensive File Processing](../concepts/defensive-file-processing.md)

