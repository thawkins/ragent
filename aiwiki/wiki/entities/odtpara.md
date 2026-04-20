---
title: "OdtPara"
entity_type: "technology"
type: entity
generated: "2026-04-19T18:13:45.480300269+00:00"
---

# OdtPara

**Type:** technology

### From: libreoffice_write

OdtPara is a private struct within the libreoffice_write.rs module that serves as an intermediate representation for paragraph-level content in ODT document generation. This struct normalizes the diverse input formats that LibreWriteTool accepts into a uniform internal format suitable for XML serialization. The struct contains two fields: `text`, which holds the string content of the paragraph, and `style`, which is an instance of the `OdtStyle` enum determining how the paragraph will be rendered in the final document. The normalization process, implemented in the `resolve_odt_paras` function, handles multiple input scenarios including plain text strings (split by lines), JSON arrays of structured elements, and objects containing nested content or paragraph arrays. This design pattern—using an intermediate representation between raw input and final output—is a classic compiler construction technique that separates parsing concerns from code generation. The OdtPara struct enables the tool to support rich document structures including six levels of headings, bullet lists, numbered lists, and preformatted code blocks while maintaining clean separation between the flexible input parsing logic and the strict XML generation requirements of the ODF specification. The struct's simple design reflects Rust's zero-cost abstraction philosophy, where the intermediate representation exists only during document construction and imposes no runtime overhead in the final binary.

## External Resources

- [ODF 1.3 specification for text document structure](https://docs.oasis-open.org/office/OpenDocument/v1.3/OpenDocument-v1.3-part4-recursive.html) - ODF 1.3 specification for text document structure

## Sources

- [libreoffice_write](../sources/libreoffice-write.md)
