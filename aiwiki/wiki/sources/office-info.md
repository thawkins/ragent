---
title: "OfficeInfoTool: Rust-based Office Document Metadata Extractor"
source: "office_info"
type: source
tags: [rust, office-documents, docx, xlsx, pptx, metadata-extraction, ooxml, async-rust, document-parsing, calamine, docx-rust, ooxmlsdk]
generated: "2026-04-19T18:39:55.596658792+00:00"
---

# OfficeInfoTool: Rust-based Office Document Metadata Extractor

The `office_info.rs` file implements a Rust tool for extracting metadata and structural information from Microsoft Office documents, specifically Word (.docx), Excel (.xlsx), and PowerPoint (.pptx) files. This component is part of a larger agent-based system (ragent-core) and provides structured access to document properties without requiring Microsoft Office installation. The implementation leverages three specialized Rust crates: `docx-rust` for Word documents, `calamine` for Excel spreadsheets, and `ooxmlsdk` for PowerPoint presentations—each chosen for their specific strengths in parsing the respective Office Open XML (OOXML) formats.

The architecture follows a common pattern where a main `Tool` trait implementation (`OfficeInfoTool`) dispatches to format-specific functions (`info_docx`, `info_xlsx`, `info_pptx`) based on detected file type. Each extraction function returns both human-readable text and structured JSON metadata, enabling flexible consumption by downstream systems. Notable design decisions include the use of `tokio::task::spawn_blocking` for CPU-bound parsing operations, preserving async runtime responsiveness, and careful handling of optional metadata fields (title, author) with sensible defaults. The PowerPoint implementation demonstrates particular sophistication in extracting slide titles by navigating the OOXML structure through shape trees and paragraph runs.

## Related

### Entities

- [OfficeInfoTool](../entities/officeinfotool.md) — technology
- [docx-rust](../entities/docx-rust.md) — technology
- [calamine](../entities/calamine.md) — technology
- [ooxmlsdk](../entities/ooxmlsdk.md) — technology

