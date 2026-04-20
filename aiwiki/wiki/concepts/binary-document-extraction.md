---
title: "Binary Document Extraction"
type: concept
generated: "2026-04-19T20:33:48.831052491+00:00"
---

# Binary Document Extraction

### From: resolve

Binary document extraction encompasses the techniques and technologies for extracting human-readable text content from proprietary binary formats, enabling text processing pipelines to consume documents created in word processors, spreadsheets, and presentation software. This module implements specialized handling for Microsoft Office Open XML formats (DOCX, XLSX, PPTX) and Adobe PDF through dedicated reader tools, recognizing these formats by file extension and delegating parsing to appropriate extraction libraries. The implementation uses `tokio::task::spawn_blocking` to isolate CPU-intensive parsing from the async runtime, acknowledging that binary document extraction involves decompression (Office formats are ZIP archives), XML parsing, and complex format-specific logic. The abstraction allows the resolution pipeline to treat binary and text sources uniformly while handling format complexities internally. Binary extraction is crucial for RAG and LLM applications where organizational knowledge resides in document formats rather than plain text, and where preserving table structures, slide content, and document metadata enhances retrieval quality.

## External Resources

- [Office Open XML format specification](https://en.wikipedia.org/wiki/Office_Open_XML) - Office Open XML format specification
- [PDF text extraction crate for Rust](https://crates.io/crates/pdf-extract) - PDF text extraction crate for Rust

## Sources

- [resolve](../sources/resolve.md)
