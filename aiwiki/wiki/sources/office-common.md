---
title: "office_common.rs - Core Utilities for Office Document Processing"
source: "office_common"
type: source
tags: [rust, office-documents, ooxml, file-format-detection, path-resolution, text-truncation, docx, xlsx, pptx]
generated: "2026-04-19T16:06:16.111907061+00:00"
---

# office_common.rs - Core Utilities for Office Document Processing

This Rust source file provides foundational utilities for working with Office documents in the ragent-core crate. It defines the `OfficeFormat` enum to represent supported modern Office document formats (DOCX, XLSX, PPTX), implements format detection based on file extensions, and includes helper functions for path resolution and output truncation. The module serves as a shared foundation for all Office tool modules, enforcing support for modern OOXML formats while explicitly rejecting legacy binary formats. The output truncation functionality ensures that tool responses remain within reasonable size limits (100KB) while attempting to preserve clean line boundaries when truncating.

## Related

### Entities

- [Office Open XML (OOXML)](../entities/office-open-xml-ooxml.md) — technology
- [anyhow](../entities/anyhow.md) — technology

### Concepts

- [File Format Detection by Extension](../concepts/file-format-detection-by-extension.md)
- [Path Resolution in Multi-Context Applications](../concepts/path-resolution-in-multi-context-applications.md)
- [Output Truncation and Resource Management](../concepts/output-truncation-and-resource-management.md)
- [Display Trait for Enum String Representation](../concepts/display-trait-for-enum-string-representation.md)

