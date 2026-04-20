---
title: "Ragent Office Document Reader Tool"
source: "office_read"
type: source
tags: [rust, office-documents, docx, xlsx, pptx, document-parsing, llm-tools, text-extraction, markdown-generation, async-rust, calamine, ooxmlsdk]
generated: "2026-04-19T18:43:07.087390406+00:00"
---

# Ragent Office Document Reader Tool

This source file implements `OfficeReadTool`, a comprehensive Rust-based utility for extracting content from Microsoft Office documents including Word (.docx), Excel (.xlsx), and PowerPoint (.pptx) files. The tool is designed to serve large language models (LLMs) by converting proprietary Office formats into structured text representations that can be processed and reasoned about. It leverages three specialized Rust libraries—`docx-rust` for Word documents, `calamine` for Excel spreadsheets, and `ooxmlsdk` for PowerPoint presentations—to provide robust parsing capabilities across the Office suite.

The implementation follows a modular architecture with format-specific reading functions (`read_docx`, `read_xlsx`, `read_pptx`) that handle the unique complexities of each file type. For Word documents, the tool extracts paragraphs and tables while preserving style information to generate markdown-formatted output with proper heading hierarchies. Excel reading supports cell range selection using standard spreadsheet notation (e.g., "A1:D10") and handles multiple data types including dates, formulas, and formatted numbers. PowerPoint processing extracts slide titles, body content, and speaker notes while maintaining slide sequence. All output can be formatted as plain text, markdown, or JSON depending on the consuming application's needs.

The tool implements the `Tool` trait from the ragent framework, making it available as an asynchronous capability that can be invoked by AI agents. It includes comprehensive error handling using `anyhow`, supports path resolution relative to a working directory, and truncates large outputs to prevent context window overflow. The permission category of "file:read" indicates this tool requires appropriate authorization before accessing user documents, reflecting security-conscious design for agent-based systems.

## Related

### Entities

- [OfficeReadTool](../entities/officereadtool.md) — technology
- [docx-rust](../entities/docx-rust.md) — technology
- [calamine](../entities/calamine.md) — technology
- [ooxmlsdk](../entities/ooxmlsdk.md) — technology

### Concepts

- [Office Document Text Extraction](../concepts/office-document-text-extraction.md)
- [Office Open XML Format](../concepts/office-open-xml-format.md)
- [LLM Tool Design Patterns](../concepts/llm-tool-design-patterns.md)
- [Markdown Generation from Structured Documents](../concepts/markdown-generation-from-structured-documents.md)

