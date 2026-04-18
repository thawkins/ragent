---
title: "O365 Tool Specification: Office Document Read/Write Support for ragent"
source: "O365_TOOL"
type: source
tags: [ragent, office-documents, microsoft-office, docx, xlsx, pptx, rust, tool-development, llm-integration, file-i-o, specification, implementation-plan]
generated: "2026-04-18T15:04:23.014719556+00:00"
---

# O365 Tool Specification: Office Document Read/Write Support for ragent

This document specifies the design and implementation plan for adding Microsoft Office document support to the ragent AI agent framework. The feature introduces three new tools—office read, office write, and office info—that enable LLMs to read from and write to Word (.docx), Excel (.xlsx), and PowerPoint (.pptx) files. The implementation leverages specific Rust crates for each format: docx-rust for Word documents, calamine and rust-xlsxwriter for Excel spreadsheets, and ooxmlsdk for PowerPoint presentations. The tools integrate with ragent's existing Tool trait, ToolContext, and permission system, supporting file:read and file:write permissions. The document details JSON schemas for each tool, content structures for different document types, and an 11-task implementation plan covering dependency management, format detection, read/write implementations for all three formats, metadata extraction, tool registration, integration testing, and documentation. Key considerations include output size limits (100KB truncation), minimal dependency footprint, best-effort formatting fidelity, and explicit support for modern OOXML formats only.

## Related

### Entities

- [ragent](../entities/ragent.md) — product
- [Microsoft](../entities/microsoft.md) — organization
- [docx-rust](../entities/docx-rust.md) — technology
- [calamine](../entities/calamine.md) — technology
- [rust-xlsxwriter](../entities/rust-xlsxwriter.md) — technology
- [ooxmlsdk](../entities/ooxmlsdk.md) — technology
- [OfficeReadTool](../entities/officereadtool.md) — product
- [OfficeWriteTool](../entities/officewritetool.md) — product
- [OfficeInfoTool](../entities/officeinfotool.md) — product
- [ToolRegistry](../entities/toolregistry.md) — technology

### Concepts

- [OOXML format](../concepts/ooxml-format.md)
- [function-calling](../concepts/function-calling.md)
- [Tool trait](../concepts/tool-trait.md)
- [ToolContext](../concepts/toolcontext.md)
- [permission system](../concepts/permission-system.md)
- [format detection](../concepts/format-detection.md)
- [round-trip testing](../concepts/round-trip-testing.md)
- [content extraction fidelity](../concepts/content-extraction-fidelity.md)
- [output truncation](../concepts/output-truncation.md)
- [workspace dependencies](../concepts/workspace-dependencies.md)

