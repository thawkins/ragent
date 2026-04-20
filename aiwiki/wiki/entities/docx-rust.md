---
title: "docx-rust"
entity_type: "technology"
type: entity
generated: "2026-04-19T18:39:55.597804331+00:00"
---

# docx-rust

**Type:** technology

### From: office_info

docx-rust is a Rust crate specifically designed for reading and manipulating Microsoft Word documents in the Office Open XML (.docx) format. It provides a type-safe, idiomatic Rust API for accessing document structure including paragraphs, tables, runs of text, and core document properties. The crate handles the complex OOXML packaging structure, including ZIP archive extraction and XML namespace management, abstracting these implementation details from users.

In the context of OfficeInfoTool, docx-rust serves as the primary parser for Word documents through the `info_docx` function. The usage pattern involves two steps: first calling `DocxFile::from_file()` to open and validate the document package, then calling `parse()` to construct an in-memory representation. The parsed document exposes a hierarchical structure where `document.body.content` contains mixed-type elements representing paragraphs, tables, and other body content.

The crate's design reflects Rust's ownership and error handling patterns, returning Results for fallible operations. A notable complexity in the OfficeInfoTool implementation is handling core document metadata (title, author), where docx-rust represents these as an enum `Core` with variants for namespaced and non-namespaced XML structures—reflecting real-world variation in how different tools generate .docx files. This requires pattern matching on both variants to extract optional metadata fields.

## External Resources

- [docx-rust crate on crates.io](https://crates.io/crates/docx-rust) - docx-rust crate on crates.io
- [docx-rust source repository on GitHub](https://github.com/benjijs/docx-rust) - docx-rust source repository on GitHub

## Sources

- [office_info](../sources/office-info.md)

### From: office_read

docx-rust is a Rust library for reading and writing Microsoft Word documents in the Office Open XML (.docx) format. In this implementation, it serves as the primary dependency for Word document processing, providing APIs to parse document structure including paragraphs, tables, styles, and text content. The library abstracts the complexity of the underlying ZIP-compressed XML structure that defines modern Word documents, exposing a higher-level interface that enables iteration over document body content and extraction of styled text elements.

The integration within office_read.rs demonstrates docx-rust's capabilities for both structural and semantic document analysis. The `DocxFile::from_file` and `parse` methods handle document loading, while the resulting document object exposes body content that can be matched against variants like `BodyContent::Paragraph` and `BodyContent::Table`. Style information is preserved through paragraph properties, enabling the `style_to_markdown` function to apply appropriate markdown formatting based on Word styles like "Heading1" or "ListBullet". This semantic understanding elevates simple text extraction to structured document conversion.

docx-rust represents part of a broader ecosystem of Rust libraries for Office document manipulation. Unlike Python's python-docx or Node.js's mammoth.js, docx-rust provides memory-safe, zero-cost abstraction over document parsing with Rust's performance characteristics. The library's design allows for both read and write operations, though this implementation focuses exclusively on reading. Its use of Rust's type system to model document structure provides compile-time guarantees about content handling, reducing runtime errors when processing varied document structures from unknown sources.

### From: office_write

docx-rust is a Rust library used by OfficeWriteTool for generating Microsoft Word documents in the modern .docx format. The library provides a builder-pattern API for constructing documents with paragraphs, runs of text, and various formatting properties. In the OfficeWriteTool implementation, docx-rust handles the complex underlying Office Open XML structure, abstracting away the intricate XML relationships, content types, and document parts that comprise a valid .docx file. The tool leverages specific components including `Paragraph` for document structure, `CharacterProperty` for inline text formatting like bold and italics, and `ParagraphProperty` for paragraph-level styling including heading levels and list styles. A notable implementation detail is the use of a private `ParaEntry` struct to own string data before passing references to the docx-rust builder. This pattern addresses Rust's lifetime requirements, ensuring that the owned strings outlive the document construction process. The library's integration enables rich document features including support for headings at six levels, bullet and numbered lists, code blocks with monospaced formatting, and inline markdown-style formatting parsed through the custom `parse_inline_formatting` function.
