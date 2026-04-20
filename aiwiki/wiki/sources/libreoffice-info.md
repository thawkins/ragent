---
title: "LibreOffice Document Info Tool - ODF Metadata Extraction in Rust"
source: "libreoffice_info"
type: source
tags: [rust, libreoffice, odf, document-processing, metadata-extraction, calamine, xml-parsing, async, tool-system]
generated: "2026-04-19T18:09:44.837389076+00:00"
---

# LibreOffice Document Info Tool - ODF Metadata Extraction in Rust

This Rust source file implements `LibreInfoTool`, a document metadata and structural analysis tool for OpenDocument Format (ODF) files. The tool provides comprehensive information extraction capabilities for three major ODF document types: ODS (spreadsheets), ODT (text documents), and ODP (presentations). Rather than reading all document content, it intelligently extracts metadata and structural statistics, making it efficient for large documents where full content reading would be unnecessary or resource-intensive.

The implementation leverages multiple specialized libraries to handle different aspects of ODF parsing. For ODS files, it uses the `calamine` crate to enumerate worksheets and calculate row/column dimensions. For ODT and ODP files, it employs `quick_xml` for streaming XML parsing of the document's `content.xml` and `meta.xml` files. The tool operates within an async runtime using `tokio::task::spawn_blocking` to prevent blocking the main thread during potentially lengthy file I/O and parsing operations. This design pattern ensures the tool remains responsive when integrated into larger asynchronous applications.

The architecture demonstrates sophisticated understanding of ODF's underlying structure. ODF files are ZIP archives containing XML files with specific organizational patterns. The tool extracts metadata from `meta.xml` (title, creator, creation date) and analyzes `content.xml` for structural elements—paragraphs and word counts for text documents, slide names and counts for presentations. Error handling is comprehensive, using the `anyhow` crate for contextual error propagation, with specific failure modes documented including missing parameters, undetectable formats, corrupt ZIP archives, and JSON serialization failures.

## Related

### Entities

- [LibreInfoTool](../entities/libreinfotool.md) — product
- [calamine](../entities/calamine.md) — technology
- [quick-xml](../entities/quick-xml.md) — technology
- [OpenDocument Format (ODF)](../entities/opendocument-format-odf.md) — technology

