---
title: "ragent-core Memory Import/Export System"
source: "import_export"
type: source
tags: [rust, memory-management, data-migration, import-export, json-serialization, sqlite, markdown-processing, cli-tools, ai-agents, data-portability]
generated: "2026-04-19T21:59:33.766844572+00:00"
---

# ragent-core Memory Import/Export System

The import_export.rs module in ragent-core provides a comprehensive data portability layer for the ragent agent memory system. This module enables bidirectional data exchange between ragent and external AI assistant tools, implementing both export functionality to serialize all memory data to portable JSON format and import adapters for multiple external formats. The system is designed with robust validation, dry-run capabilities for safe migration testing, and graceful degradation through warning accumulation rather than hard failures.

The module's architecture centers around three primary data structures—structured memories stored in SQLite, journal entries for chronological logging, and file-based memory blocks organized by scope (project or global). The export functionality captures all three data types into a versioned JSON container with ISO 8601 timestamps, enabling complete backup and migration scenarios. Import functionality supports not only ragent's native format but also adapters for Cline Memory Bank (directory of markdown files with PascalCase naming) and Claude Code auto-memory (single markdown file with heading-based sections), facilitating smooth transitions for users migrating from other AI assistant ecosystems.

Implementation details reveal careful attention to data integrity and user experience. The export_all function retrieves memories with their associated tags, reconstructs full StructuredMemory objects with all metadata fields, and similarly exports journal entries with tag preservation. Block export handles both project-scoped and global-scoped storage hierarchically. Import operations validate all incoming data against domain rules (category validity, confidence ranges, label formats, tag constraints) with detailed warning messages, and the dry_run mode allows users to preview migrations without side effects. The Cline adapter performs filename slugification converting PascalCase to kebab-case, while the Claude Code adapter leverages the existing migration module's markdown analysis capabilities to split monolithic files into logical blocks.

## Related

### Entities

- [Cline](../entities/cline.md) — product
- [Claude Code](../entities/claude-code.md) — product
- [MemoryExport](../entities/memoryexport.md) — technology
- [BlockStorage](../entities/blockstorage.md) — technology

