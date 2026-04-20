---
title: "OdpSlide"
entity_type: "technology"
type: entity
generated: "2026-04-19T18:13:45.480789281+00:00"
---

# OdpSlide

**Type:** technology

### From: libreoffice_write

OdpSlide is a private struct that provides an intermediate representation for slide content in OpenDocument Presentation (ODP) file generation. Similar to OdtPara for text documents, this struct normalizes the varied input structures that may represent presentation slides into a consistent format for XML serialization. The struct contains a `title` field for the slide heading and a `lines` vector for bullet points or content lines. The accompanying `resolve_odp_slides` function implements sophisticated content resolution logic that handles multiple input formats: structured JSON arrays with explicit slide objects, plain text with slide separation by blank lines, and even JSON strings that need parsing. The function includes a recursive JSON parsing attempt for string content, allowing LLMs to pass pre-serialized JSON structures. The flattening logic converts various element types—headings, paragraphs, bullet lists, ordered lists—into a uniform line-based representation suitable for presentation slides. This design acknowledges the simpler structural requirements of presentations compared to text documents, where slide content is typically more constrained in formatting options. The struct and its resolution function demonstrate the codebase's emphasis on defensive programming and input flexibility, anticipating the unpredictable output patterns of large language models while ensuring valid ODF output.

## External Resources

- [ODF 1.3 schema specification for presentation documents](https://docs.oasis-open.org/office/OpenDocument/v1.3/OpenDocument-v1.3-part3-schema.html) - ODF 1.3 schema specification for presentation documents

## Sources

- [libreoffice_write](../sources/libreoffice-write.md)
