---
title: "Ragent Tool Metadata Schema Documentation"
source: "tool_metadata_schema"
type: source
tags: [json-schema, metadata, tool-system, ragent, api-design, standardization, tui, documentation]
generated: "2026-04-18T15:14:57.903152962+00:00"
---

# Ragent Tool Metadata Schema Documentation

This document defines the formal JSON schema for tool metadata in the ragent system, establishing standardized field names, types, and usage patterns across all tool implementations. The metadata structure separates machine-readable execution results from human-readable content, enabling consistent terminal user interface (TUI) display formatting, programmatic access to tool outputs, and improved filtering and analysis capabilities.

The schema organizes fields into functional categories including file/path operations, counting metrics, line positioning, size measurements, status indicators, content flags, timestamps, diff/change tracking, collections, task/agent management, result values, GitHub integrations, and office/PDF document handling. Three standardized content patterns are defined: Pattern A for summary plus content (used by read, list, grep, search tools), Pattern B for summary-only confirmations (used by write, edit, bash, file operation tools), and Pattern C for structured data output (used by execution and status tools). The documentation includes comprehensive field definitions with types and examples, deprecated field mappings for migration, TUI integration guidelines, and category-specific implementation examples.

## Related

### Entities

- [ragent](../entities/ragent.md) — technology
- [TUI](../entities/tui.md) — technology

### Concepts

- [tool metadata](../concepts/tool-metadata.md)
- [content patterns](../concepts/content-patterns.md)
- [Pattern A](../concepts/pattern-a.md)
- [Pattern B](../concepts/pattern-b.md)
- [Pattern C](../concepts/pattern-c.md)
- [schema migration](../concepts/schema-migration.md)
- [deprecated fields](../concepts/deprecated-fields.md)
- [field validation](../concepts/field-validation.md)

