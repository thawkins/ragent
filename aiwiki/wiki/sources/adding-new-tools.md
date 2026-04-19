---
title: "Adding New Tools to Ragent: Developer Guide"
source: "adding_new_tools"
type: source
tags: [developer-guide, tool-development, ragent, rust, plugin-system, code-standards, TUI, testing, documentation]
generated: "2026-04-18T15:14:31.483758191+00:00"
---

# Adding New Tools to Ragent: Developer Guide

This document provides a comprehensive step-by-step guide for developers to add new tools to the Ragent framework while maintaining consistency with existing implementations. The guide covers eight major steps: defining the tool with proper naming conventions and output patterns, creating the tool implementation file in Rust using the Tool trait, registering the tool in the module system, adding TUI display support for both input and result summaries with appropriate emoji indicators, optionally defining tool aliases, writing comprehensive tests, running the test suite, and updating relevant documentation. The document also includes common implementation patterns for file operations, search operations, and execution tools, along with migration guidance for updating existing tools to new standards. A complete working example of a WordCountTool demonstrates all recommended practices in a real implementation context.

## Related

### Entities

- [Ragent](../entities/ragent.md) — product
- [MetadataBuilder](../entities/metadatabuilder.md) — technology
- [ToolRegistry](../entities/toolregistry.md) — technology
- [WordCountTool](../entities/wordcounttool.md) — product
- [ToolContext](../entities/toolcontext.md) — technology
- [ToolOutput](../entities/tooloutput.md) — technology
- [MessageWidget](../entities/messagewidget.md) — technology
- [Tokio](../entities/tokio.md) — technology
- [serde_json](../entities/serde-json.md) — technology
- [anyhow](../entities/anyhow.md) — technology

### Concepts

- [Tool Trait](../concepts/tool-trait.md)
- [Output Patterns](../concepts/output-patterns.md)
- [Permission Categories](../concepts/permission-categories.md)
- [Metadata Schema](../concepts/metadata-schema.md)
- [TUI Display Categories](../concepts/tui-display-categories.md)
- [Snake Case Naming](../concepts/snake-case-naming.md)
- [Tool Aliases](../concepts/tool-aliases.md)
- [Path Handling Standards](../concepts/path-handling-standards.md)
- [Test Coverage Requirements](../concepts/test-coverage-requirements.md)
- [Documentation Standards](../concepts/documentation-standards.md)

