---
title: "Ragent Tool Output Standards"
source: "tool_output"
type: source
tags: [ragent, tool-output, standards, formatting, metadata, TUI, API, documentation, rust]
generated: "2026-04-18T15:15:25.756485691+00:00"
---

# Ragent Tool Output Standards

This document defines standardized patterns and conventions for tool output in the ragent project, ensuring a consistent user experience across all tools in both TUI and API interfaces. Tool output consists of two parts: human-readable content displayed to users and structured JSON metadata for programmatic access.

The document specifies three main content format patterns: Pattern A (Summary + Content) for read operations and search results, Pattern B (Summary Only) for write operations and simple state changes, and Pattern C (Structured Output) for execution tools with exit codes and timing information. It also provides detailed metadata schema definitions with common fields like path, line_count, byte_count, exit_code, and duration_ms, along with tool-specific fields for tools such as grep, glob, list, bash, and edit.

Additional guidelines cover TUI display conventions including emoji indicators for different operation categories, proper pluralization, content truncation with clear markers, byte formatting with appropriate units, relative path display, and error presentation patterns. The document includes a testing checklist and migration checklist for implementing new tools or updating existing ones.

## Related

### Entities

- [ragent](../entities/ragent.md) — product
- [ragent core::tool::format](../entities/ragent-core-tool-format.md) — technology
- [ragent core::tool::metadata::MetadataBuilder](../entities/ragent-core-tool-metadata-metadatabuilder.md) — technology
- [ragent core::tool::truncate](../entities/ragent-core-tool-truncate.md) — technology
- [ragent tui::widgets::message widget](../entities/ragent-tui-widgets-message-widget.md) — technology

### Concepts

- [tool output patterns](../concepts/tool-output-patterns.md)
- [metadata schema](../concepts/metadata-schema.md)
- [content truncation](../concepts/content-truncation.md)
- [pluralization](../concepts/pluralization.md)
- [relative path display](../concepts/relative-path-display.md)
- [error presentation](../concepts/error-presentation.md)
- [emoji indicators](../concepts/emoji-indicators.md)

