//! Text extraction from various document formats.
//!
//! Uses document parsing libraries directly for PDF, DOCX, and ODT files.
//! Source code files are parsed with tree-sitter via `ragent-code` to produce
//! structured summaries of symbols, imports, and documentation.

use std::path::Path;

/// Supported document types for ingestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentType {
    /// Markdown files (.md, .markdown)
    Markdown,
    /// PDF documents (.pdf)
    Pdf,
    /// Word documents (.docx)
    Docx,
    /// OpenDocument Text (.odt)
    Odt,
    /// Plain text files (.txt)
    Text,
    /// Source code file with a known tree-sitter language.
    SourceCode {
        /// The language id recognised by `ragent_code::parser::ParserRegistry`
        /// (e.g. `"rust"`, `"python"`, `"typescript"`).
        language: String,
    },
    /// Unknown/unsupported file type
    Unknown,
}

impl DocumentType {
    /// Detect document type from file path based on extension.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref();
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase());

        match extension.as_deref() {
            Some("md") | Some("markdown") => Self::Markdown,
            Some("pdf") => Self::Pdf,
            Some("docx") => Self::Docx,
            Some("odt") => Self::Odt,
            Some("txt") | Some("text") => Self::Text,
            _ => {
                // Try tree-sitter language detection for source code files.
                if let Some(lang) = ragent_code::scanner::detect_language(path) {
                    Self::SourceCode { language: lang }
                } else {
                    Self::Unknown
                }
            }
        }
    }

    /// Check if this document type supports text extraction.
    pub fn supports_extraction(&self) -> bool {
        matches!(
            self,
            Self::Markdown
                | Self::Pdf
                | Self::Docx
                | Self::Odt
                | Self::Text
                | Self::SourceCode { .. }
        )
    }

    /// Get a human-readable name for the document type.
    pub fn name(&self) -> &str {
        match self {
            Self::Markdown => "Markdown",
            Self::Pdf => "PDF",
            Self::Docx => "Word Document",
            Self::Odt => "OpenDocument Text",
            Self::Text => "Plain Text",
            Self::SourceCode { language } => language.as_str(),
            Self::Unknown => "Unknown",
        }
    }

    /// Get the MIME type for this document type.
    pub fn mime_type(&self) -> &str {
        match self {
            Self::Markdown => "text/markdown",
            Self::Pdf => "application/pdf",
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Odt => "application/vnd.oasis.opendocument.text",
            Self::Text => "text/plain",
            Self::SourceCode { .. } => "text/plain",
            Self::Unknown => "application/octet-stream",
        }
    }
}

/// Extract text content from a document.
///
/// Uses document parsing libraries for PDF (pdf-extract),
/// DOCX (docx-rust), and ODT (zip+quick-xml).
///
/// # Arguments
/// * `path` - Path to the document file
/// * `doc_type` - The detected document type
///
/// # Returns
/// The extracted text content as a string.
///
/// # Errors
/// Returns an error if extraction fails for the document type.
pub async fn extract_text<P: AsRef<Path>>(
    path: P,
    doc_type: DocumentType,
) -> crate::Result<String> {
    let path = path.as_ref().to_path_buf();

    match doc_type {
        DocumentType::Text => tokio::fs::read_to_string(&path).await.map_err(|e| e.into()),
        DocumentType::Markdown => {
            let raw = tokio::fs::read_to_string(&path)
                .await
                .map_err(crate::AiwikiError::Io)?;
            Ok(strip_markdown(&raw))
        }
        DocumentType::Pdf => {
            let p = path.clone();
            tokio::task::spawn_blocking(move || extract_pdf_text(&p))
                .await
                .map_err(|e| crate::AiwikiError::Config(format!("PDF task panicked: {e}")))?
        }
        DocumentType::Docx => {
            let p = path.clone();
            tokio::task::spawn_blocking(move || extract_docx_text(&p))
                .await
                .map_err(|e| crate::AiwikiError::Config(format!("DOCX task panicked: {e}")))?
        }
        DocumentType::Odt => {
            let p = path.clone();
            tokio::task::spawn_blocking(move || extract_odt_text(&p))
                .await
                .map_err(|e| crate::AiwikiError::Config(format!("ODT task panicked: {e}")))?
        }
        DocumentType::SourceCode { language } => {
            let p = path.clone();
            let lang = language.clone();
            tokio::task::spawn_blocking(move || extract_code_text(&p, &lang))
                .await
                .map_err(|e| crate::AiwikiError::Config(format!("Code parse task panicked: {e}")))?
        }
        DocumentType::Unknown => Err(crate::AiwikiError::Config(format!(
            "Cannot extract text from unknown file type: {}",
            path.display()
        ))),
    }
}

/// Extract text from a PDF file using pdf-extract.
fn extract_pdf_text(path: &Path) -> crate::Result<String> {
    let bytes = std::fs::read(path).map_err(crate::AiwikiError::Io)?;
    let text = pdf_extract::extract_text_from_mem(&bytes)
        .map_err(|e| crate::AiwikiError::Config(format!("PDF text extraction failed: {e}")))?;
    Ok(text)
}

/// Extract text from a DOCX file using docx-rust.
fn extract_docx_text(path: &Path) -> crate::Result<String> {
    let docx_file = docx_rust::DocxFile::from_file(path)
        .map_err(|e| crate::AiwikiError::Config(format!("Failed to open DOCX: {e}")))?;
    let docx = docx_file
        .parse()
        .map_err(|e| crate::AiwikiError::Config(format!("Failed to parse DOCX: {e}")))?;

    let mut text = String::new();
    for content in &docx.document.body.content {
        match content {
            docx_rust::document::BodyContent::Paragraph(para) => {
                let para_text = para.text();
                if !para_text.is_empty() {
                    text.push_str(&para_text);
                    text.push('\n');
                }
            }
            docx_rust::document::BodyContent::Table(table) => {
                for row in &table.rows {
                    for cell_content in &row.cells {
                        if let docx_rust::document::TableRowContent::TableCell(tc) = cell_content {
                            for item in &tc.content {
                                if let docx_rust::document::TableCellContent::Paragraph(p) = item {
                                    let cell_text = p.text();
                                    if !cell_text.is_empty() {
                                        text.push_str(&cell_text);
                                        text.push('\t');
                                    }
                                }
                            }
                        }
                    }
                    text.push('\n');
                }
            }
            _ => {}
        }
    }
    Ok(text)
}

/// Extract text from an ODT file by reading content.xml from the ZIP archive.
fn extract_odt_text(path: &Path) -> crate::Result<String> {
    let file = std::fs::File::open(path).map_err(crate::AiwikiError::Io)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| crate::AiwikiError::Config(format!("Failed to open ODT as ZIP: {e}")))?;

    let mut content_xml = String::new();
    {
        let mut entry = archive
            .by_name("content.xml")
            .map_err(|e| crate::AiwikiError::Config(format!("No content.xml in ODT: {e}")))?;
        std::io::Read::read_to_string(&mut entry, &mut content_xml)
            .map_err(crate::AiwikiError::Io)?;
    }

    Ok(xml_to_text(&content_xml))
}

/// Extract a structured text summary from a source code file using tree-sitter.
///
/// Parses the file with the appropriate tree-sitter grammar and produces a
/// human-readable description of the file's symbols (functions, structs,
/// classes, imports, etc.) suitable for LLM analysis.
fn extract_code_text(path: &Path, language: &str) -> crate::Result<String> {
    use ragent_code::parser::ParserRegistry;
    use ragent_code::types::SymbolKind;
    use std::fmt::Write;

    let source = std::fs::read(path).map_err(crate::AiwikiError::Io)?;

    let registry = ParserRegistry::new();
    let parsed = registry
        .parse(language, &source)
        .ok_or_else(|| {
            crate::AiwikiError::Config(format!("No tree-sitter parser for language '{language}'"))
        })?
        .map_err(|e| {
            crate::AiwikiError::Config(format!(
                "Tree-sitter parse failed for {}: {e}",
                path.display()
            ))
        })?;

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let mut out = String::with_capacity(4096);
    let _ = writeln!(out, "Source code file: {filename}");
    let _ = writeln!(out, "Language: {language}");
    let _ = writeln!(out);

    // Imports
    if !parsed.imports.is_empty() {
        let _ = writeln!(out, "## Imports");
        for imp in &parsed.imports {
            if imp.alias.is_some() {
                let _ = writeln!(
                    out,
                    "- {} from {} (as {})",
                    imp.imported_name,
                    imp.source_module,
                    imp.alias.as_deref().unwrap_or(""),
                );
            } else {
                let _ = writeln!(out, "- {} from {}", imp.imported_name, imp.source_module,);
            }
        }
        let _ = writeln!(out);
    }

    // Group symbols by kind
    let kinds_order: &[SymbolKind] = &[
        SymbolKind::Module,
        SymbolKind::Class,
        SymbolKind::Struct,
        SymbolKind::Enum,
        SymbolKind::Trait,
        SymbolKind::Interface,
        SymbolKind::Function,
        SymbolKind::Method,
        SymbolKind::Constant,
        SymbolKind::Static,
        SymbolKind::TypeAlias,
        SymbolKind::Macro,
        SymbolKind::Test,
    ];

    for kind in kinds_order {
        let symbols: Vec<_> = parsed.symbols.iter().filter(|s| &s.kind == kind).collect();
        if symbols.is_empty() {
            continue;
        }
        let _ = writeln!(out, "## {}s", kind_label(kind));
        for sym in &symbols {
            let vis = format!("{:?}", sym.visibility).to_lowercase();
            if let Some(sig) = sym.signature.as_deref() {
                let _ = writeln!(out, "- [{vis}] {sig}");
            } else if let Some(qn) = sym.qualified_name.as_deref() {
                let _ = writeln!(out, "- [{vis}] {qn}");
            } else {
                let _ = writeln!(out, "- [{vis}] {}", sym.name);
            }
            if let Some(doc) = sym.doc_comment.as_deref() {
                let first_line = doc.lines().next().unwrap_or("");
                if !first_line.is_empty() {
                    let _ = writeln!(out, "  {first_line}");
                }
            }
        }
        let _ = writeln!(out);
    }

    // References summary
    if !parsed.references.is_empty() {
        let _ = writeln!(out, "## References");
        let mut seen = std::collections::HashSet::new();
        for r in &parsed.references {
            if seen.insert(&r.symbol_name) {
                let _ = writeln!(out, "- {} ({})", r.symbol_name, r.kind);
            }
        }
        let _ = writeln!(out);
    }

    // Also include raw source as context (truncated)
    let source_str = String::from_utf8_lossy(&source);
    let max_raw = 20_000;
    let raw = if source_str.len() > max_raw {
        &source_str[..max_raw]
    } else {
        &source_str
    };
    let _ = writeln!(out, "## Raw Source");
    let _ = writeln!(out, "```{language}");
    let _ = write!(out, "{raw}");
    if !raw.ends_with('\n') {
        let _ = writeln!(out);
    }
    let _ = writeln!(out, "```");

    Ok(out)
}

/// Human-readable label for a symbol kind.
fn kind_label(kind: &ragent_code::types::SymbolKind) -> &'static str {
    use ragent_code::types::SymbolKind;
    match kind {
        SymbolKind::Function => "Function",
        SymbolKind::Method => "Method",
        SymbolKind::Struct => "Struct",
        SymbolKind::Class => "Class",
        SymbolKind::Enum => "Enum",
        SymbolKind::EnumVariant => "Enum Variant",
        SymbolKind::Trait => "Trait",
        SymbolKind::Interface => "Interface",
        SymbolKind::Impl => "Impl",
        SymbolKind::Module => "Module",
        SymbolKind::Constant => "Constant",
        SymbolKind::Static => "Static",
        SymbolKind::TypeAlias => "Type Alias",
        SymbolKind::Field => "Field",
        SymbolKind::Import => "Import",
        SymbolKind::Macro => "Macro",
        SymbolKind::Test => "Test",
        SymbolKind::Unknown => "Symbol",
    }
}

/// Parse XML and extract text content, inserting newlines at paragraph boundaries.
fn xml_to_text(xml: &str) -> String {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_str(xml);
    let mut text = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(e)) => {
                let decoded = std::str::from_utf8(e.as_ref()).unwrap_or("");
                text.push_str(decoded);
            }
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");
                if matches!(name, "p" | "h" | "list-item" | "table-row") {
                    if !text.is_empty() && !text.ends_with('\n') {
                        text.push('\n');
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    text
}

/// Strip markdown formatting to produce plain text for LLM extraction.
///
/// Removes YAML frontmatter, headings markers, emphasis markers, link syntax,
/// code fences, HTML tags, and collapses excessive whitespace.
fn strip_markdown(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_frontmatter = false;
    let mut in_code_fence = false;

    for line in input.lines() {
        let trimmed = line.trim();

        // Skip YAML frontmatter blocks
        if trimmed == "---" {
            in_frontmatter = !in_frontmatter;
            continue;
        }
        if in_frontmatter {
            continue;
        }

        // Toggle fenced code blocks — keep content but drop the fence line
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_fence = !in_code_fence;
            continue;
        }

        // Strip heading markers (# ## ### etc.)
        let line = if trimmed.starts_with('#') {
            trimmed.trim_start_matches('#').trim_start()
        } else {
            trimmed
        };

        // Strip emphasis: **bold**, *italic*, __bold__, _italic_
        let line = line
            .replace("**", "")
            .replace("__", "")
            .replace('*', "")
            .replace('_', " ");

        // Strip inline code backticks
        let line = line.replace('`', "");

        // Convert markdown links [text](url) → text
        let line = strip_md_links(&line);

        // Convert images ![alt](url) → alt
        let line = strip_md_images(&line);

        // Remove HTML tags
        let line = strip_html_tags(&line);

        if !line.trim().is_empty() {
            out.push_str(line.trim());
            out.push('\n');
        } else {
            // Preserve paragraph breaks
            if !out.ends_with("\n\n") && !out.is_empty() {
                out.push('\n');
            }
        }
    }

    out
}

/// Replace markdown links `[text](url)` with just `text`.
fn strip_md_links(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '[' {
            let mut text = String::new();
            let mut found_close = false;
            for c in chars.by_ref() {
                if c == ']' {
                    found_close = true;
                    break;
                }
                text.push(c);
            }
            if found_close && chars.peek() == Some(&'(') {
                chars.next(); // skip '('
                // skip until closing ')'
                for c in chars.by_ref() {
                    if c == ')' {
                        break;
                    }
                }
                result.push_str(&text);
            } else {
                result.push('[');
                result.push_str(&text);
                if found_close {
                    result.push(']');
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// Replace markdown images `![alt](url)` with just `alt`.
fn strip_md_images(input: &str) -> String {
    input.replace("![", "[") // normalize, then strip_md_links handles it
}

/// Remove HTML tags from text.
fn strip_html_tags(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_type_from_path() {
        assert_eq!(DocumentType::from_path("file.md"), DocumentType::Markdown);
        assert_eq!(
            DocumentType::from_path("file.markdown"),
            DocumentType::Markdown
        );
        assert_eq!(DocumentType::from_path("file.pdf"), DocumentType::Pdf);
        assert_eq!(DocumentType::from_path("file.docx"), DocumentType::Docx);
        assert_eq!(DocumentType::from_path("file.odt"), DocumentType::Odt);
        assert_eq!(DocumentType::from_path("file.txt"), DocumentType::Text);
        assert_eq!(
            DocumentType::from_path("file.unknown"),
            DocumentType::Unknown
        );
    }

    #[test]
    fn test_document_type_properties() {
        assert!(DocumentType::Markdown.supports_extraction());
        assert!(DocumentType::Pdf.supports_extraction());
        assert!(DocumentType::Docx.supports_extraction());
        assert!(!DocumentType::Unknown.supports_extraction());

        assert_eq!(DocumentType::Markdown.name(), "Markdown");
        assert_eq!(DocumentType::Pdf.mime_type(), "application/pdf");
    }

    #[test]
    fn test_xml_to_text() {
        let xml = r#"<text:p>Hello</text:p><text:p>World</text:p>"#;
        let text = xml_to_text(xml);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn test_strip_markdown_headings() {
        let md = "# Title\n## Subtitle\nPlain text.";
        let text = strip_markdown(md);
        assert!(text.contains("Title"));
        assert!(text.contains("Subtitle"));
        assert!(!text.contains('#'));
    }

    #[test]
    fn test_strip_markdown_emphasis() {
        let md = "This is **bold** and *italic* text.";
        let text = strip_markdown(md);
        assert!(text.contains("bold"));
        assert!(text.contains("italic"));
        assert!(!text.contains('*'));
    }

    #[test]
    fn test_strip_markdown_links() {
        let md = "Click [here](https://example.com) for info.";
        let text = strip_markdown(md);
        assert!(text.contains("here"));
        assert!(!text.contains("https://example.com"));
        assert!(!text.contains('['));
    }

    #[test]
    fn test_strip_markdown_frontmatter() {
        let md = "---\ntitle: Test\nauthor: Bob\n---\nActual content.";
        let text = strip_markdown(md);
        assert!(text.contains("Actual content"));
        assert!(!text.contains("title: Test"));
    }

    #[test]
    fn test_strip_markdown_code_fences() {
        let md = "Before\n```rust\nlet x = 1;\n```\nAfter";
        let text = strip_markdown(md);
        assert!(text.contains("Before"));
        assert!(text.contains("let x = 1;"));
        assert!(text.contains("After"));
        assert!(!text.contains("```"));
    }

    #[test]
    fn test_strip_html_tags() {
        assert_eq!(strip_html_tags("Hello <b>world</b>!"), "Hello world!");
        assert_eq!(strip_html_tags("<p>text</p>"), "text");
    }

    #[test]
    fn test_strip_md_images() {
        let md = "See ![diagram](img.png) here.";
        let text = strip_markdown(md);
        assert!(text.contains("diagram"));
        assert!(!text.contains("img.png"));
    }

    #[test]
    fn test_detect_source_code_rust() {
        let dt = DocumentType::from_path("main.rs");
        assert!(matches!(dt, DocumentType::SourceCode { ref language } if language == "rust"));
        assert!(dt.supports_extraction());
    }

    #[test]
    fn test_detect_source_code_python() {
        let dt = DocumentType::from_path("script.py");
        assert!(matches!(dt, DocumentType::SourceCode { ref language } if language == "python"));
    }

    #[test]
    fn test_detect_source_code_typescript() {
        let dt = DocumentType::from_path("app.ts");
        assert!(
            matches!(dt, DocumentType::SourceCode { ref language } if language == "typescript")
        );
    }

    #[test]
    fn test_detect_source_code_go() {
        let dt = DocumentType::from_path("server.go");
        assert!(matches!(dt, DocumentType::SourceCode { ref language } if language == "go"));
    }

    #[test]
    fn test_detect_source_code_java() {
        let dt = DocumentType::from_path("Main.java");
        assert!(matches!(dt, DocumentType::SourceCode { ref language } if language == "java"));
    }

    #[test]
    fn test_detect_source_code_c() {
        let dt = DocumentType::from_path("main.c");
        assert!(matches!(dt, DocumentType::SourceCode { ref language } if language == "c"));
    }

    #[test]
    fn test_detect_source_code_cpp() {
        let dt = DocumentType::from_path("widget.cpp");
        assert!(matches!(dt, DocumentType::SourceCode { ref language } if language == "cpp"));
    }

    #[test]
    fn test_extract_code_text_rust() {
        let rust_src = br#"
//! Module documentation.

use std::collections::HashMap;

/// A config struct.
pub struct Config {
    pub name: String,
    pub value: i32,
}

/// Parse a config file.
pub fn parse_config(path: &str) -> Config {
    Config { name: path.to_string(), value: 42 }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse() {}
}
"#;
        let dir = std::env::temp_dir().join("aiwiki_test_code");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("sample.rs");
        std::fs::write(&file, rust_src).unwrap();

        let result = extract_code_text(&file, "rust").unwrap();
        assert!(result.contains("Language: rust"));
        assert!(result.contains("Config"));
        assert!(result.contains("parse_config"));
        let _ = std::fs::remove_file(&file);
    }
}
